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
        fn finish_area(self: Pin<&mut Engine>, output: QString, x: i32, y: i32, w: i32, h: i32, kind: QString);

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

        /// "My Colors" (ANN-13): up to ten, kept across sessions (JSON array)
        #[qinvokable]
        fn saved_colors(self: &Engine) -> QString;
        #[qinvokable]
        fn set_saved_colors(self: &Engine, json: QString);

        /// The colour around the image's edge (the most common one): what an
        /// outward crop fills the new canvas with (ANN-11)
        #[qinvokable]
        fn edge_color(self: &Engine, path: QString) -> QString;

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

        /// Wallpapers for the Background tool (JSON array of paths)
        #[qinvokable]
        fn wallpapers(self: &Engine) -> QString;

        /// The colour of pixel (x, y) of the image at `path`: "#rrggbb"
        #[qinvokable]
        fn pixel_color(self: &Engine, path: QString, x: i32, y: i32) -> QString;

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

        /// Open an editor window: {id, image, doc, width, height, title}
        #[qsignal]
        fn editor_open(self: Pin<&mut Engine>, editor: &QString);

        /// Word boxes for `path`: [{line, x, y, w, h}] in image px
        #[qsignal]
        fn editor_words(self: Pin<&mut Engine>, path: &QString, words: &QString);

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
        let app_id = std::env::var("SNIP_APP_ID").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| "snip".into());
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

    fn finish_area(self: Pin<&mut Self>, output: QString, x: i32, y: i32, w: i32, h: i32, kind: QString) {
        if let Some(i) = self.inner() {
            i.finish_area(output.to_string(), Rect::new(x, y, w, h), kind.to_string());
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_window(self: Pin<&mut Self>, address: QString, output: QString, x: i32, y: i32, w: i32, h: i32, invert: bool) {
        if let Some(i) = self.inner() {
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

    fn saved_colors(&self) -> QString {
        let j = std::fs::read_to_string(snip_core::paths::data_dir().join("colors.json")).unwrap_or_else(|_| "[]".into());
        QString::from(j.as_str())
    }

    fn set_saved_colors(&self, json: QString) {
        let dir = snip_core::paths::data_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = snip_core::store::atomic_write(&dir.join("colors.json"), json.to_string().as_bytes());
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
            "flip-vertical" => (encode::flip_vertical(d, iw, ih, bpp), iw, ih),
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

    fn pixel_color(&self, path: QString, x: i32, y: i32) -> QString {
        let c = crate::edits::decoded(&path.to_string()).and_then(|img| {
            if x < 0 || y < 0 || x as u32 >= img.width || y as u32 >= img.height {
                return None;
            }
            let i = (y as usize * img.width as usize + x as usize) * img.bpp;
            Some(format!("#{:02x}{:02x}{:02x}", img.data[i], img.data[i + 1], img.data[i + 2]))
        });
        QString::from(c.unwrap_or_default().as_str())
    }

    fn request_words(&self, path: QString) {
        if let Some(i) = self.inner() {
            i.text_words(path.to_string());
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
