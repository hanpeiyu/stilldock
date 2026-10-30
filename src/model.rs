use serde::{Deserialize, Serialize};
use std::path::Path;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Asset {
    pub url: String,
    #[serde(default)]
    pub video: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preview: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Note {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

pub fn note_id_valid(id: &str) -> bool {
    id.len() == 24 && id.bytes().all(|c| c.is_ascii_hexdigit())
}

pub fn site_allowed(raw: &str) -> bool {
    Url::parse(raw).ok().is_some_and(|u| {
        u.scheme() == "https"
            && u.host_str()
                .is_some_and(|h| h == "xiaohongshu.com" || h.ends_with(".xiaohongshu.com"))
    })
}

pub fn media_allowed(raw: &str) -> bool {
    Url::parse(raw).ok().is_some_and(|u| {
        u.scheme() == "https"
            && u.port_or_known_default() == Some(443)
            && u.host_str()
                .is_some_and(|h| h.ends_with(".xhscdn.com") || h.ends_with(".xiaohongshu.com"))
    })
}

pub fn valid_note(note: &Note) -> bool {
    note_id_valid(&note.id)
        && site_allowed(&note.url)
        && !note.assets.is_empty()
        && note.assets.len() <= 200
        && note.assets.iter().all(|a| media_allowed(&a.url))
}

pub fn safe_name(title: &str) -> String {
    let s: String = title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(70)
        .collect();
    let s = s.trim().trim_matches('.');
    if s.is_empty() {
        "笔记".into()
    } else {
        s.to_owned()
    }
}

pub fn file_extension(bytes: &[u8], video: bool) -> &'static str {
    if video {
        "mp4"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if bytes.get(4..8) == Some(b"ftyp") {
        "avif"
    } else {
        "bin"
    }
}

pub fn folder_is_safe(root: &Path, id: &str) -> bool {
    note_id_valid(id) && root.join(id).parent() == Some(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries() {
        assert!(media_allowed("https://sns-img-bd.xhscdn.com/a"));
        assert!(!media_allowed("https://xhscdn.com.evil.test/a"));
        assert!(!media_allowed("http://127.0.0.1/a"));
        assert!(!media_allowed("https://sns-img-bd.xhscdn.com:888/a"));
        assert!(!note_id_valid("../../outside"));
        assert!(folder_is_safe(
            Path::new("D:/Downloads"),
            "abcdef0123456789abcdef01"
        ));
        assert_eq!(safe_name("a/b:c?"), "abc");
        assert_eq!(file_extension(b"\xff\xd8\xff\x00", false), "jpg");
    }
}
