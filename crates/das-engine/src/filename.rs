//! File-name detection (Content-Disposition, URL path) and Windows-safe sanitising.

use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};

/// Parse a Content-Disposition header value and return the file name, preferring `filename*`.
pub fn from_content_disposition(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';').map(str::trim) {
        let lower = part.to_ascii_lowercase();
        if lower.starts_with("filename*=") {
            let v = &part["filename*=".len()..];
            let v = v.trim_matches('"');
            // charset'lang'value
            let encoded = v.splitn(3, '\'').nth(2).unwrap_or(v);
            let decoded = percent_decode_str(encoded).decode_utf8_lossy().to_string();
            if !decoded.is_empty() {
                return Some(decoded);
            }
        } else if lower.starts_with("filename=") {
            let v = part["filename=".len()..].trim().trim_matches('"');
            if !v.is_empty() {
                plain = Some(percent_decode_str(v).decode_utf8_lossy().to_string());
            }
        }
    }
    plain
}

/// Last non-empty path segment of a URL, percent-decoded.
pub fn file_name_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let seg = parsed.path_segments()?.filter(|s| !s.is_empty()).last()?;
    let name = percent_decode_str(seg).decode_utf8_lossy().to_string();
    if name.is_empty() { None } else { Some(name) }
}

/// Make a name safe for Windows file systems.
pub fn sanitize_file_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    out = out.trim().trim_end_matches(['.', ' ']).to_string();
    if out.is_empty() {
        out = "download".into();
    }
    let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        out.insert(0, '_');
    }
    // Keep names well under MAX_PATH; cut on a char boundary, keep the extension.
    if out.chars().count() > 180 {
        let (stem, ext) = split_ext(&out);
        let stem: String = stem.chars().take(170).collect();
        out = format!("{stem}{ext}");
    }
    out
}

fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 12 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// `dir/name`, or `dir/name (1).ext`, `(2)`… if taken (also checks for a pending `.dasdl`).
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let taken = |p: &Path| p.exists() || part_path(p).exists();
    let first = dir.join(name);
    if !taken(&first) {
        return first;
    }
    let (stem, ext) = split_ext(name);
    for i in 1..10_000 {
        let p = dir.join(format!("{stem} ({i}){ext}"));
        if !taken(&p) {
            return p;
        }
    }
    first
}

/// The in-progress file that sits next to the final one.
pub fn part_path(final_path: &Path) -> PathBuf {
    let mut s = final_path.as_os_str().to_owned();
    s.push(".dasdl");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disposition() {
        assert_eq!(from_content_disposition("attachment; filename=\"a b.zip\"").unwrap(), "a b.zip");
        assert_eq!(
            from_content_disposition("attachment; filename=x.bin; filename*=UTF-8''%E2%82%AC%20rates.txt").unwrap(),
            "€ rates.txt"
        );
    }

    #[test]
    fn sanitize() {
        assert_eq!(sanitize_file_name("a:b?.txt"), "a_b_.txt");
        assert_eq!(sanitize_file_name("con.txt"), "_con.txt");
        assert_eq!(sanitize_file_name("  name. "), "name");
    }

    #[test]
    fn from_url() {
        assert_eq!(file_name_from_url("https://x.org/dl/My%20File.iso?x=1").unwrap(), "My File.iso");
        assert_eq!(file_name_from_url("https://x.org/"), None);
    }
}
