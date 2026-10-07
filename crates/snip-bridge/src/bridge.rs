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
