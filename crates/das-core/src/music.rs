//! Share links from streaming services (Spotify, Amazon Music).
//!
//! Those services only stream, and their audio is DRM-protected at every quality, so DasIDM
//! never downloads from them, and never fetches the preview clip a share page points to. What it
//! does: read the public details a link preview shows (title, artist, album, year, cover) and
//! open searches on stores that sell the music as downloads in the quality the buyer chooses.
//!
//! Amazon Music pages are not fetched at all (Amazon's terms forbid automated access); for those
//! the user types the artist and title.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Service {
    Spotify,
    AmazonMusic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicKind {
    Track,
    Album,
    Playlist,
    Artist,
}

/// A recognised share link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicLink {
    pub service: Service,
    pub kind: MusicKind,
    pub id: String,
    /// The page to read (Spotify links in their plain form; `None` for Amazon, never fetched).
    pub page: Option<String>,
    /// A short link that must be followed first (spotify.link).
    pub short: bool,
}

/// What the card shows.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicInfo {
    pub service: Option<Service>,
    pub kind: Option<MusicKind>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    /// Seconds.
    pub duration: Option<u64>,
    pub tracks: Option<u32>,
    /// The cover as a data: URI (the page only shows those).
    pub cover: Option<String>,
    /// Nothing could be read: the user types artist and title.
    pub manual: bool,
    pub note: Option<String>,
}

/// A store that sells music as downloads, searched in the user's own browser.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    pub id: &'static str,
    pub name: &'static str,
    pub sells: &'static str,
    #[serde(skip)]
    template: &'static str,
}

/// Each template was opened once with a real query and lands on search results.
pub const STORES: [Store; 3] = [
    Store {
        id: "bandcamp",
        name: "Bandcamp",
        sells: "Pick the format when you buy: MP3, FLAC, ALAC, AAC, Ogg, WAV and more",
        template: "https://bandcamp.com/search?q={q}",
    },
    Store {
        id: "qobuz",
        name: "Qobuz",
        sells: "FLAC in CD quality, and 24-bit hi-res where the release has it",
        template: "https://www.qobuz.com/{region}/search?q={q}",
    },
    Store {
        id: "hdtracks",
        name: "HDtracks",
        sells: "Hi-res FLAC and ALAC",
        template: "https://www.hdtracks.com/#/search?q={q}",
    },
];

/// Recognise a Spotify or Amazon Music share link.
pub fn parse_link(s: &str) -> Option<MusicLink> {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("spotify:") {
        let mut it = rest.split(':');
        let kind = kind_of(it.next()?)?;
        let id = it.next().filter(|id| is_id(id))?;
        return Some(spotify(kind, id));
    }
    let url = url::Url::parse(s).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let segs: Vec<&str> = url.path_segments()?.filter(|p| !p.is_empty()).collect();
    if host == "spotify.link" && segs.len() == 1 {
        return Some(MusicLink { service: Service::Spotify, kind: MusicKind::Track, id: segs[0].to_string(), page: Some(s.to_string()), short: true });
    }
    if host == "open.spotify.com" {
        // /intl-de/track/ID, /track/ID, /embed/track/ID
        let segs: Vec<&str> = segs.into_iter().filter(|p| !p.starts_with("intl-") && *p != "embed").collect();
        let kind = kind_of(segs.first()?)?;
        let id = segs.get(1).filter(|id| is_id(id))?;
        return Some(spotify(kind, id));
    }
    let amazon = host.starts_with("music.amazon.") || (host.contains(".amazon.") && segs.first() == Some(&"music"));
    if amazon {
        let segs: Vec<&str> = segs.into_iter().filter(|p| *p != "music").collect();
        let track_asin = url.query_pairs().find(|(k, _)| k == "trackAsin").map(|(_, v)| v.into_owned());
        let kind = match *segs.first()? {
            "albums" if track_asin.is_some() => MusicKind::Track,
            "albums" => MusicKind::Album,
            "tracks" => MusicKind::Track,
            "playlists" | "user-playlists" => MusicKind::Playlist,
            "artists" => MusicKind::Artist,
            _ => return None,
        };
        let id = track_asin.or_else(|| segs.get(1).map(|x| x.to_string()))?;
        return Some(MusicLink { service: Service::AmazonMusic, kind, id, page: None, short: false });
    }
    None
}

fn spotify(kind: MusicKind, id: &str) -> MusicLink {
    let path = match kind {
        MusicKind::Track => "track",
        MusicKind::Album => "album",
        MusicKind::Playlist => "playlist",
        MusicKind::Artist => "artist",
    };
    MusicLink { service: Service::Spotify, kind, id: id.to_string(), page: Some(format!("https://open.spotify.com/{path}/{id}")), short: false }
}

fn kind_of(s: &str) -> Option<MusicKind> {
    Some(match s {
        "track" => MusicKind::Track,
        "album" => MusicKind::Album,
        "playlist" => MusicKind::Playlist,
        "artist" => MusicKind::Artist,
        _ => return None,
    })
}

fn is_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// `<meta property|name="…" content="…">` pairs (first one wins).
pub fn parse_meta(html: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = html;
    while let Some(i) = rest.find("<meta") {
        rest = &rest[i + 5..];
        let Some(end) = rest.find('>') else { break };
        let tag = &rest[..end];
        let key = attr(tag, "property").or_else(|| attr(tag, "name"));
        if let (Some(k), Some(v)) = (key, attr(tag, "content")) {
            out.entry(k.to_ascii_lowercase()).or_insert_with(|| decode_entities(&v));
        }
        rest = &rest[end..];
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = tag[from..].find(name) {
        let at = from + i;
        let before_ok = at == 0 || tag.as_bytes()[at - 1].is_ascii_whitespace();
        let after = tag[at + name.len()..].trim_start();
        if before_ok {
            if let Some(v) = after.strip_prefix('=') {
                let v = v.trim_start();
                let q = v.chars().next()?;
                if q == '"' || q == '\'' {
                    let body = &v[1..];
                    return body.find(q).map(|e| body[..e].to_string());
                }
            }
        }
        from = at + name.len();
    }
    None
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..semi];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Spotify's own page title, without "Listen to … | Spotify" and "- Album by …" wrapping.
fn clean_title(t: &str) -> String {
    let mut t = t.trim();
    t = t.strip_prefix("Listen to ").unwrap_or(t);
    t = t.strip_suffix(" | Spotify").unwrap_or(t);
    for cut in [" - Album by ", " - song and lyrics by ", " - Song by ", " - song by ", " - playlist by ", " - Single by ", " - EP by "] {
        if let Some(i) = t.find(cut) {
            t = &t[..i];
        }
    }
    t.trim().to_string()
}

/// The details from a Spotify page's meta tags.
pub fn info_from_meta(link: &MusicLink, meta: &HashMap<String, String>) -> MusicInfo {
    let mut info = MusicInfo { service: Some(link.service), kind: Some(link.kind), ..Default::default() };
    info.kind = match meta.get("og:type").map(String::as_str) {
        Some("music.song") => Some(MusicKind::Track),
        Some("music.album") => Some(MusicKind::Album),
        Some("music.playlist") => Some(MusicKind::Playlist),
        Some("profile") | Some("music.musician") => Some(MusicKind::Artist),
        _ => info.kind,
    };
    info.title = meta.get("og:title").or_else(|| meta.get("twitter:title")).map(|t| clean_title(t)).filter(|t| !t.is_empty());
    // "Artist · Album · Song · 1987"  /  "Artist · album · 1987 · 10 songs"
    if let Some(d) = meta.get("og:description").or_else(|| meta.get("twitter:description")) {
        let parts: Vec<&str> = d.split(" \u{b7} ").map(str::trim).collect();
        let year = |p: &str| p.parse::<u32>().ok().filter(|y| (1900..=2100).contains(y));
        match info.kind {
            Some(MusicKind::Track) if parts.len() >= 2 => {
                info.artist = Some(parts[0].to_string());
                info.album = Some(parts[1].to_string()).filter(|a| a != "Song");
                info.year = parts.iter().rev().find_map(|p| year(p));
            }
            Some(MusicKind::Album) if !parts.is_empty() => {
                info.artist = Some(parts[0].to_string());
                info.album = info.title.clone();
                info.year = parts.iter().find_map(|p| year(p));
                info.tracks = parts.iter().find_map(|p| p.strip_suffix(" songs").or(p.strip_suffix(" song")).and_then(|n| n.parse().ok()));
            }
            Some(MusicKind::Artist) => info.artist = info.title.clone(),
            _ => {}
        }
    }
    info.duration = meta.get("music:duration").and_then(|d| d.parse().ok());
    if info.year.is_none() {
        info.year = meta.get("music:release_date").and_then(|d| d.get(..4)).and_then(|y| y.parse().ok());
    }
    info.manual = info.title.is_none();
    info
}

/// What to search the stores for.
pub fn search_query(artist: Option<&str>, title: Option<&str>) -> String {
    [artist, title].into_iter().flatten().map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The search address for one store, or None for an unknown store or an empty query.
pub fn store_url(store_id: &str, query: &str, region: &str) -> Option<String> {
    let store = STORES.iter().find(|s| s.id == store_id)?;
    let q = query.trim();
    if q.is_empty() || q.len() > 300 {
        return None;
    }
    Some(store.template.replace("{region}", region).replace("{q}", &encode(q)))
}

/// Percent-encode everything but unreserved characters (spaces become %20, which every store's
/// search reads, including HDtracks' #/ routes).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Qobuz's storefront for a Windows locale like "en-GB". Only storefronts that were checked are
/// listed; everyone else gets the US one (Qobuz's site lets you switch country).
pub fn qobuz_region(locale: &str) -> &'static str {
    let country = locale.rsplit(['-', '_']).next().unwrap_or("").to_ascii_uppercase();
    match country.as_str() {
        "GB" => "gb-en",
        "DE" => "de-de",
        "FR" => "fr-fr",
        _ => "us-en",
    }
}

/// The user's Windows locale name ("en-IN").
pub fn system_locale() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buf = [0u16; 85];
        let n = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if n > 1 {
            return String::from_utf16_lossy(&buf[..(n - 1) as usize]);
        }
    }
    String::new()
}

/// Read a share link's public details: one page (Spotify only), then its cover.
pub async fn lookup(engine: &das_engine::Engine, link: &MusicLink) -> MusicInfo {
    let manual = |note: &str| MusicInfo {
        service: Some(link.service),
        kind: Some(link.kind),
        manual: true,
        note: Some(note.to_string()),
        ..Default::default()
    };
    let Some(page) = &link.page else {
        return manual("DasIDM doesn't read Amazon Music pages. Type the artist and title to search the stores.");
    };
    let Ok((bytes, _)) = engine.fetch_small(page, 1 << 20).await else {
        return manual("Couldn't read this link. Type the artist and title.");
    };
    let html = String::from_utf8_lossy(&bytes);
    let meta = parse_meta(&html);
    // A short link answers with the page it stands for, or a page linking to it.
    if link.short {
        let target = meta.get("og:url").cloned().or_else(|| find_spotify_url(&html));
        match target.as_deref().and_then(parse_link).filter(|l| !l.short) {
            Some(full) if meta.get("og:url").is_none() => return Box::pin(lookup(engine, &full)).await,
            Some(full) => return finish(engine, &full, &meta).await,
            None => return manual("Couldn't follow this short link. Paste the full open.spotify.com link instead."),
        }
    }
    finish(engine, link, &meta).await
}

async fn finish(engine: &das_engine::Engine, link: &MusicLink, meta: &HashMap<String, String>) -> MusicInfo {
    let mut info = info_from_meta(link, meta);
    if info.manual {
        info.note = Some("Couldn't read this link. Type the artist and title.".into());
        return info;
    }
    if let Some(img) = meta.get("og:image").filter(|u| u.starts_with("https://")) {
        if let Ok((bytes, ct)) = engine.fetch_small(img, 1 << 20).await {
            use base64::Engine as _;
            let mime = match ct.as_deref().unwrap_or("") {
                c if c.contains("png") => "image/png",
                c if c.contains("webp") => "image/webp",
                _ => "image/jpeg",
            };
            info.cover = Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
        }
    }
    info
}

fn find_spotify_url(html: &str) -> Option<String> {
    let i = html.find("https://open.spotify.com/")?;
    let rest = &html[i..];
    let end = rest.find(|c: char| c == '"' || c == '\'' || c == '<' || c.is_whitespace()).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_share_links() {
        let t = parse_link("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC?si=abc123").unwrap();
        assert_eq!((t.service, t.kind, t.id.as_str()), (Service::Spotify, MusicKind::Track, "4uLU6hMCjMI75M1A2tKUQC"));
        assert_eq!(t.page.as_deref(), Some("https://open.spotify.com/track/4uLU6hMCjMI75M1A2tKUQC"));
        let a = parse_link("https://open.spotify.com/intl-de/album/6N9PS4QXF1D0OWPk0Sxtb4").unwrap();
        assert_eq!((a.kind, a.page.as_deref()), (MusicKind::Album, Some("https://open.spotify.com/album/6N9PS4QXF1D0OWPk0Sxtb4")));
        assert_eq!(parse_link("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M").unwrap().kind, MusicKind::Playlist);
        assert!(parse_link("https://spotify.link/AbCdEf123").unwrap().short);
        assert!(parse_link("https://open.spotify.com/track/").is_none());
        assert!(parse_link("https://open.spotify.com/track/../../etc").is_none());

        let am = parse_link("https://music.amazon.in/albums/B0ABCDEF12?trackAsin=B0TRACK999&ref=dm_sh").unwrap();
        assert_eq!((am.service, am.kind, am.id.as_str(), am.page), (Service::AmazonMusic, MusicKind::Track, "B0TRACK999", None));
        assert_eq!(parse_link("https://music.amazon.com/albums/B0ABCDEF12").unwrap().kind, MusicKind::Album);
        assert_eq!(parse_link("https://www.amazon.co.uk/music/player/albums/B0ABCDEF12").map(|l| l.kind), None);
        assert!(parse_link("https://example.com/track/abc").is_none());
        assert!(parse_link("https://www.youtube.com/watch?v=x").is_none());
    }

    // Made-up pages in the shape Spotify's share pages use.
    const TRACK: &str = r#"<html><head><meta property="og:title" content="Paper Lanterns"/><meta property="og:description" content="The Example Band &#183; Night Harbour &#xB7; Song &#183; 2019"/><meta property="og:type" content="music.song"/><meta property="og:image" content="https://i.example/cover.jpg"/><meta property="og:audio" content="https://p.example/preview.mp3"/><meta name="music:duration" content="214"/><meta name="music:release_date" content="2019-03-01"/></head></html>"#;
    const ALBUM: &str = r#"<meta property="og:title" content="Listen to Night Harbour - Album by The Example Band | Spotify"><meta property='og:description' content='The Example Band · album · 2019 · 10 songs'><meta property="og:type" content="music.album">"#;

    #[test]
    fn reads_track_and_album_pages() {
        let link = parse_link("https://open.spotify.com/track/abc123").unwrap();
        let t = info_from_meta(&link, &parse_meta(TRACK));
        assert_eq!(t.title.as_deref(), Some("Paper Lanterns"));
        assert_eq!(t.artist.as_deref(), Some("The Example Band"));
        assert_eq!(t.album.as_deref(), Some("Night Harbour"));
        assert_eq!((t.year, t.duration, t.manual), (Some(2019), Some(214), false));

        let link = parse_link("https://open.spotify.com/album/abc123").unwrap();
        let a = info_from_meta(&link, &parse_meta(ALBUM));
        assert_eq!(a.title.as_deref(), Some("Night Harbour"));
        assert_eq!(a.artist.as_deref(), Some("The Example Band"));
        assert_eq!((a.kind, a.year, a.tracks), (Some(MusicKind::Album), Some(2019), Some(10)));

        let empty = info_from_meta(&link, &parse_meta("<html><title>x</title></html>"));
        assert!(empty.manual && empty.title.is_none());
    }

    #[test]
    fn never_offers_the_preview_clip() {
        // The page names a preview clip; nothing in MusicInfo can carry it.
        let link = parse_link("https://open.spotify.com/track/abc123").unwrap();
        let t = info_from_meta(&link, &parse_meta(TRACK));
        assert!(!serde_json::to_string(&t).unwrap().contains("preview.mp3"));
    }

    #[test]
    fn builds_store_searches() {
        assert_eq!(store_url("bandcamp", "Café Tacvba Eres", "us-en").unwrap(), "https://bandcamp.com/search?q=Caf%C3%A9%20Tacvba%20Eres");
        assert_eq!(store_url("qobuz", "A & B", "gb-en").unwrap(), "https://www.qobuz.com/gb-en/search?q=A%20%26%20B");
        assert_eq!(store_url("hdtracks", "x#y?z", "us-en").unwrap(), "https://www.hdtracks.com/#/search?q=x%23y%3Fz");
        assert!(store_url("somewhere", "x", "us-en").is_none());
        assert!(store_url("bandcamp", "   ", "us-en").is_none());
        assert_eq!(search_query(Some("The Example Band"), Some("Paper Lanterns ")), "The Example Band Paper Lanterns");
        assert_eq!((qobuz_region("en-GB"), qobuz_region("de-DE"), qobuz_region("en-IN"), qobuz_region("")), ("gb-en", "de-de", "us-en", "us-en"));
    }
}
