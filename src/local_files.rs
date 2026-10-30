//! Cheap local availability checks. Never scan directories or hold the database lock during IO.
use serde::Serialize;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Target {
    pub id: String,
    pub folder: PathBuf,
    pub expected: usize,
    pub files: Vec<(PathBuf, u64)>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub present: usize,
    pub missing: usize,
    pub changed: usize,
    pub unavailable: usize,
}
impl Status {
    pub fn repairable(&self) -> bool {
        self.unavailable == 0 && self.missing + self.changed > 0
    }
}
fn location_available(path: &Path) -> bool {
    // A deleted subdirectory is missing; an offline volume/share or denied access is unknown.
    for ancestor in path.ancestors() {
        match std::fs::metadata(ancestor) {
            Ok(m) => return m.is_dir() && std::fs::read_dir(ancestor).is_ok(),
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(_) => return false,
        }
    }
    false
}
pub fn check(target: &Target) -> Status {
    let mut status = Status::default();
    let available = location_available(&target.folder);
    for (path, size) in &target.files {
        match std::fs::metadata(path) {
            Ok(m) if m.is_file() && *size > 0 && m.len() == *size => status.present += 1,
            Ok(_) => status.changed += 1,
            Err(e) if e.kind() == ErrorKind::NotFound && available => status.missing += 1,
            Err(_) => status.unavailable += 1,
        }
    }
    let unrecorded = target.expected.saturating_sub(target.files.len());
    if available {
        status.missing += unrecorded;
    } else {
        status.unavailable += unrecorded;
    }
    status
}

/// Explicit user-selected folder only. Filename and byte size matching, not content verification.
pub fn relocated(target: &Target, folder: &Path) -> Result<Vec<PathBuf>, String> {
    if target.files.is_empty() || target.files.len() != target.expected {
        return Err("记录不完整，请使用补下缺失项".into());
    }
    target
        .files
        .iter()
        .map(|(old, size)| {
            let name = old.file_name().ok_or("原文件名无效")?;
            let path = folder.join(name);
            let m = std::fs::metadata(&path)
                .map_err(|_| "所选目录未找到全部原文件，请选择素材所在文件夹")?;
            if !m.is_file() || *size == 0 || m.len() != *size {
                return Err("文件名或大小不匹配，未更改关联".into());
            }
            Ok(path)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_present_deleted_changed_and_unreachable_without_changing_files() {
        let root = std::env::temp_dir().join(format!("stilldock-check-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let good = root.join("good.jpg");
        let changed = root.join("changed.jpg");
        std::fs::write(&good, b"good").unwrap();
        std::fs::write(&changed, b"bad").unwrap();
        let target = Target {
            id: "test".into(),
            folder: root.clone(),
            expected: 4,
            files: vec![
                (good.clone(), 4),
                (changed.clone(), 4),
                (root.join("deleted.jpg"), 4),
            ],
        };
        let s = check(&target);
        assert_eq!(
            (s.present, s.missing, s.changed, s.unavailable),
            (1, 2, 1, 0)
        );
        assert!(s.repairable());
        let deleted = Target {
            folder: root.join("moved-subfolder"),
            files: vec![(root.join("moved-subfolder/a.jpg"), 4)],
            expected: 1,
            ..target.clone()
        };
        assert_eq!(check(&deleted).missing, 1);
        // A file used as a parent cannot be accessed as a directory.
        let unavailable = Target {
            folder: good.clone(),
            files: vec![(good.join("a.jpg"), 4)],
            expected: 1,
            ..target
        };
        let s = check(&unavailable);
        assert_eq!(s.unavailable, 1);
        assert!(!s.repairable());
        assert_eq!(std::fs::read(&changed).unwrap(), b"bad");
        std::fs::remove_file(good).unwrap();
        std::fs::remove_file(changed).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
