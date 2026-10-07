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
    /// nothing. Claude Code turns a pasted image path into an attachment.
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Ocr {
    /// Tesseract languages ("eng+kat"); empty = English plus installed models.
    pub languages: String,
    /// HIS-05: read the text of every capture in the background so history
    /// can be searched by it (`snip search`). Local only.
    pub index_history: bool,
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
}

impl Default for Annotate {
    fn default() -> Self {
        Self { object_shadows: true, inverse_arrows: false, smooth_drawing: true }
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
    /// CAP-05: window captures keep their transparent background; Shift in
    /// window mode inverts this (with wallpaper comes with M2's Background tool).
    pub window_shadow: bool,
    /// Draw Hyprland's corner rounding into window captures.
    pub window_rounding: bool,
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
    pub after_capture: AfterCaptureRules,
    pub quick_access: QuickAccess,
    pub history: History,
    pub clipboard: Clipboard,
    pub capture: Capture,
    pub ocr: Ocr,
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
        }
    }
}

impl Default for AfterCaptureRules {
    fn default() -> Self {
        Self { screenshot: vec![AfterCapture::ShowQao] }
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
        Self { mode: ClipboardMode::Image, text_path: true, sensitive: false }
    }
}

impl Default for Ocr {
    fn default() -> Self {
        Self { languages: String::new(), index_history: true }
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
            window_shadow: true,
            window_rounding: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_decisions() {
        let c = Config::default();
        assert_eq!(c.after_capture.screenshot, vec![AfterCapture::ShowQao]);
        assert!(c.general.save_also_copies);
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
}
