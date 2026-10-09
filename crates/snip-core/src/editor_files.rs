//! The editors' own files beside history: the editor's preferences
//! (editor.json), its saved colours (colors.json), the Background tool's
//! presets (backgrounds.json, with the preset applied to every screenshot,
//! BG-03) and the user's own wallpapers (wallpapers/, BG-01). One owner for
//! each: QML reads and writes the JSON whole, the engine reads the
//! auto-applied preset from here.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// One of the editors' JSON files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    Prefs,
    Colors,
    Backgrounds,
}

impl File {
    fn name(self) -> &'static str {
        match self {
            File::Prefs => "editor.json",
            File::Colors => "colors.json",
            File::Backgrounds => "backgrounds.json",
        }
    }

    /// What it holds when there's none yet: a list of colours, else an object.
    fn empty(self) -> &'static str {
        if self == File::Colors { "[]" } else { "{}" }
    }
}

/// The file's JSON, or its empty value.
pub fn read(f: File) -> String {
    read_in(&crate::paths::data_dir(), f)
}

/// Write it whole. JSON only: a file never ends up unreadable.
pub fn write(f: File, json: &str) -> Result<(), String> {
    write_in(&crate::paths::data_dir(), f, json)
}

/// The preset to apply to every new screenshot (BG-03), as its background.
pub fn auto_background() -> Option<Value> {
    auto_background_in(&crate::paths::data_dir())
}

/// The user's own wallpapers, by path, in order.
pub fn wallpapers() -> Vec<String> {
    wallpapers_in(&crate::paths::data_dir())
}

/// Keep a copy of an image as one of the user's wallpapers (PNG, JPEG or
/// WebP): its path, or None.
pub fn add_wallpaper(src: &Path) -> Option<PathBuf> {
    add_wallpaper_in(&crate::paths::data_dir(), src)
}

/// One of the user's own wallpapers goes (nothing outside their folder can).
pub fn remove_wallpaper(path: &Path) {
    remove_wallpaper_in(&crate::paths::data_dir(), path)
}

// ---------------------------------------------------------------- in a data dir

fn read_in(dir: &Path, f: File) -> String {
    std::fs::read_to_string(dir.join(f.name())).unwrap_or_else(|_| f.empty().into())
}

fn write_in(dir: &Path, f: File, json: &str) -> Result<(), String> {
    serde_json::from_str::<Value>(json).map_err(|e| format!("{}: {e}", f.name()))?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    crate::store::atomic_write(&dir.join(f.name()), json.as_bytes()).map_err(|e| e.to_string())
}

fn auto_background_in(dir: &Path) -> Option<Value> {
    let j: Value = serde_json::from_str(&read_in(dir, File::Backgrounds)).ok()?;
    let name = j["auto"].as_str()?;
    j["presets"].as_array()?.iter().find(|p| p["name"] == name).map(|p| p["bg"].clone())
}

fn wallpaper_dir(dir: &Path) -> PathBuf {
    dir.join("wallpapers")
}

fn wallpapers_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(wallpaper_dir(dir))
        .map(|d| d.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    v.sort();
    v
}

fn add_wallpaper_in(dir: &Path, src: &Path) -> Option<PathBuf> {
    let image = matches!(src.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp"));
    let to = wallpaper_dir(dir);
    let name = src.file_name()?.to_string_lossy().into_owned();
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
    let dst = to.join(format!("{ms}-{name}"));
    (image && std::fs::create_dir_all(&to).is_ok() && std::fs::copy(src, &dst).is_ok()).then_some(dst)
}

fn remove_wallpaper_in(dir: &Path, path: &Path) {
    if path.parent() == Some(wallpaper_dir(dir).as_path()) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snip-editor-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn each_file_reads_back_what_was_written_and_only_json_is() {
        let d = dir("json");
        assert_eq!((read_in(&d, File::Colors), read_in(&d, File::Prefs)), ("[]".into(), "{}".into()), "empty until written");
        write_in(&d, File::Colors, r##"["#ff0000"]"##).unwrap();
        assert_eq!(read_in(&d, File::Colors), r##"["#ff0000"]"##);
        assert!(write_in(&d, File::Prefs, "{ not json").is_err(), "never unreadable");
        assert_eq!(read_in(&d, File::Prefs), "{}");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn the_auto_preset_is_the_named_one() {
        let d = dir("auto");
        assert_eq!(auto_background_in(&d), None, "no file");
        write_in(&d, File::Backgrounds, r#"{"presets":[{"name":"A","bg":{"padding":8}},{"name":"B","bg":{"padding":40}}],"auto":"B"}"#).unwrap();
        assert_eq!(auto_background_in(&d), Some(serde_json::json!({ "padding": 40 })));
        write_in(&d, File::Backgrounds, r#"{"presets":[{"name":"A","bg":{}}],"auto":null}"#).unwrap();
        assert_eq!(auto_background_in(&d), None, "none chosen");
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn wallpapers_are_copies_of_images_and_only_those_go() {
        let d = dir("walls");
        let src = d.join("sea.png");
        std::fs::write(&src, b"png").unwrap();
        std::fs::write(d.join("notes.txt"), b"txt").unwrap();
        let w = add_wallpaper_in(&d, &src).expect("an image is kept");
        assert!(src.exists() && w.exists() && w.parent() == Some(wallpaper_dir(&d).as_path()));
        assert_eq!(add_wallpaper_in(&d, &d.join("notes.txt")), None, "not an image");
        assert_eq!(wallpapers_in(&d), vec![w.to_string_lossy().into_owned()]);
        remove_wallpaper_in(&d, &src);
        assert!(src.exists(), "a file outside their folder stays");
        remove_wallpaper_in(&d, &w);
        assert!(wallpapers_in(&d).is_empty());
        std::fs::remove_dir_all(d).unwrap();
    }
}
