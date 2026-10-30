use crate::model::Note;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize)]
pub struct Job {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub author: String,
    pub state: String,
    pub message: String,
    pub folder: String,
    pub done: u64,
    pub total: u64,
    pub files: usize,
}

pub struct Store {
    db: Connection,
}
impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let db = Connection::open(path)?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;
        CREATE TABLE IF NOT EXISTS jobs(id TEXT PRIMARY KEY,note_id TEXT,title TEXT,author TEXT,state TEXT,message TEXT,folder TEXT,payload TEXT,done INTEGER DEFAULT 0,total INTEGER DEFAULT 0,created INTEGER);
        CREATE TABLE IF NOT EXISTS files(job_id TEXT,idx INTEGER,path TEXT,size INTEGER,PRIMARY KEY(job_id,idx));
        CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT);
        UPDATE jobs SET state='paused',message='上次未完成，可继续下载' WHERE state IN ('pending','running','pausing');
        UPDATE jobs SET state='cancelled' WHERE state='cancelling';")?;
        Ok(Self { db })
    }
    pub fn setting(&self, key: &str, default: &str) -> String {
        self.db
            .query_row("SELECT value FROM settings WHERE key=?", [key], |r| {
                r.get(0)
            })
            .unwrap_or_else(|_| default.to_owned())
    }
    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO settings VALUES (?,?)",
            params![key, value],
        )?;
        Ok(())
    }
    pub fn ensure_available(&self, note: &Note, except_id: &str) -> Result<(), String> {
        let mut query=self.db.prepare("SELECT payload FROM jobs WHERE note_id=? AND id<>? AND state IN ('pending','running','pausing','cancelling')").map_err(|e|e.to_string())?;
        let rows = query
            .query_map([note.id.as_str(), except_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let active: Note = serde_json::from_str(&row.map_err(|e| e.to_string())?)
                .map_err(|_| "队列数据异常，请重启程序")?;
            if note.assets.is_empty()
                || active.assets.is_empty()
                || note.assets.iter().any(|new| {
                    active.assets.iter().any(|old| {
                        new.url == old.url
                            || (new.sequence > 0
                                && old.sequence == new.sequence
                                && old.video == new.video
                                && old.live == new.live)
                    })
                })
            {
                return Err("所选图片已在下载队列中".into());
            }
        }
        Ok(())
    }
    pub fn add(&self, note: &Note, root: &Path) -> Result<String, String> {
        self.ensure_available(note, "")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("{}-{nonce}", note.id);
        if !crate::model::folder_is_safe(root, &note.id) {
            return Err("笔记目录无效".into());
        }
        // Each attempt owns its temporary files and metadata, including partial albums.
        let folder = root.join(&note.id).join(&id);
        self.db.execute("INSERT INTO jobs(id,note_id,title,author,state,message,folder,payload,created) VALUES (?,?,?,?,'pending','等待下载',?,?,?)",params![id,note.id,note.title,note.author,folder.to_string_lossy(),serde_json::to_string(note).map_err(|e|e.to_string())?,now as i64]).map_err(|e|e.to_string())?;
        Ok(id)
    }
    pub fn jobs(&self) -> rusqlite::Result<Vec<Job>> {
        let mut q=self.db.prepare("SELECT j.id,j.note_id,j.title,j.author,j.state,j.message,j.folder,j.done,j.total,COUNT(f.idx) FROM jobs j LEFT JOIN files f ON j.id=f.job_id WHERE j.state NOT IN ('done','cancelled') OR j.id IN (SELECT id FROM jobs WHERE state IN ('done','cancelled') ORDER BY created DESC LIMIT 200) GROUP BY j.id ORDER BY j.created DESC")?;
        let rows = q.query_map([], |r| {
            Ok(Job {
                id: r.get(0)?,
                note_id: r.get(1)?,
                title: r.get(2)?,
                author: r.get(3)?,
                state: r.get(4)?,
                message: r.get(5)?,
                folder: r.get(6)?,
                done: r.get::<_, i64>(7)?.max(0) as u64,
                total: r.get::<_, i64>(8)?.max(0) as u64,
                files: r.get::<_, i64>(9)?.max(0) as usize,
            })
        })?;
        rows.collect()
    }
    pub fn payload(&self, id: &str) -> Result<(Note, PathBuf, String), String> {
        let (s, p, state): (String, String, String) = self
            .db
            .query_row(
                "SELECT payload,folder,state FROM jobs WHERE id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| "找不到任务".to_owned())?;
        Ok((
            serde_json::from_str(&s).map_err(|_| "任务数据损坏")?,
            p.into(),
            state,
        ))
    }
    pub fn state(&self, id: &str, state: &str, message: &str) {
        let _ = self.db.execute(
            "UPDATE jobs SET state=?,message=? WHERE id=?",
            params![state, message, id],
        );
    }
    pub fn progress(&self, id: &str, done: u64, total: u64) {
        let _ = self.db.execute(
            "UPDATE jobs SET done=?,total=? WHERE id=?",
            params![done as i64, total as i64, id],
        );
    }
    pub fn file(&self, id: &str, index: usize) -> Option<(PathBuf, u64)> {
        self.db
            .query_row(
                "SELECT path,size FROM files WHERE job_id=? AND idx=?",
                params![id, index as i64],
                |r| {
                    Ok((
                        PathBuf::from(r.get::<_, String>(0)?),
                        r.get::<_, i64>(1)?.max(0) as u64,
                    ))
                },
            )
            .ok()
    }
    pub fn file_done(&self, id: &str, index: usize, path: &Path, size: u64) {
        let _ = self.db.execute(
            "INSERT OR REPLACE INTO files VALUES (?,?,?,?)",
            params![id, index as i64, path.to_string_lossy(), size as i64],
        );
    }
    pub fn busy(&self) -> bool {
        self.db.query_row("SELECT COUNT(*) FROM jobs WHERE state IN ('pending','running','pausing','cancelling')",[],|r|r.get::<_,i64>(0)).unwrap_or(0)>0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gesture_queue_allows_different_images_but_blocks_overlap() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let mut note = Note {
            id: "abcdef0123456789abcdef01".into(),
            assets: vec![crate::model::Asset {
                url: "https://sns-img-bd.xhscdn.com/a".into(),
                sequence: 1,
                ..Default::default()
            }],
            ..Default::default()
        };
        let root = Path::new("D:/Downloads");
        let first = store.add(&note, root).unwrap();
        note.assets[0].sequence = 2;
        note.assets[0].url = "https://sns-img-bd.xhscdn.com/b".into();
        let second = store.add(&note, root).unwrap();
        assert!(store.ensure_available(&note, &second).is_ok());
        note.assets[0].sequence = 1;
        note.assets[0].url = "https://sns-img-bd.xhscdn.com/a-new-signature".into();
        assert!(store.add(&note, root).is_err());
        assert_eq!(store.jobs().unwrap().len(), 2);
        store.state(&first, "paused", "");
        assert!(store.add(&note, root).is_ok());
        assert!(store.ensure_available(&note, &first).is_err());
    }

    #[test]
    fn attempts_are_isolated_and_active_jobs_are_not_truncated() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let root = std::env::temp_dir();
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            ..Default::default()
        };
        let first = s.add(&note, &root).unwrap();
        s.state(&first, "done", "");
        let second = s.add(&note, &root).unwrap();
        assert_ne!(s.payload(&first).unwrap().1, s.payload(&second).unwrap().1);
        s.state(&second, "paused", "");
        for _ in 0..205 {
            let id = s.add(&note, &root).unwrap();
            s.state(&id, "done", "");
        }
        let jobs = s.jobs().unwrap();
        assert_eq!(jobs.len(), 201);
        assert!(jobs.iter().any(|job| job.id == second));
    }
    #[test]
    fn recovery_and_duplicate() {
        let root = std::env::temp_dir().join(format!("xhs-rust-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("jobs.db");
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "测试".into(),
            ..Default::default()
        };
        let id;
        {
            let s = Store::open(&path).unwrap();
            id = s.add(&note, &root).unwrap();
            assert!(s.add(&note, &root).is_err());
            s.state(&id, "running", "");
        }
        {
            let s = Store::open(&path).unwrap();
            assert_eq!(s.payload(&id).unwrap().2, "paused");
            assert!(!s.busy());
            assert_eq!(s.jobs().unwrap().len(), 1);
        }
        let _ = std::fs::remove_dir_all(root);
    }
}
