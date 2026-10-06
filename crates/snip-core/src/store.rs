//! Capture lifecycle (product §5.0, decided 2026-10-06):
//!   capture -> Snip history only (not the user's folders, not the clipboard)
//!   Copy    -> clipboard only (the caller offers the bytes this returns)
//!   Save    -> export folder (+ clipboard when `save_also_copies`)

use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::config::Config;
use crate::history::{History, Record};
use crate::naming::{self, NameContext};

#[derive(Clone, Debug, Default)]
pub struct CaptureMeta {
    pub kind: String,
    pub output: String,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub struct Store {
    history: Mutex<History>,
    /// Warm tier: the fast-PNG bytes of recent captures, so Copy is instant.
    warm: Mutex<VecDeque<(String, Arc<Vec<u8>>)>>,
    warm_budget: usize,
    counter_file: PathBuf,
}

#[derive(Debug)]
pub enum StoreError {
    NotFound(String),
    Io(std::io::Error),
    Db(rusqlite::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::NotFound(id) => write!(f, "capture {id} not found"),
            StoreError::Io(e) => write!(f, "{e}"),
            StoreError::Db(e) => write!(f, "history database: {e}"),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Db(e)
    }
}

/// Write via temp file + fsync + rename: never a partially written file and
/// never an in-place overwrite (aCropalypse class of bugs).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let tmp = dir.join(format!(".{}.snip-tmp", path.file_name().and_then(|n| n.to_str()).unwrap_or("x")));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

impl Store {
    pub fn open(history_root: &Path, counter_file: PathBuf) -> Result<Self, StoreError> {
        Ok(Self {
            history: Mutex::new(History::open(history_root)?),
            warm: Mutex::new(VecDeque::new()),
            warm_budget: 96 * 1024 * 1024,
            counter_file,
        })
    }

    fn warm_put(&self, id: &str, bytes: Arc<Vec<u8>>) {
        let mut w = self.warm.lock().unwrap();
        w.retain(|(i, _)| i != id);
        w.push_front((id.to_owned(), bytes));
        let mut total: usize = w.iter().map(|(_, b)| b.len()).sum();
        while total > self.warm_budget && w.len() > 1 {
            if let Some((_, b)) = w.pop_back() {
                total -= b.len();
            }
        }
    }

    /// Persist a new capture in history. Returns its record.
    pub fn ingest(&self, png: Vec<u8>, width: u32, height: u32, meta: CaptureMeta, now_secs: i64) -> Result<Record, StoreError> {
        let id = ulid::Ulid::new().to_string().to_lowercase();
        let h = self.history.lock().unwrap();
        let dir = h.dir_for(&id);
        crate::paths::ensure_private_dir(&dir)?;
        let source = dir.join("source.png");
        atomic_write(&source, &png)?;
        let rec = Record {
            id: id.clone(),
            created_at: now_secs,
            kind: meta.kind,
            width,
            height,
            output: meta.output,
            source,
            saved: None,
            app: meta.app,
            title: meta.title,
        };
        h.insert(&rec)?;
        drop(h);
        self.warm_put(&id, Arc::new(png));
        Ok(rec)
    }

    pub fn get(&self, id: &str) -> Result<Record, StoreError> {
        self.history.lock().unwrap().get(id)?.ok_or_else(|| StoreError::NotFound(id.into()))
    }

    pub fn recent(&self, n: u32) -> Result<Vec<Record>, StoreError> {
        Ok(self.history.lock().unwrap().recent(n)?)
    }

    /// PNG bytes for Copy/drag: warm tier, else from history.
    pub fn bytes(&self, id: &str) -> Result<Arc<Vec<u8>>, StoreError> {
        if let Some(b) = self.warm.lock().unwrap().iter().find(|(i, _)| i == id).map(|(_, b)| b.clone()) {
            return Ok(b);
        }
        let rec = self.get(id)?;
        let b = Arc::new(std::fs::read(&rec.source)?);
        self.warm_put(id, b.clone());
        Ok(b)
    }

    fn next_counter(&self) -> u64 {
        let n = std::fs::read_to_string(&self.counter_file).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0u64) + 1;
        if let Some(d) = self.counter_file.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&self.counter_file, n.to_string());
        n
    }

    /// Save to the export folder with the name template. Never overwrites.
    pub fn save(&self, id: &str, cfg: &Config, now: chrono::DateTime<chrono::Local>) -> Result<PathBuf, StoreError> {
        let rec = self.get(id)?;
        if let Some(p) = rec.saved.as_ref().filter(|p| p.exists()) {
            return Ok(p.clone()); // already saved: Save is idempotent
        }
        let bytes = self.bytes(id)?;
        let dir = cfg.export_dir();
        std::fs::create_dir_all(&dir)?;
        let ctx = NameContext {
            window_title: rec.title.clone(),
            app_name: rec.app.clone(),
            counter: if cfg.general.file_name.contains("%i") { self.next_counter() } else { 0 },
            random: id.chars().rev().take(4).collect(),
        };
        let stem = naming::render(&cfg.general.file_name, now, &ctx);
        let path = naming::unique_path(&dir, &stem, "png");
        atomic_write(&path, &bytes)?;
        self.history.lock().unwrap().set_saved(id, &path)?;
        Ok(path)
    }

    pub fn delete(&self, id: &str) -> Result<(), StoreError> {
        self.warm.lock().unwrap().retain(|(i, _)| i != id);
        Ok(self.history.lock().unwrap().delete(id)?)
    }

    pub fn sweep(&self, now_secs: i64, max_age_secs: Option<i64>) -> Result<usize, StoreError> {
        Ok(self.history.lock().unwrap().sweep(now_secs, max_age_secs)?)
    }

    pub fn disk_usage(&self) -> u64 {
        self.history.lock().unwrap().disk_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("snip-store-{}", ulid::Ulid::new()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn lifecycle_capture_copy_save() {
        let root = tmp();
        let store = Store::open(&root.join("history"), root.join("counter")).unwrap();
        let rec = store.ingest(b"\x89PNGfake".to_vec(), 10, 5, CaptureMeta { kind: "area".into(), ..Default::default() }, 1000).unwrap();
        // capture -> history only
        assert!(rec.source.exists() && rec.saved.is_none());
        // copy -> bytes
        assert_eq!(&**store.bytes(&rec.id).unwrap(), b"\x89PNGfake");
        // save -> export dir with template, idempotent, never overwrites a foreign file
        let mut cfg = Config::default();
        cfg.general.export_dir = root.join("Pictures").to_string_lossy().into_owned();
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 6, 21, 5, 9).unwrap();
        std::fs::create_dir_all(cfg.export_dir()).unwrap();
        std::fs::write(cfg.export_dir().join("Snip 2026-10-06 at 21.05.09.png"), b"someone else's").unwrap();
        let p = store.save(&rec.id, &cfg, now).unwrap();
        assert_eq!(p.file_name().unwrap(), "Snip 2026-10-06 at 21.05.09 (2).png");
        assert_eq!(std::fs::read(&p).unwrap(), b"\x89PNGfake");
        assert_eq!(store.save(&rec.id, &cfg, now).unwrap(), p);
        assert_eq!(store.get(&rec.id).unwrap().saved, Some(p));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bytes_fall_back_to_disk_after_warm_eviction() {
        let root = tmp();
        let mut store = Store::open(&root.join("history"), root.join("counter")).unwrap();
        store.warm_budget = 4; // force eviction
        let a = store.ingest(vec![1; 8], 1, 1, CaptureMeta::default(), 1).unwrap();
        let _b = store.ingest(vec![2; 8], 1, 1, CaptureMeta::default(), 2).unwrap();
        assert_eq!(&**store.bytes(&a.id).unwrap(), &[1u8; 8]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
