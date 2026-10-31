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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub live_url: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub sequence: usize,
    #[serde(default)]
    pub live: bool,
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
        && note.assets.len() <= 400
        && note.assets.iter().all(|a| {
            media_allowed(&a.url)
                && (a.original_url.is_empty() || media_allowed(&a.original_url))
                && (a.live_url.is_empty() || media_allowed(&a.live_url))
        })
}

pub fn select_current_assets(
    current: &Note,
    mut captured: Note,
    indices: &[usize],
    include_live: bool,
) -> Result<Note, String> {
    if captured.id != current.id
        || captured
            .assets
            .iter()
            .map(|a| &a.url)
            .ne(current.assets.iter().map(|a| &a.url))
    {
        return Err("笔记或图片列表已变化，请重新选择".into());
    }
    if indices.is_empty() || indices.iter().any(|i| *i >= captured.assets.len()) {
        return Err("请选择有效的图片序号".into());
    }
    select_assets(&mut captured, Some(indices), include_live);
    if !valid_note(&captured) {
        return Err("图片尚未就绪，请稍后重试".into());
    }
    Ok(captured)
}

pub fn select_assets(note: &mut Note, selected: Option<&[usize]>, include_live: bool) {
    let mut assets = Vec::new();
    for (index, source) in note.assets.iter().enumerate() {
        if selected.is_some_and(|indices| !indices.contains(&index)) {
            continue;
        }
        let mut still = source.clone();
        still.sequence = if source.sequence > 0 {
            source.sequence
        } else {
            index + 1
        };
        assets.push(still);
        if include_live && !source.video && !source.live_url.is_empty() {
            assets.push(Asset {
                url: source.live_url.clone(),
                video: true,
                live: true,
                sequence: if source.sequence > 0 {
                    source.sequence
                } else {
                    index + 1
                },
                ..Default::default()
            });
        }
    }
    note.assets = assets;
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
        if bytes.get(4..8) == Some(b"ftyp") {
            "mp4"
        } else {
            "bin"
        }
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "jpg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else if bytes.get(4..8) == Some(b"ftyp") {
        match bytes.get(8..12) {
            Some(b"avif" | b"avis") => "avif",
            Some(b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1") => "heic",
            _ => "bin",
        }
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
    fn live_selection_keeps_original_sequence_and_validates_all_sources() {
        let mut note = Note {
            id: "abcdef0123456789abcdef01".into(),
            url: "https://www.xiaohongshu.com/explore/abcdef0123456789abcdef01".into(),
            assets: vec![
                Asset {
                    url: "https://sns-img-bd.xhscdn.com/1".into(),
                    ..Default::default()
                },
                Asset {
                    url: "https://sns-img-bd.xhscdn.com/2".into(),
                    live_url: "https://sns-video-bd.xhscdn.com/live".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut still = note.clone();
        select_assets(&mut still, Some(&[1]), false);
        assert_eq!(still.assets.len(), 1);
        assert_eq!(still.assets[0].sequence, 2);
        select_assets(&mut note, Some(&[1]), true);
        assert_eq!(note.assets.len(), 2);
        assert_eq!(note.assets[0].sequence, 2);
        assert!(note.assets[1].live);
        assert_eq!(note.assets[1].sequence, 2);
        assert!(valid_note(&note));
        note.assets[0].original_url = "https://evil.example/image".into();
        assert!(!valid_note(&note));
    }

    #[test]
    fn material_capture_rejects_stale_or_invalid_selection_and_keeps_live_pairs() {
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            url: "https://www.xiaohongshu.com/explore/abcdef0123456789abcdef01".into(),
            assets: vec![
                Asset {
                    url: "https://sns-img-bd.xhscdn.com/first".into(),
                    sequence: 2,
                    ..Default::default()
                },
                Asset {
                    url: "https://sns-img-bd.xhscdn.com/second".into(),
                    sequence: 7,
                    live_url: "https://sns-video-bd.xhscdn.com/live".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let selected = select_current_assets(&note, note.clone(), &[1], true).unwrap();
        assert_eq!(selected.assets.len(), 2);
        assert_eq!(selected.assets[0].sequence, 7);
        assert_eq!(selected.assets[1].sequence, 7);
        assert!(selected.assets[1].live);
        assert!(select_current_assets(&note, note.clone(), &[], false).is_err());
        assert!(select_current_assets(&note, note.clone(), &[2], false).is_err());
        let mut stale = note.clone();
        stale.assets.reverse();
        assert!(select_current_assets(&note, stale, &[0], false).is_err());
        let mut bad = note.clone();
        bad.assets[1].original_url = "https://evil.example/image".into();
        assert!(select_current_assets(&note, bad, &[1], false).is_err());
    }

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
        assert_eq!(file_extension(b"\x00\x00\x00\x18ftypheic", false), "heic");
        assert_eq!(file_extension(b"\x00\x00\x00\x18ftypavif", false), "avif");
        assert_eq!(file_extension(b"<html>not a video", true), "bin");
    }
}
