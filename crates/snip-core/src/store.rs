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

/// Next to an edited capture's `source.png` (the flattened result): the image
/// before any edit, and the editor's objects (ANN-16).
const ORIGINAL: &str = "original.png";
const EDIT: &str = "edit.json";
/// Next to a recording's video (or GIF): its first frame, for cards and history.
const POSTER: &str = "poster.png";

#[derive(Clone, Debug, Default)]
pub struct CaptureMeta {
    pub kind: String,
    /// physical px per logical px (1 for imports)
    pub scale: f64,
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
            scale: meta.scale,
            output: meta.output,
            source,
            saved: None,
            app: meta.app,
            title: meta.title,
            duration_ms: 0,
        };
        h.insert(&rec)?;
        drop(h);
        self.warm_put(&id, Arc::new(png));
        Ok(rec)
    }

    /// Persist a recording (or an opened video or GIF) in history: the file
    /// itself as its source, keeping its extension, and its first frame as the
    /// poster. `take` moves the file in (a recording's own); otherwise it's
    /// copied (a file of the user's).
    pub fn ingest_media(&self, file: &Path, take: bool, poster: &[u8], width: u32, height: u32, duration_ms: u64, meta: CaptureMeta, now_secs: i64) -> Result<Record, StoreError> {
        let id = ulid::Ulid::new().to_string().to_lowercase();
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_ascii_lowercase();
        let h = self.history.lock().unwrap();
        let dir = h.dir_for(&id);
        crate::paths::ensure_private_dir(&dir)?;
        let source = dir.join(format!("source.{ext}"));
        if !(take && std::fs::rename(file, &source).is_ok()) {
            std::fs::copy(file, &source)?;
            if take {
                let _ = std::fs::remove_file(file);
            }
        }
        atomic_write(&dir.join(POSTER), poster)?;
        let rec = Record {
            id,
            created_at: now_secs,
            kind: meta.kind,
            width,
            height,
            scale: meta.scale,
            output: meta.output,
            source,
            saved: None,
            app: meta.app,
            title: meta.title,
            duration_ms,
        };
        h.insert(&rec)?;
        Ok(rec)
    }

    /// A recording's poster (its first frame); None for images.
    pub fn poster(rec: &Record) -> Option<PathBuf> {
        (rec.media() != crate::history::Media::Image).then(|| rec.source.with_file_name(POSTER))
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
        // a recording is saved as it is: its own format, never re-encoded
        let media = rec.media() != crate::history::Media::Image;
        let ext = if media { rec.source.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_string() } else { cfg.export_format().ext().to_string() };
        let ext = ext.as_str();
        if let Some(p) = rec.saved.as_ref().filter(|p| p.exists() && p.extension().is_some_and(|e| e == ext)) {
            return Ok(p.clone()); // already saved in this format: Save is idempotent
        }
        let dir = if media { cfg.recording_dir() } else { cfg.export_dir() };
        std::fs::create_dir_all(&dir)?;
        let ctx = NameContext {
            window_title: rec.title.clone(),
            app_name: rec.app.clone(),
            counter: if cfg.general.file_name.contains("%i") { self.next_counter() } else { 0 },
            random: id.chars().rev().take(4).collect(),
        };
        let stem = cfg.file_stem(now, rec.scale, &ctx);
        let path = naming::unique_path(&dir, &stem, ext);
        if media {
            let tmp = path.with_extension(format!("{ext}.tmp"));
            std::fs::copy(&rec.source, &tmp)?;
            std::fs::rename(&tmp, &path)?;
        } else {
            let out = crate::encode::transcode(&self.bytes(id)?, cfg.export_format()).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
            atomic_write(&path, &out)?;
        }
        self.history.lock().unwrap().set_saved(id, &path)?;
        Ok(path)
    }

    /// Captures still waiting for text indexing (HIS-05), newest first.
    /// Save As (QAO-09, ANN-15): to a path the user picked. The format follows
    /// its extension (png, jpg/jpeg, webp; none: .png is added). The file
    /// becomes the capture's saved file, so later edits reach it.
    pub fn save_as(&self, id: &str, path: &Path, cfg: &Config) -> Result<PathBuf, StoreError> {
        let rec = self.get(id)?;
        if rec.media() != crate::history::Media::Image {
            // a recording keeps its own format (export converts, VED-02)
            let own = rec.source.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
            let path = if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(own)) { path.to_path_buf() } else { PathBuf::from(format!("{}.{own}", path.display())) };
            let tmp = path.with_extension(format!("{own}.tmp"));
            std::fs::copy(&rec.source, &tmp)?;
            std::fs::rename(&tmp, &path)?;
            self.history.lock().unwrap().set_saved(id, &path)?;
            return Ok(path);
        }
        let ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
        if ext.as_deref() == Some("snip") {
            return self.save_project(id, path);
        }
        let (fmt, path) = match ext.as_deref() {
            Some("jpg") | Some("jpeg") => (crate::encode::Export::Jpg(cfg.general.jpg_quality), path.to_path_buf()),
            Some("webp") => (crate::encode::Export::Webp, path.to_path_buf()),
            Some("png") => (crate::encode::Export::Png, path.to_path_buf()),
            _ => (crate::encode::Export::Png, PathBuf::from(format!("{}.png", path.display()))),
        };
        let out = crate::encode::transcode(&self.bytes(id)?, fmt).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        atomic_write(&path, &out)?;
        self.history.lock().unwrap().set_saved(id, &path)?;
        Ok(path)
    }

    /// Save As a project (`.snip`, ANN-15): the original, the objects, the
    /// files they draw from and the flattened image, so it opens editable.
    pub fn save_project(&self, id: &str, path: &Path) -> Result<PathBuf, StoreError> {
        let rec = self.get(id)?;
        let dir = rec.source.parent().unwrap_or(Path::new(".")).to_path_buf();
        let original = dir.join(ORIGINAL);
        let source = std::fs::read(if original.exists() { &original } else { &rec.source })?;
        let document = std::fs::read_to_string(dir.join(EDIT)).unwrap_or_else(|_| r#"{"v":1,"items":[]}"#.into());
        let preview = std::fs::read(&rec.source)?;
        let assets = crate::project::document_files(&document).into_iter().filter_map(|n| std::fs::read(dir.join(&n)).ok().map(|b| (n, b))).collect();
        let thumb = thumbnail(&preview).unwrap_or_default();
        let bytes = crate::project::to_bytes(&crate::project::Project { source, document, preview, assets }, &thumb)?;
        atomic_write(path, &bytes)?;
        self.history.lock().unwrap().set_saved(id, path)?;
        Ok(path.to_path_buf())
    }

    /// A project file becomes a capture whose objects stay editable: its
    /// flattened image is the capture, beside it the original, the objects
    /// and their files.
    pub fn import_project(&self, path: &Path, output: &str, now_secs: i64) -> Result<Record, StoreError> {
        let p = crate::project::read(path)?;
        let (_, w, h, _) = crate::encode::decode_png(&p.preview).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
        let title = path.file_stem().map(|s| s.to_string_lossy().into_owned());
        let rec = self.ingest(p.preview, w, h, CaptureMeta { kind: "import".into(), scale: 1.0, output: output.into(), app: None, title }, now_secs)?;
        let dir = rec.source.parent().unwrap_or(Path::new(".")).to_path_buf();
        atomic_write(&dir.join(ORIGINAL), &p.source)?;
        atomic_write(&dir.join(EDIT), p.document.as_bytes())?;
        for (name, data) in p.assets {
            atomic_write(&dir.join(name), &data)?;
        }
        Ok(rec)
    }

    pub fn unindexed(&self, limit: u32) -> Result<Vec<(String, PathBuf)>, StoreError> {
        Ok(self.history.lock().unwrap().unindexed(limit)?)
    }

    pub fn unindexed_count(&self) -> u64 {
        self.history.lock().unwrap().unindexed_count().unwrap_or(0)
    }

    pub fn set_index(&self, id: &str, text: &str) -> Result<(), StoreError> {
        Ok(self.history.lock().unwrap().set_index(id, text)?)
    }

    pub fn search(&self, query: &str, limit: u32) -> Result<Vec<(Record, String)>, StoreError> {
        Ok(self.history.lock().unwrap().search(query, limit)?)
    }

    /// Replace a capture's image (rotate, flip; later the editor). The saved
    /// file, if any, is rewritten too, in its own format.
    /// The capture was resized: physical px per logical px of what it shows.
    pub fn set_scale(&self, id: &str, scale: f64) -> Result<(), StoreError> {
        Ok(self.history.lock().unwrap().set_scale(id, scale)?)
    }

    pub fn replace_image(&self, id: &str, png: Vec<u8>, width: u32, height: u32, cfg: &Config) -> Result<Record, StoreError> {
        let rec = self.get(id)?;
        atomic_write(&rec.source, &png)?;
        if let Some(saved) = rec.saved.as_ref().filter(|p| p.exists()) {
            let fmt = match saved.extension().and_then(|e| e.to_str()) {
                Some("jpg") | Some("jpeg") => crate::encode::Export::Jpg(cfg.general.jpg_quality),
                Some("webp") => crate::encode::Export::Webp,
                _ => crate::encode::Export::Png,
            };
            let out = crate::encode::transcode(&png, fmt).map_err(|e| StoreError::Io(std::io::Error::other(e)))?;
            atomic_write(saved, &out)?;
        }
        self.history.lock().unwrap().set_size(id, width, height)?;
        self.warm_put(id, Arc::new(png));
        self.get(id)
    }

    /// HIS-01: a project is a capture with objects still editable.
    pub fn is_project(&self, rec: &Record) -> bool {
        rec.source.parent().is_some_and(|d| d.join(EDIT).exists())
    }

    /// What the editor opens (ANN-16): the image to draw on (the original
    /// when the capture was edited before) and its objects, if any.
    pub fn edit_state(&self, id: &str) -> Result<(PathBuf, Option<String>), StoreError> {
        let rec = self.get(id)?;
        let dir = rec.source.parent().unwrap_or(Path::new(".")).to_path_buf();
        let original = dir.join(ORIGINAL);
        if !original.exists() {
            return Ok((rec.source, None));
        }
        Ok((original, std::fs::read_to_string(dir.join(EDIT)).ok()))
    }

    /// Write an edit back (ANN-15): the original is kept once, next to the
    /// flattened image, together with the objects that made it. The text
    /// index is redone, so redacted text can't be found any more.
    pub fn write_edit(&self, id: &str, png: Vec<u8>, width: u32, height: u32, doc: &str, cfg: &Config) -> Result<Record, StoreError> {
        let rec = self.get(id)?;
        let dir = rec.source.parent().unwrap_or(Path::new(".")).to_path_buf();
        let original = dir.join(ORIGINAL);
        if !original.exists() {
            std::fs::copy(&rec.source, &original)?;
        }
        atomic_write(&dir.join(EDIT), doc.as_bytes())?;
        let rec = self.replace_image(id, png, width, height, cfg)?;
        self.history.lock().unwrap().reset_index(id)?;
        Ok(rec)
    }

    /// The capture's pixels changed outside the editor (rotate, flip): its
    /// current image becomes the one to edit, and old objects are dropped.
    pub fn flatten(&self, id: &str) -> Result<(), StoreError> {
        let rec = self.get(id)?;
        let dir = rec.source.parent().unwrap_or(Path::new(".")).to_path_buf();
        for f in [ORIGINAL, EDIT] {
            match std::fs::remove_file(dir.join(f)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
                _ => {}
            }
        }
        Ok(())
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

/// A small WebP of a PNG (at most 256 px on its long side), for a project.
fn thumbnail(png: &[u8]) -> Result<Vec<u8>, String> {
    let (data, w, h, alpha) = crate::encode::decode_png(png)?;
    let f = (256.0 / w.max(h) as f64).min(1.0);
    let (nw, nh) = (((w as f64 * f).round() as u32).max(1), ((h as f64 * f).round() as u32).max(1));
    let small = crate::encode::resize(&data, w, h, if alpha { 4 } else { 3 }, nw, nh);
    crate::encode::transcode(&crate::encode::encode_png(&small, nw, nh, alpha), crate::encode::Export::Webp)
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
    fn a_recording_is_kept_and_saved_as_it_is() {
        let root = tmp();
        let store = Store::open(&root.join("history"), root.join("counter")).unwrap();
        // a recording's own file is moved in; a user's file is copied
        let rec_file = root.join("take.mp4");
        std::fs::write(&rec_file, b"video bytes").unwrap();
        let rec = store.ingest_media(&rec_file, true, b"\x89PNGposter", 320, 200, 4200, CaptureMeta { kind: "recording".into(), scale: 1.25, ..Default::default() }, 1000).unwrap();
        assert!(!rec_file.exists(), "moved in");
        assert_eq!((rec.media(), rec.duration_ms), (crate::history::Media::Video, 4200));
        assert_eq!(rec.source.file_name().unwrap(), "source.mp4");
        assert_eq!(std::fs::read(Store::poster(&rec).unwrap()).unwrap(), b"\x89PNGposter");
        assert_eq!(store.get(&rec.id).unwrap(), rec, "the record round-trips, duration included");
        let users = root.join("cat.gif");
        std::fs::write(&users, b"gif bytes").unwrap();
        let gif = store.ingest_media(&users, false, b"p", 10, 10, 900, CaptureMeta { kind: "import".into(), ..Default::default() }, 1001).unwrap();
        assert!(users.exists(), "a user's file stays");
        assert_eq!(gif.media(), crate::history::Media::Gif);
        // Save copies the file as it is, with its own extension
        let mut cfg = Config::default();
        cfg.general.export_dir = root.join("Pictures").to_string_lossy().into_owned();
        cfg.recording.export_dir = root.join("Videos").to_string_lossy().into_owned();
        let now = chrono::Local.with_ymd_and_hms(2026, 10, 9, 4, 5, 6).unwrap();
        let p = store.save(&rec.id, &cfg, now).unwrap();
        assert_eq!(p, root.join("Videos/Snip 2026-10-09 at 04.05.06.mp4"), "recordings go to their own folder");
        assert_eq!(std::fs::read(&p).unwrap(), b"video bytes");
        assert_eq!(store.save(&rec.id, &cfg, now).unwrap(), p, "idempotent");
        assert!(Store::poster(&store.get(&gif.id).unwrap()).is_some() && Store::poster(&store.ingest(b"x".to_vec(), 1, 1, CaptureMeta::default(), 1).unwrap()).is_none());
        std::fs::remove_dir_all(root).unwrap();
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
    fn save_as_follows_the_extension() {
        let root = tmp();
        let store = Store::open(&root.join("history"), root.join("counter")).unwrap();
        let png = crate::encode::encode_png(&[200u8; 4 * 3 * 3], 4, 3, false);
        let rec = store.ingest(png, 4, 3, CaptureMeta::default(), 0).unwrap();
        let cfg = Config::default();
        let jpg = store.save_as(&rec.id, &root.join("shot.JPG"), &cfg).unwrap();
        assert_eq!(&std::fs::read(&jpg).unwrap()[..2], b"\xff\xd8", "a JPEG");
        let bare = store.save_as(&rec.id, &root.join("shot"), &cfg).unwrap();
        assert_eq!(bare, root.join("shot.png"));
        assert_eq!(&std::fs::read(&bare).unwrap()[1..4], b"PNG");
        assert_eq!(store.get(&rec.id).unwrap().saved, Some(bare), "the last Save As is the saved file");
    }

    #[test]
    fn edits_keep_the_original_and_objects() {
        let root = tmp();
        let store = Store::open(&root.join("history"), root.join("counter")).unwrap();
        let rec = store.ingest(b"before".to_vec(), 4, 3, CaptureMeta::default(), 0).unwrap();
        assert_eq!(store.edit_state(&rec.id).unwrap(), (rec.source.clone(), None));
        store.history.lock().unwrap().set_index(&rec.id, "secret token").unwrap();

        let cfg = Config::default();
        store.write_edit(&rec.id, b"after1".to_vec(), 4, 3, "[1]", &cfg).unwrap();
        store.write_edit(&rec.id, b"after2".to_vec(), 2, 3, "[2]", &cfg).unwrap();
        let (img, doc) = store.edit_state(&rec.id).unwrap();
        assert_eq!(std::fs::read(&img).unwrap(), b"before", "the original is kept from the first edit");
        assert_eq!(doc.as_deref(), Some("[2]"));
        assert_eq!(&**store.bytes(&rec.id).unwrap(), b"after2");
        assert_eq!(store.get(&rec.id).unwrap().width, 2);
        assert!(store.search("secret", 5).unwrap().is_empty(), "edited pixels are indexed again");

        store.flatten(&rec.id).unwrap();
        assert_eq!(store.edit_state(&rec.id).unwrap(), (rec.source.clone(), None));
        assert_eq!(std::fs::read(&rec.source).unwrap(), b"after2");
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
