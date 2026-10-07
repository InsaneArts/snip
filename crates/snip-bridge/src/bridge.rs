//! cxx-qt bridge: the `Engine` QML singleton (module Snip.Core) and the plain
//! functions the C++ items call to reach frames without copying.

#[cxx_qt::bridge(namespace = "snip")]
pub mod qobject {
    #[namespace = ""]
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    /// What C++ needs to wrap a frame in a QImage. `handle` (when non-zero)
    /// owns a reference: pass it to `frame_release` exactly once.
    #[derive(Default)]
    struct FrameInfo {
        valid: bool,
        ptr: usize,
        handle: usize,
        width: u32,
        height: u32,
        stride: u32,
        format: u32,
        epoch: u32,
        transform: u32,
    }

    /// Tiles for the editor's Pixelate / Blur (opaque RGBA, width × height).
    #[derive(Default)]
    struct RedactTiles {
        valid: bool,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    }

    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(i32, epoch)]
        /// Theme tokens (JSON), live (OMA-01)
        #[qproperty(QString, theme)]
        /// UI-relevant settings (JSON), live (SET-01)
        #[qproperty(QString, settings)]
        /// The current overlay session (JSON): mode, outputs, windows, cursor
        #[qproperty(QString, session)]
        type Engine = super::EngineRust;

        /// `mode`: "interactive" (owns shortcuts and ctl.sock) or "test".
        #[qinvokable]
        fn start(self: Pin<&mut Engine>, mode: QString);

        #[qinvokable]
        fn capture(self: Pin<&mut Engine>, mode: QString);

        /// Selection done: physical px of `output`; kind = area | window | monitor
        #[qinvokable]
        fn finish_area(self: Pin<&mut Engine>, output: QString, x: i32, y: i32, w: i32, h: i32, kind: QString, skip_preset: bool);

        /// Window-mode click: clean toplevel capture; rect is the fallback crop.
        #[qinvokable]
        fn finish_window(self: Pin<&mut Engine>, address: QString, output: QString, x: i32, y: i32, w: i32, h: i32, invert: bool);

        #[qinvokable]
        fn cancel(self: Pin<&mut Engine>);

        /// The self-timer's countdown is over and off screen: capture.
        #[qinvokable]
        fn countdown_done(self: &Engine);

        /// The overlay is on screen: hand it the pointer even if it hasn't moved.
        #[qinvokable]
        fn refocus_pointer(self: &Engine);

        /// The Quick Access stack is off screen: run the deferred capture.
        #[qinvokable]
        fn capture_ready(self: Pin<&mut Engine>, request: i32);

        #[qinvokable]
        fn copy(self: Pin<&mut Engine>, id: QString) -> bool;

        #[qinvokable]
        fn save(self: Pin<&mut Engine>, id: QString);

        /// The words in the image at `path`, for the highlighter (ANN-10);
        /// answers with editor_words, off the GUI thread.
        #[qinvokable]
        fn request_words(self: &Engine, path: QString);

        /// Add image > Take Screenshot (ANN-12): select an area or window; the
        /// pixels come back through editor_screenshot, not as a capture
        #[qinvokable]
        fn take_screenshot_for_editor(self: &Engine, token: i32);

        /// The eyedropper, anywhere on screen (ANN-13): answered with
        /// editor_color; the overlay's click calls finish_pick
        #[qinvokable]
        fn pick_color_for_editor(self: &Engine, token: i32);
        #[qinvokable]
        fn finish_pick(self: &Engine, color: QString);

        /// What the editor remembers between sessions (the last tool): JSON
        #[qinvokable]
        fn editor_prefs(self: &Engine) -> QString;
        #[qinvokable]
        fn set_editor_prefs(self: &Engine, json: QString);

        /// "My Colors" (ANN-13): up to ten, kept across sessions (JSON array)
        #[qinvokable]
        fn saved_colors(self: &Engine) -> QString;
        #[qinvokable]
        fn set_saved_colors(self: &Engine, json: QString);

        /// The colour around the image's edge (the most common one): what an
        /// outward crop fills the new canvas with (ANN-11)
        #[qinvokable]
        fn edge_color(self: &Engine, path: QString) -> QString;

        /// Auto-balance (BG-02): the plain margins of a region of the image
        /// at `path` and their colour, as JSON {l, t, r, b, color}
        #[qinvokable]
        fn content_margins(self: &Engine, path: QString, x: i32, y: i32, w: i32, h: i32) -> QString;

        /// Rotate, flip or resize the editor's base image (ANN-11) into a new
        /// file beside it: op = rotate-left | rotate-right | flip-horizontal |
        /// flip-vertical | resize (to w × h). Answers {file, width, height},
        /// or "" on failure.
        #[qinvokable]
        fn editor_transform(self: &Engine, path: QString, op: QString, w: i32, h: i32) -> QString;

        /// Add image (ANN-12): copy `src` beside the editor's image at `base`;
        /// answers the new file's name, or "" on failure.
        #[qinvokable]
        fn editor_import_image(self: &Engine, base: QString, src: QString) -> QString;

        /// Add image from the clipboard: its PNG written beside `base`; the
        /// file's name, or "" when the clipboard holds no image.
        #[qinvokable]
        fn editor_paste_image(self: &Engine, base: QString) -> QString;

        /// The Background tool's presets and previous settings (BG-03), as
        /// JSON {previous, presets: [{name, bg}]}, sizes in points
        #[qinvokable]
        fn backgrounds(self: &Engine) -> QString;
        #[qinvokable]
        fn set_backgrounds(self: &Engine, json: QString);

        /// The user's own wallpapers for the Background tool (BG-01), copied
        /// into the data dir: list them (JSON array), add one (answers the
        /// copy's path, or "" if it isn't an image) or remove one
        #[qinvokable]
        fn custom_wallpapers(self: &Engine) -> QString;
        #[qinvokable]
        fn add_wallpaper(self: &Engine, path: QString) -> QString;
        #[qinvokable]
        fn remove_wallpaper(self: &Engine, path: QString);

        /// Wallpapers for the Background tool (JSON array of paths)
        #[qinvokable]
        fn wallpapers(self: &Engine) -> QString;

        /// Omarchy's emoji list for the Text tool's picker (ANN-05): a JSON
        /// array of {e: emoji, k: keywords}, or "[]" without Omarchy
        #[qinvokable]
        fn emojis(self: &Engine) -> QString;

        /// Save As to `path` (from the portal's save dialog)
        #[qinvokable]
        fn save_as(self: Pin<&mut Engine>, id: QString, path: QString);

        /// {name, folder} the Save As dialog starts with
        #[qinvokable]
        fn save_as_suggestion(self: &Engine, id: QString) -> QString;

        #[qinvokable]
        fn card_closed(self: Pin<&mut Engine>, id: QString);

        #[qinvokable]
        fn drag_file(self: Pin<&mut Engine>, id: QString) -> QString;

        #[qinvokable]
        fn show_in_files(self: Pin<&mut Engine>, id: QString);

        #[qinvokable]
        fn trash(self: Pin<&mut Engine>, id: QString);

        #[qinvokable]
        fn annotate(self: Pin<&mut Engine>, id: QString);
        /// Pin the capture to the screen (PIN-01)
        #[qinvokable]
        fn pin(self: Pin<&mut Engine>, id: QString);
        /// Resize one of Snip's own windows (by its title) through Hyprland,
        /// which keeps a floating window's size otherwise (a pin's zoom)
        #[qinvokable]
        fn resize_window(self: &Engine, title: QString, w: i32, h: i32);
        /// Move one of Snip's own windows (by its title) to (x, y), or by
        /// (x, y) when `relative`, through Hyprland (a pin's arrow keys)
        #[qinvokable]
        fn move_window(self: &Engine, title: QString, x: i32, y: i32, relative: bool);
        /// Pin one of Snip's own windows (the editor's "Always on top")
        #[qinvokable]
        fn pin_window(self: &Engine, title: QString);
        /// Where one of Snip's own windows is: JSON [x, y], or "null"
        #[qinvokable]
        fn window_position(self: &Engine, title: QString) -> QString;
        /// The output a window is on and the window's place in it: JSON
        /// {output, x, y} (logical px from the output's corner), or "null"
        #[qinvokable]
        fn window_place(self: &Engine, title: QString) -> QString;
        /// The pointer, in layout px: JSON [x, y], or "null"
        #[qinvokable]
        fn cursor_position(self: &Engine) -> QString;

        #[qinvokable]
        fn extract_text(self: Pin<&mut Engine>, id: QString);
        /// Open Snip's editor on a capture (ANN-19).
        #[qinvokable]
        fn edit(self: Pin<&mut Engine>, id: QString);

        /// Write an editor's render (handed over by `Platform.grabExact` under
        /// `token`) and its objects back to the capture (ANN-15), then do
        /// `action`: "" | "copy" | "save". Answers with editor_done.
        #[qinvokable]
        fn editor_commit(self: Pin<&mut Engine>, id: QString, token: i32, doc: QString, action: QString);
        /// A capture back in Quick Access, unless its card is there: after its
        /// editor closes, and history's Restore
        #[qinvokable]
        fn restore_card(self: Pin<&mut Engine>, id: QString);
        /// The history window's captures: the newest, or those matching
        /// `query` (JSON array of card JSON with title and app)
        #[qinvokable]
        fn history(self: &Engine, query: QString, limit: i32) -> QString;


        /// rotate-left | flip-horizontal
        #[qinvokable]
        fn transform_capture(self: Pin<&mut Engine>, id: QString, op: QString);

        #[qinvokable]
        fn copy_text(self: Pin<&mut Engine>, text: QString);

        #[qinvokable]
        fn restore_last(self: Pin<&mut Engine>);

        #[qinvokable]
        fn log(self: &Engine, ev: QString, extra_json: QString);

        /// Every installed Omarchy theme as tokens (JSON array): the gallery.
        #[qinvokable]
        fn themes(self: &Engine) -> QString;

        #[qsignal]
        fn frames_ready(self: Pin<&mut Engine>, epoch: i32);

        #[qsignal]
        fn cancel_requested(self: Pin<&mut Engine>);

        /// CAP-06: show a countdown on `output` around the logical area
        /// (x < 0: the whole output), then call countdown_done.
        #[qsignal]
        fn countdown(self: Pin<&mut Engine>, output: &QString, x: i32, y: i32, w: i32, h: i32, seconds: i32);

        #[qsignal]
        fn countdown_cancel(self: Pin<&mut Engine>);

        /// Take Snip's own cards off screen before capture `request`.
        #[qsignal]
        fn prepare_capture(self: Pin<&mut Engine>, request: i32);

        #[qsignal]
        fn capture_done(self: Pin<&mut Engine>);

        #[qsignal]
        fn card_add(self: Pin<&mut Engine>, card: &QString);

        #[qsignal]
        fn card_update(self: Pin<&mut Engine>, id: &QString, card: &QString);

        /// "close-all" | "hide" | "show"
        #[qsignal]
        fn cards_command(self: Pin<&mut Engine>, cmd: &QString);

        /// A short confirmation, on `output` (the monitor under the pointer)
        #[qsignal]
        fn toast(self: Pin<&mut Engine>, text: &QString, output: &QString);

        /// A result to show on a card: text-copied | qr-copied | no-text | error
        #[qsignal]
        fn card_status(self: Pin<&mut Engine>, id: &QString, status: &QString, detail: &QString);

        /// Render a new screenshot with the auto-applied background (BG-03):
        /// a card's JSON plus `bg` (sizes in points) and its output's `scale`;
        /// answered with editor_commit(id, token, doc, "auto-bg")
        #[qsignal]
        fn background_auto(self: Pin<&mut Engine>, capture: &QString);

        /// Open a pin (PIN-01): a card's JSON plus the scale of its output
        #[qsignal]
        fn pin_open(self: Pin<&mut Engine>, pin: &QString);
        /// Show the history window (HIS-01) on `output`
        #[qsignal]
        fn history_open(self: Pin<&mut Engine>, output: &QString);

        /// Every pin at once (PIN-07): toggle (hide or show) | close
        #[qsignal]
        fn pins_command(self: Pin<&mut Engine>, command: &QString);

        /// Open an editor window: {id, image, doc, width, height, title}
        #[qsignal]
        fn editor_open(self: Pin<&mut Engine>, editor: &QString);

        /// Word boxes for `path`: [{line, x, y, w, h}] in image px
        #[qsignal]
        fn editor_words(self: Pin<&mut Engine>, path: &QString, words: &QString);
        /// Add image > Take Screenshot finished for editor `token`: the new
        /// image's file, or "" when cancelled (ANN-12)
        #[qsignal]
        fn editor_screenshot(self: Pin<&mut Engine>, token: i32, path: &QString);
        /// The eyedropper picked `color` ("#rrggbb", or "" when cancelled)
        /// for editor `token` (ANN-13)
        #[qsignal]
        fn editor_color(self: Pin<&mut Engine>, token: i32, color: &QString);

        /// An editor_commit finished (ok) or failed (detail says why).
        #[qsignal]
        fn editor_done(self: Pin<&mut Engine>, id: &QString, action: &QString, ok: bool, detail: &QString);

        #[qsignal]
        fn debug_command(self: Pin<&mut Engine>, cmd: &QString);

        #[qsignal]
        fn quit_requested(self: Pin<&mut Engine>);
    }

    impl cxx_qt::Threading for Engine {}

    extern "Rust" {
        /// Size/format only (no reference taken).
        fn frame_info(output: &QString) -> FrameInfo;
        /// Like frame_info, plus an owned reference in `handle`.
        fn frame_acquire(output: &QString) -> FrameInfo;
        unsafe fn frame_release(handle: usize);
        fn mark_time(ev: &QString, extra_json: &QString);
        /// An editor render, straight RGBA8888 rows `stride` bytes apart,
        /// kept under `token` for editor_commit.
        fn edit_put(token: u32, data: &[u8], width: u32, height: u32, stride: u32);
        /// Pixelate (style 0), Blur (1) or Text shape (2) tiles for a region
        /// of the image at `path` (ANN-06, D26, D30).
        fn redact_tiles(path: &QString, x: i32, y: i32, w: i32, h: i32, style: u32, seed: u32, level: f32) -> RedactTiles;
        /// Another snipd already answers on ctl.sock.
        fn already_running() -> bool;
    }
}

use std::pin::Pin;
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use snip_core::geometry::Rect;
use snip_core::trace;

use crate::engine::Inner;
use crate::frames;

#[derive(Default)]
pub struct EngineRust {
    epoch: i32,
    theme: QString,
    settings: QString,
    session: QString,
    inner: Option<Arc<Inner>>,
}

impl qobject::Engine {
    fn inner(&self) -> Option<Arc<Inner>> {
        self.rust().inner.clone()
    }

    fn start(mut self: Pin<&mut Self>, mode: QString) {
        if self.rust().inner.is_some() {
            return;
        }
        trace::init();
        let interactive = mode.to_string() == "interactive";
        let inner = match Inner::new(self.qt_thread()) {
            Ok(i) => i,
            Err(e) => {
                trace::error("start", &format!("\"msg\":{}", trace::js(&e)));
                return;
            }
        };
        self.as_mut().rust_mut().inner = Some(inner.clone());

        let sink_inner = inner.clone();
        // Test instances use their own namespace (SNIP_APP_ID=snip-e2e) so
        // their shortcuts never reach the user's daemon, and vice versa.
        let app_id = app_id();
        let wl = snip_wayland::spawn(&app_id, Box::new(move |ev| sink_inner.on_wayland(ev)));
        if interactive {
            // Only the user's daemon owns the shortcuts (a test instance must
            // not react to their key presses). Bound in hypr/snip.lua as
            // hl.dsp.global("snip:<id>").
            for (id, d) in [
                ("area", "Capture area"),
                ("fullscreen", "Capture fullscreen"),
                ("window", "Capture window"),
                ("previous", "Capture previous area"),
                ("text", "Capture text (OCR)"),
                ("all-in-one", "All-In-One"),
                ("restore", "Restore the last closed capture"),
                ("timer-area", "Self-timer: area"),
                ("timer-fullscreen", "Self-timer: fullscreen"),
                ("close-all", "Close all Quick Access cards"),
                ("edit-clipboard", "Edit the image on the clipboard"),
                ("area-pin", "Capture area and pin it"),
                ("history", "Open capture history"),
                ("pins-toggle", "Hide or show all pins"),
                ("pins-close", "Close all pins"),
                ("pins-unlock", "Unlock all pins"),
            ] {
                wl.send(snip_wayland::Request::RegisterShortcut { id: id.into(), description: d.into() });
            }
        }
        inner.set_wayland(wl);

        let ctl = inner.clone();
        if let Err(e) = snip_ipc::serve(move |req| ctl.handle_ctl(req)) {
            trace::error("ctl_bind", &format!("\"msg\":{}", trace::js(&e.to_string())));
        }

        inner.push_settings();
        inner.push_theme();
        let w = inner.clone();
        crate::watch::spawn(move |c| match c {
            crate::watch::Change::Config => w.reload_config(),
            crate::watch::Change::Theme => w.push_theme(),
        });
        // Omarchy's theme switch and `hyprctl reload` drop runtime rules and
        // may change rounding: re-apply and re-resolve.
        let _ = snip_hypr::install_runtime_rules();
        let h = inner.clone();
        let _ = snip_hypr::subscribe(move |name, _| {
            if name == "configreloaded" {
                let _ = snip_hypr::install_runtime_rules();
                h.push_theme();
            }
            true
        });
        inner.start_indexer();
        let s = inner.clone();
        std::thread::Builder::new()
            .name("snip-sweep".into())
            .spawn(move || loop {
                s.sweep();
                // the week of daily use is the long-running soak (replaces
                // spike S5, which flashed overlays while idle)
                trace::warn("mem", &mem_snapshot());
                std::thread::sleep(std::time::Duration::from_secs(3600));
            })
            .ok();
    }

    fn capture(self: Pin<&mut Self>, mode: QString) {
        if let Some(i) = self.inner() {
            i.begin(&mode.to_string(), None, None, None, 0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_area(self: Pin<&mut Self>, output: QString, x: i32, y: i32, w: i32, h: i32, kind: QString, skip_preset: bool) {
        if let Some(i) = self.inner() {
            i.skip_preset(skip_preset);
            i.finish_area(output.to_string(), Rect::new(x, y, w, h), kind.to_string());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_window(self: Pin<&mut Self>, address: QString, output: QString, x: i32, y: i32, w: i32, h: i32, invert: bool) {
        if let Some(i) = self.inner() {
            i.skip_preset(invert); // Shift: also skips an auto-applied preset (D32)
            i.finish_window(&address.to_string(), output.to_string(), Rect::new(x, y, w, h), invert);
        }
    }

    fn cancel(self: Pin<&mut Self>) {
        if let Some(i) = self.inner() {
            i.cancel();
        }
    }

    fn capture_ready(self: Pin<&mut Self>, request: i32) {
        if let Some(i) = self.inner() {
            i.capture_ready(request as u64);
        }
    }

    fn countdown_done(&self) {
        if let Some(i) = self.inner() {
            i.countdown_done();
        }
    }

    fn refocus_pointer(&self) {
        trace::trace("refocus_pointer", "");
        if let Some(i) = self.inner() {
            i.nudge_pointer();
        }
    }

    fn copy(self: Pin<&mut Self>, id: QString) -> bool {
        self.inner().map(|i| i.copy(&id.to_string()).is_ok()).unwrap_or(false)
    }

    fn save(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            let id = id.to_string();
            // fsync and rename off the GUI thread; the card updates by signal
            std::thread::spawn(move || {
                if let Err(e) = i.save(&id) {
                    trace::warn("save_failed", &format!("\"msg\":{}", trace::js(&e)));
                }
            });
        }
    }

    fn save_as(self: Pin<&mut Self>, id: QString, path: QString) {
        if let Some(i) = self.inner() {
            let (id, path) = (id.to_string(), path.to_string());
            std::thread::spawn(move || {
                if let Err(e) = i.save_as(&id, std::path::Path::new(&path)) {
                    trace::warn("save_failed", &format!("\"msg\":{}", trace::js(&e)));
                    i.toast(format!("Couldn't save: {e}"));
                }
            });
        }
    }

    fn editor_prefs(&self) -> QString {
        let j = std::fs::read_to_string(snip_core::paths::data_dir().join("editor.json")).unwrap_or_else(|_| "{}".into());
        QString::from(j.as_str())
    }

    fn set_editor_prefs(&self, json: QString) {
        let dir = snip_core::paths::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = snip_core::store::atomic_write(&dir.join("editor.json"), json.to_string().as_bytes());
    }

    fn saved_colors(&self) -> QString {
        let j = std::fs::read_to_string(snip_core::paths::data_dir().join("colors.json")).unwrap_or_else(|_| "[]".into());
        QString::from(j.as_str())
    }

    fn set_saved_colors(&self, json: QString) {
        let dir = snip_core::paths::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = snip_core::store::atomic_write(&dir.join("colors.json"), json.to_string().as_bytes());
    }

    fn backgrounds(&self) -> QString {
        let j = std::fs::read_to_string(snip_core::paths::data_dir().join("backgrounds.json")).unwrap_or_else(|_| "{}".into());
        QString::from(j.as_str())
    }

    fn set_backgrounds(&self, json: QString) {
        let dir = snip_core::paths::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = snip_core::store::atomic_write(&dir.join("backgrounds.json"), json.to_string().as_bytes());
    }

    fn custom_wallpapers(&self) -> QString {
        let mut v: Vec<String> = std::fs::read_dir(snip_core::paths::data_dir().join("wallpapers"))
            .map(|d| d.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect())
            .unwrap_or_default();
        v.sort();
        QString::from(serde_json::to_string(&v).unwrap_or_else(|_| "[]".into()).as_str())
    }

    fn add_wallpaper(&self, path: QString) -> QString {
        let src = std::path::PathBuf::from(path.to_string());
        let image = matches!(src.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg" | "webp"));
        let dir = snip_core::paths::data_dir().join("wallpapers");
        let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let dst = dir.join(format!("{ms}-{name}"));
        if !image || std::fs::create_dir_all(&dir).is_err() || std::fs::copy(&src, &dst).is_err() {
            return QString::default();
        }
        QString::from(dst.to_string_lossy().as_ref())
    }

    fn remove_wallpaper(&self, path: QString) {
        let p = std::path::PathBuf::from(path.to_string());
        if p.parent() == Some(snip_core::paths::data_dir().join("wallpapers").as_path()) {
            let _ = std::fs::remove_file(p);
        }
    }

    fn editor_transform(&self, path: QString, op: QString, w: i32, h: i32) -> QString {
        use snip_core::encode;
        let path = std::path::PathBuf::from(path.to_string());
        let Some(img) = crate::edits::decoded(&path.to_string_lossy()) else { return QString::default() };
        let (d, bpp, iw, ih) = (&img.data, img.bpp, img.width, img.height);
        let (data, nw, nh) = match op.to_string().as_str() {
            "rotate-left" => encode::rotate_left(d, iw, ih, bpp),
            "rotate-right" => encode::rotate_right(d, iw, ih, bpp),
            "flip-horizontal" => (encode::flip_horizontal(d, iw, ih, bpp), iw, ih),
            "flip-vertical" => (encode::flip_vertical(d, iw, bpp), iw, ih),
            "resize" if w > 0 && h > 0 => (encode::resize(d, iw, ih, bpp, w as u32, h as u32), w as u32, h as u32),
            _ => return QString::default(),
        };
        let file = format!("base-{}.png", chrono::Local::now().timestamp_millis());
        let out = path.with_file_name(&file);
        if snip_core::store::atomic_write(&out, &encode::encode_png(&data, nw, nh, bpp == 4)).is_err() {
            return QString::default();
        }
        trace::trace("editor_transformed", &format!("\"op\":{},\"w\":{nw},\"h\":{nh}", trace::js(&op.to_string())));
        QString::from(serde_json::json!({ "file": file, "width": nw, "height": nh }).to_string().as_str())
    }

    fn editor_import_image(&self, base: QString, src: QString) -> QString {
        let (base, src) = (std::path::PathBuf::from(base.to_string()), std::path::PathBuf::from(src.to_string()));
        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
        let file = format!("img-{}.{ext}", chrono::Local::now().timestamp_millis());
        match std::fs::copy(&src, base.with_file_name(&file)) {
            Ok(_) => QString::from(file.as_str()),
            Err(_) => QString::default(),
        }
    }

    fn editor_paste_image(&self, base: QString) -> QString {
        let Some(png) = self.inner().and_then(|i| i.clipboard_png()) else { return QString::default() };
        let file = format!("img-{}.png", chrono::Local::now().timestamp_millis());
        match snip_core::store::atomic_write(&std::path::PathBuf::from(base.to_string()).with_file_name(&file), &png) {
            Ok(()) => QString::from(file.as_str()),
            Err(_) => QString::default(),
        }
    }

    fn wallpapers(&self) -> QString {
        QString::from(serde_json::to_string(&snip_core::theme::wallpapers(24)).unwrap_or_else(|_| "[]".into()).as_str())
    }

    fn emojis(&self) -> QString {
        let root = std::env::var("OMARCHY_PATH").unwrap_or_else(|_| "/usr/share/omarchy".into());
        let j = std::fs::read_to_string(format!("{root}/shell/plugins/emojis/emojis.json")).unwrap_or_else(|_| "[]".into());
        QString::from(j.as_str())
    }

    fn edge_color(&self, path: QString) -> QString {
        let c = crate::edits::decoded(&path.to_string()).map(|img| {
            let (w, h) = (img.width as usize, img.height as usize);
            let px = |x: usize, y: usize| {
                let i = (y * w + x) * img.bpp;
                [img.data[i], img.data[i + 1], img.data[i + 2]]
            };
            let mut bins: std::collections::HashMap<[u8; 3], usize> = Default::default();
            let edge = (0..w).flat_map(|x| [(x, 0), (x, h - 1)]).chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]));
            for (x, y) in edge {
                *bins.entry(px(x, y)).or_default() += 1;
            }
            let c = bins.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c).unwrap_or([255; 3]);
            format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
        });
        QString::from(c.unwrap_or_else(|| "#ffffff".into()).as_str())
    }

    fn content_margins(&self, path: QString, x: i32, y: i32, w: i32, h: i32) -> QString {
        let j = crate::edits::decoded(&path.to_string()).map(|img| {
            let (x0, y0) = (x.max(0), y.max(0));
            let rect = (x0 as u32, y0 as u32, (x + w - x0).max(0) as u32, (y + h - y0).max(0) as u32);
            let ([l, t, r, b], c) = snip_core::balance::margins(&img.data, img.width, img.height, img.bpp, rect);
            format!(r##"{{"l":{l},"t":{t},"r":{r},"b":{b},"color":"#{:02x}{:02x}{:02x}"}}"##, c[0], c[1], c[2])
        });
        QString::from(j.unwrap_or_else(|| "null".into()).as_str())
    }

    fn request_words(&self, path: QString) {
        if let Some(i) = self.inner() {
            i.text_words(path.to_string());
        }
    }

    fn take_screenshot_for_editor(&self, token: i32) {
        if let Some(i) = self.inner() {
            i.editor_screenshot(token);
        }
    }

    fn pick_color_for_editor(&self, token: i32) {
        if let Some(i) = self.inner() {
            i.editor_pick_color(token);
        }
    }

    fn finish_pick(&self, color: QString) {
        if let Some(i) = self.inner() {
            i.finish_pick(color.to_string());
        }
    }

    fn save_as_suggestion(&self, id: QString) -> QString {
        let j = self.inner().map(|i| i.save_as_suggestion(&id.to_string()).to_string()).unwrap_or_default();
        QString::from(j.as_str())
    }

    fn card_closed(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.card_closed(&id.to_string());
        }
    }

    fn drag_file(self: Pin<&mut Self>, id: QString) -> QString {
        let p = self.inner().and_then(|i| i.drag_file(&id.to_string()).ok()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        QString::from(p.as_str())
    }

    fn show_in_files(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.show_in_files(&id.to_string());
        }
    }

    fn trash(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.trash(&id.to_string());
        }
    }

    fn pin(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            let _ = i.pin(&id.to_string());
        }
    }

    fn resize_window(&self, title: QString, w: i32, h: i32) {
        let (class, title) = (app_id(), title.to_string());
        // off the GUI thread: it talks to Hyprland's socket
        std::thread::spawn(move || {
            let _ = snip_hypr::resize_window(&class, &title, w, h);
        });
    }

    fn move_window(&self, title: QString, x: i32, y: i32, relative: bool) {
        let (class, title) = (app_id(), title.to_string());
        std::thread::spawn(move || {
            let _ = snip_hypr::move_window(&class, &title, x, y, relative);
        });
    }

    fn pin_window(&self, title: QString) {
        let (class, title) = (app_id(), title.to_string());
        std::thread::spawn(move || {
            let _ = snip_hypr::pin_window(&class, &title);
        });
    }

    fn window_position(&self, title: QString) -> QString {
        let p = snip_hypr::window(&app_id(), &title.to_string()).map(|c| format!("[{},{}]", c.x, c.y)).unwrap_or_else(|_| "null".into());
        QString::from(p.as_str())
    }

    fn window_place(&self, title: QString) -> QString {
        let p = snip_hypr::window(&app_id(), &title.to_string()).ok().and_then(|c| {
            let m = snip_hypr::monitors().into_iter().find(|m| c.x >= m.x && c.y >= m.y && c.x < m.x + m.width && c.y < m.y + m.height)?;
            Some(serde_json::json!({ "output": m.name, "x": c.x - m.x, "y": c.y - m.y }).to_string())
        });
        QString::from(p.unwrap_or_else(|| "null".into()).as_str())
    }

    fn cursor_position(&self) -> QString {
        let p = snip_hypr::cursor_pos().map(|(x, y)| format!("[{x},{y}]")).unwrap_or_else(|| "null".into());
        QString::from(p.as_str())
    }

    fn annotate(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.annotate(&id.to_string());
        }
    }

    fn edit(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.edit(&id.to_string());
        }
    }

    fn restore_card(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.restore_card(&id.to_string());
        }
    }

    fn history(&self, query: QString, limit: i32) -> QString {
        let j = self.inner().map(|i| i.history(&query.to_string(), limit.max(0) as u32).to_string()).unwrap_or_else(|| "[]".into());
        QString::from(j.as_str())
    }

    fn editor_commit(self: Pin<&mut Self>, id: QString, token: i32, doc: QString, action: QString) {
        if let Some(i) = self.inner() {
            i.editor_commit(&id.to_string(), token as u32, doc.to_string(), action.to_string());
        }
    }

    fn extract_text(self: Pin<&mut Self>, id: QString) {
        if let Some(i) = self.inner() {
            i.extract_text(&id.to_string());
        }
    }

    fn transform_capture(self: Pin<&mut Self>, id: QString, op: QString) {
        if let Some(i) = self.inner() {
            i.transform(&id.to_string(), &op.to_string());
        }
    }

    fn copy_text(self: Pin<&mut Self>, text: QString) {
        if let Some(i) = self.inner() {
            i.copy_text(&text.to_string());
        }
    }

    fn restore_last(self: Pin<&mut Self>) {
        if let Some(i) = self.inner() {
            let _ = i.restore_last();
        }
    }

    fn log(&self, ev: QString, extra_json: QString) {
        trace::trace(&ev.to_string(), &extra_json.to_string());
    }

    fn themes(&self) -> QString {
        QString::from(snip_core::theme::themes_json().as_str())
    }
}

fn info(output: &QString, acquire: bool) -> qobject::FrameInfo {
    match frames::get(&output.to_string()) {
        Some(f) => qobject::FrameInfo {
            valid: true,
            ptr: f.buf.ptr() as usize,
            handle: if acquire { frames::acquire(&f.buf) } else { 0 },
            width: f.buf.width,
            height: f.buf.height,
            stride: f.buf.stride,
            format: f.buf.format.wl_shm(),
            epoch: f.epoch as u32,
            transform: f.transform.to_wl(),
        },
        None => qobject::FrameInfo::default(),
    }
}

fn frame_info(output: &QString) -> qobject::FrameInfo {
    info(output, false)
}

fn frame_acquire(output: &QString) -> qobject::FrameInfo {
    info(output, true)
}

unsafe fn frame_release(handle: usize) {
    frames::release(handle);
}

fn edit_put(token: u32, data: &[u8], width: u32, height: u32, stride: u32) {
    crate::edits::put(token, data, width, height, stride);
}

/// Our Wayland app id, the windows' class: test instances set their own.
fn app_id() -> String {
    std::env::var("SNIP_APP_ID").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "snip".into())
}

#[allow(clippy::too_many_arguments)]
fn redact_tiles(path: &QString, x: i32, y: i32, w: i32, h: i32, style: u32, seed: u32, level: f32) -> qobject::RedactTiles {
    let style = match style {
        1 => snip_core::redact::Style::Blur,
        2 => snip_core::redact::Style::TextShape,
        3 => snip_core::redact::Style::NaturalBlur,
        _ => snip_core::redact::Style::Pixelate,
    };
    match crate::edits::decoded(&path.to_string()) {
        Some(img) => {
            let (rgba, width, height) = snip_core::redact::tiles(&img.data, img.width, img.height, img.bpp, (x as i64, y as i64, w as i64, h as i64), style, seed, level);
            qobject::RedactTiles { valid: true, width, height, rgba }
        }
        None => qobject::RedactTiles::default(),
    }
}

fn mark_time(ev: &QString, extra_json: &QString) {
    trace::trace(&ev.to_string(), &extra_json.to_string());
}

/// Memory, fds and threads of this process, as a JSON fragment.
fn mem_snapshot() -> String {
    let mut out = Vec::new();
    if let Ok(t) = std::fs::read_to_string("/proc/self/smaps_rollup") {
        for l in t.lines() {
            let mut it = l.split_whitespace();
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                let k = k.trim_end_matches(':');
                if matches!(k, "Rss" | "Pss" | "Pss_Anon" | "Pss_Shmem") {
                    out.push(format!("\"{}_mb\":{}", k.to_lowercase(), v.parse::<u64>().unwrap_or(0) / 1024));
                }
            }
        }
    }
    let n = |d: &str| std::fs::read_dir(d).map(|d| d.count()).unwrap_or(0);
    out.push(format!("\"fds\":{},\"threads\":{}", n("/proc/self/fd"), n("/proc/self/task")));
    out.join(",")
}

fn already_running() -> bool {
    snip_ipc::call(&snip_ipc::Request::Ping, std::time::Duration::from_millis(300)).map(|r| r.ok).unwrap_or(false)
}
