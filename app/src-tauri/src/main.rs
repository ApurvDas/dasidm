#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! DasIDM desktop shell. The download core keeps running in the tray; the WebView window
//! is destroyed whenever it is closed so an idle DasIDM holds almost no memory.

mod browser_host;

use browser_host::{PendingAdd, Registration};
use das_core::{AddRequest, ItemView, Manager, ProbeInfo, Settings};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::window::{Effect, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct App {
    core: Manager,
    /// A link waiting for the window to open the Add sheet with it (a video page, a torrent,
    /// or a download Firefox handed over).
    pending_add: parking_lot::Mutex<Option<PendingAdd>>,
    /// When the Firefox extension last reached DasIDM (ms since 1970, 0 = never this run).
    browser_seen: AtomicU64,
    browser_reg: parking_lot::Mutex<Option<Registration>>,
}

type Res<T> = Result<T, String>;

// Commands that may start downloads are async so they run on the tokio runtime.

#[tauri::command]
fn list_downloads(app: State<'_, App>) -> Vec<ItemView> {
    app.core.list()
}

#[tauri::command]
async fn probe_url(app: State<'_, App>, url: String, headers: Option<Vec<(String, String)>>) -> Res<ProbeInfo> {
    app.core.probe(url, headers.unwrap_or_default()).await
}

#[tauri::command]
async fn add_download(app: State<'_, App>, req: AddRequest) -> Res<ItemView> {
    app.core.add(req)
}

#[tauri::command]
async fn pause(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.pause(&ids);
    Ok(())
}

#[tauri::command]
async fn resume(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.resume(&ids);
    Ok(())
}

#[tauri::command]
async fn restart(app: State<'_, App>, ids: Vec<String>) -> Res<()> {
    app.core.restart(&ids);
    Ok(())
}

#[tauri::command]
async fn remove(app: State<'_, App>, ids: Vec<String>, delete_files: bool) -> Res<()> {
    app.core.remove(&ids, delete_files);
    Ok(())
}

#[tauri::command]
async fn pause_all(app: State<'_, App>) -> Res<()> {
    app.core.pause_all();
    Ok(())
}

#[tauri::command]
async fn resume_all(app: State<'_, App>) -> Res<()> {
    app.core.resume_all();
    Ok(())
}

#[tauri::command]
fn speed_history(app: State<'_, App>) -> Vec<u64> {
    app.core.speed_history()
}

#[tauri::command]
fn get_settings(app: State<'_, App>) -> Settings {
    app.core.settings()
}

#[tauri::command]
async fn set_settings(app: State<'_, App>, settings: Settings) -> Res<()> {
    app.core.set_settings(settings)
}

/// Open a finished file with its default program.
#[tauri::command]
fn open_file(app: State<'_, App>, id: String) -> Res<()> {
    let item = app.core.get(&id).ok_or("Download not found")?;
    let path = item.file_path.ok_or("The file is not finished yet")?;
    std::process::Command::new("explorer.exe").arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Show the file (or its folder) in Explorer.
#[tauri::command]
fn open_folder(app: State<'_, App>, id: String) -> Res<()> {
    use std::os::windows::process::CommandExt;
    let item = app.core.get(&id).ok_or("Download not found")?;
    let mut cmd = std::process::Command::new("explorer.exe");
    match item.file_path.filter(|p| std::path::Path::new(p).exists()) {
        Some(p) => cmd.raw_arg(format!("/select,\"{p}\"")),
        None => cmd.arg(&item.save_dir),
    };
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn tools_status(app: State<'_, App>) -> das_core::tools::ToolsStatus {
    app.core.tools_status()
}

/// Download yt-dlp, ffmpeg and deno. The UI calls this only after the user agrees.
#[tauri::command]
async fn install_tools(app: State<'_, App>) -> Res<()> {
    app.core.install_tools().await
}

#[tauri::command]
async fn update_tools(app: State<'_, App>) -> Res<bool> {
    app.core.update_tools().await
}

#[tauri::command]
async fn video_info(app: State<'_, App>, url: String) -> Res<das_core::video::VideoInfo> {
    app.core.video_info(url).await
}

#[tauri::command]
async fn add_video(app: State<'_, App>, req: das_core::model::AddVideoRequest) -> Res<Vec<ItemView>> {
    app.core.add_video(req).await
}

#[tauri::command]
fn thumbnail(app: State<'_, App>, id: String) -> Option<String> {
    app.core.thumbnail(&id)
}

// ---- Torrents ------------------------------------------------------------------------

#[tauri::command]
async fn torrent_preview(app: State<'_, App>, link: Option<String>, path: Option<String>) -> Res<das_core::TorrentPreview> {
    app.core.torrent_preview(link, path).await
}

#[tauri::command]
async fn add_torrent(app: State<'_, App>, req: das_core::model::AddTorrentRequest) -> Res<ItemView> {
    app.core.add_torrent(req).await
}

#[tauri::command]
fn torrent_trackers(app: State<'_, App>, id: String) -> Vec<das_core::TrackerView> {
    app.core.torrent_trackers(&id)
}

#[tauri::command]
fn add_trackers(app: State<'_, App>, id: String, urls: Vec<String>) -> Res<usize> {
    app.core.add_trackers(&id, urls)
}

#[tauri::command]
fn remove_trackers(app: State<'_, App>, id: String, urls: Vec<String>) -> Res<usize> {
    app.core.remove_trackers(&id, urls)
}

#[tauri::command]
fn replace_tracker(app: State<'_, App>, id: String, old: String, new: String) -> Res<()> {
    app.core.replace_tracker(&id, old, new)
}

#[tauri::command]
fn reannounce(app: State<'_, App>, id: String) -> Res<()> {
    app.core.reannounce(&id)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TorrentFiles {
    files: Vec<das_core::FileEntry>,
    chosen: Option<Vec<usize>>,
}

#[tauri::command]
fn torrent_files(app: State<'_, App>, id: String) -> TorrentFiles {
    let (files, chosen) = app.core.torrent_files(&id);
    TorrentFiles { files, chosen }
}

#[tauri::command]
async fn set_torrent_files(app: State<'_, App>, id: String, only: Vec<usize>) -> Res<()> {
    app.core.set_torrent_files(&id, only).await
}

#[tauri::command]
fn seed(app: State<'_, App>, ids: Vec<String>) {
    app.core.seed(&ids);
}

/// The Windows file picker for a .torrent file.
#[tauri::command]
async fn pick_torrent_file(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file().set_title("Open a .torrent file").add_filter("Torrent files", &["torrent"]);
    if let Some(w) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&w);
    }
    dialog.blocking_pick_file()?.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}

/// Make DasIDM open magnet links and .torrent files (for this Windows user only).
#[tauri::command]
fn register_torrent_handler() -> Res<()> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let open = format!("\"{}\" \"%1\"", exe.display());
    let keys: [(&str, Option<&str>, &str); 6] = [
        (r"HKCU\Software\Classes\magnet", None, "URL:Magnet link"),
        (r"HKCU\Software\Classes\magnet", Some("URL Protocol"), ""),
        (r"HKCU\Software\Classes\magnet\shell\open\command", None, open.as_str()),
        (r"HKCU\Software\Classes\DasIDM.torrent", None, "Torrent file"),
        (r"HKCU\Software\Classes\DasIDM.torrent\shell\open\command", None, open.as_str()),
        (r"HKCU\Software\Classes\.torrent", None, "DasIDM.torrent"),
    ];
    for (key, name, value) in keys {
        let mut cmd = std::process::Command::new("reg.exe");
        cmd.arg("add").arg(key);
        match name {
            Some(n) => cmd.arg("/v").arg(n),
            None => cmd.arg("/ve"),
        };
        cmd.args(["/t", "REG_SZ", "/d", value, "/f"]).creation_flags(0x0800_0000);
        let ok = cmd.status().map_err(|e| e.to_string())?.success();
        if !ok {
            return Err(format!("Windows refused to register {key}"));
        }
    }
    Ok(())
}

/// A link handed over (command line, Firefox) that needs the Add sheet.
#[tauri::command]
fn take_pending_add(app: State<'_, App>) -> Option<PendingAdd> {
    app.pending_add.lock().take()
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserStatus {
    #[serde(flatten)]
    reg: Option<Registration>,
    last_seen: Option<u64>,
}

/// Is Firefox set up to reach DasIDM, and has the extension been in touch?
#[tauri::command]
fn browser_status(app: State<'_, App>) -> BrowserStatus {
    let seen = app.browser_seen.load(Ordering::Relaxed);
    BrowserStatus { reg: app.browser_reg.lock().clone(), last_seen: (seen > 0).then_some(seen) }
}

/// Write the Firefox registration again (after moving DasIDM, or if something removed it).
#[tauri::command]
async fn repair_browser(app: State<'_, App>) -> Res<BrowserStatus> {
    let reg = tauri::async_runtime::spawn_blocking(browser_host::register).await.map_err(|e| e.to_string())?;
    if let Some(e) = &reg.error {
        return Err(e.clone());
    }
    *app.browser_reg.lock() = Some(reg);
    Ok(browser_status(app))
}

/// Explorer at the unpacked extension (for Firefox's "Load Temporary Add-on").
#[tauri::command]
fn open_extension_folder() -> Res<()> {
    let dir = browser_host::browser_dir().join("firefox");
    std::process::Command::new("explorer").arg(&dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Answers the Firefox helper: a ping, or a download to open in the Add sheet.
fn browser_handler(handle: AppHandle) -> browser_host::Handler {
    use browser_host::Request;
    Arc::new(move |req| {
        let app = handle.state::<App>();
        app.browser_seen.store(now_ms(), Ordering::Relaxed);
        match req {
            Request::Ping => serde_json::json!({ "ok": true, "app": "DasIDM", "version": env!("CARGO_PKG_VERSION") }),
            Request::Download(d) => match d.into_pending() {
                Ok(p) => {
                    *app.pending_add.lock() = Some(p.clone());
                    let h = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        show_main(&h);
                        let _ = h.emit("open-add", p);
                    });
                    serde_json::json!({ "accepted": true })
                }
                Err(e) => serde_json::json!({ "accepted": false, "error": e }),
            },
        }
    })
}

/// The first link on the clipboard, so Add URL can open prefilled.
#[tauri::command]
fn clipboard_link() -> Option<String> {
    let text = arboard::Clipboard::new().ok()?.get_text().ok()?;
    das_core::links::first_link(&text)
}

/// The native Windows folder picker, starting at `start` when it exists.
#[tauri::command]
async fn pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dialog = app.dialog().file().set_title("Choose a folder");
    if let Some(w) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&w);
    }
    if let Some(dir) = start.filter(|d| std::path::Path::new(d).is_dir()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.blocking_pick_folder()?.into_path().ok().map(|p| p.to_string_lossy().into_owned())
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DasIDM")
        .inner_size(1280.0, 820.0)
        .min_inner_size(920.0, 580.0)
        .decorations(false)
        .transparent(true)
        .theme(Some(Theme::Dark))
        .effects(EffectsBuilder::new().effect(Effect::Mica).build())
        // Fewer helper processes and background services in WebView2. The glass is drawn by
        // the Windows compositor (Mica), so the page renders in software: measured ~111 MB
        // private with the window open instead of ~200 MB with GPU rendering.
        .additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,Translate,AutofillServerCommunication,msEdgeShopping \
             --disable-background-networking --renderer-process-limit=1 \
             --disable-gpu --in-process-gpu --disable-gpu-compositing",
        )
        .build();
    if built.is_ok() {
        app.state::<App>().core.set_visible(true);
    }
}

/// Links passed on the command line (`dasidm.exe <url>…`) are added as downloads.
fn add_urls(app: &AppHandle, args: &[String]) {
    // A magnet link or a .torrent file opens the Add sheet with its file list.
    if let Some(t) = args.iter().skip(1).find(|a| das_core::is_torrent_link(a)) {
        *app.state::<App>().pending_add.lock() = Some(PendingAdd::link(t.clone()));
        show_main(app);
        let _ = app.emit("open-add", PendingAdd::link(t.clone()));
    }
    let mut urls: Vec<String> = args
        .iter()
        .filter(|a| (a.starts_with("http://") || a.starts_with("https://")) && !das_core::is_torrent_link(a))
        .cloned()
        .collect();
    // A video page needs a quality choice: open the Add sheet with it instead of saving the page.
    if let Some(i) = urls.iter().position(|u| das_core::video::is_video_site(u)) {
        let link = urls.remove(i);
        *app.state::<App>().pending_add.lock() = Some(PendingAdd::link(link.clone()));
        show_main(app);
        let _ = app.emit("open-add", PendingAdd::link(link));
    }
    if urls.is_empty() {
        return;
    }
    let core = app.state::<App>().core.clone();
    tauri::async_runtime::spawn(async move {
        for url in urls {
            let _ = core.add(AddRequest {
                url,
                file_name: None,
                save_dir: None,
                connections: None,
                headers: vec![],
                start: true,
                allow_page: false,
            });
        }
    });
}

fn quit(app: &AppHandle) {
    let core = app.state::<App>().core.clone();
    tauri::async_runtime::block_on(core.shutdown());
    app.exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Started by Firefox as its helper: relay messages, no window, no WebView, then exit.
    if browser_host::is_host_launch(&args) {
        std::process::exit(browser_host::run_host());
    }
    let start_hidden = args.iter().any(|a| a == "--background");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A second launch hands its links to the running DasIDM.
            add_urls(app, &args);
            if !args.iter().any(|a| a == "--background") {
                show_main(app);
            }
        }))
        .invoke_handler(tauri::generate_handler![
            list_downloads,
            speed_history,
            probe_url,
            add_download,
            pause,
            resume,
            restart,
            remove,
            pause_all,
            resume_all,
            get_settings,
            set_settings,
            open_file,
            open_folder,
            clipboard_link,
            pick_folder,
            take_pending_add,
            tools_status,
            install_tools,
            update_tools,
            video_info,
            add_video,
            thumbnail,
            torrent_preview,
            add_torrent,
            torrent_trackers,
            add_trackers,
            remove_trackers,
            replace_tracker,
            reannounce,
            torrent_files,
            set_torrent_files,
            seed,
            pick_torrent_file,
            register_torrent_handler,
            browser_status,
            repair_browser,
            open_extension_folder
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            // DASIDM_DATA_DIR runs a separate, portable profile (tests, USB sticks).
            let data_dir = match std::env::var_os("DASIDM_DATA_DIR") {
                Some(d) => std::path::PathBuf::from(d),
                None => app.path().app_data_dir()?,
            };
            let emitter = handle.clone();
            let core = Manager::open(&data_dir, Arc::new(move |e| {
                let _ = emitter.emit("core", e);
            }))?;
            if let Some(dir) = std::env::var_os("DASIDM_DOWNLOAD_DIR") {
                let mut s = core.settings();
                s.download_dir = dir.to_string_lossy().into_owned();
                let c = core.clone();
                tauri::async_runtime::block_on(async move { c.set_settings(s) })?;
            }
            tauri::async_runtime::spawn(core.clone().run());
            app.manage(App {
                core,
                pending_add: parking_lot::Mutex::new(None),
                browser_seen: AtomicU64::new(0),
                browser_reg: parking_lot::Mutex::new(None),
            });
            add_urls(&handle, &args);

            // Firefox: keep the helper registered (cheap when unchanged) and answer it.
            let h = handle.clone();
            std::thread::spawn(move || {
                let reg = browser_host::register();
                *h.state::<App>().browser_reg.lock() = Some(reg);
            });
            tauri::async_runtime::spawn(browser_host::serve(browser_host::pipe_name(), browser_handler(handle.clone())));

            let open = MenuItem::with_id(app, "open", "Open DasIDM", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause_all", "Pause all", true, None::<&str>)?;
            let resume = MenuItem::with_id(app, "resume_all", "Resume all", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit DasIDM", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &pause, &resume, &sep, &quit_item])?;
            TrayIconBuilder::with_id("tray")
                .icon(app.default_window_icon().cloned().ok_or("missing icon")?)
                .tooltip("DasIDM")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "open" => show_main(app),
                    "pause_all" => app.state::<App>().core.pause_all(),
                    "resume_all" => {
                        let core = app.state::<App>().core.clone();
                        tauri::async_runtime::spawn(async move { core.resume_all() });
                    }
                    "quit" => quit(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_main(tray.app_handle());
                    }
                })
                .build(app)?;

            if !start_hidden {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // A .torrent file dropped on the window opens it in the Add sheet.
            if let WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if let Some(p) = paths.iter().find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("torrent"))) {
                    let _ = window.app_handle().emit("open-add", PendingAdd::link(p.to_string_lossy()));
                }
            }
            if let WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    window.app_handle().state::<App>().core.set_visible(false);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start DasIDM")
        .run(|app, event| {
            if let RunEvent::ExitRequested { code: None, api, .. } = event {
                // Last window closed: keep running in the tray unless the user turned that off.
                if app.state::<App>().core.settings().close_to_tray {
                    api.prevent_exit();
                } else {
                    let core = app.state::<App>().core.clone();
                    tauri::async_runtime::block_on(core.shutdown());
                }
            }
        });
}
