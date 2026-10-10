use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calloop::channel::{self, Channel};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, RegistrationToken};
use calloop_wayland_source::WaylandSource;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer::WlBuffer, wl_compositor::WlCompositor, wl_output, wl_output::WlOutput, wl_pointer, wl_registry, wl_registry::WlRegistry,
    wl_seat::WlSeat, wl_shm, wl_shm::WlShm, wl_shm_pool::WlShmPool, wl_surface::WlSurface,
};
use wayland_client::{event_created_child, Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::{self, ExtDataControlSourceV1},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self, ExtImageCopyCaptureSessionV1},
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1, zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};
use wayland_protocols_hyprland::global_shortcuts::v1::client::{
    hyprland_global_shortcut_v1::{self, HyprlandGlobalShortcutV1},
    hyprland_global_shortcuts_manager_v1::HyprlandGlobalShortcutsManagerV1,
};
use wayland_protocols_hyprland::toplevel_mapping::v1::client::{
    hyprland_toplevel_mapping_manager_v1::HyprlandToplevelMappingManagerV1,
    hyprland_toplevel_window_mapping_handle_v1::{self, HyprlandToplevelWindowMappingHandleV1},
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1, zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

use snip_core::frame::{Frame, PixelFormat, ShmBuf};
use snip_core::geometry::{Rect, Transform};
use snip_core::trace::{self, now_ns};

use crate::{CaptureResult, Event, Features, OutputInfo, Request, Sink, Source};

const OUTPUT_TIMEOUT: Duration = Duration::from_millis(150);
const WINDOW_TIMEOUT: Duration = Duration::from_millis(400);
const POOL_PER_KEY: usize = 3;

struct Output {
    global: u32,
    wl: WlOutput,
    xdg: Option<ZxdgOutputV1>,
    name: String,
    logical: Rect,
    transform: u32,
}

struct Toplevel {
    handle: ExtForeignToplevelHandleV1,
    address: Option<u64>,
}

struct PoolBuf {
    buf: Arc<ShmBuf>,
    _pool: WlShmPool,
    wl_buffer: WlBuffer,
}

/// One capture target inside a request.
struct Item {
    key: String,
    logical: Rect,
    source: ExtImageCaptureSourceV1,
    session: ExtImageCopyCaptureSessionV1,
    frame: Option<ExtImageCopyCaptureFrameV1>,
    buf: Option<Arc<ShmBuf>>,
    size: Option<(u32, u32)>,
    formats: Vec<wl_shm::Format>,
    transform: u32,
    finished: bool,
    ok: bool,
}

struct Capture {
    request: u64,
    items: Vec<Item>,
    t_start: u64,
    timer: Option<RegistrationToken>,
}

pub(crate) struct State {
    qh: QueueHandle<State>,
    lh: LoopHandle<'static, State>,
    conn: Connection,
    registry: WlRegistry,
    sink: *const Sink,
    shm: WlShm,
    seat: Option<WlSeat>,
    xdg_mgr: Option<ZxdgOutputManagerV1>,
    outputs: Vec<Output>,
    out_src: ExtOutputImageCaptureSourceManagerV1,
    top_src: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    icc: ExtImageCopyCaptureManagerV1,
    toplevels: HashMap<u32, Toplevel>,
    mapping: Option<HyprlandToplevelMappingManagerV1>,
    _toplevel_list: Option<ExtForeignToplevelListV1>,
    dc_mgr: Option<ExtDataControlManagerV1>,
    dc_device: Option<ExtDataControlDeviceV1>,
    clip_source: Option<ExtDataControlSourceV1>,
    clip_offers: Arc<HashMap<String, Arc<Vec<u8>>>>,
    /// The current selection (anyone's) and the MIME types it offers
    selection: Option<(ExtDataControlOfferV1, Offered)>,
    gs_mgr: Option<HyprlandGlobalShortcutsManagerV1>,
    shortcuts: Vec<HyprlandGlobalShortcutV1>,
    vptr_mgr: Option<ZwlrVirtualPointerManagerV1>,
    vptr: Option<(String, ZwlrVirtualPointerV1)>,
    pools: HashMap<String, Vec<PoolBuf>>,
    captures: Vec<Capture>,
    app_id: String,
    ready_sent: bool,
    compositor: Option<WlCompositor>,
    idle_mgr: Option<ZwpIdleInhibitManagerV1>,
    /// an idle inhibitor while recording, on a surface of ours with no role
    awake: Option<(WlSurface, ZwpIdleInhibitorV1)>,
    idle_notifier: Option<ExtIdleNotifierV1>,
    /// tests: whether the session would go idle
    idle_watch: Option<ExtIdleNotificationV1>,
}

// The sink pointer is only used on the Wayland thread for the lifetime of run().
unsafe impl Send for State {}

impl State {
    fn emit(&self, ev: Event) {
        unsafe { (*self.sink)(ev) }
    }

    fn output_infos(&self) -> Vec<OutputInfo> {
        self.outputs
            .iter()
            .filter(|o| !o.name.is_empty())
            .map(|o| OutputInfo { name: o.name.clone(), logical: o.logical, transform: o.transform })
            .collect()
    }

    fn bind_output(&mut self, global: u32, version: u32) {
        let idx = global;
        let wl = self.registry.bind::<WlOutput, u32, State>(global, version.min(4), &self.qh, idx);
        let xdg = self.xdg_mgr.as_ref().map(|m| m.get_xdg_output(&wl, &self.qh, idx));
        self.outputs.push(Output { global, wl, xdg, name: String::new(), logical: Rect::default(), transform: 0 });
    }
}

pub(crate) fn run(app_id: String, rx: Channel<Request>, sink: &Sink) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, State> = EventLoop::try_new()?;

    let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
    let out_src: ExtOutputImageCaptureSourceManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let icc: ExtImageCopyCaptureManagerV1 = globals.bind(&qh, 1..=1, ())?;
    let top_src: Option<ExtForeignToplevelImageCaptureSourceManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let xdg_mgr: Option<ZxdgOutputManagerV1> = globals.bind(&qh, 2..=3, ()).ok();
    let seat: Option<WlSeat> = globals.bind(&qh, 1..=7, ()).ok();
    let dc_mgr: Option<ExtDataControlManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let gs_mgr: Option<HyprlandGlobalShortcutsManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let vptr_mgr: Option<ZwlrVirtualPointerManagerV1> = globals.bind(&qh, 1..=2, ()).ok();
    let mapping: Option<HyprlandToplevelMappingManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let toplevel_list: Option<ExtForeignToplevelListV1> = globals.bind(&qh, 1..=1, ()).ok();
    let compositor: Option<WlCompositor> = globals.bind(&qh, 4..=6, ()).ok();
    let idle_mgr: Option<ZwpIdleInhibitManagerV1> = globals.bind(&qh, 1..=1, ()).ok();
    let idle_notifier: Option<ExtIdleNotifierV1> = globals.bind(&qh, 1..=1, ()).ok();
    let dc_device = match (&dc_mgr, &seat) {
        (Some(m), Some(s)) => Some(m.get_data_device(s, &qh, ())),
        _ => None,
    };

    let mut state = State {
        qh: qh.clone(),
        lh: event_loop.handle(),
        conn: conn.clone(),
        registry: globals.registry().clone(),
        sink: sink as *const Sink,
        shm,
        seat,
        xdg_mgr,
        outputs: Vec::new(),
        out_src,
        top_src,
        icc,
        toplevels: HashMap::new(),
        mapping,
        _toplevel_list: toplevel_list,
        dc_mgr,
        dc_device,
        clip_source: None,
        clip_offers: Arc::new(HashMap::new()),
        selection: None,
        gs_mgr,
        shortcuts: Vec::new(),
        vptr_mgr,
        vptr: None,
        pools: HashMap::new(),
        captures: Vec::new(),
        app_id,
        ready_sent: false,
        compositor,
        idle_mgr,
        awake: None,
        idle_notifier,
        idle_watch: None,
    };
    for g in globals.contents().clone_list() {
        if g.interface == WlOutput::interface().name {
            state.bind_output(g.name, g.version);
        }
    }
    // Two roundtrips: wl_output/xdg_output events, then toplevel mappings.
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;

    let features = Features {
        data_control: state.dc_device.is_some(),
        global_shortcuts: state.gs_mgr.is_some(),
        virtual_pointer: state.vptr_mgr.is_some(),
        toplevel_capture: state.top_src.is_some() && state.mapping.is_some(),
    };
    state.ready_sent = true;
    state.emit(Event::Ready { outputs: state.output_infos(), features });

    WaylandSource::new(conn.clone(), queue).insert(event_loop.handle())?;
    event_loop
        .handle()
        .insert_source(rx, |ev, _, st: &mut State| {
            if let channel::Event::Msg(req) = ev {
                st.handle(req);
                let _ = st.conn.flush();
            }
        })
        .map_err(|e| e.error)?;
    loop {
        event_loop.dispatch(None, &mut state)?;
    }
}

impl State {
    fn handle(&mut self, req: Request) {
        match req {
            Request::Capture { request, source, cursor } => self.start_capture(request, source, cursor),
            Request::SetClipboard { offers } => self.set_clipboard(offers),
            Request::ReadClipboard { mimes, reply } => self.read_clipboard(mimes, reply),
            Request::RegisterShortcut { id, description } => {
                if let Some(m) = &self.gs_mgr {
                    let s = m.register_shortcut(id.clone(), self.app_id.clone(), description, String::new(), &self.qh, id);
                    self.shortcuts.push(s);
                }
            }
            Request::PointerMove { output, x, y, w, h } => {
                if let Some(p) = self.pointer_for(&output) {
                    p.motion_absolute(now_ms(), x, y, w, h);
                    p.frame();
                }
            }
            Request::PointerButton { down, button } => {
                if let Some((_, p)) = &self.vptr {
                    let st = if down { wl_pointer::ButtonState::Pressed } else { wl_pointer::ButtonState::Released };
                    p.button(now_ms(), button, st);
                    p.frame();
                }
            }
            Request::PointerNudge => {
                // the overlay gets the pointer only on motion: a pointer of
                // Snip's own (unbound: relative motion), if none moved before
                if self.vptr.is_none() {
                    self.pointer_for("");
                }
                let moved = if let Some((_, p)) = &self.vptr {
                    p.motion(now_ms(), 1.0, 0.0);
                    p.frame();
                    p.motion(now_ms(), -1.0, 0.0);
                    p.frame();
                    true
                } else {
                    false
                };
                trace::trace("pointer_nudged", &format!("\"moved\":{moved}"));
            }
            Request::PointerWheel { notches } => {
                if let Some((_, p)) = &self.vptr {
                    p.axis_source(wl_pointer::AxisSource::Wheel);
                    p.axis_discrete(now_ms(), wl_pointer::Axis::VerticalScroll, 15.0 * notches as f64, notches);
                    p.frame();
                }
            }
            Request::PointerAxis { dy } => {
                if let Some((_, p)) = &self.vptr {
                    p.axis_source(wl_pointer::AxisSource::Wheel);
                    p.axis(now_ms(), wl_pointer::Axis::VerticalScroll, dy);
                    p.frame();
                }
            }
            Request::KeepAwake(on) => self.keep_awake(on),
            Request::IdleWatch(ms) => {
                if let Some(w) = self.idle_watch.take() {
                    w.destroy();
                }
                if let (Some(n), Some(seat), true) = (&self.idle_notifier, &self.seat, ms > 0) {
                    self.idle_watch = Some(n.get_idle_notification(ms, seat, &self.qh, ()));
                }
            }
        }
    }

    /// The screen doesn't idle (and lock) while recording: an idle inhibitor
    /// on a surface of ours with no role, which Hyprland counts as visible.
    /// (On a layer it judged the inhibitor before the layer mapped, and never
    /// again: a take nobody touched still went idle.)
    fn keep_awake(&mut self, on: bool) {
        if let Some((surface, inhibitor)) = self.awake.take() {
            inhibitor.destroy();
            surface.destroy();
        }
        if let (true, Some(comp), Some(mgr)) = (on, &self.compositor, &self.idle_mgr) {
            let surface = comp.create_surface(&self.qh, ());
            let inhibitor = mgr.create_inhibitor(&surface, &self.qh, ());
            self.awake = Some((surface, inhibitor));
        }
        trace::trace("keep_awake", &format!("\"on\":{}", self.awake.is_some()));
    }

    fn pointer_for(&mut self, output: &str) -> Option<&ZwlrVirtualPointerV1> {
        if self.vptr.as_ref().map(|(o, _)| o.as_str()) != Some(output) {
            let mgr = self.vptr_mgr.as_ref()?;
            let seat = self.seat.as_ref()?;
            let out = self.outputs.iter().find(|o| o.name == output).map(|o| o.wl.clone());
            let p = match (&out, mgr.version() >= 2) {
                (Some(o), true) => mgr.create_virtual_pointer_with_output(Some(seat), Some(o), &self.qh, ()),
                _ => mgr.create_virtual_pointer(Some(seat), &self.qh, ()),
            };
            if let Some((_, old)) = self.vptr.replace((output.to_owned(), p)) {
                old.destroy();
            }
        }
        self.vptr.as_ref().map(|(_, p)| p)
    }

    // ------------------------------------------------------------ capture

    fn start_capture(&mut self, request: u64, source: Source, cursor: bool) {
        let t_start = now_ns();
        let options = if cursor { Options::PaintCursors } else { Options::empty() };
        let mut items = Vec::new();
        let mut failed = Vec::new();
        let timeout = match &source {
            Source::Outputs(_) => OUTPUT_TIMEOUT,
            Source::Window(_) => WINDOW_TIMEOUT,
        };
        match source {
            Source::Outputs(filter) => {
                for o in &self.outputs {
                    if o.name.is_empty() || filter.as_ref().is_some_and(|f| !f.contains(&o.name)) {
                        continue;
                    }
                    // Fresh session per capture: a reused session can stall on a static screen.
                    let src = self.out_src.create_source(&o.wl, &self.qh, ());
                    let session = self.icc.create_session(&src, options, &self.qh, (request, o.name.clone()));
                    items.push(Item::new(o.name.clone(), o.logical, src, session));
                }
            }
            Source::Window(address) => {
                let handle = self.toplevels.values().find(|t| t.address == Some(address)).map(|t| t.handle.clone());
                match (handle, &self.top_src) {
                    (Some(h), Some(mgr)) => {
                        let key = format!("window:{address:x}");
                        let src = mgr.create_source(&h, &self.qh, ());
                        let session = self.icc.create_session(&src, options, &self.qh, (request, key.clone()));
                        items.push(Item::new(key, Rect::default(), src, session));
                    }
                    _ => failed.push(format!("window:{address:x}")),
                }
            }
        }
        if items.is_empty() {
            self.emit(Event::Captured(CaptureResult { request, frames: vec![], failed, t_start_ns: t_start, t_done_ns: now_ns() }));
            return;
        }
        let timer = self
            .lh
            .insert_source(Timer::from_duration(timeout), move |_, _, st: &mut State| {
                st.on_timeout(request);
                TimeoutAction::Drop
            })
            .ok();
        self.captures.push(Capture { request, items, t_start, timer });
        trace::trace("capture_issued", &format!("\"request\":{request}"));
    }

    fn item_mut(&mut self, request: u64, key: &str) -> Option<&mut Item> {
        self.captures.iter_mut().find(|c| c.request == request)?.items.iter_mut().find(|i| i.key == key)
    }

    fn take_buffer(&mut self, key: &str, w: u32, h: u32, fmt: wl_shm::Format) -> Option<(Arc<ShmBuf>, WlBuffer)> {
        let pixfmt = match fmt {
            wl_shm::Format::Argb8888 => PixelFormat::Argb8888,
            wl_shm::Format::Xrgb8888 => PixelFormat::Xrgb8888,
            _ => return None,
        };
        let pool = self.pools.entry(key.to_owned()).or_default();
        // reuse a buffer nobody else holds (the UI may still show an older frame)
        pool.retain(|p| !(Arc::strong_count(&p.buf) == 1 && (p.buf.width, p.buf.height) != (w, h)));
        if let Some(p) = pool.iter().find(|p| Arc::strong_count(&p.buf) == 1 && p.buf.format == pixfmt) {
            return Some((p.buf.clone(), p.wl_buffer.clone()));
        }
        let t0 = now_ns();
        let buf = Arc::new(ShmBuf::new(w, h, pixfmt).ok()?);
        let wl_pool = self.shm.create_pool(buf.fd(), buf.len() as i32, &self.qh, ());
        let wl_buffer = wl_pool.create_buffer(0, w as i32, h as i32, buf.stride as i32, fmt, &self.qh, ());
        trace::trace("shm_alloc", &format!("\"key\":{},\"w\":{w},\"h\":{h},\"ms\":{:.2}", trace::js(key), trace::ms_since(t0)));
        if pool.len() >= POOL_PER_KEY {
            if let Some(i) = pool.iter().position(|p| Arc::strong_count(&p.buf) == 1) {
                pool.remove(i);
            }
        }
        pool.push(PoolBuf { buf: buf.clone(), _pool: wl_pool, wl_buffer: wl_buffer.clone() });
        Some((buf, wl_buffer))
    }

    fn on_session_done(&mut self, request: u64, key: &str) {
        let Some(item) = self.item_mut(request, key) else { return };
        if item.frame.is_some() || item.finished {
            return;
        }
        let Some((w, h)) = item.size else { return };
        // Windows keep their alpha (translucent terminals, CSD rounded corners);
        // outputs are opaque.
        let prefer = if key.starts_with("window:") {
            [wl_shm::Format::Argb8888, wl_shm::Format::Xrgb8888]
        } else {
            [wl_shm::Format::Xrgb8888, wl_shm::Format::Argb8888]
        };
        let fmt = prefer.into_iter().find(|f| item.formats.contains(f));
        let Some(fmt) = fmt else {
            item.finished = true;
            trace::warn("no_supported_shm_format", &format!("\"key\":{}", trace::js(key)));
            self.check_complete(request);
            return;
        };
        let Some((buf, wl_buffer)) = self.take_buffer(key, w, h, fmt) else { return };
        let qh = self.qh.clone();
        let item = self.item_mut(request, key).unwrap();
        let frame = item.session.create_frame(&qh, (request, key.to_owned()));
        frame.attach_buffer(&wl_buffer);
        frame.damage_buffer(0, 0, w as i32, h as i32);
        frame.capture();
        item.frame = Some(frame);
        item.buf = Some(buf);
        let _ = self.conn.flush();
    }

    fn on_timeout(&mut self, request: u64) {
        if let Some(c) = self.captures.iter_mut().find(|c| c.request == request) {
            c.timer = None;
            for i in c.items.iter_mut().filter(|i| !i.finished) {
                i.finished = true;
                i.ok = false;
            }
            trace::warn("capture_timeout", &format!("\"request\":{request}"));
        }
        self.check_complete(request);
    }

    fn check_complete(&mut self, request: u64) {
        let Some(pos) = self.captures.iter().position(|c| c.request == request) else { return };
        if !self.captures[pos].items.iter().all(|i| i.finished) {
            return;
        }
        let mut c = self.captures.remove(pos);
        if let Some(t) = c.timer.take() {
            self.lh.remove(t);
        }
        let mut frames = Vec::new();
        let mut failed = Vec::new();
        for it in c.items.drain(..) {
            if let (true, Some(buf)) = (it.ok, it.buf.clone()) {
                let transform = Transform::from_wl(it.transform);
                let (uw, _) = snip_core::geometry::upright_size(buf.width as i32, buf.height as i32, transform);
                let scale = if it.logical.w > 0 { uw as f64 / it.logical.w as f64 } else { 1.0 };
                frames.push(Frame { output: it.key.clone(), buf, transform, scale, logical: it.logical, epoch: request });
            } else {
                failed.push(it.key.clone());
            }
            if let Some(f) = it.frame {
                f.destroy();
            }
            it.session.destroy();
            it.source.destroy();
        }
        let _ = self.conn.flush();
        let done = now_ns();
        trace::trace(
            "capture_complete",
            &format!("\"request\":{request},\"ms\":{:.2},\"frames\":{},\"failed\":{}", (done - c.t_start) as f64 / 1e6, frames.len(), failed.len()),
        );
        self.emit(Event::Captured(CaptureResult { request, frames, failed, t_start_ns: c.t_start, t_done_ns: done }));
    }

    // ------------------------------------------------------------ clipboard

    fn set_clipboard(&mut self, offers: Vec<(String, Arc<Vec<u8>>)>) {
        let (Some(mgr), Some(dev)) = (&self.dc_mgr, &self.dc_device) else {
            self.emit(Event::Error("clipboard unavailable (no ext-data-control)".into()));
            return;
        };
        let src = mgr.create_data_source(&self.qh, ());
        for (mime, _) in &offers {
            src.offer(mime.clone());
        }
        dev.set_selection(Some(&src));
        if let Some(old) = self.clip_source.replace(src) {
            old.destroy();
        }
        self.clip_offers = Arc::new(offers.into_iter().collect());
        let _ = self.conn.flush();
    }

    /// `snip edit --clipboard` (ANN-19). The source writes into a pipe that a
    /// helper thread drains, so the Wayland thread never waits on another
    /// client (or on itself, when the selection is Snip's own).
    fn read_clipboard(&mut self, mimes: Vec<String>, reply: std::sync::mpsc::Sender<Option<(String, Vec<u8>)>>) {
        let Some((offer, offered)) = &self.selection else {
            let _ = reply.send(None);
            return;
        };
        let offered = offered.lock().unwrap().clone();
        let Some(mime) = mimes.into_iter().find(|m| offered.contains(m)) else {
            let _ = reply.send(None);
            return;
        };
        let Ok((rd, wr)) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC) else {
            let _ = reply.send(None);
            return;
        };
        offer.receive(mime.clone(), wr.as_fd());
        let _ = self.conn.flush();
        drop(wr); // the compositor has its own copy; EOF comes when the source closes it
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let ok = std::fs::File::from(rd).read_to_end(&mut buf).is_ok();
            let _ = reply.send(ok.then_some((mime, buf)));
        });
    }
}

impl Item {
    fn new(key: String, logical: Rect, source: ExtImageCaptureSourceV1, session: ExtImageCopyCaptureSessionV1) -> Self {
        Self { key, logical, source, session, frame: None, buf: None, size: None, formats: vec![], transform: 0, finished: false, ok: false }
    }
}

fn now_ms() -> u32 {
    (now_ns() / 1_000_000) as u32
}

// ---------------------------------------------------------------- dispatch

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(st: &mut Self, _: &WlRegistry, ev: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            wl_registry::Event::Global { name, interface, version } if interface == WlOutput::interface().name => {
                st.bind_output(name, version);
            }
            wl_registry::Event::GlobalRemove { name } => {
                if let Some(i) = st.outputs.iter().position(|o| o.global == name) {
                    let o = st.outputs.remove(i);
                    st.pools.remove(&o.name);
                    if let Some(x) = o.xdg {
                        x.destroy();
                    }
                    if st.ready_sent {
                        st.emit(Event::OutputsChanged { outputs: st.output_infos() });
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, u32> for State {
    fn event(st: &mut Self, _: &WlOutput, ev: wl_output::Event, global: &u32, _: &Connection, _: &QueueHandle<Self>) {
        let Some(o) = st.outputs.iter_mut().find(|o| o.global == *global) else { return };
        match ev {
            wl_output::Event::Name { name } => o.name = name,
            wl_output::Event::Geometry { transform: WEnum::Value(t), .. } => o.transform = t as u32,
            wl_output::Event::Done => {
                if st.ready_sent {
                    st.emit(Event::OutputsChanged { outputs: st.output_infos() });
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<ZxdgOutputV1, u32> for State {
    fn event(st: &mut Self, _: &ZxdgOutputV1, ev: zxdg_output_v1::Event, global: &u32, _: &Connection, _: &QueueHandle<Self>) {
        let Some(o) = st.outputs.iter_mut().find(|o| o.global == *global) else { return };
        match ev {
            zxdg_output_v1::Event::LogicalPosition { x, y } => {
                o.logical.x = x;
                o.logical.y = y;
            }
            zxdg_output_v1::Event::LogicalSize { width, height } => {
                o.logical.w = width;
                o.logical.h = height;
            }
            _ => {}
        }
    }
}

macro_rules! ignore_events {
    ($($ty:ty),* $(,)?) => {$(
        impl Dispatch<$ty, ()> for State {
            fn event(_: &mut Self, _: &$ty, _: <$ty as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
        }
    )*};
}
impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn event(st: &mut Self, _: &ExtIdleNotificationV1, ev: ext_idle_notification_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            ext_idle_notification_v1::Event::Idled => st.emit(Event::Idle(true)),
            ext_idle_notification_v1::Event::Resumed => st.emit(Event::Idle(false)),
            _ => {}
        }
    }
}

ignore_events!(
    WlCompositor,
    WlSurface,
    ZwpIdleInhibitManagerV1,
    ZwpIdleInhibitorV1,
    ExtIdleNotifierV1,
    WlShm,
    WlShmPool,
    WlBuffer,
    WlSeat,
    ZxdgOutputManagerV1,
    ExtOutputImageCaptureSourceManagerV1,
    ExtForeignToplevelImageCaptureSourceManagerV1,
    ExtImageCaptureSourceV1,
    ExtImageCopyCaptureManagerV1,
    ExtDataControlManagerV1,
    HyprlandGlobalShortcutsManagerV1,
    HyprlandToplevelMappingManagerV1,
    ZwlrVirtualPointerManagerV1,
    ZwlrVirtualPointerV1,
);

type Key = (u64, String);

impl Dispatch<ExtImageCopyCaptureSessionV1, Key> for State {
    fn event(st: &mut Self, _: &ExtImageCopyCaptureSessionV1, ev: ext_image_copy_capture_session_v1::Event, key: &Key, _: &Connection, _: &QueueHandle<Self>) {
        use ext_image_copy_capture_session_v1::Event as E;
        let (request, k) = (key.0, key.1.as_str());
        match ev {
            E::BufferSize { width, height } => {
                if let Some(i) = st.item_mut(request, k) {
                    i.size = Some((width, height));
                }
            }
            E::ShmFormat { format: WEnum::Value(f) } => {
                if let Some(i) = st.item_mut(request, k) {
                    i.formats.push(f);
                }
            }
            E::Done => st.on_session_done(request, k),
            E::Stopped => {
                if let Some(i) = st.item_mut(request, k) {
                    i.finished = true;
                    i.ok = false;
                }
                st.check_complete(request);
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, Key> for State {
    fn event(st: &mut Self, _: &ExtImageCopyCaptureFrameV1, ev: ext_image_copy_capture_frame_v1::Event, key: &Key, _: &Connection, _: &QueueHandle<Self>) {
        use ext_image_copy_capture_frame_v1::Event as E;
        let (request, k) = (key.0, key.1.as_str());
        {
            let Some(i) = st.item_mut(request, k) else { return };
            match ev {
                E::Transform { transform: WEnum::Value(t) } => i.transform = t as u32,
                E::Ready => {
                    i.finished = true;
                    i.ok = true;
                }
                E::Failed { reason } => {
                    i.finished = true;
                    i.ok = false;
                    trace::warn("frame_failed", &format!("\"key\":{},\"reason\":\"{reason:?}\"", trace::js(k)));
                }
                _ => return,
            }
        }
        st.check_complete(request);
    }
}

impl Dispatch<HyprlandGlobalShortcutV1, String> for State {
    fn event(st: &mut Self, _: &HyprlandGlobalShortcutV1, ev: hyprland_global_shortcut_v1::Event, id: &String, _: &Connection, _: &QueueHandle<Self>) {
        let (pressed, hi, lo, ns) = match ev {
            hyprland_global_shortcut_v1::Event::Pressed { tv_sec_hi, tv_sec_lo, tv_nsec } => (true, tv_sec_hi, tv_sec_lo, tv_nsec),
            hyprland_global_shortcut_v1::Event::Released { tv_sec_hi, tv_sec_lo, tv_nsec } => (false, tv_sec_hi, tv_sec_lo, tv_nsec),
            _ => return,
        };
        // Hyprland stamps with steady_clock == CLOCK_MONOTONIC.
        let t_ns = ((hi as u64) << 32 | lo as u64) * 1_000_000_000 + ns as u64;
        st.emit(Event::Shortcut { id: id.clone(), pressed, t_ns });
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(st: &mut Self, _: &ExtForeignToplevelListV1, ev: ext_foreign_toplevel_list_v1::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>) {
        if let ext_foreign_toplevel_list_v1::Event::Toplevel { toplevel } = ev {
            let id = toplevel.id().protocol_id();
            if let Some(m) = &st.mapping {
                m.get_window_for_toplevel(&toplevel, qh, id);
            }
            st.toplevels.insert(id, Toplevel { handle: toplevel, address: None });
        }
    }

    event_created_child!(State, ExtForeignToplevelListV1, [
        ext_foreign_toplevel_list_v1::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(st: &mut Self, h: &ExtForeignToplevelHandleV1, ev: ext_foreign_toplevel_handle_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let ext_foreign_toplevel_handle_v1::Event::Closed = ev {
            st.toplevels.remove(&h.id().protocol_id());
            h.destroy();
        }
    }
}

impl Dispatch<HyprlandToplevelWindowMappingHandleV1, u32> for State {
    fn event(st: &mut Self, h: &HyprlandToplevelWindowMappingHandleV1, ev: hyprland_toplevel_window_mapping_handle_v1::Event, id: &u32, _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            hyprland_toplevel_window_mapping_handle_v1::Event::WindowAddress { address_hi, address } => {
                if let Some(t) = st.toplevels.get_mut(id) {
                    t.address = Some((address_hi as u64) << 32 | address as u64);
                }
                h.destroy();
            }
            hyprland_toplevel_window_mapping_handle_v1::Event::Failed => h.destroy(),
            _ => {}
        }
    }
}

/// MIME types an offer announced, filled in before its Selection event.
type Offered = Arc<Mutex<Vec<String>>>;

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(st: &mut Self, _: &ExtDataControlDeviceV1, ev: ext_data_control_device_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        // Keep only the current selection's offer (read on request); the
        // primary selection is never read.
        match ev {
            ext_data_control_device_v1::Event::Selection { id } => {
                if let Some((old, _)) = st.selection.take() {
                    if Some(&old) != id.as_ref() {
                        old.destroy();
                    }
                }
                st.selection = id.and_then(|o| o.data::<Offered>().cloned().map(|m| (o, m)));
            }
            ext_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } => offer.destroy(),
            _ => {}
        }
    }

    event_created_child!(State, ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, Offered::default()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, Offered> for State {
    fn event(_: &mut Self, _: &ExtDataControlOfferV1, ev: <ExtDataControlOfferV1 as Proxy>::Event, mimes: &Offered, _: &Connection, _: &QueueHandle<Self>) {
        if let wayland_protocols::ext::data_control::v1::client::ext_data_control_offer_v1::Event::Offer { mime_type } = ev {
            mimes.lock().unwrap().push(mime_type);
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for State {
    fn event(st: &mut Self, src: &ExtDataControlSourceV1, ev: ext_data_control_source_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            ext_data_control_source_v1::Event::Send { mime_type, fd } => {
                let Some(bytes) = st.clip_offers.get(&mime_type).cloned() else { return };
                // Several readers ask at once (Omarchy's history watchers + the
                // target app); never block the Wayland thread on any of them.
                std::thread::spawn(move || {
                    let _ = std::fs::File::from(fd).write_all(&bytes);
                });
            }
            ext_data_control_source_v1::Event::Cancelled => {
                let ours = st.clip_source.as_ref() == Some(src);
                src.destroy();
                if ours {
                    st.clip_source = None;
                    st.emit(Event::ClipboardLost);
                }
            }
            _ => {}
        }
    }
}
