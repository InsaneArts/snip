//! cxx-qt bridge: the SnipCore QML singleton (Rust-owned state, Qt-thread
//! signals) plus plain Rust functions the C++ render code calls directly.

#[cxx_qt::bridge(namespace = "snip")]
pub mod qobject {
    #[namespace = ""]
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    /// Everything the C++ FrozenFrame item needs to wrap a captured frame
    /// in a QImage without copying.
    #[derive(Default)]
    struct FrameInfo {
        valid: bool,
        ptr: usize,
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
        #[qproperty(QString, mode)]
        type SnipCore = super::SnipCoreRust;

        /// Start the Wayland thread. `mode`: interactive | bench | selftest.
        #[qinvokable]
        fn start(self: Pin<&mut SnipCore>, mode: QString);

        /// Capture without a hotkey (selftest / manual).
        #[qinvokable]
        fn trigger(self: Pin<&mut SnipCore>);

        /// Re-capture while the overlay is up and diff against the frozen frame.
        #[qinvokable]
        fn verify(self: Pin<&mut SnipCore>);

        /// Crop (physical px), encode PNG, offer on the clipboard.
        #[qinvokable]
        fn copy_region(self: Pin<&mut SnipCore>, output: QString, x: i32, y: i32, w: i32, h: i32);

        #[qinvokable]
        fn set_overlay_visible(self: Pin<&mut SnipCore>, visible: bool);

        #[qinvokable]
        fn log(self: &SnipCore, ev: QString, extra_json: QString);

        #[qsignal]
        fn frames_ready(self: Pin<&mut SnipCore>, epoch: i32);

        #[qsignal]
        fn verify_done(self: Pin<&mut SnipCore>, report: QString);

        #[qsignal]
        fn cancel_requested(self: Pin<&mut SnipCore>);
    }

    impl cxx_qt::Threading for SnipCore {}

    extern "Rust" {
        fn frame_info(output: &QString) -> FrameInfo;
        fn mark_time(ev: &QString, extra_json: &QString);
    }
}

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;

use crate::clock::{mark, now_ns};
use crate::frames;
use crate::wl::{self, Cmd, Purpose, UiEvent};

#[derive(Default)]
pub struct SnipCoreRust {
    epoch: i32,
    mode: QString,
    tx: Option<calloop::channel::Sender<Cmd>>,
    overlay_visible: Arc<AtomicBool>,
}

impl qobject::SnipCore {
    fn start(mut self: Pin<&mut Self>, mode: QString) {
        if self.rust().tx.is_some() {
            return;
        }
        self.as_mut().set_mode(mode);
        let (tx, rx) = calloop::channel::channel::<Cmd>();
        let thread = self.qt_thread();
        let ui: wl::UiSink = Box::new(move |ev| {
            let _ = thread.queue(move |mut obj| match ev {
                UiEvent::FramesReady { epoch } => {
                    mark("qt_frames_ready", &format!("\"epoch\":{epoch}"));
                    obj.as_mut().set_epoch(epoch as i32);
                    obj.as_mut().frames_ready(epoch as i32);
                }
                UiEvent::VerifyDone { report } => {
                    obj.as_mut().verify_done(QString::from(report.as_str()));
                }
                UiEvent::CancelRequested => obj.as_mut().cancel_requested(),
            });
        });
        let visible = self.rust().overlay_visible.clone();
        let register = self.mode().to_string() != "soak";
        std::thread::Builder::new()
            .name("snip-wayland".into())
            .spawn({
                let self_tx = tx.clone();
                move || wl::run(rx, self_tx, visible, ui, register)
            })
            .expect("spawn wayland thread");
        if self.mode().to_string() == "soak" {
            let _ = tx.send(Cmd::StartSoak { idle_ms: 120_000, interval_ms: 30_000 });
            std::thread::Builder::new()
                .name("snip-memlog".into())
                .spawn(|| loop {
                    mark("mem", &mem_snapshot());
                    std::thread::sleep(std::time::Duration::from_secs(60));
                })
                .expect("spawn memlog");
        }
        self.as_mut().rust_mut().tx = Some(tx);
    }

    fn trigger(self: Pin<&mut Self>) {
        if let Some(tx) = &self.rust().tx {
            let _ = tx.send(Cmd::Capture(Purpose::Freeze));
        }
    }

    fn verify(self: Pin<&mut Self>) {
        if let Some(tx) = &self.rust().tx {
            let _ = tx.send(Cmd::Capture(Purpose::Verify));
        }
    }

    fn set_overlay_visible(self: Pin<&mut Self>, visible: bool) {
        self.rust().overlay_visible.store(visible, Ordering::Relaxed);
    }

    fn log(&self, ev: QString, extra_json: QString) {
        mark(&ev.to_string(), &extra_json.to_string());
    }

    fn copy_region(self: Pin<&mut Self>, output: QString, x: i32, y: i32, w: i32, h: i32) {
        let Some(tx) = self.rust().tx.clone() else { return };
        let output = output.to_string();
        // Off the GUI thread: crop + convert + encode.
        std::thread::spawn(move || {
            let t0 = now_ns();
            let Some(slot) = frames::get(&output) else {
                mark("copy_error", "\"msg\":\"no frame\"");
                return;
            };
            let e = crate::encode::crop_png(&slot.buf, x, y, w, h);
            mark(
                "copy_encoded",
                &format!(
                    "\"w\":{},\"h\":{},\"crop_ms\":{:.2},\"encode_ms\":{:.2},\"png_bytes\":{}",
                    e.w, e.h, e.crop_ms, e.encode_ms, e.png.len()
                ),
            );
            let png_bytes = e.png;
            let _ = tx.send(Cmd::SetClipboard { png: Arc::new(png_bytes), t0 });
        });
    }
}

fn frame_info(output: &QString) -> qobject::FrameInfo {
    match frames::get(&output.to_string()) {
        Some(s) => qobject::FrameInfo {
            valid: true,
            ptr: s.buf.ptr() as usize,
            width: s.buf.width,
            height: s.buf.height,
            stride: s.buf.stride,
            format: s.buf.format,
            epoch: s.epoch,
            transform: s.transform,
        },
        None => qobject::FrameInfo::default(),
    }
}

fn mark_time(ev: &QString, extra_json: &QString) {
    mark(&ev.to_string(), &extra_json.to_string());
}

fn mem_snapshot() -> String {
    let mut out = Vec::new();
    if let Ok(t) = std::fs::read_to_string("/proc/self/smaps_rollup") {
        for l in t.lines() {
            let mut it = l.split_whitespace();
            if let (Some(k), Some(v)) = (it.next(), it.next()) {
                let k = k.trim_end_matches(':');
                if matches!(k, "Rss" | "Pss" | "Pss_Anon" | "Pss_Shmem" | "Anonymous") {
                    out.push(format!("\"{}_mb\":{}", k, v.parse::<u64>().unwrap_or(0) / 1024));
                }
            }
        }
    }
    let fds = std::fs::read_dir("/proc/self/fd").map(|d| d.count()).unwrap_or(0);
    let threads = std::fs::read_dir("/proc/self/task").map(|d| d.count()).unwrap_or(0);
    out.push(format!("\"fds\":{fds},\"threads\":{threads}"));
    out.join(",")
}
