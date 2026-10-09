//! The daemon's brain (everything that is not drawing): capture sessions,
//! the capture lifecycle (product §5.0), the Quick Access card list, the
//! ctl.sock API, and live config/theme. It runs on whichever thread an event
//! arrives on (Wayland, ctl, Qt) and talks to QML only through `ui()`.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

use cxx_qt::CxxQtThread;
use cxx_qt_lib::QString;
use serde_json::{json, Value};

use snip_core::config::{AfterCapture, ClipboardMode, Config, OpenLinks, Retention};
use snip_core::geometry::Rect;
use snip_core::history::{Media, Record};
use snip_core::store::{CaptureMeta, Store};
use snip_core::trace::{self, js};
use snip_core::{desktop, encode, media, naming, ocr, paths, qr, theme, window};
use snip_hypr::Client;
use snip_ipc::{Request as Ctl, Response};
use snip_wayland::{CaptureResult, Event, Features, Frame, OutputInfo, Request as Wl, Source};

use crate::bridge::qobject::Engine;
use crate::frames;

mod scroll;

pub type Waiter = mpsc::Sender<Response>;

const SHUTTER: &[u8] = include_bytes!("../../../app/sounds/shutter.wav");
const CLOSED_KEEP: usize = 50;
/// How much desktop a "With the desktop" window capture keeps around the window (logical px)
const WINDOW_DESKTOP_MARGIN: f64 = 48.0;

#[derive(Default)]
struct Snapshot {
    clients: Vec<Client>,
    cursor: Option<(i32, i32)>,
}

fn snapshot() -> Snapshot {
    Snapshot { clients: snip_hypr::visible_windows(), cursor: snip_hypr::cursor_pos() }
}

enum Pending {
    /// Frames for a selection overlay (area / window / text)
    Overlay { mode: String, waiter: Option<Waiter>, snap: JoinHandle<Snapshot>, cursor: bool, delay: u32 },
    /// A rectangle (or the whole output when `rect` is None) of one output
    Region { waiter: Option<Waiter>, output: String, rect: Option<Rect>, kind: String },
    /// Every monitor, for one image (CAP-04)
    AllOutputs { waiter: Option<Waiter> },
    /// A clean window buffer (CAP-05)
    Window { waiter: Option<Waiter>, client: Client, output: String, shadow: bool, fallback: Option<Frame>, rect: Rect },
    /// A frame of a scrolling capture (SCR-01)
    Scroll { tx: mpsc::Sender<CaptureResult> },
}

struct Session {
    mode: String,
    waiter: Option<Waiter>,
    clients: Vec<Client>,
    cursor: bool,
    delay: u32,
}

/// A running self-timer (CAP-06): captured live when the countdown ends.
struct Countdown {
    output: String,
    rect: Option<Rect>,
    kind: String,
    waiter: Option<Waiter>,
    cursor: bool,
}

pub struct Inner {
    qt: CxxQtThread<Engine>,
    wl: OnceLock<snip_wayland::Handle>,
    pub store: Store,
    cfg: RwLock<Config>,
    cfg_error: Mutex<Option<String>>,
    outputs: Mutex<Vec<OutputInfo>>,
    features: Mutex<Features>,
    next_req: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    session: Mutex<Option<Session>>,
    last_area: Mutex<Option<(String, Rect)>>,
    /// Captures waiting for their auto-applied background (BG-03) before
    /// their card: id → output
    pending_bg: Mutex<HashMap<String, String>>,
    /// Shift held when the selection ended (D32): this capture skips the
    /// auto-applied preset. Cleared when a capture begins.
    skip_preset: AtomicBool,
    /// open cards (id, output), oldest first
    cards: Mutex<Vec<(String, String)>>,
    closed: Mutex<Vec<String>>,
    ocr_langs: OnceLock<String>,
    /// Captures waiting for the Quick Access stack to leave the screen
    deferred: Mutex<HashMap<u64, (Source, bool)>>,
    /// Wakes the history indexer (HIS-05)
    index_wake: Mutex<Option<mpsc::Sender<()>>>,
    countdown: Mutex<Option<Countdown>>,
    /// What the capture under way does instead of the After Capture rules
    /// (CAP-02 variants, `--action`)
    capture_action: Mutex<Option<AfterCapture>>,
    /// CAP-12: the wallpaper and the bands cut from it, until it changes
    cover: Mutex<Option<Cover>>,
    /// CAP-12, the live toggle: clutter is hidden; DND was on before it
    desktop_hidden: AtomicBool,
    dnd_before: Mutex<Option<bool>>,
    /// OCR-02: the capture under way keeps (Some(true)) or joins line breaks,
    /// whatever ocr.keep_line_breaks says ("text-lines", "text-one-line")
    line_breaks: Mutex<Option<bool>>,
    /// A scrolling capture (SCR-01), from the chosen area until Done
    scroll: Mutex<Option<scroll::Scroll>>,
}

fn reply(w: Option<Waiter>, r: Response) {
    if let Some(w) = w {
        let _ = w.send(r);
    }
}

fn cancelled() -> Response {
    Response::err("cancelled")
}

fn now_secs() -> i64 {
    chrono::Local::now().timestamp()
}

/// CAP-12: the decoded wallpaper (its path and mtime) and the bands cut from
/// it, by output size and band.
struct Cover {
    key: (PathBuf, std::time::SystemTime),
    wall: Arc<desktop::Wallpaper>,
    blocks: HashMap<(i32, i32, i32, i32, i32, i32), Arc<Vec<u8>>>,
}

pub fn card_json(rec: &Record, output: &str) -> Value {
    // changes whenever the image does (rotate, flip, edits), so QML reloads it
    let rev = std::fs::metadata(&rec.source).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64).unwrap_or(0);
    json!({
        "rev": rev,
        "id": rec.id,
        "path": rec.source,
        "width": rec.width,
        "height": rec.height,
        "kind": rec.kind,
        "scale": rec.scale,
        // REC: a recording shows its poster and its length
        "media": rec.media(),
        "duration": rec.duration_ms,
        "thumb": Store::poster(rec),
        "saved": rec.saved,
        "created": rec.created_at,
        "output": output,
    })
}

/// Width and height from a PNG's IHDR.
fn png_file_size(p: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = [0u8; 24];
    std::fs::File::open(p).ok()?.read_exact(&mut head).ok()?;
    png_size(&head)
}

fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || &b[1..4] != b"PNG" {
        return None;
    }
    Some((u32::from_be_bytes(b[16..20].try_into().ok()?), u32::from_be_bytes(b[20..24].try_into().ok()?)))
}

impl Inner {
    pub fn new(qt: CxxQtThread<Engine>) -> Result<Arc<Self>, String> {
        let (cfg, err) = Config::load();
        if let Some(e) = &err {
            trace::warn("config_error", &format!("\"msg\":{}", js(e)));
        }
        let store = Store::open(&paths::history_dir(), paths::data_dir().join("counter")).map_err(|e| e.to_string())?;
        Ok(Arc::new(Self {
            qt,
            wl: OnceLock::new(),
            store,
            cfg: RwLock::new(cfg),
            cfg_error: Mutex::new(err),
            outputs: Mutex::new(vec![]),
            features: Mutex::new(Features::default()),
            next_req: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            session: Mutex::new(None),
            last_area: Mutex::new(load_last_area()),
            pending_bg: Mutex::new(HashMap::new()),
            skip_preset: AtomicBool::new(false),
            cards: Mutex::new(vec![]),
            closed: Mutex::new(vec![]),
            ocr_langs: OnceLock::new(),
            deferred: Mutex::new(HashMap::new()),
            index_wake: Mutex::new(None),
            countdown: Mutex::new(None),
            capture_action: Mutex::new(None),
            cover: Mutex::new(None),
            desktop_hidden: AtomicBool::new(false),
            dnd_before: Mutex::new(None),
            line_breaks: Mutex::new(None),
            scroll: Mutex::new(None),
        }))
    }

    pub fn set_wayland(&self, h: snip_wayland::Handle) {
        let _ = self.wl.set(h);
    }

    fn wl(&self, r: Wl) {
        if let Some(h) = self.wl.get() {
            h.send(r);
        }
    }

    pub fn cfg(&self) -> Config {
        self.cfg.read().unwrap().clone()
    }

    fn ui(&self, f: impl FnOnce(std::pin::Pin<&mut Engine>) + Send + 'static) {
        let _ = self.qt.queue(f);
    }

    /// A short confirmation on the monitor the user is looking at.
    pub fn toast(&self, text: impl Into<String>) {
        let t = text.into();
        let out = self.cursor_output().unwrap_or_default();
        self.ui(move |e| e.toast(&QString::from(t.as_str()), &QString::from(out.as_str())));
    }

    /// A result shown on the card itself (CleanShot's "Link copied" style).
    fn card_status(&self, id: &str, status: &str, detail: &str) {
        trace::trace("card_status", &format!("\"id\":\"{id}\",\"status\":\"{status}\",\"detail\":{}", js(detail)));
        let (id, status, detail) = (id.to_string(), status.to_string(), detail.to_string());
        self.ui(move |e| e.card_status(&QString::from(id.as_str()), &QString::from(status.as_str()), &QString::from(detail.as_str())));
    }

    fn next(&self) -> u64 {
        self.next_req.fetch_add(1, Ordering::Relaxed)
    }

    fn output_names(&self) -> Vec<String> {
        self.outputs.lock().unwrap().iter().map(|o| o.name.clone()).collect()
    }

    /// The output under the cursor (CAP-04, QAO-01), else the focused one.
    /// A test instance (SNIP_OUTPUTS) always uses its own output.
    /// The scale of the monitor in use (1 if unknown)
    pub fn cursor_scale(&self) -> f64 {
        let out = self.cursor_output();
        snip_hypr::monitors().into_iter().find(|m| Some(&m.name) == out.as_ref()).map_or(1.0, |m| m.scale)
    }

    fn cursor_output(&self) -> Option<String> {
        let names = self.output_names();
        if let Some(o) = std::env::var("SNIP_OUTPUTS").ok().filter(|o| names.contains(o)) {
            return Some(o);
        }
        if let Some((x, y)) = snip_hypr::cursor_pos() {
            if let Some(o) = self.outputs.lock().unwrap().iter().find(|o| o.logical.contains(x, y)) {
                return Some(o.name.clone());
            }
        }
        snip_hypr::monitors().into_iter().find(|m| m.focused).map(|m| m.name).filter(|n| names.contains(n)).or_else(|| names.first().cloned())
    }

    // ------------------------------------------------------------------ events

    pub fn on_wayland(self: &Arc<Self>, ev: Event) {
        match ev {
            Event::Ready { outputs, features } => {
                trace::trace(
                    "wayland_ready",
                    &format!(
                        "\"outputs\":{},\"data_control\":{},\"shortcuts\":{},\"toplevel\":{}",
                        outputs.len(),
                        features.data_control,
                        features.global_shortcuts,
                        features.toplevel_capture
                    ),
                );
                *self.outputs.lock().unwrap() = outputs;
                *self.features.lock().unwrap() = features;
            }
            Event::OutputsChanged { outputs } => *self.outputs.lock().unwrap() = outputs,
            Event::Shortcut { id, pressed: true, t_ns } => {
                trace::trace("shortcut", &format!("\"id\":{},\"t_ns\":{t_ns}", js(&id)));
                self.shortcut(&id);
            }
            Event::Shortcut { .. } => {}
            Event::Captured(r) => {
                let p = self.pending.lock().unwrap().remove(&r.request);
                if let Some(p) = p {
                    let me = self.clone();
                    std::thread::spawn(move || me.captured(p, r));
                }
            }
            Event::ClipboardLost => {}
            Event::Error(e) => trace::error("wayland", &format!("\"msg\":{}", js(&e))),
        }
    }

    /// An action by its shortcut id ("area", "settings"…), as its key runs it.
    pub fn shortcut(self: &Arc<Self>, id: &str) {
        match id {
            "area" | "fullscreen" | "window" | "previous" | "text" | "all-in-one" | "area-pin" | "scroll" | "fullscreen-all" | "text-lines" | "text-one-line" => {
                self.begin(id, None, None, None, 0, None)
            }
            // CAP-02: Capture Area & Copy / & Save / & Annotate
            "area-copy" => self.begin("area", None, None, None, 0, Some(AfterCapture::Copy)),
            "area-save" => self.begin("area", None, None, None, 0, Some(AfterCapture::Save)),
            "area-annotate" => self.begin("area", None, None, None, 0, Some(AfterCapture::Annotate)),
            "scroll-go" => self.scroll_go(),
            "scroll-cancel" => self.scroll_cancel(),
            "timer-area" | "timer-fullscreen" => {
                let delay = self.cfg().capture.timer_seconds;
                self.begin(&id["timer-".len()..], None, None, None, delay, None)
            }
            "restore" => self.ui(|e| e.cards_command(&QString::from("restore"))),
            "close-all" => self.ui(|e| e.cards_command(&QString::from("close-all"))),
            "history" => self.open_history(),
            "settings" => self.ui(|e| e.settings_open(&QString::from(""))),
            "pins-toggle" => self.ui(|e| e.pins_command(&QString::from("toggle"))),
            "pins-close" => self.ui(|e| e.pins_command(&QString::from("close"))),
            "desktop-toggle" => {
                self.desktop("toggle");
            }
            "pins-unlock" => self.ui(|e| e.pins_command(&QString::from("unlock"))),
            "edit-clipboard" => {
                let me = self.clone();
                std::thread::spawn(move || {
                    if let Err(e) = me.edit_clipboard() {
                        me.toast(e);
                    }
                });
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ capture

    /// Start a capture. `waiter` gets the outcome (the CLI blocks on it).
    /// `delay` > 0 runs the self-timer (area: after the selection; fullscreen:
    /// right away).
    #[allow(clippy::too_many_arguments)]
    pub fn begin(self: &Arc<Self>, mode: &str, monitor: Option<String>, cursor: Option<bool>, waiter: Option<Waiter>, delay: u32, action: Option<AfterCapture>) {
        let t0 = trace::now_ns();
        self.skip_preset.store(false, Ordering::Relaxed);
        *self.capture_action.lock().unwrap() = action;
        // OCR-02: Capture Text with or without line breaks
        *self.line_breaks.lock().unwrap() = match mode {
            "text-lines" => Some(true),
            "text-one-line" => Some(false),
            _ => None,
        };
        let mode = if mode.starts_with("text-") { "text" } else { mode };
        // CAP-11: the pointer in screen captures (config, or per capture)
        let cursor = cursor.unwrap_or_else(|| self.cfg().capture.show_cursor);
        let delay = delay.min(60);
        // a capture hotkey during a countdown cancels it
        if let Some(c) = self.countdown.lock().unwrap().take() {
            reply(c.waiter, cancelled());
            reply(waiter, cancelled());
            self.ui(|e| e.countdown_cancel());
            return;
        }
        match mode {
            "area" | "window" | "text" | "all-in-one" | "insert" | "pick" | "area-pin" | "scroll" => {
                // SEL-09: the capture hotkey while the overlay is up cancels it.
                if let Some(s) = self.session.lock().unwrap().take() {
                    reply(s.waiter, cancelled());
                    reply(waiter, cancelled());
                    frames::clear();
                    self.ui(|e| e.cancel_requested());
                    return;
                }
                if self.pending.lock().unwrap().values().any(|p| matches!(p, Pending::Overlay { .. })) {
                    reply(waiter, cancelled());
                    return;
                }
                // All-In-One (CAP-07) is an M1 stand-in: the area overlay.
                let mode = if mode == "all-in-one" { "area" } else { mode }.to_string();
                let req = self.next();
                let snap = std::thread::spawn(snapshot);
                // --monitor: the overlay on that output only. SNIP_OUTPUTS does the
                // same for a whole (test) instance, so it never covers the
                // user's monitors.
                let monitor = monitor.or_else(|| std::env::var("SNIP_OUTPUTS").ok().filter(|s| !s.is_empty()));
                let outputs = monitor.filter(|m| self.output_names().contains(m)).map(|m| vec![m]);
                // the eyedropper reads the frozen pixels, so it always freezes
                if self.cfg().capture.freeze || mode == "pick" {
                    self.pending.lock().unwrap().insert(req, Pending::Overlay { mode, waiter, snap, cursor, delay });
                    self.capture_clean(req, Source::Outputs(outputs), cursor);
                } else {
                    let snap = snap.join().unwrap_or_default();
                    self.open_session(mode, req, waiter, snap, vec![], cursor, delay);
                }
                trace::trace("capture_begin", &format!("\"req\":{req},\"ms\":{:.2}", trace::ms_since(t0)));
            }
            "fullscreen-all" => {
                self.play("shutter");
                let req = self.next();
                self.pending.lock().unwrap().insert(req, Pending::AllOutputs { waiter });
                self.capture_clean(req, Source::Outputs(None), cursor);
            }
            "fullscreen" => {
                let Some(output) = monitor.filter(|m| self.output_names().contains(m)).or_else(|| self.cursor_output()) else {
                    reply(waiter, Response::err("no output"));
                    return;
                };
                if delay > 0 {
                    self.start_countdown(Countdown { output, rect: None, kind: "fullscreen".into(), waiter, cursor }, delay, None);
                    return;
                }
                self.play("shutter");
                let req = self.next();
                self.pending.lock().unwrap().insert(req, Pending::Region { waiter, output: output.clone(), rect: None, kind: "fullscreen".into() });
                self.capture_clean(req, Source::Outputs(Some(vec![output])), cursor);
            }
            "previous" => {
                // CAP-03: the last rectangle, in physical pixels, on its monitor.
                let Some((mut output, rect)) = self.last_area.lock().unwrap().clone() else {
                    self.toast("No previous area yet");
                    reply(waiter, Response::err("no previous area"));
                    return;
                };
                if !self.output_names().contains(&output) {
                    let Some(o) = self.cursor_output() else {
                        reply(waiter, Response::err("no output"));
                        return;
                    };
                    self.toast(format!("{output} is gone; captured the same area on {o}"));
                    output = o;
                }
                self.play("shutter");
                let req = self.next();
                self.pending.lock().unwrap().insert(req, Pending::Region { waiter, output: output.clone(), rect: Some(rect), kind: "area".into() });
                self.capture_clean(req, Source::Outputs(Some(vec![output])), cursor);
            }
            other => reply(waiter, Response::err(format!("unknown capture mode '{other}'"))),
        }
    }

    /// API-01 capture-area: `rect` (logical px on the monitor) with no
    /// overlay; it becomes the previous area too (CAP-03).
    fn capture_rect(self: &Arc<Self>, monitor: Option<String>, rect: Rect, cursor: Option<bool>, waiter: Option<Waiter>, action: Option<AfterCapture>) {
        let Some(output) = monitor.filter(|m| self.output_names().contains(m)).or_else(|| self.cursor_output()) else {
            return reply(waiter, Response::err("no output"));
        };
        let s = snip_hypr::monitors().into_iter().find(|m| m.name == output).map_or(1.0, |m| m.scale);
        let phys = |v: i32| (v as f64 * s).round() as i32;
        let rect = Rect::new(phys(rect.x), phys(rect.y), phys(rect.w), phys(rect.h));
        *self.capture_action.lock().unwrap() = action;
        *self.last_area.lock().unwrap() = Some((output.clone(), rect));
        self.play("shutter");
        let req = self.next();
        self.pending.lock().unwrap().insert(req, Pending::Region { waiter, output: output.clone(), rect: Some(rect), kind: "area".into() });
        self.capture_clean(req, Source::Outputs(Some(vec![output])), cursor.unwrap_or_else(|| self.cfg().capture.show_cursor));
    }

    /// CAP-12: paint the wallpaper over the bands bars reserve, on every
    /// whole-output frame (not a window's own buffer, not under a fullscreen
    /// window). Before anyone sees the frames: the overlay shows it too.
    fn cover_desktop(&self, frames: &[Frame]) {
        let t0 = trace::now_ns();
        let key = std::fs::canonicalize(theme::current_dir().join("background"))
            .ok()
            .and_then(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok().map(|t| (p, t)));
        let Some(key) = key else { return };
        let mut cache = self.cover.lock().unwrap();
        if cache.as_ref().is_none_or(|c| c.key != key) {
            let Some(wall) = desktop::load(&key.0) else {
                trace::warn("cover_wallpaper", &format!("\"path\":{}", js(&key.0.to_string_lossy())));
                return;
            };
            *cache = Some(Cover { key, wall: Arc::new(wall), blocks: HashMap::new() });
        }
        let Some(c) = cache.as_mut() else { return };
        let mons = snip_hypr::monitors();
        let fullscreen: Vec<i64> = snip_hypr::clients().into_iter().filter(|c| c.fullscreen).map(|c| c.workspace).collect();
        let mut n = 0;
        for f in frames {
            let Some(m) = mons.iter().find(|m| m.name == f.output && (m.width, m.height) == (f.buf.width as i32, f.buf.height as i32)) else { continue };
            if fullscreen.contains(&m.active_workspace) {
                continue;
            }
            let (uw, uh) = f.upright_size();
            for band in desktop::bands(m.reserved, uw, uh, f.scale) {
                let wall = c.wall.clone();
                let block = c.blocks.entry((uw, uh, band.x, band.y, band.w, band.h)).or_insert_with(|| Arc::new(desktop::cover_block(&wall, uw, uh, band)));
                desktop::paint(f, band, block);
                n += 1;
            }
        }
        trace::trace("desktop_covered", &format!("\"bands\":{n},\"ms\":{:.1}", trace::ms_since(t0)));
    }

    /// CAP-12, the live toggle ("hide", "show", "toggle"): the wallpaper over
    /// the bars on screen, notifications held (DND), pins and cards hidden.
    pub fn desktop(&self, action: &str) -> Response {
        let hide = match action {
            "hide" => true,
            "show" => false,
            "toggle" => !self.desktop_hidden.load(Ordering::Relaxed),
            _ => return Response::err("desktop: hide, show or toggle"),
        };
        if self.desktop_hidden.swap(hide, Ordering::Relaxed) != hide {
            self.dnd(hide);
        }
        self.push_desktop();
        Response::ok(json!({ "hidden": hide }))
    }

    /// What the covers show: per output, the bands bars reserve (logical px
    /// from its corner). None under a fullscreen window, which is above the bar.
    pub fn push_desktop(&self) {
        let hidden = self.desktop_hidden.load(Ordering::Relaxed);
        let mut outputs = vec![];
        if hidden {
            let fullscreen: Vec<i64> = snip_hypr::clients().into_iter().filter(|c| c.fullscreen).map(|c| c.workspace).collect();
            let outs = self.outputs.lock().unwrap().clone();
            for m in snip_hypr::monitors().into_iter().filter(|m| !fullscreen.contains(&m.active_workspace)) {
                let Some(o) = outs.iter().find(|o| o.name == m.name) else { continue };
                let bands: Vec<[i32; 4]> = desktop::bands(m.reserved, o.logical.w, o.logical.h, 1.0).iter().map(|b| [b.x, b.y, b.w, b.h]).collect();
                outputs.push(json!({ "output": m.name, "bands": bands }));
            }
        }
        // the file itself, so a new theme's wallpaper is a new image
        let wallpaper = std::fs::canonicalize(theme::current_dir().join("background")).ok();
        let j = json!({ "hidden": hidden, "wallpaper": wallpaper, "outputs": outputs }).to_string();
        trace::trace("desktop", &j[1..j.len() - 1]);
        self.ui(move |e| e.desktop_cover(&QString::from(j.as_str())));
    }

    pub fn desktop_is_hidden(&self) -> bool {
        self.desktop_hidden.load(Ordering::Relaxed)
    }

    /// Omarchy's do-not-disturb while clutter is hidden, as it was after.
    /// Tests (SNIP_NO_PORTALS) never touch the user's notifications.
    fn dnd(&self, on: bool) {
        if std::env::var_os("SNIP_NO_PORTALS").is_some() {
            return;
        }
        let shell = |args: &[&str]| std::process::Command::new("omarchy-shell").args(args).output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        if on {
            let was = shell(&["notifications", "isDnd"]).is_some_and(|s| s == "on");
            *self.dnd_before.lock().unwrap() = Some(was);
            if !was {
                shell(&["-q", "notifications", "setDnd", "on"]);
            }
        } else if self.dnd_before.lock().unwrap().take() == Some(false) {
            shell(&["-q", "notifications", "setDnd", "off"]);
        }
    }

    /// Screen captures never contain Snip's own cards (CleanShot hides its
    /// UI from captures). With cards on screen, QML unmaps the stack first and
    /// calls `capture_ready` once that is committed. With Hide desktop clutter
    /// (CAP-12) QML hides the pins as well.
    fn capture_clean(self: &Arc<Self>, req: u64, source: Source, cursor: bool) {
        if self.cards.lock().unwrap().is_empty() && !self.cfg().capture.hide_desktop {
            self.wl(Wl::Capture { request: req, source, cursor });
            return;
        }
        self.deferred.lock().unwrap().insert(req, (source, cursor));
        trace::trace("capture_deferred", &format!("\"req\":{req}"));
        self.ui(move |e| e.prepare_capture(req as i32));
        // never stall a capture on the UI: go anyway after 150 ms
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            me.capture_ready(req);
        });
    }

    /// Hyprland hands pointer focus to a freshly mapped overlay only on real
    /// motion; without it a click made without moving the mouse would go to
    /// the window underneath.
    pub fn nudge_pointer(self: &Arc<Self>) {
        // At the overlay's first frame Hyprland has not taken the new layer
        // into account yet (measured: an immediate nudge is ignored).
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(40));
            me.wl(Wl::PointerNudge);
        });
    }

    pub fn capture_ready(&self, req: u64) {
        let source = self.deferred.lock().unwrap().remove(&req);
        if let Some((source, cursor)) = source {
            trace::trace("capture_ready", &format!("\"req\":{req}"));
            self.wl(Wl::Capture { request: req, source, cursor });
        }
    }

    fn captured(self: &Arc<Self>, p: Pending, r: CaptureResult) {
        if let Pending::Scroll { tx } = p {
            let _ = tx.send(r);
            return;
        }
        if self.cfg().capture.hide_desktop {
            self.cover_desktop(&r.frames);
        }
        // The stack comes back now, except for a selection: then it returns
        // when the overlay closes (QML), so it can't flash in between.
        if !matches!(p, Pending::Overlay { .. }) || r.frames.is_empty() {
            self.ui(|e| e.capture_done());
        }
        if !r.failed.is_empty() {
            trace::warn("capture_failed", &format!("\"which\":{}", js(&r.failed.join(","))));
        }
        match p {
            Pending::Overlay { mode, waiter, snap, cursor, delay } => {
                if r.frames.is_empty() {
                    self.toast("Screen capture failed");
                    reply(waiter, Response::err("capture failed"));
                    return;
                }
                let snap = snap.join().unwrap_or_default();
                trace::trace(
                    "frames_captured",
                    &format!("\"req\":{},\"n\":{},\"ms\":{:.2}", r.request, r.frames.len(), (r.t_done_ns - r.t_start_ns) as f64 / 1e6),
                );
                self.open_session(mode, r.request, waiter, snap, r.frames, cursor, delay);
            }
            Pending::Region { waiter, output, rect, kind } => {
                let Some(frame) = r.frames.into_iter().next() else {
                    self.toast("Screen capture failed");
                    reply(waiter, Response::err("capture failed"));
                    return;
                };
                let (uw, uh) = frame.upright_size();
                let rect = rect.unwrap_or(Rect::new(0, 0, uw, uh));
                self.finish_from_frame(&frame, rect, &kind, &output, waiter);
            }
            Pending::Scroll { .. } => {}
            Pending::AllOutputs { waiter } => {
                let parts: Vec<_> = r.frames.iter().map(|f| {
                    let (uw, uh) = f.upright_size();
                    let (data, w, h, alpha) = encode::crop_upright(f, Rect::new(0, 0, uw, uh));
                    let rgba = if alpha { data } else { data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
                    (f.logical, rgba, w, h)
                }).collect();
                if parts.is_empty() {
                    self.toast("Screen capture failed");
                    return reply(waiter, Response::err("capture failed"));
                }
                let (rgba, w, h) = encode::composite(&parts);
                let png = encode::encode_png(&rgba, w, h, true);
                let output = self.cursor_output().unwrap_or_default();
                let scale = r.frames.iter().map(|f| f.scale).fold(1.0, f64::max);
                let meta = CaptureMeta { kind: "fullscreen".into(), scale, output: output.clone(), app: None, title: None };
                self.ingest(png, w, h, meta, &output, waiter);
            }
            Pending::Window { waiter, client, output, shadow, fallback, rect } => {
                match r.frames.into_iter().next() {
                    Some(f) => self.finish_window_frame(&f, &client, &output, shadow, waiter),
                    None => match fallback {
                        // no toplevel capture: what was on screen (frozen frame)
                        Some(f) => self.finish_from_frame(&f, rect, "window", &output, waiter),
                        None => {
                            self.toast("Window capture failed");
                            reply(waiter, Response::err("capture failed"));
                        }
                    },
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn open_session(self: &Arc<Self>, mode: String, epoch: u64, waiter: Option<Waiter>, snap: Snapshot, frames_: Vec<Frame>, cursor: bool, delay: u32) {
        let outputs = self.outputs.lock().unwrap().clone();
        // over the live screen there's no frame to tell an output's scale:
        // Hyprland's (QScreen rounds 1.25 up to 2)
        let monitors = if frames_.is_empty() { snip_hypr::monitors() } else { vec![] };
        let mut out_json = serde_json::Map::new();
        for o in &outputs {
            let f = frames_.iter().find(|f| f.output == o.name);
            if !frames_.is_empty() && f.is_none() {
                continue; // not part of this capture (snip area --monitor)
            }
            let wins: Vec<Value> = snap
                .clients
                .iter()
                .filter(|c| Rect::new(c.x, c.y, c.w, c.h).intersect(&o.logical).area() > 0)
                .map(|c| {
                    json!({
                        "address": format!("{:x}", c.address),
                        "x": c.x - o.logical.x, "y": c.y - o.logical.y, "w": c.w, "h": c.h,
                        "title": c.title, "class": c.class,
                    })
                })
                .collect();
            out_json.insert(
                o.name.clone(),
                json!({
                    "x": o.logical.x, "y": o.logical.y, "w": o.logical.w, "h": o.logical.h,
                    "scale": f.map(|f| f.scale).or_else(|| monitors.iter().find(|m| m.name == o.name).map(|m| m.scale)).unwrap_or(0.0),
                    "frozen": f.is_some(),
                    "windows": wins,
                }),
            );
        }
        let pointer = snap.cursor.and_then(|(x, y)| {
            outputs.iter().find(|o| o.logical.contains(x, y)).map(|o| json!({"output": o.name, "x": x - o.logical.x, "y": y - o.logical.y}))
        });
        let cfg = self.cfg();
        let session = json!({
            "mode": mode,
            "epoch": epoch,
            "freeze": !frames_.is_empty(),
            "outputs": out_json,
            "cursor": pointer,
            "crosshair": cfg.capture.crosshair,
            "magnifier": cfg.capture.magnifier || mode == "pick",
            "toplevel": self.features.lock().unwrap().toplevel_capture,
        })
        .to_string();
        trace::trace("session", &format!("\"json\":{session}"));
        frames::publish(frames_);
        *self.session.lock().unwrap() = Some(Session { mode, waiter, clients: snap.clients, cursor, delay });
        self.ui(move |mut e| {
            e.as_mut().set_session(QString::from(session.as_str()));
            e.as_mut().set_epoch(epoch as i32);
            e.as_mut().frames_ready(epoch as i32);
        });
    }

    /// The user finished a selection (physical px of `output`).
    pub fn finish_area(self: &Arc<Self>, output: String, rect: Rect, kind: String) {
        trace::trace("finish_area", &format!("\"output\":{},\"rect\":[{},{},{},{}],\"kind\":{}", js(&output), rect.x, rect.y, rect.w, rect.h, js(&kind)));
        let Some(s) = self.session.lock().unwrap().take() else { return };
        if s.mode == "scroll" {
            let scale = frames::get(&output).map(|f| f.scale).unwrap_or(1.0);
            frames::clear();
            return self.scroll_open(output, rect, scale, s.waiter, &s.clients);
        }
        // Capture Area & Pin (PIN-01): an area, pinned where it was
        let kind = if s.mode == "text" || s.mode == "insert" || (s.mode == "area-pin" && kind == "area") { s.mode.clone() } else { kind };
        if kind == "area" || kind == "area-pin" {
            *self.last_area.lock().unwrap() = Some((output.clone(), rect));
            save_last_area(&output, rect);
        }
        // self-timer: the selected area is captured live after the countdown
        if s.delay > 0 && kind != "text" && kind != "insert" {
            let scale = frames::get(&output).map(|f| f.scale).unwrap_or(1.0);
            frames::clear();
            let shown = Rect::new(
                (rect.x as f64 / scale).round() as i32,
                (rect.y as f64 / scale).round() as i32,
                (rect.w as f64 / scale).round() as i32,
                (rect.h as f64 / scale).round() as i32,
            );
            self.start_countdown(Countdown { output, rect: Some(rect), kind, waiter: s.waiter, cursor: s.cursor }, s.delay, Some(shown));
            return;
        }
        if kind != "text" {
            self.play("shutter");
        }
        match frames::get(&output) {
            Some(frame) => {
                frames::clear();
                let me = self.clone();
                std::thread::spawn(move || me.finish_from_frame(&frame, rect, &kind, &output, s.waiter));
            }
            None => {
                // live mode (freeze off): re-capture once the overlay is gone
                let req = self.next();
                self.pending.lock().unwrap().insert(req, Pending::Region { waiter: s.waiter, output: output.clone(), rect: Some(rect), kind });
                let me = self.clone();
                let cursor = s.cursor;
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(90));
                    me.wl(Wl::Capture { request: req, source: Source::Outputs(Some(vec![output])), cursor });
                });
            }
        }
    }

    /// CAP-06: show the countdown (logical `shown` area, or the whole output)
    /// and capture live when QML calls `countdown_done`.
    fn start_countdown(&self, c: Countdown, seconds: u32, shown: Option<Rect>) {
        let out = c.output.clone();
        let r = shown.unwrap_or(Rect::new(-1, -1, 0, 0));
        trace::trace("countdown", &format!("\"output\":{},\"seconds\":{seconds}", js(&out)));
        *self.countdown.lock().unwrap() = Some(c);
        self.ui(move |e| e.countdown(&QString::from(out.as_str()), r.x, r.y, r.w, r.h, seconds as i32));
    }

    /// The countdown reached zero and is off screen: capture now.
    pub fn countdown_done(self: &Arc<Self>) {
        let Some(c) = self.countdown.lock().unwrap().take() else { return };
        self.play("shutter");
        let req = self.next();
        self.pending.lock().unwrap().insert(req, Pending::Region { waiter: c.waiter, output: c.output.clone(), rect: c.rect, kind: c.kind });
        self.capture_clean(req, Source::Outputs(Some(vec![c.output])), c.cursor);
    }

    /// Window mode click (CAP-05): the window's own buffer, never occluded.
    pub fn finish_window(self: &Arc<Self>, address: &str, output: String, rect: Rect, invert: bool) {
        let Some(s) = self.session.lock().unwrap().take() else { return };
        if s.mode == "scroll" {
            // Space then a click: the window's area, scrolled
            let scale = frames::get(&output).map(|f| f.scale).unwrap_or(1.0);
            frames::clear();
            return self.scroll_open(output, rect, scale, s.waiter, &s.clients);
        }
        if s.mode == "insert" {
            // for an editor: the window as it is on screen
            self.play("shutter");
            let frame = frames::get(&output);
            frames::clear();
            let me = self.clone();
            return match frame {
                Some(f) => drop(std::thread::spawn(move || me.finish_from_frame(&f, rect, "insert", &output, s.waiter))),
                None => reply(s.waiter, Response::err("window not found")),
            };
        }
        // With the desktop: the window as it is on screen and what's around it
        if self.cfg().capture.window_desktop != invert {
            if let Some(f) = frames::get(&output) {
                frames::clear();
                self.play("shutter");
                let (m, (uw, uh)) = ((WINDOW_DESKTOP_MARGIN * f.scale).round() as i32, f.upright_size());
                let around = Rect::new(rect.x - m, rect.y - m, rect.w + 2 * m, rect.h + 2 * m).intersect(&Rect::new(0, 0, uw, uh));
                let me = self.clone();
                std::thread::spawn(move || me.finish_from_frame(&f, around, "window", &output, s.waiter));
                return;
            }
        }
        let Some(addr) = u64::from_str_radix(address.trim_start_matches("0x"), 16).ok() else { return };
        let client = s.clients.iter().find(|c| c.address == addr).cloned();
        self.play("shutter");
        let fallback = frames::get(&output);
        frames::clear();
        let shadow = self.cfg().capture.window_shadow != (invert && !self.cfg().capture.window_desktop);
        match client {
            Some(client) if self.features.lock().unwrap().toplevel_capture => {
                let req = self.next();
                self.pending.lock().unwrap().insert(req, Pending::Window { waiter: s.waiter, client, output, shadow, fallback, rect });
                self.wl(Wl::Capture { request: req, source: Source::Window(addr), cursor: false });
            }
            _ => match fallback {
                Some(f) => {
                    let me = self.clone();
                    std::thread::spawn(move || me.finish_from_frame(&f, rect, "window", &output, s.waiter));
                }
                None => reply(s.waiter, Response::err("window not found")),
            },
        }
    }

    /// Shift was held as the selection ended: skip the preset (D32).
    pub fn skip_preset(&self, skip: bool) {
        self.skip_preset.store(skip, Ordering::Relaxed);
    }

    /// The eyedropper's click: the colour under the pointer (ANN-13).
    pub fn finish_pick(&self, color: String) {
        if let Some(s) = self.session.lock().unwrap().take() {
            reply(s.waiter, Response::ok(json!(color)));
        }
        frames::clear();
    }

    pub fn cancel(&self) {
        if let Some(s) = self.session.lock().unwrap().take() {
            reply(s.waiter, cancelled());
        }
        frames::clear();
    }

    fn finish_from_frame(self: &Arc<Self>, frame: &Frame, rect: Rect, kind: &str, output: &str, waiter: Option<Waiter>) {
        if kind == "insert" {
            // for an editor (ANN-12): a file to add, not a capture in history
            let (data, w, h, alpha) = encode::crop_upright(frame, rect);
            let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let path = snip_core::paths::runtime_dir().join(format!("insert-{ms}.png"));
            let r = std::fs::write(&path, encode::encode_png(&data, w, h, alpha));
            return reply(waiter, match r {
                Ok(()) => Response::ok(json!(path.to_string_lossy())),
                Err(e) => Response::err(e.to_string()),
            });
        }
        if kind == "text" {
            let (data, w, h, alpha) = encode::crop_upright(frame, rect);
            return reply(waiter, self.read_text(&data, w, h, alpha, frame.scale));
        }
        let (uw, uh) = frame.upright_size();
        if rect.intersect(&Rect::new(0, 0, uw, uh)).area() <= 0 {
            trace::warn("empty_crop", &format!("\"output\":{},\"rect\":[{},{},{},{}],\"frame\":[{uw},{uh}]", js(output), rect.x, rect.y, rect.w, rect.h));
            reply(waiter, Response::err("empty selection"));
            return;
        }
        let e = encode::crop_and_encode(frame, rect);
        trace::trace(
            "encoded",
            &format!("\"w\":{},\"h\":{},\"crop_ms\":{:.2},\"encode_ms\":{:.2},\"bytes\":{}", e.width, e.height, e.crop_ms, e.encode_ms, e.png.len()),
        );
        let (app, title) = self.window_meta_at(output, rect, frame.scale);
        let meta = CaptureMeta { kind: kind.trim_end_matches("-pin").into(), scale: frame.scale, output: output.into(), app, title };
        if kind == "area-pin" {
            // where the area was, in layout px: the pin goes right there
            let origin = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| (o.logical.x, o.logical.y)).unwrap_or((0, 0));
            let at = (origin.0 + (rect.x as f64 / frame.scale).round() as i32, origin.1 + (rect.y as f64 / frame.scale).round() as i32);
            return self.ingest_and_pin(e.png, e.width, e.height, meta, at, waiter);
        }
        self.ingest(e.png, e.width, e.height, meta, output, waiter);
    }

    /// Capture Text's reading (CAP-09): a QR code's exact value if there is one
    /// (OCR-03), else the OCR'd text; copied, with a toast.
    fn read_text(&self, data: &[u8], w: u32, h: u32, alpha: bool, scale: f64) -> Response {
        let t0 = trace::now_ns();
        let cfg = self.cfg().ocr;
        let bpp = if alpha { 4 } else { 3 };
        let mut codes = qr::decode(data, w, h, bpp);
        if codes.is_empty() {
            codes = qr::decode_others(data, w, h, bpp, cfg.barcodes);
        }
        if !codes.is_empty() {
            let text = codes.join("\n");
            trace::trace("qr", &format!("\"codes\":{},\"ms\":{:.1}", codes.len(), trace::ms_since(t0)));
            let summary = self.copy_qr(&text);
            self.toast(format!("Copied {summary}"));
            self.offer_link(&text);
            return Response::ok(json!(text));
        }
        let one_line = !self.line_breaks.lock().unwrap().take().unwrap_or(cfg.keep_line_breaks);
        match ocr::recognize(data, w, h, bpp, scale, &self.ocr_languages()) {
            Ok(text) if !text.trim().is_empty() => {
                let text = if one_line { snip_core::text::one_line(&text) } else { text };
                trace::trace("ocr", &format!("\"chars\":{},\"ms\":{:.1}", text.chars().count(), trace::ms_since(t0)));
                if cfg.detect_links {
                    self.offer_link(&text);
                }
                self.set_clipboard_text(&text);
                let n = text.chars().count();
                self.toast(format!("Copied {n} character{}", if n == 1 { "" } else { "s" }));
                Response::ok(json!(text))
            }
            Ok(_) => {
                self.toast("No text found");
                Response::err("no text found")
            }
            Err(e) => {
                self.toast(format!("Text recognition failed: {e}"));
                Response::err(e.to_string())
            }
        }
    }

    /// OCR-03/05: a link in what was read: opened, offered, or left alone
    /// (`ocr.open_links`).
    fn offer_link(&self, text: &str) {
        let Some(url) = snip_core::text::links(text).into_iter().next() else { return };
        match self.cfg().ocr.open_links {
            OpenLinks::Never => {}
            OpenLinks::Always => self.open_link(&url),
            OpenLinks::Ask => {
                let out = self.cursor_output().unwrap_or_default();
                self.ui(move |e| e.link_offer(&QString::from(url.as_str()), &QString::from(out.as_str())));
            }
        }
    }

    /// Opens a link in the browser. Tests (SNIP_NO_PORTALS) only trace it.
    pub fn open_link(&self, url: &str) {
        trace::trace("link_open", &format!("\"url\":{}", js(url)));
        if std::env::var_os("SNIP_NO_PORTALS").is_none() {
            spawn_detached("xdg-open", &[url]);
        }
    }

    /// OCR-06: Capture Text on a PNG file, or on the clipboard's image.
    pub fn text_of(&self, path: Option<&str>) -> Response {
        *self.line_breaks.lock().unwrap() = None;
        let bytes = match path {
            Some(p) => std::fs::read(p).map_err(|e| format!("{p}: {e}")),
            None => self.clipboard_png().ok_or_else(|| "no image on the clipboard".to_string()),
        };
        match bytes.and_then(|b| encode::decode_png(&b).map_err(|_| "only PNG images can be read for now".to_string())) {
            Ok((data, w, h, alpha)) => self.read_text(&data, w, h, alpha, 1.0),
            Err(e) => Response::err(e),
        }
    }

    /// Capture Area & Pin: the capture goes to history and straight into a
    /// pin at `at`, with no card.
    fn ingest_and_pin(self: &Arc<Self>, png: Vec<u8>, w: u32, h: u32, meta: CaptureMeta, at: (i32, i32), waiter: Option<Waiter>) {
        match self.store.ingest(png, w, h, meta, now_secs()) {
            Ok(rec) => {
                self.wake_indexer();
                let r = match self.pin_at(&rec.id, Some(at)) {
                    Ok(j) => Response::ok(j),
                    Err(e) => Response::err(e),
                };
                reply(waiter, r);
            }
            Err(e) => reply(waiter, Response::err(e.to_string())),
        }
    }

    fn finish_window_frame(self: &Arc<Self>, f: &Frame, client: &Client, output: &str, shadow: bool, waiter: Option<Waiter>) {
        let t0 = trace::now_ns();
        let (uw, uh) = f.upright_size();
        let (data, w, h, alpha) = encode::crop_upright(f, Rect::new(0, 0, uw, uh));
        let rgba = if alpha { data } else { data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
        let scale = if client.w > 0 { w as f64 / client.w as f64 } else { 1.0 };
        let cfg = self.cfg();
        let corners = (cfg.capture.window_rounding && !client.fullscreen).then(|| window::Corners {
            radius: snip_hypr::option_int("decoration:rounding").unwrap_or(0) as f64 * scale,
            power: snip_hypr::option_float("decoration:rounding_power").unwrap_or(2.0),
        });
        let shadow = (shadow && !client.fullscreen).then(|| window::Shadow::float(scale));
        let (out, ow, oh) = window::decorate(rgba, w, h, corners, shadow);
        let png = encode::encode_png(&out, ow, oh, true);
        trace::trace("window_encoded", &format!("\"w\":{ow},\"h\":{oh},\"ms\":{:.1}", trace::ms_since(t0)));
        let meta = CaptureMeta { kind: "window".into(), scale, output: output.into(), app: Some(client.class.clone()), title: Some(client.title.clone()) };
        self.ingest(png, ow, oh, meta, output, waiter);
    }

    /// Window title and class for %t / %a: the window that contains the
    /// selection's centre (Omarchy's "smart" semantics: the smallest wins).
    fn window_meta_at(&self, output: &str, rect: Rect, scale: f64) -> (Option<String>, Option<String>) {
        let o = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| o.logical);
        let Some(o) = o else { return (None, None) };
        let s = if scale > 0.0 { scale } else { 1.0 };
        let cx = o.x + ((rect.x as f64 + rect.w as f64 / 2.0) / s) as i32;
        let cy = o.y + ((rect.y as f64 + rect.h as f64 / 2.0) / s) as i32;
        snip_hypr::visible_windows()
            .into_iter()
            .filter(|c| Rect::new(c.x, c.y, c.w, c.h).contains(cx, cy))
            .min_by_key(|c| c.w as i64 * c.h as i64)
            .map(|c| (Some(c.class), Some(c.title)))
            .unwrap_or((None, None))
    }

    // ------------------------------------------------------------------ history text index (HIS-05)

    /// Background indexer: reads the text (and QR values) of each capture at
    /// the lowest CPU priority, newest first, so history is searchable.
    pub fn start_indexer(self: &Arc<Self>) {
        let (tx, rx) = mpsc::channel::<()>();
        *self.index_wake.lock().unwrap() = Some(tx);
        let me = self.clone();
        std::thread::Builder::new()
            .name("snip-index".into())
            .spawn(move || loop {
                // wake on a new capture, or every 10 minutes for older ones
                let _ = rx.recv_timeout(Duration::from_secs(600));
                while rx.try_recv().is_ok() {}
                if !me.cfg().ocr.index_history {
                    continue;
                }
                loop {
                    let batch = me.store.unindexed(8).unwrap_or_default();
                    if batch.is_empty() {
                        break;
                    }
                    for (id, source) in batch {
                        me.index_one(&id, &source);
                    }
                }
            })
            .ok();
        self.wake_indexer();
    }

    fn wake_indexer(&self) {
        if let Some(tx) = self.index_wake.lock().unwrap().as_ref() {
            let _ = tx.send(());
        }
    }

    fn index_one(&self, id: &str, source: &Path) {
        let t0 = trace::now_ns();
        // a recording is searched by its first frame
        let poster = source.with_file_name("poster.png");
        let source = if Media::of(source) != Media::Image { poster.as_path() } else { source };
        let text = std::fs::read(source)
            .ok()
            .and_then(|b| encode::decode_png(&b).ok())
            .map(|(data, w, h, alpha)| {
                let bpp = if alpha { 4 } else { 3 };
                let mut parts = qr::decode(&data, w, h, bpp);
                // small captures were taken at screen density: upscale them for Tesseract
                let scale = if (w as u64) * (h as u64) < 4_000_000 { 1.25 } else { 2.0 };
                if let Ok(t) = ocr::recognize_with(&data, w, h, bpp, scale, &self.ocr_languages(), true) {
                    parts.push(t);
                }
                parts.join("\n")
            })
            .unwrap_or_default();
        // indexed even when empty, so a capture is never retried forever
        let _ = self.store.set_index(id, &text);
        trace::trace("indexed", &format!("\"id\":\"{id}\",\"chars\":{},\"ms\":{:.0}", text.chars().count(), trace::ms_since(t0)));
    }

    fn ingest(self: &Arc<Self>, png: Vec<u8>, w: u32, h: u32, meta: CaptureMeta, output: &str, waiter: Option<Waiter>) {
        let t0 = trace::now_ns();
        match self.store.ingest(png, w, h, meta, now_secs()) {
            Ok(rec) => {
                trace::trace("ingested", &format!("\"id\":\"{}\",\"ms\":{:.2}", rec.id, trace::ms_since(t0)));
                self.wake_indexer();
                if !self.apply_auto_background(&rec, output) {
                    self.after_capture(&rec, output);
                }
                reply(waiter, Response::ok(serde_json::to_value(&rec).unwrap_or(Value::Null)));
            }
            Err(e) => {
                self.toast(format!("Could not store the capture: {e}"));
                reply(waiter, Response::err(e.to_string()));
            }
        }
    }

    /// BG-03 "Automatically apply preset to all screenshots": the preset set
    /// to auto-apply (backgrounds.json), its sizes in points.
    fn auto_background(&self) -> Option<Value> {
        let j: Value = serde_json::from_str(&std::fs::read_to_string(paths::data_dir().join("backgrounds.json")).ok()?).ok()?;
        let name = j["auto"].as_str()?;
        j["presets"].as_array()?.iter().find(|p| p["name"] == name).map(|p| p["bg"].clone())
    }

    /// A new screenshot gets the auto-applied background before its card:
    /// QML renders it and commits it like an edit (action "auto-bg"), then
    /// the After Capture rules run. False when there is none to apply.
    fn apply_auto_background(&self, rec: &Record, output: &str) -> bool {
        if !matches!(rec.kind.as_str(), "area" | "fullscreen" | "window") || self.skip_preset.swap(false, Ordering::Relaxed) {
            return false;
        }
        let Some(bg) = self.auto_background() else { return false };
        let scale = snip_hypr::monitors().into_iter().find(|m| m.name == output).map(|m| m.scale).unwrap_or(1.0);
        self.pending_bg.lock().unwrap().insert(rec.id.clone(), output.to_string());
        let mut j = card_json(rec, output);
        j["bg"] = bg;
        j["scale"] = json!(scale);
        let s = j.to_string();
        trace::trace("auto_background", &format!("\"id\":\"{}\"", rec.id));
        self.ui(move |e| e.background_auto(&QString::from(s.as_str())));
        true
    }

    /// SET-03 After Capture rules (default: only the Quick Access card), or a
    /// variant's own action (CAP-02).
    fn after_capture(self: &Arc<Self>, rec: &Record, output: &str) {
        let cfg = self.cfg().after_capture;
        let mut rules = cfg.screenshot;
        if let Some(a) = self.capture_action.lock().unwrap().take() {
            if cfg.variants_ignore_rules {
                rules.clear();
            }
            rules.push(a);
        }
        if rules.contains(&AfterCapture::Pin) {
            let _ = self.pin(&rec.id);
        }
        if rules.contains(&AfterCapture::Save) {
            let _ = self.save(&rec.id);
        } else if rules.contains(&AfterCapture::Copy) {
            let _ = self.copy(&rec.id);
        }
        if rules.contains(&AfterCapture::Annotate) {
            self.annotate(&rec.id);
        }
        if rules.contains(&AfterCapture::ShowQao) || rules.is_empty() {
            self.show_card(rec, output);
        } else if !rules.contains(&AfterCapture::Annotate) && !rules.contains(&AfterCapture::Pin) {
            self.notify_capture(&rec.id, rules.contains(&AfterCapture::Save));
        }
    }

    /// OMA-05: no card shows the capture, so a notification does, with the
    /// image; a click brings the card back. Tests (SNIP_NO_PORTALS) only trace it.
    fn notify_capture(&self, id: &str, saved: bool) {
        let Ok(rec) = self.store.get(id) else { return };
        let (title, body) = match rec.saved.as_ref().filter(|_| saved) {
            Some(p) => ("Screenshot saved", p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()),
            None => ("Screenshot copied", format!("{} × {}", rec.width, rec.height)),
        };
        trace::trace("notified", &format!("\"id\":\"{id}\",\"title\":{},\"body\":{}", js(title), js(&body)));
        if std::env::var_os("SNIP_NO_PORTALS").is_some() {
            return;
        }
        // the CLI beside this snipd (a checkout's build, or the package's)
        let snip = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("snip"))).filter(|p| p.exists());
        let snip = snip.map_or_else(|| "snip".to_string(), |p| p.to_string_lossy().into_owned());
        let image = rec.source.to_string_lossy();
        spawn_detached("omarchy-notification-send", &["--app-name", "Snip", "-g", "󰹑", "--image", &image, title, &body, "--exec", &snip, "restore", id]);
    }

    fn show_card(&self, rec: &Record, output: &str) {
        self.cards.lock().unwrap().push((rec.id.clone(), output.to_string()));
        self.closed.lock().unwrap().retain(|i| i != &rec.id);
        let j = card_json(rec, output).to_string();
        self.ui(move |e| e.card_add(&QString::from(j.as_str())));
    }

    /// A capture back in Quick Access, unless its card is there already:
    /// after its editor closes (the card may have timed out meanwhile) and
    /// when history restores it.
    pub fn restore_card(&self, id: &str) {
        if self.cards.lock().unwrap().iter().any(|(i, _)| i == id) {
            return;
        }
        let Ok(rec) = self.store.get(id) else { return };
        let output = self.cursor_output().unwrap_or_default();
        self.show_card(&rec, &output);
    }

    /// Show the history window on the monitor with the pointer.
    pub fn open_history(&self) {
        let out = self.cursor_output().unwrap_or_default();
        self.ui(move |e| e.history_open(&QString::from(out.as_str())));
    }

    /// The history window (HIS-01, HIS-05): the newest captures, or the ones
    /// whose text (OCR, window title, app) matches `query`.
    pub fn history(&self, query: &str, limit: u32) -> Value {
        let recs = if query.trim().is_empty() {
            self.store.recent(limit)
        } else {
            self.store.search(query, limit).map(|v| v.into_iter().map(|(r, _)| r).collect())
        };
        Value::Array(
            recs.unwrap_or_default()
                .iter()
                .map(|r| {
                    let mut j = card_json(r, "");
                    j["title"] = json!(r.title);
                    j["app"] = json!(r.app);
                    j["project"] = json!(self.store.is_project(r));
                    j
                })
                .collect(),
        )
    }

    pub fn card_closed(&self, id: &str) {
        self.cards.lock().unwrap().retain(|(i, _)| i != id);
        let mut c = self.closed.lock().unwrap();
        c.retain(|i| i != id);
        c.push(id.to_string());
        let n = c.len();
        if n > CLOSED_KEEP {
            c.drain(..n - CLOSED_KEEP);
        }
        drop(c);
        // history "Never": nothing outlives its card (unless it was saved)
        if self.cfg().history.retention == Retention::Never {
            let _ = self.store.delete(id);
        }
    }

    pub fn restore_last(&self) -> Result<Value, String> {
        let id = self.closed.lock().unwrap().pop().ok_or("nothing to restore")?;
        let rec = self.store.get(&id).map_err(|e| e.to_string())?;
        let out = self.cursor_output().unwrap_or_default();
        self.show_card(&rec, &out);
        Ok(card_json(&rec, &out))
    }

    // ------------------------------------------------------------------ actions

    fn resolve_id(&self, id: Option<String>) -> Result<String, String> {
        match id.filter(|s| !s.is_empty()) {
            Some(id) => Ok(id),
            None => self.store.recent(1).map_err(|e| e.to_string())?.into_iter().next().map(|r| r.id).ok_or_else(|| "history is empty".into()),
        }
    }

    /// Copy (§5.0): clipboard only.
    pub fn copy(&self, id: &str) -> Result<(), String> {
        let cfg = self.cfg();
        let rec = self.store.get(id).map_err(|e| e.to_string())?;
        if rec.media() != Media::Image {
            return self.copy_media(&rec);
        }
        let png = self.store.bytes(id).map_err(|e| e.to_string())?;
        let mut offers: Vec<(String, Arc<Vec<u8>>)> = vec![];
        if cfg.clipboard.mode != ClipboardMode::File {
            offers.push(("image/png".into(), png));
        }
        // A named file (the saved one, else a copy in the drag cache) for the
        // file and text-path offers.
        let file = if cfg.clipboard.mode != ClipboardMode::Image || cfg.clipboard.text_path { Some(self.drag_file(id)?) } else { None };
        if let (Some(p), true) = (&file, cfg.clipboard.mode != ClipboardMode::Image) {
            let uri = format!("file://{}", percent_encode(p));
            offers.push(("text/uri-list".into(), Arc::new(format!("{uri}\r\n").into_bytes())));
            offers.push(("x-special/gnome-copied-files".into(), Arc::new(format!("copy\n{uri}").into_bytes())));
        }
        if let (Some(p), true) = (&file, cfg.clipboard.text_path) {
            // Image-aware apps take image/png; text-only targets get the path.
            let text = Arc::new(p.to_string_lossy().into_owned().into_bytes());
            for t in ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"] {
                offers.push((t.into(), text.clone()));
            }
        }
        if cfg.clipboard.sensitive {
            offers.push(("x-kde-passwordManagerHint".into(), Arc::new(b"secret".to_vec())));
        }
        self.wl(Wl::SetClipboard { offers });
        trace::trace("copied", &format!("\"id\":\"{id}\""));
        Ok(())
    }

    /// A recording on the clipboard is its file (named like a saved one): file
    /// managers and chat apps take it, terminals get its path; a GIF also as
    /// image/gif for apps that paste images.
    fn copy_media(&self, rec: &Record) -> Result<(), String> {
        let file = self.drag_file(&rec.id)?;
        let uri = format!("file://{}", percent_encode(&file));
        let mut offers: Vec<(String, Arc<Vec<u8>>)> = vec![];
        if rec.media() == Media::Gif {
            offers.push(("image/gif".into(), Arc::new(std::fs::read(&rec.source).map_err(|e| e.to_string())?)));
        }
        offers.push(("text/uri-list".into(), Arc::new(format!("{uri}\r\n").into_bytes())));
        offers.push(("x-special/gnome-copied-files".into(), Arc::new(format!("copy\n{uri}").into_bytes())));
        let text = Arc::new(file.to_string_lossy().into_owned().into_bytes());
        for t in ["text/plain;charset=utf-8", "text/plain", "UTF8_STRING"] {
            offers.push((t.into(), text.clone()));
        }
        self.wl(Wl::SetClipboard { offers });
        trace::trace("copied", &format!("\"id\":\"{}\",\"media\":true", rec.id));
        Ok(())
    }

    /// QR values can be Wi-Fi passwords or 2FA secrets: always copied as
    /// sensitive, so Omarchy's clipboard history skips them (as Omarchy's own
    /// QR capture does).
    /// Returns a short label ("QR code · https://…") for the confirmation.
    fn copy_qr(&self, text: &str) -> String {
        self.set_clipboard_text_with(text, true);
        let one = text.lines().next().unwrap_or("");
        let shown: String = if one.chars().count() > 56 { one.chars().take(55).collect::<String>() + "…" } else { one.to_string() };
        let n = text.lines().count();
        if n > 1 { format!("{n} QR codes · {shown}") } else { format!("QR code · {shown}") }
    }

    fn set_clipboard_text(&self, text: &str) {
        self.set_clipboard_text_with(text, false);
    }

    fn set_clipboard_text_with(&self, text: &str, sensitive: bool) {
        let b = Arc::new(text.as_bytes().to_vec());
        let mut offers = vec![("text/plain;charset=utf-8".to_string(), b.clone()), ("text/plain".to_string(), b.clone()), ("UTF8_STRING".to_string(), b)];
        if sensitive || self.cfg().clipboard.sensitive {
            offers.push(("x-kde-passwordManagerHint".into(), Arc::new(b"secret".to_vec())));
        }
        self.wl(Wl::SetClipboard { offers });
    }

    pub fn copy_text(&self, text: &str) {
        self.set_clipboard_text(text);
    }

    /// Save (§5.0): export folder, and the clipboard when save_also_copies.
    pub fn save(&self, id: &str) -> Result<PathBuf, String> {
        let cfg = self.cfg();
        let path = self.store.save(id, &cfg, chrono::Local::now()).map_err(|e| e.to_string())?;
        if cfg.general.save_also_copies {
            let _ = self.copy(id);
        }
        trace::trace("saved", &format!("\"id\":\"{id}\",\"path\":{}", js(&path.to_string_lossy())));
        if let Ok(rec) = self.store.get(id) {
            let out = self.cards.lock().unwrap().iter().find(|(i, _)| i == id).map(|(_, o)| o.clone()).unwrap_or_default();
            let j = card_json(&rec, &out).to_string();
            let id = id.to_string();
            self.ui(move |e| e.card_update(&QString::from(id.as_str()), &QString::from(j.as_str())));
        }
        Ok(path)
    }

    /// Save As (QAO-09, ANN-15): to a path the user picked; also copies, as
    /// Save does when save_also_copies.
    pub fn save_as(&self, id: &str, path: &Path) -> Result<PathBuf, String> {
        let cfg = self.cfg();
        let path = self.store.save_as(id, path, &cfg).map_err(|e| e.to_string())?;
        if cfg.general.save_also_copies {
            let _ = self.copy(id);
        }
        trace::trace("saved", &format!("\"id\":\"{id}\",\"path\":{},\"as\":true", js(&path.to_string_lossy())));
        if let Ok(rec) = self.store.get(id) {
            let out = self.cards.lock().unwrap().iter().find(|(i, _)| i == id).map(|(_, o)| o.clone()).unwrap_or_default();
            let j = card_json(&rec, &out).to_string();
            let id = id.to_string();
            self.ui(move |e| e.card_update(&QString::from(id.as_str()), &QString::from(j.as_str())));
        }
        Ok(path)
    }

    /// What the Save As dialog starts with: the export folder and the name
    /// Save would use.
    pub fn save_as_suggestion(&self, id: &str) -> Value {
        let cfg = self.cfg();
        let rec = self.store.get(id).ok();
        let when = rec.as_ref().and_then(|r| chrono::DateTime::from_timestamp(r.created_at, 0)).map(|t| t.with_timezone(&chrono::Local)).unwrap_or_else(chrono::Local::now);
        let ctx = naming::NameContext {
            window_title: rec.as_ref().and_then(|r| r.title.clone()),
            app_name: rec.as_ref().and_then(|r| r.app.clone()),
            counter: 0,
            random: id.chars().rev().take(4).collect(),
        };
        let name = format!("{}.{}", cfg.file_stem(when, rec.as_ref().map_or(1.0, |r| r.scale), &ctx), cfg.export_format().ext());
        // the dialog opens a folder only if it exists (Save creates it too)
        let _ = std::fs::create_dir_all(cfg.export_dir());
        json!({ "name": name, "folder": cfg.export_dir() })
    }

    /// A file to drag out (QAO-04): the saved file, or a nicely named copy in
    /// the drag cache (never the history path, which has no useful name).
    pub fn drag_file(&self, id: &str) -> Result<PathBuf, String> {
        let rec = self.store.get(id).map_err(|e| e.to_string())?;
        if let Some(p) = rec.saved.filter(|p| p.exists()) {
            return Ok(p);
        }
        let dir = paths::drag_dir().join(id);
        if let Some(existing) = std::fs::read_dir(&dir).ok().and_then(|mut rd| rd.next()).and_then(|e| e.ok()).map(|e| e.path()) {
            return Ok(existing);
        }
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let cfg = self.cfg();
        let when = chrono::DateTime::from_timestamp(rec.created_at, 0).map(|t| t.with_timezone(&chrono::Local)).unwrap_or_else(chrono::Local::now);
        let ctx = naming::NameContext { window_title: rec.title.clone(), app_name: rec.app.clone(), counter: 0, random: id.chars().rev().take(4).collect() };
        let stem = cfg.file_stem(when, rec.scale, &ctx);
        let ext = rec.source.extension().and_then(|e| e.to_str()).unwrap_or("png");
        let path = dir.join(format!("{stem}.{ext}"));
        if std::fs::hard_link(&rec.source, &path).is_err() {
            std::fs::copy(&rec.source, &path).map_err(|e| e.to_string())?;
        }
        Ok(path)
    }

    pub fn show_in_files(&self, id: &str) {
        let Ok(rec) = self.store.get(id) else { return };
        let path = match rec.saved.filter(|p| p.exists()) {
            Some(p) => p,
            None => match self.drag_file(id) {
                Ok(p) => p,
                Err(_) => return,
            },
        };
        let uri = format!("file://{}", percent_encode(&path));
        spawn_detached(
            "gdbus",
            &[
                "call", "--session", "--dest", "org.freedesktop.FileManager1", "--object-path", "/org/freedesktop/FileManager1",
                "--method", "org.freedesktop.FileManager1.ShowItems", &format!("['{uri}']"), "",
            ],
        );
    }

    /// Delete from history (HIS-03): the capture leaves history; a copy saved
    /// in your folders stays.
    pub fn delete_capture(&self, id: &str) {
        let _ = std::fs::remove_dir_all(paths::drag_dir().join(id));
        let _ = self.store.delete(id);
        self.cards.lock().unwrap().retain(|(i, _)| i != id);
        self.closed.lock().unwrap().retain(|i| i != id);
    }

    pub fn history_bytes(&self) -> u64 {
        self.store.disk_usage()
    }

    /// Clear History: every capture leaves history (saved copies stay).
    pub fn clear_history(&self) -> usize {
        let all = self.store.recent(u32::MAX).unwrap_or_default();
        for r in &all {
            self.delete_capture(&r.id);
        }
        all.len()
    }

    /// Move to Trash: the saved file goes to the trash; the capture leaves history.
    pub fn trash(&self, id: &str) {
        if let Ok(rec) = self.store.get(id) {
            if let Some(p) = rec.saved.filter(|p| p.exists()) {
                spawn_detached("gio", &["trash", &p.to_string_lossy()]);
            }
        }
        let _ = std::fs::remove_dir_all(paths::drag_dir().join(id));
        let _ = self.store.delete(id);
        self.cards.lock().unwrap().retain(|(i, _)| i != id);
        self.closed.lock().unwrap().retain(|i| i != id);
    }

    /// Annotate / double-click: Snip's editor, or the configured external one.
    pub fn annotate(&self, id: &str) {
        let editor = self.cfg().general.external_editor;
        if editor.trim().is_empty() {
            self.edit(id);
            return;
        }
        if let Ok(p) = self.drag_file(id) {
            let mut parts = editor.split_whitespace();
            if let Some(cmd) = parts.next() {
                let mut args: Vec<String> = parts.map(String::from).collect();
                args.push(p.to_string_lossy().into_owned());
                let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                spawn_detached(cmd, &refs);
            }
        }
    }

    /// Pin a capture to the screen (PIN-01): its image floats above every
    /// window, on every workspace of its monitor, at its own size.
    /// Pin to the Screen… (PIN-01): an image file, pinned as a new capture.
    pub fn pin_file(&self, path: &str) -> Result<Value, String> {
        self.import(path).and_then(|rec| self.pin(&rec.id))
    }

    pub fn pin(&self, id: &str) -> Result<Value, String> {
        self.pin_at(id, None)
    }

    /// A pin whose image's corner goes at `at` (layout px), or centred.
    fn pin_at(&self, id: &str, at: Option<(i32, i32)>) -> Result<Value, String> {
        let rec = self.store.get(id).map_err(|e| e.to_string())?;
        if rec.media() != Media::Image {
            return Err("only images can be pinned".into());
        }
        let monitors = snip_hypr::monitors();
        let here = self.cursor_output();
        let scale = monitors.iter().find(|m| m.name == rec.output).or_else(|| monitors.iter().find(|m| Some(&m.name) == here.as_ref())).map(|m| m.scale).unwrap_or(1.0);
        let mut j = card_json(&rec, &rec.output);
        j["scale"] = json!(scale);
        if let Some((x, y)) = at {
            j["at"] = json!([x, y]);
        }
        let s = j.to_string();
        trace::trace("pin_open", &format!("\"id\":\"{id}\""));
        self.ui(move |e| e.pin_open(&QString::from(s.as_str())));
        Ok(j)
    }

    /// Open Snip's editor on a capture (ANN-16: its objects stay editable).
    pub fn edit(&self, id: &str) {
        let Ok(rec) = self.store.get(id) else { return };
        if rec.media() != Media::Image {
            return; // recordings open in the video editor (VED)
        }
        let Ok((image, doc)) = self.store.edit_state(id) else { return };
        let (width, height) = png_file_size(&image).unwrap_or((rec.width, rec.height));
        let output = self.cursor_output();
        // QScreen rounds fractional scales up; the window's first size needs the real one
        let scale = snip_hypr::monitors().into_iter().find(|m| Some(&m.name) == output.as_ref()).map(|m| m.scale).unwrap_or(1.0);
        let j = json!({
            "id": id, "image": image, "source": rec.source, "width": width, "height": height,
            "doc": doc.and_then(|d| serde_json::from_str::<Value>(&d).ok()),
            "title": rec.title, "output": output, "scale": scale,
        })
        .to_string();
        trace::trace("editor_open", &format!("\"id\":\"{id}\""));
        self.ui(move |e| e.editor_open(&QString::from(j.as_str())));
    }

    /// `snip edit FILE` (ANN-19): the file becomes a capture (the user's file
    /// is never written to), opened in the editor without a card.
    pub fn edit_file(self: &Arc<Self>, path: &str) -> Result<Value, String> {
        let rec = self.import(path)?;
        self.edit(&rec.id);
        Ok(serde_json::to_value(&rec).unwrap_or(Value::Null))
    }

    /// An editor's render and objects go back to its capture (ANN-15), then
    /// Copy or Save run on the result. QML keeps one commit in flight per
    /// editor, so they land in order.
    pub fn editor_commit(self: &Arc<Self>, id: &str, token: u32, doc: String, action: String) {
        let (me, id) = (self.clone(), id.to_string());
        std::thread::spawn(move || {
            let t0 = trace::now_ns();
            let done = |ok: bool, detail: String| {
                // an auto-applied background (BG-03): now the card, rendered or not
                if action == "auto-bg" {
                    let out = me.pending_bg.lock().unwrap().remove(&id);
                    if let (Some(out), Ok(rec)) = (out, me.store.get(&id)) {
                        me.after_capture(&rec, &out);
                    }
                    return;
                }
                let (id, action) = (id.clone(), action.clone());
                me.ui(move |e| e.editor_done(&QString::from(id.as_str()), &QString::from(action.as_str()), ok, &QString::from(detail.as_str())));
            };
            let Some(r) = crate::edits::take(token) else { return done(false, "nothing was rendered".into()) };
            let opaque = r.rgba.chunks_exact(4).all(|p| p[3] == 255);
            let png = if opaque {
                let rgb: Vec<u8> = r.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                encode::encode_png(&rgb, r.width, r.height, false)
            } else {
                encode::encode_png(&r.rgba, r.width, r.height, true)
            };
            let rec = match me.store.write_edit(&id, png, r.width, r.height, &doc, &me.cfg()) {
                Ok(rec) => rec,
                Err(e) => return done(false, e.to_string()),
            };
            let _ = std::fs::remove_dir_all(paths::drag_dir().join(&id));
            me.wake_indexer();
            let out = me.cards.lock().unwrap().iter().find(|(i, _)| i == &id).map(|(_, o)| o.clone()).unwrap_or_default();
            let j = card_json(&rec, &out).to_string();
            let cid = id.clone();
            me.ui(move |e| e.card_update(&QString::from(cid.as_str()), &QString::from(j.as_str())));
            trace::trace("edited", &format!("\"id\":\"{id}\",\"w\":{},\"h\":{},\"ms\":{:.1}", r.width, r.height, trace::ms_since(t0)));
            let res = match action.as_str() {
                "copy" => me.copy(&id),
                "save" => me.save(&id).map(|_| ()),
                "pin" => me.pin(&id).map(|_| ()),
                _ => Ok(()),
            };
            match res {
                Ok(()) => done(true, String::new()),
                Err(e) => done(false, e),
            }
        });
    }

    /// QAO-09 Rotate Left, Flip Horizontal, Scale to 1×, Resize ("resize W H"):
    /// rewrite the capture (and its saved file), then refresh its card.
    pub fn transform(self: &Arc<Self>, id: &str, op: &str) {
        let (me, id, op) = (self.clone(), id.to_string(), op.to_string());
        std::thread::spawn(move || {
            let Ok(rec) = me.store.get(&id) else { return };
            if rec.media() != Media::Image {
                return;
            }
            let Some((data, w, h, alpha)) = me.store.bytes(&id).ok().and_then(|b| encode::decode_png(&b).ok()) else { return };
            let bpp = if alpha { 4 } else { 3 };
            let size: Option<(u32, u32)> = match op.split_whitespace().collect::<Vec<_>>()[..] {
                ["scale-1x"] if rec.scale > 1.0 => Some(((w as f64 / rec.scale).round() as u32, (h as f64 / rec.scale).round() as u32)),
                ["resize", nw, nh] => nw.parse().ok().zip(nh.parse().ok()).filter(|&(nw, nh): &(u32, u32)| nw > 0 && nh > 0 && nw <= 20000 && nh <= 20000),
                _ => None,
            };
            let (data, w, h) = match (op.as_str(), size) {
                ("rotate-left", _) => encode::rotate_left(&data, w, h, bpp),
                ("flip-horizontal", _) => (encode::flip_horizontal(&data, w, h, bpp), w, h),
                (_, Some((nw, nh))) => {
                    // what it shows stays the same size: its scale follows the pixels
                    let _ = me.store.set_scale(&id, rec.scale * nw as f64 / w as f64);
                    (encode::resize(&data, w, h, bpp, nw, nh), nw, nh)
                }
                _ => return,
            };
            let png = encode::encode_png(&data, w, h, alpha);
            // the rotated pixels are the new starting point for the editor
            let _ = me.store.flatten(&id);
            match me.store.replace_image(&id, png, w, h, &me.cfg()) {
                Ok(rec) => {
                    // the drag copy is stale now; it is recreated on demand
                    let _ = std::fs::remove_dir_all(paths::drag_dir().join(&id));
                    let out = me.cards.lock().unwrap().iter().find(|(i, _)| i == &id).map(|(_, o)| o.clone()).unwrap_or_default();
                    let j = card_json(&rec, &out).to_string();
                    trace::trace("transformed", &format!("\"id\":\"{id}\",\"op\":{},\"w\":{w},\"h\":{h}", js(&op)));
                    me.ui(move |e| e.card_update(&QString::from(id.as_str()), &QString::from(j.as_str())));
                }
                Err(e) => me.toast(format!("Could not change the capture: {e}")),
            }
        });
    }

    /// Extract text from a stored capture (QAO-09).
    pub fn extract_text(self: &Arc<Self>, id: &str) {
        let me = self.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            // the card shows "Reading text…" until one of these arrives; a
            // recording is read from its first frame
            let bytes = match me.store.get(&id).ok().and_then(|r| Store::poster(&r)) {
                Some(poster) => std::fs::read(poster).ok().map(Arc::new),
                None => me.store.bytes(&id).ok(),
            };
            let Some((data, w, h, alpha)) = bytes.and_then(|b| encode::decode_png(&b).ok()) else {
                me.card_status(&id, "error", "Couldn't open the image");
                return;
            };
            let bpp = if alpha { 4 } else { 3 };
            let codes = qr::decode(&data, w, h, bpp);
            if !codes.is_empty() {
                let summary = me.copy_qr(&codes.join("\n"));
                me.card_status(&id, "qr-copied", summary.split(" · ").nth(1).unwrap_or(""));
                return;
            }
            let langs = me.ocr_languages();
            match ocr::recognize(&data, w, h, bpp, 2.0, &langs) {
                Ok(t) if !t.trim().is_empty() => {
                    me.set_clipboard_text(&t);
                    let n = t.chars().count();
                    me.card_status(&id, "text-copied", &format!("{n} character{}", if n == 1 { "" } else { "s" }));
                }
                Ok(_) => me.card_status(&id, "no-text", ""),
                Err(e) => me.card_status(&id, "error", &e.to_string()),
            }
        });
    }

    /// Word boxes for the editor's highlighter (ANN-10), in the main OCR language.
    pub fn text_words(self: &Arc<Self>, path: String) {
        let me = self.clone();
        std::thread::spawn(move || {
            let t0 = trace::now_ns();
            let words = crate::edits::decoded(&path)
                .and_then(|img| {
                    let langs = me.ocr_languages();
                    let lang = langs.split('+').next().unwrap_or("eng").to_string();
                    ocr::words(&img.data, img.width, img.height, img.bpp, 1.0, &lang).ok()
                })
                .unwrap_or_default();
            trace::trace("editor_words", &format!("\"n\":{},\"ms\":{:.0}", words.len(), trace::ms_since(t0)));
            let j = serde_json::to_string(&words).unwrap_or_else(|_| "[]".into());
            me.ui(move |e| e.editor_words(&QString::from(path.as_str()), &QString::from(j.as_str())));
        });
    }

    fn ocr_languages(&self) -> String {
        let configured = self.cfg().ocr.languages;
        if !configured.trim().is_empty() {
            return configured;
        }
        self.ocr_langs
            .get_or_init(|| {
                let layouts = snip_hypr::option_str("input:kb_layout").unwrap_or_default();
                ocr::languages_for_layouts(&layouts, &ocr::installed_languages())
            })
            .clone()
    }

    pub fn open_file(self: &Arc<Self>, path: &str) -> Result<Value, String> {
        let rec = self.import(path)?;
        let out = self.cursor_output().unwrap_or_default();
        self.show_card(&rec, &out);
        Ok(card_json(&rec, &out))
    }

    /// `snip edit --clipboard` (ANN-19): the clipboard's image (or a copied
    /// PNG file) becomes a capture, opened in the editor without a card.
    pub fn edit_clipboard(self: &Arc<Self>) -> Result<Value, String> {
        let (tx, rx) = mpsc::channel();
        self.wl(Wl::ReadClipboard { mimes: vec!["image/png".into(), "text/uri-list".into()], reply: tx });
        let (mime, bytes) = rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "the clipboard did not answer".to_string())?
            .ok_or("no image on the clipboard")?;
        let rec = if mime == "image/png" {
            let (w, h) = png_size(&bytes).ok_or("the clipboard image is not a valid PNG")?;
            let out = self.cursor_output().unwrap_or_default();
            let meta = CaptureMeta { kind: "clipboard".into(), scale: 1.0, output: out, app: None, title: Some("Clipboard".into()) };
            let rec = self.store.ingest(bytes, w, h, meta, now_secs()).map_err(|e| e.to_string())?;
            self.wake_indexer();
            rec
        } else {
            // a file copied in a file manager
            let text = String::from_utf8_lossy(&bytes);
            let uri = text.lines().map(str::trim).find(|l| l.starts_with("file://")).ok_or("no image on the clipboard")?;
            self.import(&percent_decode(&uri["file://".len()..]))?
        };
        self.edit(&rec.id);
        Ok(serde_json::to_value(&rec).unwrap_or(Value::Null))
    }

    /// Add image > Take Screenshot (ANN-12): an area (or window) selection
    /// whose pixels go into editor `token` as a file, not into history.
    /// Answered with editor_screenshot (an empty path when cancelled).
    pub fn editor_screenshot(self: &Arc<Self>, token: i32) {
        let (tx, rx) = mpsc::channel();
        self.begin("insert", None, None, Some(tx), 0, None);
        let me = self.clone();
        std::thread::spawn(move || {
            let path = rx.recv().ok().filter(|r| r.ok).and_then(|r| r.result.as_str().map(String::from)).unwrap_or_default();
            me.ui(move |e| e.editor_screenshot(token, &QString::from(path.as_str())));
        });
    }

    /// The editor's eyedropper (ANN-13): the frozen screen with the
    /// magnifier; a click answers editor_color with "#rrggbb" ("" if cancelled).
    pub fn editor_pick_color(self: &Arc<Self>, token: i32) {
        let (tx, rx) = mpsc::channel();
        self.begin("pick", None, None, Some(tx), 0, None);
        let me = self.clone();
        std::thread::spawn(move || {
            let c = rx.recv().ok().filter(|r| r.ok).and_then(|r| r.result.as_str().map(String::from)).unwrap_or_default();
            me.ui(move |e| e.editor_color(token, &QString::from(c.as_str())));
        });
    }

    /// The clipboard's PNG, if it holds one (2 s at most).
    pub fn clipboard_png(&self) -> Option<Vec<u8>> {
        let (tx, rx) = mpsc::channel();
        self.wl(Wl::ReadClipboard { mimes: vec!["image/png".into()], reply: tx });
        rx.recv_timeout(Duration::from_secs(2)).ok().flatten().map(|(_, b)| b)
    }

    /// An image file (or a Snip project) as a new capture in history.
    fn import(&self, path: &str) -> Result<Record, String> {
        let title = Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned());
        if Media::of(Path::new(path)) != Media::Image {
            let out = self.cursor_output().unwrap_or_default();
            return self.ingest_media(Path::new(path), false, CaptureMeta { kind: "import".into(), scale: 1.0, output: out, app: None, title });
        }
        if path.to_ascii_lowercase().ends_with(".snip") {
            let out = self.cursor_output().unwrap_or_default();
            let rec = self.store.import_project(Path::new(path), &out, now_secs()).map_err(|e| e.to_string())?;
            self.wake_indexer();
            return Ok(rec);
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let (w, h) = png_size(&bytes).ok_or("only PNG files, videos and GIFs can be opened for now")?;
        let out = self.cursor_output().unwrap_or_default();
        let rec = self
            .store
            .ingest(bytes, w, h, CaptureMeta { kind: "import".into(), scale: 1.0, output: out, app: None, title }, now_secs())
            .map_err(|e| e.to_string())?;
        self.wake_indexer();
        Ok(rec)
    }

    /// A video or GIF into history (a recording's own file is `take`n):
    /// ffprobe reads it, its first frame becomes the poster.
    fn ingest_media(&self, file: &Path, take: bool, meta: CaptureMeta) -> Result<Record, String> {
        let p = media::probe(file)?;
        let poster = media::poster(file)?;
        let rec = self.store.ingest_media(file, take, &poster, p.width, p.height, p.duration_ms, meta, now_secs()).map_err(|e| e.to_string())?;
        trace::trace("media_ingested", &format!("\"id\":\"{}\",\"media\":{},\"w\":{},\"h\":{},\"ms\":{}", rec.id, js(&format!("{:?}", rec.media()).to_lowercase()), p.width, p.height, p.duration_ms));
        self.wake_indexer();
        Ok(rec)
    }

    // ------------------------------------------------------------------ sound

    pub fn play(&self, name: &str) {
        if !self.cfg().general.sounds {
            return;
        }
        let user = paths::config_dir().join("sounds").join(format!("{name}.wav"));
        let path = if user.exists() {
            user
        } else {
            let p = paths::runtime_dir().join(format!("{name}.wav"));
            if !p.exists() {
                let _ = paths::ensure_private_dir(&paths::runtime_dir());
                let _ = std::fs::write(&p, SHUTTER);
            }
            p
        };
        spawn_detached("pw-play", &["--volume=0.6", &path.to_string_lossy()]);
    }

    // ------------------------------------------------------------------ live config / theme

    pub fn reload_config(&self) {
        let (cfg, err) = Config::load();
        let before = std::mem::replace(&mut *self.cfg.write().unwrap(), cfg).shortcuts;
        if before != self.cfg().shortcuts {
            self.apply_shortcuts(&before);
        }
        *self.cfg_error.lock().unwrap() = err.clone();
        if let Some(e) = err {
            self.toast(format!("config.toml: {e}"));
        }
        self.push_settings();
    }

    /// Settings › Shortcuts (SET-08): bind [shortcuts] over snip.lua's
    /// defaults. `before` is what was applied last (empty after a start or
    /// Hyprland's reload, which dropped them); an action that left the table
    /// gets snip.lua's default back.
    pub fn apply_shortcuts(&self, before: &BTreeMap<String, String>) {
        use snip_hypr::keys;
        let now = self.cfg().shortcuts;
        let app = crate::bridge::app_id();
        let current = |binds: &[keys::Bind], action: &str| -> Vec<String> {
            let d = keys::description(action);
            binds.iter().filter(|b| b.submap.is_empty() && Some(&b.description) == d.as_ref()).map(|b| b.spec()).collect()
        };
        let gone: Vec<&String> = before.keys().filter(|a| !now.contains_key(*a)).collect();
        if !gone.is_empty() {
            let binds = keys::binds();
            let _ = snip_hypr::eval(&keys::set_lua(&app, &gone.iter().map(|a| (a.to_string(), current(&binds, a), String::new())).collect::<Vec<_>>()));
            let _ = snip_hypr::eval("if snip_bind_defaults then snip_bind_defaults() end");
        }
        if now.is_empty() {
            return;
        }
        let binds = keys::binds();
        let changes: Vec<_> = now.iter().map(|(a, spec)| (a.clone(), current(&binds, a), spec.clone())).collect();
        let r = snip_hypr::eval(&keys::set_lua(&app, &changes));
        trace::trace("shortcuts", &format!("\"n\":{},\"reply\":{}", now.len(), js(&format!("{r:?}"))));
    }

    /// Settings › Shortcuts: every action, with the keys Hyprland has for it.
    pub fn shortcuts(&self) -> Value {
        use snip_hypr::keys;
        let binds = keys::binds();
        let cfg = self.cfg();
        let list: Vec<Value> = keys::ACTIONS
            .iter()
            .map(|(id, name)| {
                let d = keys::description(id);
                let labels: Vec<String> = binds.iter().filter(|b| b.submap.is_empty() && Some(&b.description) == d.as_ref()).map(|b| keys::label(b.modmask, &b.key)).collect();
                json!({ "id": id, "name": name, "keys": labels, "custom": cfg.shortcuts.contains_key(*id) })
            })
            .collect();
        Value::Array(list)
    }

    /// A recorded key (SET-08): its label and the bind it would take over.
    pub fn shortcut_conflict(&self, action: &str, spec: &str) -> Value {
        use snip_hypr::keys;
        let (mask, key) = keys::parse_spec(spec);
        let binds = keys::binds();
        let takes = keys::conflict(&binds, action, spec).map(|b| if b.description.is_empty() { "another bind".to_string() } else { b.description.clone() });
        json!({ "label": keys::label(mask, &key), "takes": takes })
    }

    /// Restore Defaults: [shortcuts] leaves the file; snip.lua's keys return.
    pub fn restore_shortcuts(&self) -> Result<(), String> {
        let path = Config::path();
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let out = snip_core::config::remove_value(&text, "shortcuts")?;
        std::fs::write(&path, out).map_err(|e| e.to_string())?;
        self.reload_config();
        Ok(())
    }

    /// The settings window changes one setting (SET-02): written into
    /// config.toml (comments kept) only if the file stays valid.
    pub fn set_setting(&self, dotted: &str, value: &str) -> Result<(), String> {
        let value: Value = serde_json::from_str(value).map_err(|e| e.to_string())?;
        let path = Config::path();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.to_string()),
        };
        let out = snip_core::config::set_json(&text, dotted, &value)?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::create_dir_all(path.parent().unwrap_or(&path))
            .and_then(|_| std::fs::write(&tmp, out))
            .and_then(|_| std::fs::rename(&tmp, &path))
            .map_err(|e| e.to_string())?;
        self.reload_config();
        Ok(())
    }

    pub fn push_settings(&self) {
        let c = self.cfg();
        let j = json!({
            "quickAccess": {
                "position": c.quick_access.position,
                "autoClose": c.quick_access.auto_close,
                "autoCloseSeconds": c.quick_access.auto_close_seconds,
                "autoCloseAction": c.quick_access.auto_close_action,
                "maxVisible": c.quick_access.max_visible,
                "size": c.quick_access.size,
                "closeAfterDrag": c.quick_access.close_after_drag,
            },
            "capture": c.capture,
            "annotate": c.annotate,
            "pins": c.pins,
            "sounds": c.general.sounds,
            "saveAlsoCopies": c.general.save_also_copies,
            // the settings window (SET-02): everything, as in the file
            "config": c,
            "configError": *self.cfg_error.lock().unwrap(),
            "configPath": Config::path(),
            "version": env!("SNIP_VERSION"),
        })
        .to_string();
        self.ui(move |mut e| e.as_mut().set_settings(QString::from(j.as_str())));
    }

    pub fn push_theme(&self) {
        let rounding = snip_hypr::option_int("decoration:rounding").unwrap_or(0).max(0) as u32;
        let mut t = theme::current(rounding);
        t["roundingPower"] = json!(snip_hypr::option_float("decoration:rounding_power").unwrap_or(2.0));
        let j = t.to_string();
        trace::trace("theme", &format!("\"name\":{}", t["name"]));
        self.ui(move |mut e| e.as_mut().set_theme(QString::from(j.as_str())));
    }

    pub fn sweep(&self) {
        let r = self.cfg().history.retention;
        // "Never" removes captures as their cards close; at start nothing is open.
        let max_age = if r == Retention::Never && !self.cards.lock().unwrap().is_empty() { return } else { r.max_age_secs() };
        if let Ok(n) = self.store.sweep(now_secs(), max_age) {
            if n > 0 {
                trace::trace("history_sweep", &format!("\"removed\":{n}"));
            }
        }
        // drag copies of captures that are gone
        if let Ok(rd) = std::fs::read_dir(paths::drag_dir()) {
            for e in rd.flatten() {
                let id = e.file_name().to_string_lossy().into_owned();
                if self.store.get(&id).is_err() {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }

    // ------------------------------------------------------------------ ctl.sock

    pub fn handle_ctl(self: &Arc<Self>, req: Ctl) -> Response {
        let ok_or = |r: Result<Value, String>| match r {
            Ok(v) => Response::ok(v),
            Err(e) => Response::err(e),
        };
        match req {
            Ctl::Ping => Response::ok(json!("pong")),
            Ctl::ReadText { path } => self.text_of(path.as_deref()),
            Ctl::Capture { mode, monitor, cursor, delay, action, rect } => {
                let action = match action.map(|a| serde_json::from_value::<AfterCapture>(json!(a))) {
                    Some(Err(_)) => return Response::err("--action is copy, save, annotate or pin"),
                    a => a.and_then(Result::ok),
                };
                let (tx, rx) = mpsc::channel();
                match rect {
                    Some([x, y, w, h]) => self.capture_rect(monitor, Rect::new(x, y, w, h), cursor, Some(tx), action),
                    None => self.begin(&mode, monitor, cursor, Some(tx), delay.unwrap_or(0), action),
                }
                rx.recv_timeout(Duration::from_secs(590)).unwrap_or_else(|_| Response::err("timed out"))
            }
            Ctl::Open { path } => ok_or(self.open_file(&path)),
            Ctl::Edit { path } => ok_or(self.edit_file(&path)),
            Ctl::EditClipboard => ok_or(self.edit_clipboard()),
            Ctl::Copy { id } => ok_or(self.resolve_id(id).and_then(|id| self.copy(&id).map(|_| json!(id)))),
            Ctl::Save { id } => ok_or(self.resolve_id(id).and_then(|id| self.save(&id).map(|p| json!(p)))),
            // Pin File… (PIN-01): the file becomes a capture first
            Ctl::Pin { id, path: Some(path) } if id.is_none() => ok_or(self.import(&path).and_then(|rec| self.pin(&rec.id))),
            Ctl::Pin { id, .. } => ok_or(self.resolve_id(id).and_then(|id| self.pin(&id))),
            Ctl::History => {
                self.open_history();
                Response::ok(Value::Null)
            }
            Ctl::Welcome => {
                self.ui(|e| e.welcome_open());
                Response::ok(Value::Null)
            }
            Ctl::Settings { tab } => {
                self.ui(move |e| e.settings_open(&QString::from(tab.unwrap_or_default().as_str())));
                Response::ok(Value::Null)
            }
            Ctl::Desktop { action } => self.desktop(&action),
            Ctl::Pins { action } => match action.as_str() {
                "toggle" | "close" | "unlock" => {
                    self.ui(move |e| e.pins_command(&QString::from(action.as_str())));
                    Response::ok(Value::Null)
                }
                _ => Response::err("pins: toggle, close or unlock"),
            },
            Ctl::Search { query, n } => ok_or(
                self.store
                    .search(&query, n)
                    .map(|v| {
                        Value::Array(
                            v.into_iter()
                                .map(|(rec, snippet)| {
                                    let mut j = serde_json::to_value(&rec).unwrap_or(Value::Null);
                                    j["snippet"] = json!(snippet.replace('\n', " "));
                                    j
                                })
                                .collect(),
                        )
                    })
                    .map_err(|e| e.to_string()),
            ),
            Ctl::List { n } => ok_or(self.store.recent(n).map(|v| serde_json::to_value(v).unwrap_or(Value::Null)).map_err(|e| e.to_string())),
            Ctl::RestoreCard { id } => match self.store.get(&id) {
                Ok(_) => {
                    self.restore_card(&id);
                    Response::ok(Value::Null)
                }
                Err(e) => Response::err(e.to_string()),
            },
            Ctl::RestoreLast => {
                // QML decides: un-hide hidden cards, else reopen the last closed one
                self.ui(|e| e.cards_command(&QString::from("restore")));
                Response::ok(Value::Null)
            }
            Ctl::CloseAll => {
                self.ui(|e| e.cards_command(&QString::from("close-all")));
                Response::ok(Value::Null)
            }
            Ctl::Status => Response::ok(self.status()),
            Ctl::Reload => {
                self.reload_config();
                self.push_theme();
                let _ = snip_hypr::install_runtime_rules();
                match self.cfg_error.lock().unwrap().clone() {
                    Some(e) => Response::err(format!("config.toml: {e}")),
                    None => Response::ok(Value::Null),
                }
            }
            Ctl::Quit => {
                self.ui(|e| e.quit_requested());
                Response::ok(Value::Null)
            }
            Ctl::Debug { op, args } => self.debug(&op, &args),
        }
    }

    fn status(&self) -> Value {
        let f = *self.features.lock().unwrap();
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "outputs": self.outputs.lock().unwrap().iter().map(|o| json!({"name": o.name, "x": o.logical.x, "y": o.logical.y, "w": o.logical.w, "h": o.logical.h, "transform": o.transform})).collect::<Vec<_>>(),
            "features": {"data_control": f.data_control, "global_shortcuts": f.global_shortcuts, "virtual_pointer": f.virtual_pointer, "toplevel_capture": f.toplevel_capture},
            "overlay": self.session.lock().unwrap().as_ref().map(|s| s.mode.clone()),
            "cards": self.cards.lock().unwrap().iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(),
            "closed": self.closed.lock().unwrap().len(),
            "history_bytes": self.store.disk_usage(),
            "index_pending": self.store.unindexed_count(),
            "config_error": *self.cfg_error.lock().unwrap(),
        })
    }

    /// Automation for tests/e2e: synthetic pointer (wlr virtual pointer) and
    /// UI commands. Only meaningful on dedicated test workspaces.
    fn debug(self: &Arc<Self>, op: &str, a: &[String]) -> Response {
        let n = |i: usize| a.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        match op {
            "move" => {
                let output = a.first().cloned().unwrap_or_default();
                let lw = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| (o.logical.w, o.logical.h));
                let Some((w, h)) = lw else { return Response::err("no such output") };
                self.wl(Wl::PointerMove { output, x: n(1) as u32, y: n(2) as u32, w: w as u32, h: h as u32 });
                Response::ok(Value::Null)
            }
            "down" | "up" | "rdown" | "rup" | "mdown" | "mup" => {
                let button = if op.starts_with('r') { 0x111 } else if op.starts_with('m') { 0x112 } else { 0x110 };
                self.wl(Wl::PointerButton { down: op.ends_with("down"), button });
                Response::ok(Value::Null)
            }
            "scroll" => {
                self.wl(Wl::PointerAxis { dy: n(0) });
                Response::ok(Value::Null)
            }
            "wheel" => {
                self.wl(Wl::PointerWheel { notches: n(0) as i32 });
                Response::ok(Value::Null)
            }
            "ui" => {
                // forward a command to QML (e.g. "key Escape", "key space", "finish")
                let cmd = a.join(" ");
                self.ui(move |e| e.debug_command(&QString::from(cmd.as_str())));
                Response::ok(Value::Null)
            }
            "sweep" => {
                self.sweep();
                Response::ok(Value::Null)
            }
            "transform" => {
                let (Some(id), Some(op)) = (a.first(), a.get(1)) else { return Response::err("transform needs an id and an op") };
                self.transform(id, op);
                Response::ok(Value::Null)
            }
            "save-as" => {
                let (Some(id), Some(path)) = (a.first(), a.get(1)) else { return Response::err("save-as needs an id and a path") };
                match self.save_as(id, Path::new(path)) {
                    Ok(p) => Response::ok(json!(p)),
                    Err(e) => Response::err(e),
                }
            }
            "annotate" => {
                let Some(id) = a.first() else { return Response::err("annotate needs an id") };
                self.annotate(id);
                Response::ok(Value::Null)
            }
            "nudge" => {
                self.nudge_pointer();
                Response::ok(Value::Null)
            }
            "extract" => {
                // Extract Text on a stored capture, as from its card
                let Some(id) = a.first() else { return Response::err("extract needs an id") };
                self.extract_text(id);
                Response::ok(Value::Null)
            }
            _ => Response::err(format!("unknown debug op '{op}'")),
        }
    }
}

// ---------------------------------------------------------------------- helpers

fn last_area_file() -> PathBuf {
    paths::data_dir().join("last-area.json")
}

fn load_last_area() -> Option<(String, Rect)> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(last_area_file()).ok()?).ok()?;
    let g = |k: &str| v[k].as_i64().map(|x| x as i32);
    Some((v["output"].as_str()?.to_string(), Rect::new(g("x")?, g("y")?, g("w")?, g("h")?)))
}

fn save_last_area(output: &str, r: Rect) {
    let _ = std::fs::create_dir_all(paths::data_dir());
    let _ = std::fs::write(last_area_file(), json!({"output": output, "x": r.x, "y": r.y, "w": r.w, "h": r.h}).to_string());
}

fn percent_encode(p: &Path) -> String {
    let mut s = String::new();
    for b in p.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => s.push(b as char),
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

/// Reverses `percent_encode` (and any %XX); a stray % stays as it is.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Fire and forget, reaped on a helper thread (no zombies in a long-lived daemon).
fn spawn_detached(cmd: &str, args: &[&str]) {
    use std::process::{Command, Stdio};
    match Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        Ok(mut c) => {
            std::thread::spawn(move || {
                let _ = c.wait();
            });
        }
        Err(e) => trace::warn("spawn_failed", &format!("\"cmd\":{},\"msg\":{}", js(cmd), js(&e.to_string()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_reverses_encoding() {
        let p = Path::new("/home/me/Pictures/Snip 2026 #1 ქ.png");
        assert_eq!(super::percent_decode(&super::percent_encode(p)), p.to_string_lossy());
        assert_eq!(super::percent_decode("a%2"), "a%2");
    }

    #[test]
    fn percent_encoding_keeps_paths_readable() {
        assert_eq!(percent_encode(Path::new("/a b/Snip 2026.png")), "/a%20b/Snip%202026.png");
    }

    #[test]
    fn png_size_reads_ihdr() {
        let png = encode::encode_png(&[0u8; 3 * 6], 3, 2, false);
        assert_eq!(png_size(&png), Some((3, 2)));
    }
}
