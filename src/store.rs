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

#[derive(Clone, Debug)]
pub struct DownloadOptions {
    pub layout: String,
    pub temp_folder: PathBuf,
    pub export_json: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SavedFile {
    pub index: usize,
    pub sequence: usize,
    pub live: bool,
    pub video: bool,
    pub filename: String,
    pub bytes: u64,
    pub width: usize,
    pub height: usize,
    pub format: String,
    pub source: String,
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
        let migration = db.unchecked_transaction()?;
        for (table, definitions) in [
            (
                "jobs",
                vec![
                    ("layout", "TEXT NOT NULL DEFAULT 'legacy'"),
                    ("temp_folder", "TEXT NOT NULL DEFAULT ''"),
                    ("export_json", "INTEGER NOT NULL DEFAULT 0"),
                ],
            ),
            (
                "files",
                vec![
                    ("width", "INTEGER NOT NULL DEFAULT 0"),
                    ("height", "INTEGER NOT NULL DEFAULT 0"),
                    ("format", "TEXT NOT NULL DEFAULT ''"),
                    ("source", "TEXT NOT NULL DEFAULT ''"),
                    ("sequence", "INTEGER NOT NULL DEFAULT 0"),
                    ("live", "INTEGER NOT NULL DEFAULT 0"),
                    ("video", "INTEGER NOT NULL DEFAULT 0"),
                ],
            ),
        ] {
            let columns: Vec<String> = {
                let mut statement = migration.prepare(&format!("PRAGMA table_info({table})"))?;
                let rows = statement.query_map([], |row| row.get(1))?;
                rows.collect::<Result<_, _>>()?
            };
            for (column, definition) in definitions {
                if !columns.iter().any(|name| name == column) {
                    migration.execute_batch(&format!(
                        "ALTER TABLE {table} ADD COLUMN {column} {definition};"
                    ))?;
                }
            }
        }
        migration.commit()?;
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
        let layout = if self.setting("output_layout", "note") == "flat" {
            "flat"
        } else {
            "note"
        };
        let folder = if layout == "flat" {
            root.to_path_buf()
        } else {
            root.join(&note.id)
        };
        let temp_folder = folder.join(".stilldock-cache").join(&id);
        let export_json = self.setting("export_json", "false") == "true";
        self.db.execute("INSERT INTO jobs(id,note_id,title,author,state,message,folder,payload,created,layout,temp_folder,export_json) VALUES (?,?,?,?,'pending','等待下载',?,?,?,?,?,?)",params![id,note.id,note.title,note.author,folder.to_string_lossy(),serde_json::to_string(note).map_err(|e|e.to_string())?,now as i64,layout,temp_folder.to_string_lossy(),export_json]).map_err(|e|e.to_string())?;
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
    pub fn download_options(&self, id: &str) -> Result<DownloadOptions, String> {
        let options = self
            .db
            .query_row(
                "SELECT layout,temp_folder,export_json,folder FROM jobs WHERE id=?",
                [id],
                |row| {
                    let temporary: String = row.get(1)?;
                    Ok(DownloadOptions {
                        layout: row.get(0)?,
                        temp_folder: PathBuf::from(if temporary.is_empty() {
                            row.get::<_, String>(3)?
                        } else {
                            temporary
                        }),
                        export_json: row.get(2)?,
                    })
                },
            )
            .map_err(|_| "无法读取任务保存设置".to_owned())?;
        let folder = self.payload(id)?.1;
        if options.layout != "legacy"
            && options.temp_folder != folder.join(".stilldock-cache").join(id)
        {
            return Err("任务缓存目录异常".into());
        }
        Ok(options)
    }
    pub fn saved_files(&self, id: &str) -> Result<Vec<SavedFile>, String> {
        let mut query=self.db.prepare("SELECT idx,sequence,live,video,path,size,width,height,format,source FROM files WHERE job_id=? ORDER BY idx").map_err(|e|e.to_string())?;
        let rows = query
            .query_map([id], |row| {
                let path: String = row.get(4)?;
                Ok(SavedFile {
                    index: row.get::<_, i64>(0)? as usize,
                    sequence: row.get::<_, i64>(1)? as usize,
                    live: row.get(2)?,
                    video: row.get(3)?,
                    filename: Path::new(&path)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    bytes: row.get::<_, i64>(5)? as u64,
                    width: row.get::<_, i64>(6)? as usize,
                    height: row.get::<_, i64>(7)? as usize,
                    format: row.get(8)?,
                    source: row.get(9)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    pub fn file_done(
        &self,
        id: &str,
        index: usize,
        path: &Path,
        record: &SavedFile,
    ) -> Result<(), String> {
        self.db.execute("INSERT OR REPLACE INTO files(job_id,idx,path,size,width,height,format,source,sequence,live,video) VALUES(?,?,?,?,?,?,?,?,?,?,?)",params![id,index as i64,path.to_string_lossy(),record.bytes as i64,record.width as i64,record.height as i64,record.format,record.source,record.sequence as i64,record.live,record.video]).map_err(|_|"无法记录下载信息，请检查数据目录空间")?;
        Ok(())
    }
    pub fn busy(&self) -> bool {
        self.db.query_row("SELECT COUNT(*) FROM jobs WHERE state IN ('pending','running','pausing','cancelling')",[],|r|r.get::<_,i64>(0)).unwrap_or(0)>0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_settings_are_frozen_per_job_and_caches_are_isolated() {
        let store = Store::open(Path::new(":memory:")).unwrap();
        let root = Path::new("D:/Downloads");
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            ..Default::default()
        };
        let grouped = store.add(&note, root).unwrap();
        store.state(&grouped, "paused", "");
        assert_eq!(store.payload(&grouped).unwrap().1, root.join(&note.id));
        assert!(!store.download_options(&grouped).unwrap().export_json);
        store.set_setting("output_layout", "flat").unwrap();
        store.set_setting("export_json", "true").unwrap();
        let flat = store.add(&note, root).unwrap();
        store.state(&flat, "done", "");
        let another = store.add(&note, root).unwrap();
        assert_eq!(store.payload(&flat).unwrap().1, root);
        assert!(store.download_options(&flat).unwrap().export_json);
        assert_eq!(store.download_options(&grouped).unwrap().layout, "note");
        assert_ne!(
            store.download_options(&flat).unwrap().temp_folder,
            store.download_options(&another).unwrap().temp_folder
        );
    }
    #[test]
    fn old_database_migrates_without_moving_files_or_changing_resume_path() {
        let root = std::env::temp_dir().join(format!("stilldock-schema-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("old.db");
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            title: "old".into(),
            ..Default::default()
        };
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch("CREATE TABLE jobs(id TEXT PRIMARY KEY,note_id TEXT,title TEXT,author TEXT,state TEXT,message TEXT,folder TEXT,payload TEXT,done INTEGER DEFAULT 0,total INTEGER DEFAULT 0,created INTEGER);CREATE TABLE files(job_id TEXT,idx INTEGER,path TEXT,size INTEGER,PRIMARY KEY(job_id,idx));").unwrap();
            old.execute("INSERT INTO jobs(id,note_id,title,author,state,message,folder,payload,created) VALUES('old',?,'old','','running','',?,?,1)",params![note.id,root.to_string_lossy(),serde_json::to_string(&note).unwrap()]).unwrap();
            old.execute(
                "INSERT INTO files VALUES('old',0,?,42)",
                [root.join("saved.jpg").to_string_lossy().as_ref()],
            )
            .unwrap();
        }
        {
            let migrated = Store::open(&path).unwrap();
            let options = migrated.download_options("old").unwrap();
            assert_eq!(options.layout, "legacy");
            assert_eq!(options.temp_folder, root);
            assert!(!options.export_json);
            assert_eq!(migrated.payload("old").unwrap().2, "paused");
            assert_eq!(migrated.saved_files("old").unwrap()[0].bytes, 42);
        }
        let again = Store::open(&path).unwrap();
        assert_eq!(again.download_options("old").unwrap().temp_folder, root);
        drop(again);
        let _ = std::fs::remove_dir_all(root);
    }

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
    fn partial_caches_are_isolated_and_active_jobs_are_not_truncated() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let root = std::env::temp_dir();
        let note = Note {
            id: "abcdef0123456789abcdef01".into(),
            ..Default::default()
        };
        let first = s.add(&note, &root).unwrap();
        s.state(&first, "done", "");
        let second = s.add(&note, &root).unwrap();
        assert_eq!(s.payload(&first).unwrap().1, s.payload(&second).unwrap().1);
        assert_ne!(
            s.download_options(&first).unwrap().temp_folder,
            s.download_options(&second).unwrap().temp_folder
        );
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
