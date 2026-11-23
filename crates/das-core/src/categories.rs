//! Built-in file categories by extension.

pub struct Category {
    pub id: &'static str,
    pub name: &'static str,
    /// Subfolder used when per-category folders are on.
    pub folder: &'static str,
    pub extensions: &'static [&'static str],
}

pub const CATEGORIES: &[Category] = &[
    Category {
        id: "video",
        name: "Video",
        folder: "Video",
        extensions: &["mp4", "mkv", "webm", "avi", "mov", "m4v", "flv", "wmv", "ts", "m2ts", "mpg", "mpeg", "3gp"],
    },
    Category {
        id: "music",
        name: "Music",
        folder: "Music",
        extensions: &["mp3", "flac", "wav", "m4a", "aac", "ogg", "opus", "wma", "alac", "aiff", "ape", "dsf", "wv"],
    },
    Category {
        id: "programs",
        name: "Programs",
        folder: "Programs",
        extensions: &["exe", "msi", "msix", "msixbundle", "appx", "appxbundle", "bat", "cmd", "apk", "jar", "ps1"],
    },
    Category {
        id: "archives",
        name: "Archives",
        folder: "Archives",
        extensions: &["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst", "cab", "lz", "lzma"],
    },
    Category {
        id: "discs",
        name: "Disc images",
        folder: "Disc images",
        extensions: &["iso", "img", "vhd", "vhdx", "vmdk", "bin", "cue", "dmg"],
    },
    Category {
        id: "documents",
        name: "Documents",
        folder: "Documents",
        extensions: &["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "txt", "rtf", "epub", "csv", "md"],
    },
    Category {
        id: "images",
        name: "Images",
        folder: "Images",
        extensions: &["jpg", "jpeg", "png", "gif", "webp", "avif", "heic", "bmp", "svg", "tif", "tiff", "psd", "raw"],
    },
    Category { id: "torrents", name: "Torrents", folder: "Torrents", extensions: &["torrent"] },
    Category { id: "web", name: "Web pages", folder: "Web pages", extensions: &["html", "htm", "mhtml", "xhtml"] },
];

pub fn categorize(file_name: &str) -> &'static str {
    let ext = match file_name.rsplit_once('.') {
        Some((_, e)) => e.to_ascii_lowercase(),
        None => return "other",
    };
    CATEGORIES
        .iter()
        .find(|c| c.extensions.contains(&ext.as_str()))
        .map(|c| c.id)
        .unwrap_or("other")
}

/// Category from the name first; when the name says nothing (no or unknown extension),
/// fall back to the server's Content-Type.
pub fn categorize_with(file_name: &str, content_type: Option<&str>) -> &'static str {
    let by_name = categorize(file_name);
    if by_name != "other" {
        return by_name;
    }
    content_type.map(categorize_mime).unwrap_or("other")
}

pub fn categorize_mime(content_type: &str) -> &'static str {
    let mime = content_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let (kind, sub) = mime.split_once('/').unwrap_or((mime.as_str(), ""));
    match (kind, sub) {
        ("video", _) => "video",
        ("audio", _) => "music",
        ("image", _) => "images",
        ("text", "html") | ("application", "xhtml+xml") => "web",
        ("text", _) => "documents",
        ("application", s) => match s {
            "pdf" | "msword" | "rtf" | "epub+zip" => "documents",
            s if s.starts_with("vnd.openxmlformats") || s.starts_with("vnd.ms-") || s.starts_with("vnd.oasis.opendocument") => {
                "documents"
            }
            "zip" | "x-zip-compressed" | "x-7z-compressed" | "x-rar-compressed" | "vnd.rar" | "gzip" | "x-gzip" | "x-tar"
            | "x-bzip2" | "x-xz" | "zstd" => "archives",
            "x-msdownload" | "x-msi" | "x-ms-installer" | "vnd.microsoft.portable-executable" | "x-dosexec"
            | "vnd.android.package-archive" | "java-archive" => "programs",
            "x-iso9660-image" | "x-cd-image" | "x-apple-diskimage" => "discs",
            "x-bittorrent" => "torrents",
            _ => "other",
        },
        _ => "other",
    }
}

pub fn folder_for(category: &str) -> Option<&'static str> {
    CATEGORIES.iter().find(|c| c.id == category).map(|c| c.folder)
}

#[cfg(test)]
mod tests {
    #[test]
    fn categorizes() {
        assert_eq!(super::categorize("Song.FLAC"), "music");
        assert_eq!(super::categorize("setup.exe"), "programs");
        assert_eq!(super::categorize("ubuntu.iso"), "discs");
        assert_eq!(super::categorize("noext"), "other");
        assert_eq!(super::categorize("page.HTML"), "web");
    }

    #[test]
    fn falls_back_to_content_type() {
        use super::categorize_with as c;
        assert_eq!(c("watch", Some("video/mp4")), "video");
        assert_eq!(c("track", Some("audio/flac")), "music");
        assert_eq!(c("download", Some("application/x-msdownload")), "programs");
        assert_eq!(c("get", Some("application/zip; charset=binary")), "archives");
        assert_eq!(c("doc", Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")), "documents");
        assert_eq!(c("index", Some("text/html; charset=utf-8")), "web");
        // The name wins when it is specific.
        assert_eq!(c("song.mp3", Some("application/octet-stream")), "music");
        assert_eq!(c("blob", Some("application/octet-stream")), "other");
    }
}
