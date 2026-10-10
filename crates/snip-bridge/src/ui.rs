//! What the engine tells the UI, through one seam: Engine's signals and
//! properties in QML. In snipd the cxx-qt adapter queues each onto the Qt
//! thread (every QString is made here); a test keeps them in a list.

use cxx_qt::CxxQtThread;
use cxx_qt_lib::QString;
use snip_core::geometry::Rect;

use crate::bridge::qobject::Engine;

/// One thing the UI is told. JSON payloads stay JSON strings.
#[derive(Clone, Debug, PartialEq)]
pub enum Ui {
    /// A selection's overlays open (its `session` JSON, under `epoch`).
    Session { json: String, epoch: i32 },
    CancelRequested,
    /// The self-timer over `rect` (logical px; -1, -1 for the whole output).
    Countdown { output: String, rect: Rect, seconds: i32 },
    CountdownCancel,
    /// Snip's surfaces step aside before capture `request`.
    PrepareCapture(i32),
    CaptureDone,
    CardAdd(String),
    CardUpdate { id: String, json: String },
    CardsCommand(String),
    CardStatus { id: String, status: String, detail: String },
    Toast { text: String, output: String },
    BackgroundAuto(String),
    PinOpen(String),
    PinsCommand(String),
    LinkOffer { url: String, output: String },
    DesktopCover(String),
    HistoryOpen(String),
    SettingsOpen(String),
    WelcomeOpen,
    EditorOpen(String),
    EditorWords { path: String, json: String },
    EditorScreenshot { token: i32, path: String },
    EditorColor { token: i32, color: String },
    EditorDone { id: String, action: String, ok: bool, detail: String },
    VideoEditorOpen(String),
    VideoThumbs { id: String, json: String },
    VideoExportStatus { id: String, json: String },
    ScrollOpen(String),
    ScrollStatus(String),
    ScrollClose,
    RecordOpen(String),
    RecordStatus(String),
    RecordInput(String),
    /// D53: the chosen microphone's level, 0–1, about 30 times a second
    RecordLevel(f64),
    /// D53: the chosen microphone's mute switch, first and when it changes
    RecordMic { mic: String, muted: bool },
    RecordClose,
    /// The `settings` property: the config as JSON.
    Settings(String),
    /// The `theme` property: the resolved theme tokens as JSON.
    Theme(String),
    DebugCommand(String),
    QuitRequested,
}

/// Where the engine's `Ui` goes.
pub trait UiOut: Send + Sync {
    fn tell(&self, ui: Ui);
}

/// snipd's adapter: each `Ui` becomes its Engine signal (or property), on
/// the Qt thread.
pub struct Qt(pub CxxQtThread<Engine>);

impl UiOut for Qt {
    fn tell(&self, ui: Ui) {
        let _ = self.0.queue(move |mut e| {
            let q = |s: &str| QString::from(s);
            match ui {
                Ui::Session { json, epoch } => {
                    e.as_mut().set_session(q(&json));
                    e.as_mut().set_epoch(epoch);
                    e.as_mut().frames_ready(epoch);
                }
                Ui::CancelRequested => e.cancel_requested(),
                Ui::Countdown { output, rect, seconds } => e.countdown(&q(&output), rect.x, rect.y, rect.w, rect.h, seconds),
                Ui::CountdownCancel => e.countdown_cancel(),
                Ui::PrepareCapture(request) => e.prepare_capture(request),
                Ui::CaptureDone => e.capture_done(),
                Ui::CardAdd(json) => e.card_add(&q(&json)),
                Ui::CardUpdate { id, json } => e.card_update(&q(&id), &q(&json)),
                Ui::CardsCommand(cmd) => e.cards_command(&q(&cmd)),
                Ui::CardStatus { id, status, detail } => e.card_status(&q(&id), &q(&status), &q(&detail)),
                Ui::Toast { text, output } => e.toast(&q(&text), &q(&output)),
                Ui::BackgroundAuto(json) => e.background_auto(&q(&json)),
                Ui::PinOpen(json) => e.pin_open(&q(&json)),
                Ui::PinsCommand(cmd) => e.pins_command(&q(&cmd)),
                Ui::LinkOffer { url, output } => e.link_offer(&q(&url), &q(&output)),
                Ui::DesktopCover(json) => e.desktop_cover(&q(&json)),
                Ui::HistoryOpen(output) => e.history_open(&q(&output)),
                Ui::SettingsOpen(tab) => e.settings_open(&q(&tab)),
                Ui::WelcomeOpen => e.welcome_open(),
                Ui::EditorOpen(json) => e.editor_open(&q(&json)),
                Ui::EditorWords { path, json } => e.editor_words(&q(&path), &q(&json)),
                Ui::EditorScreenshot { token, path } => e.editor_screenshot(token, &q(&path)),
                Ui::EditorColor { token, color } => e.editor_color(token, &q(&color)),
                Ui::EditorDone { id, action, ok, detail } => e.editor_done(&q(&id), &q(&action), ok, &q(&detail)),
                Ui::VideoEditorOpen(json) => e.video_editor_open(&q(&json)),
                Ui::VideoThumbs { id, json } => e.video_thumbs(&q(&id), &q(&json)),
                Ui::VideoExportStatus { id, json } => e.video_export_status(&q(&id), &q(&json)),
                Ui::ScrollOpen(json) => e.scroll_open(&q(&json)),
                Ui::ScrollStatus(json) => e.scroll_status(&q(&json)),
                Ui::ScrollClose => e.scroll_close(),
                Ui::RecordOpen(json) => e.record_open(&q(&json)),
                Ui::RecordStatus(json) => e.record_status(&q(&json)),
                Ui::RecordInput(json) => e.record_input(&q(&json)),
                Ui::RecordLevel(level) => e.record_level(level),
                Ui::RecordMic { mic, muted } => e.record_mic(&q(&mic), muted),
                Ui::RecordClose => e.record_close(),
                Ui::Settings(json) => e.set_settings(q(&json)),
                Ui::Theme(json) => e.set_theme(q(&json)),
                Ui::DebugCommand(cmd) => e.debug_command(&q(&cmd)),
                Ui::QuitRequested => e.quit_requested(),
            }
        });
    }
}

/// Tests' adapter: what the UI was told, in order.
#[cfg(test)]
#[derive(Default)]
pub struct Told(pub std::sync::Mutex<Vec<Ui>>);

#[cfg(test)]
impl UiOut for std::sync::Arc<Told> {
    fn tell(&self, ui: Ui) {
        self.0.lock().unwrap().push(ui);
    }
}
