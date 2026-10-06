//! "Connection A": the Rust core's own Wayland connection, on its own thread.
//! Capture (ext-image-copy-capture), global shortcuts (hyprland), clipboard
//! (ext-data-control). No UI surfaces live here.

use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use calloop::channel::{self, Channel};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, RegistrationToken};
use calloop_wayland_source::WaylandSource;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer::WlBuffer, wl_output, wl_output::WlOutput, wl_registry::WlRegistry, wl_seat::WlSeat,
    wl_shm, wl_shm::WlShm, wl_shm_pool::WlShmPool,
};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};
use wayland_protocols_hyprland::global_shortcuts::v1::client::{
    hyprland_global_shortcut_v1::{self, HyprlandGlobalShortcutV1},
    hyprland_global_shortcuts_manager_v1::HyprlandGlobalShortcutsManagerV1,
};

use crate::clock::{mark, now_ns};
use crate::frames::{self, FrameSlot, ShmBuf};

pub const APP_ID: &str = "snipspike";
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(150);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Purpose {
    Freeze,
    Verify,
    /// SUPER+SHIFT+3: focused monitor, no overlay.
    Fullscreen,
}

#[derive(Clone, Copy, Debug)]
pub enum ShortcutKind {
    Area,
    Fullscreen,
}

pub enum Cmd {
    Capture(Purpose),
    SetClipboard { png: Arc<Vec<u8>>, t0: u64 },
    /// S5 soak: while the user is idle for `idle_ms`, capture every `interval_ms`.
    StartSoak { idle_ms: u32, interval_ms: u64 },
}

pub enum UiEvent {
    FramesReady { epoch: u32 },
    VerifyDone { report: String },
    CancelRequested,
}

pub type UiSink = Box<dyn Fn(UiEvent) + Send>;

struct Output {
    wl: WlOutput,
    name: String,
}

struct Cap {
    out: usize,
    source: ExtImageCaptureSourceV1,
    session: ExtImageCopyCaptureSessionV1,
    frame: Option<ExtImageCopyCaptureFrameV1>,
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
    transform: u32,
    finished: bool,
    ok: bool,
    t_ready: u64,
}

struct Capture {
    purpose: Purpose,
    caps: Vec<Cap>,
    t_start: u64,
    t_hotkey: Option<u64>,
    timer: Option<RegistrationToken>,
}

struct Slot {
    buf: Arc<ShmBuf>,
    _pool: WlShmPool,
    wl_buffer: WlBuffer,
}

struct State {
    qh: QueueHandle<State>,
    lh: LoopHandle<'static, State>,
    conn: Connection,
    shm: WlShm,
    outputs: Vec<Output>,
    src_mgr: ExtOutputImageCaptureSourceManagerV1,
    icc: ExtImageCopyCaptureManagerV1,
    dc_mgr: Option<ExtDataControlManagerV1>,
    dc_device: Option<ExtDataControlDeviceV1>,
    _shortcuts: Vec<HyprlandGlobalShortcutV1>,
    self_tx: channel::Sender<Cmd>,
    capture: Option<Capture>,
    slots: HashMap<(usize, Purpose), Slot>,
    epoch: u32,
    clip_source: Option<ExtDataControlSourceV1>,
    clip_bytes: Arc<Vec<u8>>,
    overlay_visible: Arc<AtomicBool>,
    ui: UiSink,
    seat: Option<WlSeat>,
    idle_mgr: Option<ExtIdleNotifierV1>,
    soak: Option<Soak>,
}

struct Soak {
    _notification: ExtIdleNotificationV1,
    interval: Duration,
    timer: Option<RegistrationToken>,
    trials: u64,
}

pub fn run(
    cmds: Channel<Cmd>,
    self_tx: channel::Sender<Cmd>,
    overlay_visible: Arc<AtomicBool>,
    ui: UiSink,
    register_shortcut: bool,
) {
    if let Err(e) = run_inner(cmds, self_tx, overlay_visible, ui, register_shortcut) {
        mark("wl_error", &format!("\"msg\":\"{e}\""));
    }
}

fn run_inner(
    cmds: Channel<Cmd>,
    self_tx: channel::Sender<Cmd>,
    overlay_visible: Arc<AtomicBool>,
    ui: UiSink,
    register_shortcut: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;

    let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
    let src_mgr: ExtOutputImageCaptureSourceManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let icc: ExtImageCopyCaptureManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let dc_mgr: Option<ExtDataControlManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let seat: Option<WlSeat> = globals.bind(&qh, 1..=7, ()).ok();
    let gs_mgr: Option<HyprlandGlobalShortcutsManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let idle_mgr: Option<ExtIdleNotifierV1> = globals.bind(&qh, 1..=1, ()).ok();

    let mut outputs = Vec::new();
    for g in globals.contents().clone_list() {
        if g.interface == WlOutput::interface().name {
            let idx = outputs.len();
            let wl: WlOutput = globals.registry().bind(g.name, g.version.min(4), &qh, idx);
            outputs.push(Output { wl, name: String::new() });
        }
    }

    let dc_device = match (&dc_mgr, &seat) {
        (Some(m), Some(s)) => Some(m.get_data_device(s, &qh, ())),
        _ => None,
    };
    let mut shortcuts = Vec::new();
    if let (Some(m), true) = (&gs_mgr, register_shortcut) {
        for (id, desc, kind) in [
            ("area", "Snip spike: capture area", ShortcutKind::Area),
            ("fullscreen", "Snip spike: capture fullscreen", ShortcutKind::Fullscreen),
        ] {
            shortcuts.push(m.register_shortcut(id.into(), APP_ID.into(), desc.into(), "".into(), &qh, kind));
        }
    }

    let mut state = State {
        qh: qh.clone(),
        lh: event_loop.handle(),
        conn: conn.clone(),
        shm,
        outputs,
        src_mgr,
        icc,
        dc_mgr,
        dc_device,
        _shortcuts: shortcuts,
        self_tx,
        capture: None,
        slots: HashMap::new(),
        epoch: 0,
        clip_source: None,
        clip_bytes: Arc::new(Vec::new()),
        overlay_visible,
        ui,
        seat,
        idle_mgr,
        soak: None,
    };

    // Roundtrip so every wl_output has delivered its name before we report ready.
    queue.roundtrip(&mut state)?;
    WaylandSource::new(conn.clone(), queue).insert(event_loop.handle())?;
    event_loop
        .handle()
        .insert_source(cmds, |ev, _, state: &mut State| {
            if let channel::Event::Msg(cmd) = ev {
                state.handle_cmd(cmd);
                let _ = state.conn.flush();
            }
        })
        .map_err(|e| e.error)?;

    let names: Vec<String> = state.outputs.iter().map(|o| o.name.clone()).collect();
    mark(
        "wl_ready",
        &format!(
            "\"outputs\":{:?},\"data_control\":{},\"global_shortcuts\":{}",
            names,
            state.dc_device.is_some(),
            state._shortcuts.len()
        ),
    );

    loop {
        event_loop.dispatch(None, &mut state)?;
    }
}

impl State {
    fn handle_cmd(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Capture(p) => self.start_capture(p, None, None),
            Cmd::SetClipboard { png, t0 } => self.set_clipboard(png, t0),
            Cmd::StartSoak { idle_ms, interval_ms } => {
                let (Some(mgr), Some(seat)) = (&self.idle_mgr, &self.seat) else {
                    mark("soak_unavailable", "");
                    return;
                };
                let n = mgr.get_idle_notification(idle_ms, seat, &self.qh, ());
                self.soak = Some(Soak { _notification: n, interval: Duration::from_millis(interval_ms), timer: None, trials: 0 });
                mark("soak_started", &format!("\"idle_ms\":{idle_ms},\"interval_ms\":{interval_ms}"));
            }
        }
    }

    fn start_capture(&mut self, purpose: Purpose, t_hotkey: Option<u64>, only: Option<&str>) {
        if self.capture.is_some() {
            mark("capture_busy", "");
            return;
        }
        let t_start = now_ns();
        let mut caps = Vec::new();
        for (i, o) in self.outputs.iter().enumerate() {
            if only.is_some_and(|n| n != o.name) {
                continue;
            }
            let source = self.src_mgr.create_source(&o.wl, &self.qh, ());
            // Fresh session per screenshot (a reused session can stall on a static screen).
            let session = self.icc.create_session(&source, Options::empty(), &self.qh, i);
            caps.push(Cap {
                out: i,
                source,
                session,
                frame: None,
                size: None,
                formats: Vec::new(),
                transform: 0,
                finished: false,
                ok: false,
                t_ready: 0,
            });
        }
        let timer = self
            .lh
            .insert_source(Timer::from_duration(CAPTURE_TIMEOUT), |_, _, st: &mut State| {
                st.on_timeout();
                TimeoutAction::Drop
            })
            .ok();
        self.capture = Some(Capture { purpose, caps, t_start, t_hotkey, timer });
        let _ = self.conn.flush();
        mark("capture_issued", &format!("\"purpose\":\"{purpose:?}\""));
    }

    fn on_timeout(&mut self) {
        if let Some(c) = self.capture.as_mut() {
            c.timer = None;
            for cap in c.caps.iter_mut().filter(|c| !c.finished) {
                cap.finished = true;
                cap.ok = false;
            }
            mark("capture_timeout", "");
        }
        self.check_complete();
    }

    fn ensure_slot(&mut self, out: usize, purpose: Purpose, w: u32, h: u32, fmt: wl_shm::Format) -> &Slot {
        let key = (out, purpose);
        let fits = self
            .slots
            .get(&key)
            .map(|s| s.buf.width == w && s.buf.height == h && s.buf.format == fmt as u32)
            .unwrap_or(false);
        if !fits {
            let t0 = now_ns();
            let buf = Arc::new(ShmBuf::new(w, h, fmt as u32).expect("shm alloc"));
            let pool = self.shm.create_pool(buf.fd_borrowed(), buf.len as i32, &self.qh, ());
            let wl_buffer = pool.create_buffer(0, w as i32, h as i32, buf.stride as i32, fmt, &self.qh, ());
            mark(
                "shm_alloc",
                &format!("\"out\":{out},\"w\":{w},\"h\":{h},\"ms\":{:.2}", (now_ns() - t0) as f64 / 1e6),
            );
            self.slots.insert(key, Slot { buf, _pool: pool, wl_buffer });
        }
        &self.slots[&key]
    }

    fn on_session_done(&mut self, out: usize) {
        let Some(c) = self.capture.as_ref() else { return };
        let purpose = c.purpose;
        let Some(cap) = c.caps.iter().find(|c| c.out == out) else { return };
        if cap.frame.is_some() || cap.finished {
            return;
        }
        let Some((w, h)) = cap.size else { return };
        let fmt = [wl_shm::Format::Xrgb8888, wl_shm::Format::Argb8888]
            .into_iter()
            .find(|f| cap.formats.contains(f));
        let Some(fmt) = fmt else {
            mark("no_supported_format", &format!("\"formats\":\"{:?}\"", cap.formats));
            return;
        };
        let wl_buffer = self.ensure_slot(out, purpose, w, h, fmt).wl_buffer.clone();
        let c = self.capture.as_mut().unwrap();
        let cap = c.caps.iter_mut().find(|c| c.out == out).unwrap();
        let frame = cap.session.create_frame(&self.qh, out);
        frame.attach_buffer(&wl_buffer);
        frame.damage_buffer(0, 0, w as i32, h as i32);
        frame.capture();
        cap.frame = Some(frame);
        let _ = self.conn.flush();
    }

    fn check_complete(&mut self) {
        let Some(c) = self.capture.as_ref() else { return };
        if !c.caps.iter().all(|c| c.finished) {
            return;
        }
        let mut c = self.capture.take().unwrap();
        if let Some(t) = c.timer.take() {
            self.lh.remove(t);
        }
        let t_done = now_ns();
        let per_out: Vec<String> = c
            .caps
            .iter()
            .map(|cap| {
                format!(
                    "{{\"output\":\"{}\",\"ok\":{},\"ready_ms\":{:.2}}}",
                    self.outputs[cap.out].name,
                    cap.ok,
                    if cap.ok { (cap.t_ready - c.t_start) as f64 / 1e6 } else { -1.0 }
                )
            })
            .collect();
        let hot = c
            .t_hotkey
            .map(|t| format!(",\"since_hotkey_ms\":{:.2}", (t_done - t) as f64 / 1e6))
            .unwrap_or_default();
        mark(
            "capture_complete",
            &format!(
                "\"purpose\":\"{:?}\",\"total_ms\":{:.2}{hot},\"outputs\":[{}]",
                c.purpose,
                (t_done - c.t_start) as f64 / 1e6,
                per_out.join(",")
            ),
        );

        match c.purpose {
            Purpose::Freeze => {
                self.epoch += 1;
                for cap in c.caps.iter().filter(|c| c.ok) {
                    let slot = &self.slots[&(cap.out, Purpose::Freeze)];
                    frames::publish(
                        &self.outputs[cap.out].name,
                        FrameSlot { buf: slot.buf.clone(), epoch: self.epoch, transform: cap.transform },
                    );
                }
                (self.ui)(UiEvent::FramesReady { epoch: self.epoch });
            }
            Purpose::Fullscreen => {
                // Spike stand-in for "capture -> history + QAO": encode and copy.
                if let Some(cap) = c.caps.iter().find(|c| c.ok) {
                    let buf = self.slots[&(cap.out, Purpose::Fullscreen)].buf.clone();
                    let tx = self.self_tx.clone();
                    let t0 = c.t_hotkey.unwrap_or(c.t_start);
                    std::thread::spawn(move || {
                        let e = crate::encode::crop_png(&buf, 0, 0, buf.width as i32, buf.height as i32);
                        mark(
                            "fullscreen_encoded",
                            &format!("\"w\":{},\"h\":{},\"encode_ms\":{:.2},\"png_bytes\":{}", e.w, e.h, e.encode_ms, e.png.len()),
                        );
                        let _ = tx.send(Cmd::SetClipboard { png: Arc::new(e.png), t0 });
                    });
                }
            }
            Purpose::Verify => {
                let mut parts = Vec::new();
                for cap in c.caps.iter() {
                    let name = &self.outputs[cap.out].name;
                    let (Some(a), Some(b)) = (
                        self.slots.get(&(cap.out, Purpose::Freeze)),
                        self.slots.get(&(cap.out, Purpose::Verify)),
                    ) else {
                        parts.push(format!("{{\"output\":\"{name}\",\"error\":\"missing buffer\"}}"));
                        continue;
                    };
                    let rep = diff_report(name, &a.buf, &b.buf);
                    if let Ok(dir) = std::env::var("SNIP_DUMP_DIR") {
                        if !rep.contains("\"diff_pixels\":0,") {
                            for (tag, buf) in [("freeze", &a.buf), ("verify", &b.buf)] {
                                let e = crate::encode::crop_png(buf, 0, 0, buf.width as i32, buf.height as i32);
                                let _ = std::fs::write(format!("{dir}/{name}-{tag}.png"), &e.png);
                            }
                        }
                    }
                    parts.push(rep);
                }
                (self.ui)(UiEvent::VerifyDone { report: format!("[{}]", parts.join(",")) });
            }
        }
        for cap in c.caps {
            if let Some(f) = cap.frame {
                f.destroy();
            }
            cap.session.destroy();
            cap.source.destroy();
        }
        let _ = self.conn.flush();
    }

    fn set_clipboard(&mut self, png: Arc<Vec<u8>>, t0: u64) {
        let (Some(mgr), Some(dev)) = (&self.dc_mgr, &self.dc_device) else {
            mark("clipboard_unavailable", "");
            return;
        };
        let src = mgr.create_data_source(&self.qh, ());
        src.offer("image/png".into());
        dev.set_selection(Some(&src));
        if let Some(old) = self.clip_source.replace(src) {
            old.destroy();
        }
        self.clip_bytes = png;
        let _ = self.conn.flush();
        mark(
            "clipboard_offered",
            &format!("\"since_copy_ms\":{:.2},\"bytes\":{}", (now_ns() - t0) as f64 / 1e6, self.clip_bytes.len()),
        );
    }
}

fn diff_report(name: &str, a: &ShmBuf, b: &ShmBuf) -> String {
    if a.width != b.width || a.height != b.height {
        return format!("{{\"output\":\"{name}\",\"error\":\"size mismatch\"}}");
    }
    let (w, h, stride) = (a.width as usize, a.height as usize, a.stride as usize);
    let (ab, bb) = (a.bytes(), b.bytes());
    let mut diff = 0u64;
    let mut max_delta = 0u8;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..h {
        let ra = &ab[y * stride..y * stride + w * 4];
        let rb = &bb[y * stride..y * stride + w * 4];
        if ra == rb {
            continue;
        }
        for x in 0..w {
            let pa = &ra[x * 4..x * 4 + 3];
            let pb = &rb[x * 4..x * 4 + 3];
            if pa != pb {
                diff += 1;
                for k in 0..3 {
                    max_delta = max_delta.max(pa[k].abs_diff(pb[k]));
                }
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    let bbox = if diff > 0 { format!("[{x0},{y0},{x1},{y1}]") } else { "null".into() };
    format!(
        "{{\"output\":\"{name}\",\"w\":{w},\"h\":{h},\"diff_pixels\":{diff},\"max_channel_delta\":{max_delta},\"bbox\":{bbox}}}"
    )
}

impl ShmBuf {
    fn fd_borrowed(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        self.fd.as_fd()
    }
}

// ---------------------------------------------------------------- dispatch

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(_: &mut Self, _: &WlRegistry, _: <WlRegistry as Proxy>::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<WlOutput, usize> for State {
    fn event(st: &mut Self, _: &WlOutput, ev: wl_output::Event, idx: &usize, _: &Connection, _: &QueueHandle<Self>) {
        if let wl_output::Event::Name { name } = ev {
            st.outputs[*idx].name = name;
        }
    }
}

macro_rules! ignore_events {
    ($($ty:ty),*) => {$(
        impl Dispatch<$ty, ()> for State {
            fn event(_: &mut Self, _: &$ty, _: <$ty as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
ignore_events!(
    WlShm,
    WlShmPool,
    WlBuffer,
    WlSeat,
    ExtOutputImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1,
    ExtImageCopyCaptureManagerV1,
    ExtDataControlManagerV1,
    HyprlandGlobalShortcutsManagerV1,
    ExtIdleNotifierV1
);

impl Dispatch<ExtImageCopyCaptureSessionV1, usize> for State {
    fn event(st: &mut Self, _: &ExtImageCopyCaptureSessionV1, ev: ext_image_copy_capture_session_v1::Event, out: &usize, _: &Connection, _: &QueueHandle<Self>) {
        use ext_image_copy_capture_session_v1::Event as E;
        let out = *out;
        let cap = st.capture.as_mut().and_then(|c| c.caps.iter_mut().find(|c| c.out == out));
        match ev {
            E::BufferSize { width, height } => {
                if let Some(cap) = cap {
                    cap.size = Some((width, height));
                }
            }
            E::ShmFormat { format: WEnum::Value(f) } => {
                if let Some(cap) = cap {
                    cap.formats.push(f);
                }
            }
            E::Done => st.on_session_done(out),
            E::Stopped => {
                if let Some(cap) = cap {
                    cap.finished = true;
                    cap.ok = false;
                }
                st.check_complete();
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, usize> for State {
    fn event(st: &mut Self, _: &ExtImageCopyCaptureFrameV1, ev: ext_image_copy_capture_frame_v1::Event, out: &usize, _: &Connection, _: &QueueHandle<Self>) {
        use ext_image_copy_capture_frame_v1::Event as E;
        let out = *out;
        {
            let Some(cap) = st.capture.as_mut().and_then(|c| c.caps.iter_mut().find(|c| c.out == out)) else { return };
            match ev {
                E::Transform { transform: WEnum::Value(t) } => cap.transform = t as u32,
                E::Ready => {
                    cap.finished = true;
                    cap.ok = true;
                    cap.t_ready = now_ns();
                }
                E::Failed { reason } => {
                    cap.finished = true;
                    cap.ok = false;
                    mark("frame_failed", &format!("\"out\":{out},\"reason\":\"{reason:?}\""));
                }
                _ => return,
            }
        }
        st.check_complete();
    }
}

impl Dispatch<HyprlandGlobalShortcutV1, ShortcutKind> for State {
    fn event(st: &mut Self, _: &HyprlandGlobalShortcutV1, ev: hyprland_global_shortcut_v1::Event, kind: &ShortcutKind, _: &Connection, _: &QueueHandle<Self>) {
        if let hyprland_global_shortcut_v1::Event::Pressed { tv_sec_hi, tv_sec_lo, tv_nsec } = ev {
            let t_hotkey = ((tv_sec_hi as u64) << 32 | tv_sec_lo as u64) * 1_000_000_000 + tv_nsec as u64;
            mark(
                "hotkey_pressed",
                &format!(
                    "\"kind\":\"{kind:?}\",\"t_hotkey_ns\":{t_hotkey},\"delivery_ms\":{:.3}",
                    (now_ns().saturating_sub(t_hotkey)) as f64 / 1e6
                ),
            );
            if st.overlay_visible.load(Ordering::Relaxed) {
                (st.ui)(UiEvent::CancelRequested);
                return;
            }
            match kind {
                ShortcutKind::Area => st.start_capture(Purpose::Freeze, Some(t_hotkey), None),
                ShortcutKind::Fullscreen => {
                    let focused = crate::hypr::focused_monitor();
                    st.start_capture(Purpose::Fullscreen, Some(t_hotkey), focused.as_deref());
                }
            }
        }
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(_: &mut Self, _: &ExtDataControlDeviceV1, ev: ext_data_control_device_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        // We never read other clients' selections in this spike.
        if let ext_data_control_device_v1::Event::Selection { id: Some(offer) } = ev {
            offer.destroy();
        }
    }
    event_created_child!(State, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for State {
    fn event(_: &mut Self, _: &ExtDataControlOfferV1, _: <ExtDataControlOfferV1 as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}

impl Dispatch<ExtDataControlSourceV1, ()> for State {
    fn event(st: &mut Self, src: &ExtDataControlSourceV1, ev: ext_data_control_source_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let bytes = st.clip_bytes.clone();
                mark("clipboard_send", &format!("\"mime\":\"{mime_type}\""));
                // Never block the Wayland thread on a slow reader.
                std::thread::spawn(move || {
                    let mut f = std::fs::File::from(fd);
                    let _ = f.write_all(&bytes);
                });
            }
            ext_data_control_source_v1::Event::Cancelled => {
                src.destroy();
                if st.clip_source.as_ref() == Some(src) {
                    st.clip_source = None;
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn event(st: &mut Self, _: &ExtIdleNotificationV1, ev: ext_idle_notification_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        let Some(soak) = st.soak.as_mut() else { return };
        match ev {
            ext_idle_notification_v1::Event::Idled => {
                mark("soak_idle", &format!("\"trials\":{}", soak.trials));
                let interval = soak.interval;
                let token = st
                    .lh
                    .insert_source(Timer::from_duration(interval), move |_, _, st: &mut State| {
                        if !st.overlay_visible.load(Ordering::Relaxed) {
                            if let Some(s) = st.soak.as_mut() {
                                s.trials += 1;
                            }
                            st.start_capture(Purpose::Freeze, None, None);
                        }
                        TimeoutAction::ToDuration(interval)
                    })
                    .ok();
                if let Some(s) = st.soak.as_mut() {
                    s.timer = token;
                }
            }
            ext_idle_notification_v1::Event::Resumed => {
                mark("soak_resumed", &format!("\"trials\":{}", soak.trials));
                if let Some(t) = soak.timer.take() {
                    st.lh.remove(t);
                }
            }
            _ => {}
        }
    }
}
