//! Finding a downloadable link in free text (the clipboard, a drop, a pasted message).

/// The first `http(s)://` or `magnet:` link in `text`, with trailing punctuation trimmed.
pub fn first_link(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = ["https://", "http://", "magnet:?"]
        .iter()
        .filter_map(|p| lower.find(p))
        .min()?;
    let rest = &text[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '"' | '<' | '>' | '`' | '\''))
        .unwrap_or(rest.len());
    let link = rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}']);
    // A bare scheme is not a link.
    (link.len() > "https://".len() && !link.ends_with("://")).then(|| link.to_string())
}

#[cfg(test)]
mod tests {
    use super::first_link;

    #[test]
    fn finds_links_in_text() {
        assert_eq!(first_link("https://youtu.be/abc?si=x").as_deref(), Some("https://youtu.be/abc?si=x"));
        assert_eq!(first_link("  look: https://a.com/f.zip, thanks").as_deref(), Some("https://a.com/f.zip"));
        assert_eq!(first_link("(see https://a.com/x)").as_deref(), Some("https://a.com/x"));
        assert_eq!(first_link("HTTP://A.COM/B").as_deref(), Some("HTTP://A.COM/B"));
        assert_eq!(first_link("magnet:?xt=urn:btih:abc&dn=x y").as_deref(), Some("magnet:?xt=urn:btih:abc&dn=x"));
        assert_eq!(first_link("first http://one.com then https://two.com").as_deref(), Some("http://one.com"));
    }

    #[test]
    fn ignores_non_links() {
        assert_eq!(first_link(""), None);
        assert_eq!(first_link("just some words"), None);
        assert_eq!(first_link("https://"), None);
        assert_eq!(first_link("ftp://a.com/x"), None);
    }
}
