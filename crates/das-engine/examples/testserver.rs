//! Local HTTP server for trying DasIDM without the internet.
//!
//!   cargo run -p das-engine --example testserver -- 8765
//!
//! GET /f/<name>?size=<bytes>&rate=<bytes per second per connection>  range-capable file
//! GET /gone/<name>                                                    404

use axum::body::Body;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use bytes::Bytes;
use std::collections::HashMap;
use std::time::Duration;

fn byte_at(i: u64) -> u8 {
    ((i.wrapping_mul(31) + i / 7) % 251) as u8
}

async fn file(Path(name): Path<String>, Query(q): Query<HashMap<String, String>>, h: HeaderMap) -> Response {
    let size: u64 = q.get("size").and_then(|s| s.parse().ok()).unwrap_or(64 << 20);
    let rate: u64 = q.get("rate").and_then(|s| s.parse().ok()).unwrap_or(0);
    let range = h
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .and_then(|v| v.split_once('-'))
        .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok())));
    let (start, end, status) = match range {
        Some((a, b)) if a < size => (a, b.unwrap_or(size - 1).min(size - 1), StatusCode::PARTIAL_CONTENT),
        Some(_) => return StatusCode::RANGE_NOT_SATISFIABLE.into_response(),
        None => (0, size - 1, StatusCode::OK),
    };
    const CHUNK: u64 = 32 * 1024;
    // vary=1: each connection gets its own speed (0.3x to 2x), like real servers and routes.
    let factor = if q.contains_key("vary") { 0.3 + fastrand::f64() * 1.7 } else { 1.0 };
    let rate = (rate as f64 * factor) as u64;
    let delay = if rate > 0 { Duration::from_secs_f64(CHUNK as f64 / rate as f64) } else { Duration::ZERO };
    let stream = futures_util::stream::unfold(start, move |pos| async move {
        if pos > end {
            return None;
        }
        let n = (end + 1 - pos).min(CHUNK);
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let chunk: Vec<u8> = (pos..pos + n).map(byte_at).collect();
        Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), pos + n))
    });
    let mut b = Response::builder()
        .status(status)
        .header(header::CONTENT_LENGTH, end - start + 1)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, format!("\"{name}-{size}\""))
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""));
    if status == StatusCode::PARTIAL_CONTENT {
        b = b.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    b.body(Body::from_stream(stream)).unwrap()
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(8765);
    let app = axum::Router::new()
        .route("/f/{name}", get(file))
        .route("/gone/{name}", get(|| async { StatusCode::NOT_FOUND }));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
    println!("test server on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.unwrap();
}
