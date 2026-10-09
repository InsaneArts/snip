//! `~/.config/snip/config.toml`: the single source of truth for settings
//! (product SET-01). Unknown keys are ignored with a warning; missing keys use
//! defaults; edits made by the settings UI go through `toml_edit` so the
//! user's comments and ordering survive.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// Where Save writes. Default: $XDG_PICTURES_DIR/Screenshots
    pub export_dir: String,
    pub file_name: String,
    pub format: ImageFormat,
    /// JPG quality (1-100)
    pub jpg_quality: u8,
    /// Decided 2026-10-06: Save writes the file *and* copies it.
    pub save_also_copies: bool,
    pub sounds: bool,
    /// Annotate / double-click: empty opens Snip's editor; a command (say
    /// "satty --filename") opens that editor with the image path instead.
    pub external_editor: String,
    /// SET-04: the time in file names is UTC
    pub utc_time: bool,
    /// SET-04: file names of HiDPI captures end in "@2x" (their scale)
    pub scale_suffix: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    Jpg,
    Webp,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AfterCapture {
    ShowQao,
    Copy,
    Save,
    Annotate,
    Pin,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct AfterCaptureRules {
    /// Capture lifecycle §5.0: by default only the Quick Access card appears.
    pub screenshot: Vec<AfterCapture>,
    /// CAP-02: Capture Area & Copy / & Save / & Annotate do just that, without
    /// these rules (CleanShot's setting); off, they add theirs to the rules.
    pub variants_ignore_rules: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct QuickAccess {
    pub position: Side,
    pub auto_close: bool,
    pub auto_close_seconds: u32,
    /// What auto-close does (QAO-07). The capture stays in history either way.
    pub auto_close_action: AutoCloseAction,
    pub max_visible: u32,
    /// card width in logical px (CleanShot "overlay size" slider)
    pub size: u32,
    /// CleanShot "Close after dragging" (Alt while dragging keeps the card)
    pub close_after_drag: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoCloseAction {
    Close,
    SaveAndClose,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Retention {
    Never,
    #[serde(rename = "1d")]
    Day1,
    #[serde(rename = "3d")]
    Days3,
    #[serde(rename = "1w")]
    Week1,
    #[serde(rename = "1m")]
    Month1,
    Forever,
}

impl Retention {
    pub fn max_age_secs(self) -> Option<i64> {
        match self {
            Retention::Never => Some(0),
            Retention::Day1 => Some(86_400),
            Retention::Days3 => Some(3 * 86_400),
            Retention::Week1 => Some(7 * 86_400),
            Retention::Month1 => Some(30 * 86_400),
            Retention::Forever => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct History {
    pub retention: Retention,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Clipboard {
    /// What Copy offers (SET-07).
    pub mode: ClipboardMode,
    /// Also offer the file's path as text, so pasting where only text is
    /// accepted (terminals, plain text fields) gives the path instead of
    /// nothing. Off by default (D38): browsers then hand web apps the text
    /// too (Zen lists it first), and many paste the path, not the image.
    pub text_path: bool,
    /// Offer copies with x-kde-passwordManagerHint so Omarchy's clipboard
    /// history skips them (SET-07b).
    pub sensitive: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClipboardMode {
    Image,
    File,
    ImageAndFile,
}

/// Screen recording (REC).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Recording {
    /// Where Save writes recordings. Default: $XDG_VIDEOS_DIR/Screencasts
    pub export_dir: String,
    /// REC-12: frames per second
    pub fps: u32,
    /// REC-12: gpu-screen-recorder's quality: medium, high, very_high, ultra
    pub quality: String,
    /// REC-12: the largest video ("" as recorded, or "2160p", "1440p", "1080p", "720p")
    pub max_size: String,
    /// CAP-11 for videos: the pointer in recordings
    pub show_cursor: bool,
    /// REC-03: seconds counted down before a recording starts (0: none)
    pub countdown: u32,
    /// REC-11: notifications held while recording
    pub dnd: bool,
    /// REC-04: the screen outside the area dimmed (else only a border)
    pub dim: bool,
    /// REC-07: the microphone: "" none, "default_input", or a device's name
    pub mic: String,
    /// REC-07: what the computer plays
    pub system_audio: bool,
    /// REC-07: one track per source (for other editors) instead of one mixed
    pub separate_tracks: bool,
    /// REC-13: a GIF's frames per second and width (height follows)
    pub gif_fps: u32,
    pub gif_width: u32,
    /// REC-02: the last kind chosen in the toolbar: "video" or "gif"
    pub kind: String,
    /// REC-08: the camera shown in the recording, by its name ("" none)
    pub camera: String,
    /// REC-08: "circle", "square", "rectangle" or "vertical"
    pub camera_shape: String,
    /// REC-08: "small", "medium", "large" or "huge", for the recorded area
    pub camera_size: String,
    /// REC-08: shown as in a mirror
    pub camera_mirror: bool,
    /// REC-09: a ring where the pointer clicks
    pub highlight_clicks: bool,
    /// REC-10: the command keys pressed (never plain typing)
    pub show_keystrokes: bool,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            export_dir: crate::paths::default_recording_dir().to_string_lossy().into_owned(),
            fps: 60,
            quality: "very_high".into(),
            max_size: String::new(),
            show_cursor: true,
            countdown: 3,
            dnd: true,
            dim: true,
            mic: String::new(),
            system_audio: false,
            separate_tracks: false,
            gif_fps: 15,
            gif_width: 800,
            kind: "video".into(),
            camera: String::new(),
            camera_shape: "circle".into(),
            camera_size: "medium".into(),
            camera_mirror: true,
            highlight_clicks: false,
            show_keystrokes: false,
        }
    }
}

impl Recording {
    /// gsr's `-s` for `max_size`: a 16:9 box the video is scaled to fit in.
    pub fn max_box(&self) -> Option<(u32, u32)> {
        match self.max_size.as_str() {
            "2160p" => Some((3840, 2160)),
            "1440p" => Some((2560, 1440)),
            "1080p" => Some((1920, 1080)),
            "720p" => Some((1280, 720)),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Ocr {
    /// Tesseract languages ("eng+kat"); empty = English plus installed models.
    pub languages: String,
    /// HIS-05: read the text of every capture in the background so history
    /// can be searched by it (`snip search`). Local only.
    pub index_history: bool,
    /// OCR-02: Capture Text keeps the text's line breaks (off: one line per
    /// paragraph). "Capture text as one line" ignores it.
    pub keep_line_breaks: bool,
    /// OCR-05: offer to open a link found in the text
    pub detect_links: bool,
    /// OCR-03: what a link (in a code or the text) does: ask, always, never
    pub open_links: OpenLinks,
    /// OCR-03: read 1D barcodes too (EAN, UPC, Code 128…); off by default,
    /// since they misread busy UI. QR, Data Matrix, Aztec and PDF417 always.
    pub barcodes: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OpenLinks {
    Ask,
    Always,
    Never,
}

/// The editor's settings (CleanShot: Settings › Annotate).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Annotate {
    /// "Draw shadow on objects": a soft shadow under every object.
    pub object_shadows: bool,
    /// "Inverse arrow direction": arrows point where the drag started;
    /// Alt while drawing flips it either way.
    pub inverse_arrows: bool,
    /// "Smooth drawing": the pencil drops points that add nothing.
    pub smooth_drawing: bool,
    /// "Auto-expand canvas" (ANN-17): an object drawn past the canvas grows
    /// it, filled with the colour around the image's edge.
    pub auto_expand: bool,
    /// "Remember if the Background tool was opened" (ANN-18): an editor
    /// opens with it when the last one closed with it.
    pub remember_background_tool: bool,
    /// "Always on top" (ANN-18): the editor is pinned (Hyprland: above
    /// other windows, on every workspace).
    pub always_on_top: bool,
}

impl Default for Annotate {
    fn default() -> Self {
        Self { object_shadows: true, inverse_arrows: false, smooth_drawing: true, auto_expand: false, remember_background_tool: false, always_on_top: false }
    }
}

/// How pins look (PIN-08; CleanShot: Settings › Pin).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Pins {
    /// Corner radius in px (0: square)
    pub corner_radius: u32,
    /// A soft shadow around each pin
    pub shadow: bool,
    /// A hairline border around each pin
    pub border: bool,
}

impl Default for Pins {
    fn default() -> Self {
        Self { corner_radius: 10, shadow: true, border: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Capture {
    pub freeze: bool,
    /// CAP-11: include the mouse pointer in screen captures.
    pub show_cursor: bool,
    /// CAP-06: countdown of the self-timer shortcuts, in seconds.
    pub timer_seconds: u32,
    /// Keep the GPU context warm this long after the last capture (S01).
    pub keep_warm_minutes: u32,
    /// SEL-04: full-screen crosshair lines with a coordinate readout.
    pub crosshair: Crosshair,
    /// SEL-05: loupe with pixel grid and hex colour (shown with the crosshair).
    pub magnifier: bool,
    /// SEL-03: the size readout in logical px instead of physical
    pub logical_size: bool,
    /// CAP-05: window captures keep their transparent background; Shift in
    /// window mode inverts this (with wallpaper comes with M2's Background tool).
    pub window_shadow: bool,
    /// Draw Hyprland's corner rounding into window captures.
    pub window_rounding: bool,
    /// "With the desktop" (product owner, 2026-10-08): a window capture is
    /// the window as it is on screen with the desktop around it (wallpaper,
    /// other windows, the bar), not the clean window. Shift inverts it.
    pub window_desktop: bool,
    /// CAP-12 Hide desktop clutter: screen captures show the wallpaper where
    /// bars are, and no pins (cards never show)
    pub hide_desktop: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Crosshair {
    Off,
    Always,
    /// only while Ctrl is held
    Ctrl,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub general: General,
    pub annotate: Annotate,
    pub pins: Pins,
    pub after_capture: AfterCaptureRules,
    pub quick_access: QuickAccess,
    pub history: History,
    pub clipboard: Clipboard,
    pub capture: Capture,
    pub recording: Recording,
    pub ocr: Ocr,
    /// Settings › Shortcuts (SET-08): an action's key over snip.lua's
    /// default ("SUPER + SHIFT + code:13"; "" for none). snipd binds them.
    pub shortcuts: std::collections::BTreeMap<String, String>,
}

impl Default for General {
    fn default() -> Self {
        Self {
            export_dir: crate::paths::default_screenshot_dir().to_string_lossy().into_owned(),
            file_name: crate::naming::DEFAULT_TEMPLATE.into(),
            format: ImageFormat::Png,
            jpg_quality: 90,
            save_also_copies: true,
            sounds: true,
            external_editor: String::new(),
            utc_time: false,
            scale_suffix: false,
        }
    }
}

impl Default for AfterCaptureRules {
    fn default() -> Self {
        Self { screenshot: vec![AfterCapture::ShowQao], variants_ignore_rules: true }
    }
}

impl Default for QuickAccess {
    fn default() -> Self {
        Self {
            position: Side::Left,
            auto_close: true,
            auto_close_seconds: 10,
            auto_close_action: AutoCloseAction::Close,
            max_visible: 5,
            size: 280,
            close_after_drag: true,
        }
    }
}

impl Default for History {
    fn default() -> Self {
        Self { retention: Retention::Week1 }
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self { mode: ClipboardMode::Image, text_path: false, sensitive: false }
    }
}

impl Default for Ocr {
    fn default() -> Self {
        Self { languages: String::new(), index_history: true, keep_line_breaks: true, detect_links: true, open_links: OpenLinks::Ask, barcodes: false }
    }
}

impl Default for Capture {
    fn default() -> Self {
        Self {
            freeze: true,
            show_cursor: false,
            timer_seconds: 5,
            keep_warm_minutes: 10,
            crosshair: Crosshair::Ctrl,
            magnifier: true,
            logical_size: false,
            window_shadow: true,
            window_rounding: true,
            window_desktop: false,
            hide_desktop: false,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        crate::paths::config_dir().join("config.toml")
    }

    /// Parse; on error return defaults plus the error so the UI can show it
    /// instead of refusing to start.
    pub fn parse(text: &str) -> (Config, Option<String>) {
        match toml_edit::de::from_str::<Config>(text) {
            Ok(c) => (c, None),
            Err(e) => (Config::default(), Some(e.to_string())),
        }
    }

    pub fn load() -> (Config, Option<String>) {
        match std::fs::read_to_string(Self::path()) {
            Ok(t) => Self::parse(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Config::default(), None),
            Err(e) => (Config::default(), Some(e.to_string())),
        }
    }

    pub fn export_dir(&self) -> PathBuf {
        crate::paths::expand(&self.general.export_dir)
    }

    /// Where Save writes a recording (video or GIF).
    pub fn recording_dir(&self) -> PathBuf {
        crate::paths::expand(&self.recording.export_dir)
    }

    /// SET-04: a capture's file name, without its extension.
    pub fn file_stem(&self, at: chrono::DateTime<chrono::Local>, scale: f64, ctx: &crate::naming::NameContext) -> String {
        let t = if self.general.utc_time { at.naive_utc() } else { at.naive_local() };
        let stem = crate::naming::render(&self.general.file_name, t, ctx);
        if self.general.scale_suffix { stem + &crate::naming::scale_suffix(scale) } else { stem }
    }

    pub fn export_format(&self) -> crate::encode::Export {
        match self.general.format {
            ImageFormat::Png => crate::encode::Export::Png,
            ImageFormat::Jpg => crate::encode::Export::Jpg(self.general.jpg_quality),
            ImageFormat::Webp => crate::encode::Export::Webp,
        }
    }
}

/// Set one dotted key (e.g. "quick_access.position") in the user's file,
/// preserving comments and ordering.
pub fn set_value(text: &str, dotted: &str, value: toml_edit::Item) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut parts = dotted.split('.').collect::<Vec<_>>();
    let key = parts.pop().ok_or("empty key")?;
    let mut table = doc.as_table_mut();
    for p in parts {
        table = table
            .entry(p)
            .or_insert(toml_edit::Item::Table(toml_edit::Table::new()))
            .as_table_mut()
            .ok_or_else(|| format!("{p} is not a table"))?;
    }
    table.insert(key, value);
    Ok(doc.to_string())
}

/// Remove one dotted key (or a whole table: "shortcuts") from the user's
/// file, preserving the rest.
pub fn remove_value(text: &str, dotted: &str) -> Result<String, String> {
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e: toml_edit::TomlError| e.to_string())?;
    let mut parts = dotted.split('.').collect::<Vec<_>>();
    let key = parts.pop().ok_or("empty key")?;
    let mut table = doc.as_table_mut();
    for p in parts {
        match table.get_mut(p).and_then(|i| i.as_table_mut()) {
            Some(t) => table = t,
            None => return Ok(doc.to_string()),
        }
    }
    table.remove(key);
    Ok(doc.to_string())
}

/// The settings window's write (SET-02): one setting from JSON (a bool,
/// number, string or list of strings). The file must still be a valid
/// config afterwards (known key, right type), or nothing changes.
pub fn set_json(text: &str, dotted: &str, value: &serde_json::Value) -> Result<String, String> {
    use serde_json::Value;
    let item = match value {
        Value::Bool(b) => toml_edit::value(*b),
        Value::Number(n) => match n.as_i64() {
            Some(i) => toml_edit::value(i),
            None => toml_edit::value(n.as_f64().unwrap_or_default()),
        },
        Value::String(s) => toml_edit::value(s.as_str()),
        Value::Array(a) => toml_edit::value(a.iter().map(|v| v.as_str().unwrap_or_default()).collect::<toml_edit::Array>()),
        _ => return Err(format!("{dotted}: unsupported value {value}")),
    };
    let out = set_value(text, dotted, item)?;
    match Config::parse(&out) {
        (_, Some(e)) => Err(e),
        _ => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_stem_in_utc_and_with_the_scale() {
        let at = chrono::DateTime::from_timestamp(1_791_400_000, 0).unwrap().with_timezone(&chrono::Local);
        let ctx = crate::naming::NameContext::default();
        let mut c = Config::default();
        c.general.file_name = "Shot %H.%M".into();
        assert_eq!(c.file_stem(at, 2.0, &ctx), at.format("Shot %H.%M").to_string());
        c.general.utc_time = true;
        c.general.scale_suffix = true;
        assert_eq!(c.file_stem(at, 2.0, &ctx), at.naive_utc().format("Shot %H.%M@2x").to_string());
        assert_eq!(c.file_stem(at, 1.0, &ctx), at.naive_utc().format("Shot %H.%M").to_string(), "no suffix at 1x");
    }

    #[test]
    fn defaults_follow_decisions() {
        let c = Config::default();
        assert_eq!(c.after_capture.screenshot, vec![AfterCapture::ShowQao]);
        assert!(c.general.save_also_copies);
        assert!(!c.clipboard.text_path, "D38: Copy offers the image only");
        assert_eq!(c.history.retention, Retention::Week1);
        assert_eq!(c.quick_access.position, Side::Left);
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let (c, err) = Config::parse("[quick_access]\nposition = \"right\"\n[history]\nretention = \"1m\"\n");
        assert!(err.is_none(), "{err:?}");
        assert_eq!(c.quick_access.position, Side::Right);
        assert_eq!(c.quick_access.auto_close_seconds, 10);
        assert_eq!(c.history.retention, Retention::Month1);
    }

    #[test]
    fn bad_file_falls_back_with_error() {
        let (c, err) = Config::parse("[quick_access]\nposition = 7\n");
        assert!(err.is_some());
        assert_eq!(c, Config::default());
    }

    #[test]
    fn set_value_preserves_comments() {
        let src = "# my settings\n[quick_access] # bottom card\nposition = \"left\" # keep\n";
        let out = set_value(src, "quick_access.position", toml_edit::value("right")).unwrap();
        assert!(out.contains("# my settings") && out.contains("# bottom card"));
        assert!(out.contains("position = \"right\""));
        let out = set_value(&out, "history.retention", toml_edit::value("1d")).unwrap();
        assert_eq!(Config::parse(&out).0.history.retention, Retention::Day1);
    }

    #[test]
    fn set_json_writes_only_valid_settings() {
        let src = "# mine\n[general]\nsounds = true # click\n";
        let out = set_json(src, "general.sounds", &serde_json::json!(false)).unwrap();
        assert!(out.contains("# mine") && out.contains("sounds = false"));
        let out = set_json(&out, "after_capture.screenshot", &serde_json::json!(["show_qao", "copy"])).unwrap();
        assert_eq!(Config::parse(&out).0.after_capture.screenshot, vec![AfterCapture::ShowQao, AfterCapture::Copy]);
        let out = set_json(&out, "quick_access.size", &serde_json::json!(320)).unwrap();
        assert_eq!(Config::parse(&out).0.quick_access.size, 320);
        assert!(set_json(&out, "quick_access.position", &serde_json::json!("middle")).is_err());
        assert!(set_json(&out, "general.nope", &serde_json::json!(1)).is_err());
    }

    #[test]
    fn shortcuts_are_set_and_removed() {
        let src = "# mine\n[general]\nsounds = true\n";
        let out = set_json(src, "shortcuts.all-in-one", &serde_json::json!("SUPER + SHIFT + code:14")).unwrap();
        let out = set_json(&out, "shortcuts.history", &serde_json::json!("")).unwrap();
        let c = Config::parse(&out).0;
        assert_eq!(c.shortcuts.get("all-in-one").map(String::as_str), Some("SUPER + SHIFT + code:14"));
        assert_eq!(c.shortcuts.get("history").map(String::as_str), Some(""));
        let out = remove_value(&out, "shortcuts").unwrap();
        assert!(Config::parse(&out).0.shortcuts.is_empty() && out.contains("# mine") && out.contains("sounds = true"), "{out}");
        assert_eq!(remove_value(&out, "shortcuts").unwrap(), out, "nothing to remove");
    }
}
