use das_core::{AddRequest, CoreEvent, Manager, Settings, Status};
use std::sync::{Arc, Mutex};
use std::time::Duration;

async fn server(size: usize) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let (mut sock, _) = listener.accept().await.unwrap();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let range = req
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").map(str::to_owned));
                let (a, b) = match range {
                    Some(r) => {
                        let (a, b) = r.split_once('-').unwrap();
                        let a: usize = a.parse().unwrap();
                        let b: usize = b.trim().parse().unwrap_or(size - 1);
                        (a, b.min(size - 1))
                    }
                    None => (0, size - 1),
                };
                let body: Vec<u8> = (a..=b).map(|i| (i % 251) as u8).collect();
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {a}-{b}/{size}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = sock.write_all(head.as_bytes()).await;
                let _ = sock.write_all(&body).await;
            });
        }
    });
    format!("http://{addr}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn add_download_complete_and_persist() {
    let data = tempfile::tempdir().unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let events: Arc<Mutex<Vec<CoreEvent>>> = Arc::default();
    let ev = events.clone();
    let m = Manager::open(data.path(), Arc::new(move |e| ev.lock().unwrap().push(e))).unwrap();
    m.set_settings(Settings {
        download_dir: downloads.path().to_string_lossy().into_owned(),
        category_folders: true,
        ..Settings::default()
    })
    .unwrap();
    tokio::spawn(m.clone().run());

    let base = server(3 * 1024 * 1024).await;
    let item = m
        .add(AddRequest {
            url: format!("{base}/music/track.flac"),
            file_name: None,
            save_dir: None,
            connections: Some(4),
            headers: vec![],
            start: true,
        })
        .unwrap();
    assert_eq!(item.category, "music");

    for _ in 0..200 {
        if m.get(&item.id).unwrap().status == Status::Done {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let done = m.get(&item.id).unwrap();
    assert_eq!(done.status, Status::Done, "error: {:?}", done.error);
    let path = done.file_path.unwrap();
    assert!(path.contains("Music"));
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(bytes.len(), 3 * 1024 * 1024);
    assert!(bytes.iter().enumerate().all(|(i, b)| *b == (i % 251) as u8));
    assert!(events.lock().unwrap().iter().any(|e| matches!(e, CoreEvent::Completed { .. })));
    drop(m);

    // Reopen: the finished item is still there.
    let m2 = Manager::open(data.path(), Arc::new(|_| {})).unwrap();
    assert_eq!(m2.list().len(), 1);
    assert_eq!(m2.list()[0].status, Status::Done);
}
