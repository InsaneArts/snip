//! Capture history: Snip's private store and the default home of every
//! capture (product §5.0, HIS-04). One directory per capture plus a SQLite
//! index (WAL). Retention is enforced by `sweep`.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Record {
    pub id: String,
    pub created_at: i64,
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub output: String,
    pub source: PathBuf,
    pub saved: Option<PathBuf>,
    pub app: Option<String>,
    pub title: Option<String>,
}

pub struct History {
    root: PathBuf,
    db: Connection,
}

impl History {
    pub fn open(root: &Path) -> rusqlite::Result<Self> {
        let _ = crate::paths::ensure_private_dir(root);
        let db = Connection::open(root.join("history.db"))?;
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS captures (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                kind TEXT NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                output TEXT NOT NULL,
                source TEXT NOT NULL,
                saved TEXT,
                app TEXT,
                title TEXT
            );
            CREATE INDEX IF NOT EXISTS captures_created ON captures(created_at);",
        )?;
        // HIS-05 (added 2026-10-07): the text of every capture, searchable.
        let has_indexed: bool = db
            .prepare("SELECT 1 FROM pragma_table_info('captures') WHERE name = 'indexed'")?
            .exists([])?;
        if !has_indexed {
            db.execute_batch(
                "ALTER TABLE captures ADD COLUMN ocr_text TEXT;
                 ALTER TABLE captures ADD COLUMN indexed INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        db.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS captures_fts
               USING fts5(id UNINDEXED, body, tokenize = 'unicode61 remove_diacritics 2');",
        )?;
        Ok(Self { root: root.to_path_buf(), db })
    }

    /// Captures whose text is not indexed yet, newest first.
    pub fn unindexed(&self, limit: u32) -> rusqlite::Result<Vec<(String, PathBuf)>> {
        let mut st = self.db.prepare("SELECT id, source FROM captures WHERE indexed = 0 ORDER BY created_at DESC LIMIT ?1")?;
        let rows = st.query_map([limit], |r| Ok((r.get::<_, String>(0)?, PathBuf::from(r.get::<_, String>(1)?))))?;
        rows.collect()
    }

    pub fn unindexed_count(&self) -> rusqlite::Result<u64> {
        self.db.query_row("SELECT COUNT(*) FROM captures WHERE indexed = 0", [], |r| r.get(0))
    }

    /// Store a capture's recognised text; the search body also carries the
    /// window title and app so "firefox" or a page title finds it too.
    pub fn set_index(&self, id: &str, text: &str) -> rusqlite::Result<()> {
        let rec = match self.get(id)? {
            Some(r) => r,
            None => return Ok(()),
        };
        let body = [rec.title.as_deref().unwrap_or(""), rec.app.as_deref().unwrap_or(""), text].join("\n");
        self.db.execute("UPDATE captures SET ocr_text = ?2, indexed = 1 WHERE id = ?1", params![id, text])?;
        self.db.execute("DELETE FROM captures_fts WHERE id = ?1", [id])?;
        self.db.execute("INSERT INTO captures_fts (id, body) VALUES (?1, ?2)", params![id, body])?;
        Ok(())
    }

    /// Forget a capture's recognised text (its pixels changed, say a
    /// redaction): the indexer reads it again.
    pub fn reset_index(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("UPDATE captures SET ocr_text = NULL, indexed = 0 WHERE id = ?1", [id])?;
        self.db.execute("DELETE FROM captures_fts WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Full-text search: every word must match (as a prefix), best first.
    /// Returns records with a short snippet around the match.
    pub fn search(&self, query: &str, limit: u32) -> rusqlite::Result<Vec<(Record, String)>> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|t| t.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == '.' || *c == '@').collect::<String>())
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{}\"*", t.replace('"', "")))
            .collect();
        if terms.is_empty() {
            return Ok(vec![]);
        }
        let cols = Self::COLS.split(", ").map(|c| format!("c.{c}")).collect::<Vec<_>>().join(", ");
        let sql = format!(
            "SELECT {cols}, snippet(captures_fts, 1, '[', ']', '…', 12)
             FROM captures_fts JOIN captures c ON c.id = captures_fts.id
             WHERE captures_fts MATCH ?1 ORDER BY rank LIMIT ?2"
        );
        let mut st = self.db.prepare(&sql)?;
        let rows = st.query_map(params![terms.join(" "), limit], |r| Ok((Self::row(r)?, r.get::<_, String>(10)?)))?;
        rows.collect()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir_for(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    pub fn insert(&self, r: &Record) -> rusqlite::Result<()> {
        self.db.execute(
            "INSERT INTO captures (id, created_at, kind, width, height, output, source, saved, app, title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                r.id,
                r.created_at,
                r.kind,
                r.width,
                r.height,
                r.output,
                r.source.to_string_lossy(),
                r.saved.as_ref().map(|p| p.to_string_lossy().into_owned()),
                r.app,
                r.title
            ],
        )?;
        Ok(())
    }

    fn row(r: &rusqlite::Row) -> rusqlite::Result<Record> {
        Ok(Record {
            id: r.get(0)?,
            created_at: r.get(1)?,
            kind: r.get(2)?,
            width: r.get(3)?,
            height: r.get(4)?,
            output: r.get(5)?,
            source: PathBuf::from(r.get::<_, String>(6)?),
            saved: r.get::<_, Option<String>>(7)?.map(PathBuf::from),
            app: r.get(8)?,
            title: r.get(9)?,
        })
    }

    const COLS: &'static str = "id, created_at, kind, width, height, output, source, saved, app, title";

    pub fn get(&self, id: &str) -> rusqlite::Result<Option<Record>> {
        self.db
            .query_row(&format!("SELECT {} FROM captures WHERE id = ?1", Self::COLS), [id], Self::row)
            .optional()
    }

    pub fn recent(&self, limit: u32) -> rusqlite::Result<Vec<Record>> {
        let mut st = self.db.prepare(&format!("SELECT {} FROM captures ORDER BY created_at DESC, id DESC LIMIT ?1", Self::COLS))?;
        let rows = st.query_map([limit], Self::row)?;
        rows.collect()
    }

    pub fn set_size(&self, id: &str, width: u32, height: u32) -> rusqlite::Result<()> {
        self.db.execute("UPDATE captures SET width = ?2, height = ?3 WHERE id = ?1", params![id, width, height])?;
        Ok(())
    }

    pub fn set_saved(&self, id: &str, path: &Path) -> rusqlite::Result<()> {
        self.db.execute("UPDATE captures SET saved = ?2 WHERE id = ?1", params![id, path.to_string_lossy()])?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> rusqlite::Result<()> {
        self.db.execute("DELETE FROM captures WHERE id = ?1", [id])?;
        self.db.execute("DELETE FROM captures_fts WHERE id = ?1", [id])?;
        let _ = std::fs::remove_dir_all(self.dir_for(id));
        Ok(())
    }

    /// Delete captures older than `max_age_secs` (relative to `now`). Saved
    /// files in the user's folders are never touched. Returns how many went.
    pub fn sweep(&self, now: i64, max_age_secs: Option<i64>) -> rusqlite::Result<usize> {
        let Some(age) = max_age_secs else { return Ok(0) };
        let cutoff = now - age;
        let ids: Vec<String> = {
            let mut st = self.db.prepare("SELECT id FROM captures WHERE created_at < ?1")?;
            let rows = st.query_map([cutoff], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        for id in &ids {
            self.delete(id)?;
        }
        Ok(ids.len())
    }

    /// Bytes used by the store (Settings shows this).
    pub fn disk_usage(&self) -> u64 {
        fn walk(p: &Path) -> u64 {
            std::fs::read_dir(p)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| {
                            let m = e.metadata().ok();
                            if m.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                                walk(&e.path())
                            } else {
                                m.map(|m| m.len()).unwrap_or(0)
                            }
                        })
                        .sum()
                })
                .unwrap_or(0)
        }
        walk(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("snip-hist-{}-{}", std::process::id(), ulid::Ulid::new()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn rec(id: &str, t: i64, root: &Path) -> Record {
        let dir = root.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("source.png"), b"png").unwrap();
        Record {
            id: id.into(),
            created_at: t,
            kind: "area".into(),
            width: 10,
            height: 5,
            output: "DP-2".into(),
            source: dir.join("source.png"),
            saved: None,
            app: Some("zen".into()),
            title: None,
        }
    }

    #[test]
    fn insert_get_recent_saved() {
        let root = tmp();
        let h = History::open(&root).unwrap();
        h.insert(&rec("a", 100, &root)).unwrap();
        h.insert(&rec("b", 200, &root)).unwrap();
        assert_eq!(h.recent(10).unwrap().iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        h.set_saved("a", Path::new("/x/y.png")).unwrap();
        assert_eq!(h.get("a").unwrap().unwrap().saved, Some(PathBuf::from("/x/y.png")));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn search_finds_text_title_and_georgian() {
        let root = tmp();
        let h = History::open(&root).unwrap();
        h.insert(&rec("a", 100, &root)).unwrap();
        h.insert(&rec("b", 200, &root)).unwrap();
        assert_eq!(h.unindexed_count().unwrap(), 2);
        h.set_index("a", "error: connection refused on port 5432").unwrap();
        h.set_index("b", "გამარჯობა მსოფლიო invoice total").unwrap();
        assert_eq!(h.unindexed_count().unwrap(), 0);
        let ids = |q: &str| h.search(q, 10).unwrap().into_iter().map(|(r, _)| r.id).collect::<Vec<_>>();
        assert_eq!(ids("connection ref"), ["a"]);      // prefix match
        assert_eq!(ids("გამარჯ"), ["b"]);               // Georgian
        let mut by_app = ids("zen");                   // the app name is searchable too
        by_app.sort();
        assert_eq!(by_app, ["a", "b"]);
        assert!(ids("nothing-like-this").is_empty());
        let (_, snip) = &h.search("refused", 1).unwrap()[0];
        assert!(snip.contains("[refused]"), "{snip}");
        h.delete("a").unwrap();
        assert!(ids("connection").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sweep_respects_retention_and_forever() {
        let root = tmp();
        let h = History::open(&root).unwrap();
        h.insert(&rec("old", 1_000, &root)).unwrap();
        h.insert(&rec("new", 9_000, &root)).unwrap();
        assert_eq!(h.sweep(10_000, None).unwrap(), 0); // Forever
        assert_eq!(h.sweep(10_000, Some(5_000)).unwrap(), 1);
        assert!(h.get("old").unwrap().is_none() && !root.join("old").exists());
        assert!(h.get("new").unwrap().is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
}
