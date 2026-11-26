//! Live check of share-link lookups through the real engine (one page and its cover per link).
//!
//!   cargo run -p das-core --example musictest -- <share link> [<share link> ...]

use das_engine::{Engine, EngineConfig};

#[tokio::main]
async fn main() {
    let engine = Engine::new(EngineConfig::default());
    for url in std::env::args().skip(1) {
        let Some(link) = das_core::music::parse_link(&url) else {
            println!("{url}\n  not a music link");
            continue;
        };
        let info = das_core::music::lookup(&engine, &link).await;
        println!("{url}");
        println!("  {:?} {:?}  title {:?}  artist {:?}  album {:?}", info.service, info.kind, info.title, info.artist, info.album);
        println!("  year {:?}  length {:?} s  tracks {:?}  cover {}  manual {}  note {:?}",
            info.year, info.duration, info.tracks, info.cover.as_ref().map(|c| format!("{} chars", c.len())).unwrap_or("none".into()), info.manual, info.note);
        let q = das_core::music::search_query(info.artist.as_deref(), info.title.as_deref());
        for s in das_core::music::STORES {
            println!("  {:<9} {}", s.name, das_core::music::store_url(s.id, &q, das_core::music::qobuz_region(&das_core::music::system_locale())).unwrap_or_default());
        }
    }
}
