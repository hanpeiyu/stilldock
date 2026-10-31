//! Bounded history lookup. Database snapshots are read under lock; filesystem IO happens afterwards.
use crate::{
    download::SharedStore,
    model::{media_keys, Asset, Note},
    store::{SavedFile, Store},
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
};
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub asset: Asset,
    pub record: SavedFile,
}
pub struct Archive {
    pub entries: Vec<Entry>,
    active: Vec<Asset>,
    index: HashMap<String, Vec<usize>>,
    queued: HashSet<String>,
}
fn file_state(path: &std::path::Path, bytes: u64) -> &'static str {
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && bytes > 0 && m.len() == bytes => "present",
        Ok(_) => "changed",
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Reuse the same offline-drive/permission distinction as task-level checks.
            let target = crate::local_files::Target {
                id: String::new(),
                folder: path.parent().unwrap_or(path).into(),
                expected: 1,
                files: vec![(path.into(), bytes)],
            };
            if crate::local_files::check(&target).unavailable > 0 {
                "unavailable"
            } else {
                "missing"
            }
        }
        Err(_) => "unavailable",
    }
}
impl Archive {
    pub fn load(store: &Store, note_id: &str) -> Result<Self, String> {
        let mut entries = Vec::new();
        let mut active = Vec::new();
        for (id, state, note) in store.note_history(note_id)? {
            if ["pending", "running", "pausing", "cancelling"].contains(&state.as_str()) {
                active.extend(note.assets.clone());
            }
            for (path, record) in store.file_records(&id)? {
                if let Some(asset) = note.assets.get(record.index) {
                    entries.push(Entry {
                        path,
                        asset: asset.clone(),
                        record,
                    });
                }
                if entries.len() >= 4000 {
                    break;
                }
            }
            if entries.len() >= 4000 {
                break;
            }
        }
        Ok(Self {
            entries,
            active,
            index: HashMap::new(),
            queued: HashSet::new(),
        })
    }
    pub fn indexed(mut self) -> Self {
        for (i, entry) in self.entries.iter().enumerate() {
            for key in media_keys(&entry.asset) {
                self.index.entry(key).or_default().push(i);
            }
        }
        self.queued = self.active.iter().flat_map(media_keys).collect();
        self.active.clear();
        self
    }
    fn candidates(&self, asset: &Asset) -> Vec<&Entry> {
        let mut indices: Vec<_> = media_keys(asset)
            .iter()
            .flat_map(|key| self.index.get(key).into_iter().flatten())
            .copied()
            .collect();
        indices.sort_unstable();
        indices.dedup();
        indices.into_iter().map(|i| &self.entries[i]).collect()
    }
    pub fn reusable(&self, asset: &Asset) -> Option<Entry> {
        self.candidates(asset)
            .into_iter()
            .find(|e| {
                file_state(&e.path, e.record.bytes) == "present"
                    && (asset.video
                        || (e.record.width as u64 * e.record.height as u64)
                            >= asset.width as u64 * asset.height as u64)
                    && (asset.video
                        || asset.original_url.is_empty()
                        || e.record.source == "high_resolution")
            })
            .cloned()
    }
    fn status(
        &self,
        asset: &Asset,
        cache: &mut HashMap<(PathBuf, u64), &'static str>,
    ) -> (String, Option<PathBuf>) {
        let mut best = "new";
        let mut rank = 0;
        for entry in self.candidates(asset) {
            let state = *cache
                .entry((entry.path.clone(), entry.record.bytes))
                .or_insert_with(|| file_state(&entry.path, entry.record.bytes));
            let priority = match state {
                "present" => 5,
                "unavailable" => 3,
                "changed" => 2,
                "missing" => 1,
                _ => 0,
            };
            if state == "present" {
                return (state.into(), Some(entry.path.clone()));
            }
            if priority > rank {
                best = state;
                rank = priority;
            }
        }
        if media_keys(asset)
            .iter()
            .any(|key| self.queued.contains(key))
        {
            return ("queued".into(), None);
        }
        (best.into(), None)
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct CardState {
    pub index: usize,
    pub state: String,
    pub live_state: Option<String>,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub note_id: String,
    pub expected_urls: Vec<String>,
    pub assets: Vec<CardState>,
}
pub fn inspect(store: &SharedStore, note: &Note) -> Snapshot {
    let archive = {
        let db = store.lock().unwrap();
        Archive::load(&db, &note.id)
    }
    .map(Archive::indexed);
    let mut cache = HashMap::new();
    let assets = note
        .assets
        .iter()
        .enumerate()
        .map(|(index, asset)| {
            let (state, path) = archive
                .as_ref()
                .map(|a| a.status(asset, &mut cache))
                .unwrap_or(("unavailable".into(), None));
            let live_state = if !asset.live_url.is_empty() {
                let live = Asset {
                    url: asset.live_url.clone(),
                    video: true,
                    live: true,
                    sequence: asset.sequence,
                    ..Default::default()
                };
                Some(
                    archive
                        .as_ref()
                        .map(|a| a.status(&live, &mut cache).0)
                        .unwrap_or("unavailable".into()),
                )
            } else {
                None
            };
            CardState {
                index,
                state,
                live_state,
                path,
            }
        })
        .collect();
    Snapshot {
        note_id: note.id.clone(),
        expected_urls: note.assets.iter().map(|a| a.url.clone()).collect(),
        assets,
    }
}
pub struct Checker {
    latest: Arc<Mutex<Option<Note>>>,
    wake: mpsc::SyncSender<()>,
}
impl Checker {
    pub fn start(store: SharedStore, report: impl Fn(Snapshot) + Send + 'static) -> Self {
        let latest = Arc::new(Mutex::new(None::<Note>));
        let pending = latest.clone();
        let (wake, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                let note = pending.lock().unwrap().take();
                if let Some(note) = note {
                    report(inspect(&store, &note));
                }
            }
        });
        Self { latest, wake }
    }
    pub fn request(&self, note: &Note) {
        if note.id.is_empty() {
            return;
        }
        *self.latest.lock().unwrap() = Some(note.clone());
        let _ = self.wake.try_send(());
    }
}
