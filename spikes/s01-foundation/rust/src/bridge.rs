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

        /// Area capture result -> stored file + QAO card (lifecycle §5.0, no clipboard).
        #[qinvokable]
        fn capture_region_to_card(self: Pin<&mut SnipCore>, output: QString, x: i32, y: i32, w: i32, h: i32);

        /// Card "Copy": put the stored PNG on the clipboard.
        #[qinvokable]
        fn copy_file(self: Pin<&mut SnipCore>, path: QString);

        #[qinvokable]
        fn log(self: &SnipCore, ev: QString, extra_json: QString);

        /// S15: the wallpaper omarchy-shell is drawing (resolved symlink).
        #[qinvokable]
        fn wallpaper_path(self: &SnipCore) -> QString;

        /// S17: every installed Omarchy theme resolved into Snip tokens (JSON).
        #[qinvokable]
        fn themes(self: &SnipCore) -> QString;

        #[qsignal]
        fn frames_ready(self: Pin<&mut SnipCore>, epoch: i32);

        #[qsignal]
        fn verify_done(self: Pin<&mut SnipCore>, report: QString);

        #[qsignal]
        fn cancel_requested(self: Pin<&mut SnipCore>);

        #[qsignal]
        fn card_show(self: Pin<&mut SnipCore>, path: QString);

        #[qsignal]
        fn card_variant(self: Pin<&mut SnipCore>, variant: QString);

        #[qsignal]
        fn pin_command(self: Pin<&mut SnipCore>, cmd: QString, arg: QString);
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
                UiEvent::CardShow { path } => {
                    mark("qt_card_show", &format!("\"path\":\"{path}\""));
                    obj.as_mut().card_show(QString::from(path.as_str()));
                }
            });
        });
        let visible = self.rust().overlay_visible.clone();
        // Only the interactive instance owns the global shortcuts; test instances
        // running next to it must not also react to the user's key presses.
        let register = self.mode().to_string() == "interactive";
        std::thread::Builder::new()
            .name("snip-wayland".into())
            .spawn({
                let self_tx = tx.clone();
                move || wl::run(rx, self_tx, visible, ui, register)
            })
            .expect("spawn wayland thread");
        let mode = self.mode().to_string();
        if mode == "interactive" || mode == "test" || mode == "editor" {
            spawn_control_socket(tx.clone(), self.qt_thread());
        }
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

    fn capture_region_to_card(self: Pin<&mut Self>, output: QString, x: i32, y: i32, w: i32, h: i32) {
        let Some(tx) = self.rust().tx.clone() else { return };
        let output = output.to_string();
        std::thread::spawn(move || {
            let Some(slot) = frames::get(&output) else { return };
            let e = crate::encode::crop_png(&slot.buf, x, y, w, h);
            let path = crate::encode::store(&e.png, "area");
            mark(
                "area_stored",
                &format!("\"w\":{},\"h\":{},\"crop_ms\":{:.2},\"encode_ms\":{:.2},\"path\":\"{path}\"", e.w, e.h, e.crop_ms, e.encode_ms),
            );
            let _ = tx.send(Cmd::Ui(UiEvent::CardShow { path }));
        });
    }

    fn copy_file(self: Pin<&mut Self>, path: QString) {
        let Some(tx) = self.rust().tx.clone() else { return };
        let t0 = now_ns();
        match std::fs::read(path.to_string()) {
            Ok(bytes) => {
                let _ = tx.send(Cmd::SetClipboard { png: Arc::new(bytes), t0, sensitive: false });
            }
            Err(e) => mark("copy_file_error", &format!("\"msg\":\"{e}\"")),
        }
    }

    fn wallpaper_path(&self) -> QString {
        let link = format!("{}/.local/state/omarchy/current/background", std::env::var("HOME").unwrap_or_default());
        let p = std::fs::canonicalize(link).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        QString::from(p.as_str())
    }

    fn themes(&self) -> QString {
        let t0 = now_ns();
        let json = crate::theme::themes_json();
        mark("themes_resolved", &format!("\"ms\":{:.1}", (now_ns() - t0) as f64 / 1e6));
        QString::from(json.as_str())
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
            let _ = tx.send(Cmd::SetClipboard { png: Arc::new(png_bytes), t0, sensitive: false });
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

/// Line-based control socket ($XDG_RUNTIME_DIR/snip-spike.sock): a prototype of
/// the product's ctl.sock, used by the test driver.
fn spawn_control_socket(
    tx: calloop::channel::Sender<Cmd>,
    thread: cxx_qt::CxxQtThread<qobject::SnipCore>,
) {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    let path = format!("{}/snip-spike.sock", std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into()));
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        mark("ctl_bind_failed", "");
        return;
    };
    std::thread::Builder::new()
        .name("snip-ctl".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut out = stream.try_clone().expect("clone");
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let p: Vec<&str> = line.split_whitespace().collect();
                    let reply = match p.as_slice() {
                        ["ping"] => "pong".to_string(),
                        ["move", out_name, x, y, w, h] => {
                            let n = |v: &str| v.parse::<u32>().unwrap_or(0);
                            let _ = tx.send(Cmd::PointerMove { output: out_name.to_string(), x: n(x), y: n(y), w: n(w), h: n(h) });
                            "ok".into()
                        }
                        ["down"] => { let _ = tx.send(Cmd::PointerButton { down: true }); "ok".into() }
                        ["up"] => { let _ = tx.send(Cmd::PointerButton { down: false }); "ok".into() }
                        ["scroll", dy] => { let _ = tx.send(Cmd::PointerAxis { dy: dy.parse().unwrap_or(0.0) }); "ok".into() }
                        ["card", path] => {
                            let path = path.to_string();
                            let _ = thread.queue(move |mut o| o.as_mut().card_show(QString::from(path.as_str())));
                            "ok".into()
                        }
                        ["variant", v] => {
                            let v = v.to_string();
                            let _ = thread.queue(move |mut o| o.as_mut().card_variant(QString::from(v.as_str())));
                            "ok".into()
                        }
                        ["pin", path] => {
                            let path = path.to_string();
                            let _ = thread.queue(move |mut o| o.as_mut().pin_command(QString::from("show"), QString::from(path.as_str())));
                            "ok".into()
                        }
                        ["grab", out_name, x, y, w, h, path] => {
                            let n = |v: &str| v.parse::<i32>().unwrap_or(0);
                            let (rtx, rrx) = std::sync::mpsc::channel();
                            let _ = tx.send(Cmd::Grab { output: out_name.to_string(), rect: (n(x), n(y), n(w), n(h)), path: path.to_string(), reply: rtx });
                            rrx.recv_timeout(std::time::Duration::from_secs(3)).unwrap_or_else(|_| "err timeout".into())
                        }
                        ["trigger-area"] => { let _ = tx.send(Cmd::Capture(Purpose::Freeze)); "ok".into() }
                        ["export", path] => {
                            let path = path.to_string();
                            let _ = thread.queue(move |mut o| o.as_mut().pin_command(QString::from("export"), QString::from(path.as_str())));
                            "ok".into()
                        }
                        ["pinlock", "on", x, y] => {
                            let arg = format!("{x} {y}");
                            let _ = thread.queue(move |mut o| o.as_mut().pin_command(QString::from("pinlock-layer"), QString::from(arg.as_str())));
                            "ok".into()
                        }
                        ["pinlock", v] | ["unpin", v] => {
                            let (c, v) = (p[0].to_string(), v.to_string());
                            let _ = thread.queue(move |mut o| o.as_mut().pin_command(QString::from(c.as_str()), QString::from(v.as_str())));
                            "ok".into()
                        }
                        ["copyfile", path] | ["copyfile-sensitive", path] => match std::fs::read(path) {
                            Ok(b) => {
                                let sensitive = p[0] == "copyfile-sensitive";
                                let _ = tx.send(Cmd::SetClipboard { png: Arc::new(b), t0: now_ns(), sensitive });
                                "ok".into()
                            }
                            Err(e) => format!("err {e}"),
                        },
                        _ => "err unknown".into(),
                    };
                    let _ = writeln!(out, "{reply}");
                }
            }
        })
        .expect("spawn ctl");
}
