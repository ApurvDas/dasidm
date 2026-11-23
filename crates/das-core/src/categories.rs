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
];

pub fn categorize(file_name: &str) -> &'static str {
    let ext = file_name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    CATEGORIES
        .iter()
        .find(|c| c.extensions.contains(&ext.as_str()))
        .map(|c| c.id)
        .unwrap_or("other")
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
    }
}
