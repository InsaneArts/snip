//! Scrolling capture (SCR-01…03): between the chosen area and the stitched
//! capture. Frames come from the window's own buffer when the area lies in
//! one window (no translucency, and never Snip's frame and panel), else from
//! the output; each goes through Snip's stitcher on the session's thread and
//! the panel shows the image growing. Enter and Esc are bound in Hyprland for
//! the session's length (the page under the frame keeps the keyboard).

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use serde_json::json;
use snip_core::encode;
use snip_core::geometry::Rect;
use snip_core::stitch::{Step, Stitcher};
use snip_core::store::CaptureMeta;
use snip_core::trace::{self, js};
use snip_hypr::Client;
use snip_wayland::{Request as Wl, Source};

use super::{cancelled, reply, Inner, Pending, Waiter};
use cxx_qt_lib::QString;

const READY: u8 = 0;
const RUNNING: u8 = 1;
const DONE: u8 = 2;
const CANCEL: u8 = 3;

pub(super) struct Scroll {
    output: String,
    source: Source,
    /// the window when frames come from its buffer: the area is cut out of it
    window: Option<Client>,
    /// the area: physical px on the output, and global logical px
    area: Rect,
    logical: (f64, f64, f64, f64),
    waiter: Option<Waiter>,
    app: Option<String>,
    title: Option<String>,
    state: Arc<AtomicU8>,
}

impl Inner {
    /// The area is chosen: show the frame and panel, ready to start.
    pub(super) fn scroll_open(self: &Arc<Self>, output: String, rect: Rect, scale: f64, waiter: Option<Waiter>, clients: &[Client]) {
        let o = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| o.logical).unwrap_or_default();
        let s = if scale > 0.0 { scale } else { 1.0 };
        let logical = (o.x as f64 + rect.x as f64 / s, o.y as f64 + rect.y as f64 / s, rect.w as f64 / s, rect.h as f64 / s);
        // the smallest window holding the whole area gives its own buffer
        let inside = |c: &&Client| {
            c.x as f64 <= logical.0 + 0.5 && c.y as f64 <= logical.1 + 0.5 && (c.x + c.w) as f64 >= logical.0 + logical.2 - 0.5 && (c.y + c.h) as f64 >= logical.1 + logical.3 - 0.5
        };
        let window = clients.iter().filter(inside).min_by_key(|c| c.w as i64 * c.h as i64).cloned().filter(|_| self.features.lock().unwrap().toplevel_capture);
        let source = match &window {
            Some(c) => Source::Window(c.address),
            None => Source::Outputs(Some(vec![output.clone()])),
        };
        let (app, title) = self.window_meta_at(&output, rect, s);
        trace::trace("scroll_open", &format!("\"output\":{},\"rect\":[{},{},{},{}],\"window\":{}", js(&output), rect.x, rect.y, rect.w, rect.h, window.is_some()));
        let state = Arc::new(AtomicU8::new(READY));
        if let Some(old) = self.scroll.lock().unwrap().replace(Scroll { output: output.clone(), source, window, area: rect, logical, waiter, app, title, state }) {
            old.state.store(CANCEL, Ordering::Relaxed);
            reply(old.waiter, cancelled());
        }
        let _ = snip_hypr::eval(&snip_hypr::keys::session_lua(&crate::bridge::app_id(), true));
        let j = json!({ "output": output, "x": (rect.x as f64 / s).round(), "y": (rect.y as f64 / s).round(), "w": (rect.w as f64 / s).round(), "h": (rect.h as f64 / s).round() }).to_string();
        self.ui(move |e| e.scroll_open(&QString::from(j.as_str())));
    }

    /// Start Capture: frames until Done or Cancel.
    pub fn scroll_start(self: &Arc<Self>) {
        let state = match self.scroll.lock().unwrap().as_ref() {
            Some(s) if s.state.compare_exchange(READY, RUNNING, Ordering::Relaxed, Ordering::Relaxed).is_ok() => s.state.clone(),
            _ => return,
        };
        let me = self.clone();
        std::thread::Builder::new().name("snip-scroll".into()).spawn(move || me.scroll_run(state)).ok();
    }

    /// Enter: Start Capture when ready, else Done.
    pub fn scroll_go(self: &Arc<Self>) {
        let ready = self.scroll.lock().unwrap().as_ref().map(|s| s.state.load(Ordering::Relaxed) == READY);
        match ready {
            Some(true) => self.scroll_start(),
            Some(false) => self.scroll_done(),
            None => {}
        }
    }

    pub fn scroll_done(&self) {
        if let Some(s) = self.scroll.lock().unwrap().as_ref() {
            let _ = s.state.compare_exchange(RUNNING, DONE, Ordering::Relaxed, Ordering::Relaxed);
        }
    }

    pub fn scroll_cancel(&self) {
        let ready = {
            let g = self.scroll.lock().unwrap();
            let Some(s) = g.as_ref() else { return };
            s.state.swap(CANCEL, Ordering::Relaxed) == READY
        };
        // a running session ends on its own thread; one not started ends here
        if ready {
            if let Some(s) = self.scroll_close() {
                reply(s.waiter, cancelled());
            }
        }
    }

    fn scroll_close(&self) -> Option<Scroll> {
        let s = self.scroll.lock().unwrap().take();
        let _ = snip_hypr::eval(&snip_hypr::keys::session_lua("", false));
        self.ui(|e| e.scroll_close());
        s
    }

    /// One frame of the area, as RGBA.
    fn scroll_frame(&self, source: &Source, window: Option<&Client>, area: Rect, logical: (f64, f64, f64, f64)) -> Option<(Vec<u8>, u32, u32)> {
        let req = self.next();
        let (tx, rx) = mpsc::channel();
        self.pending.lock().unwrap().insert(req, Pending::Scroll { tx });
        self.wl(Wl::Capture { request: req, source: source.clone(), cursor: false });
        let Ok(r) = rx.recv_timeout(Duration::from_secs(2)) else {
            self.pending.lock().unwrap().remove(&req);
            return None;
        };
        let f = r.frames.into_iter().next()?;
        let crop = match window {
            // the area in the window's buffer (its scale may differ from the output's)
            Some(c) => {
                let (bw, _) = f.upright_size();
                let k = if c.w > 0 { bw as f64 / c.w as f64 } else { 1.0 };
                Rect::new(
                    ((logical.0 - c.x as f64) * k).round() as i32,
                    ((logical.1 - c.y as f64) * k).round() as i32,
                    (logical.2 * k).round() as i32,
                    (logical.3 * k).round() as i32,
                )
            }
            None => area,
        };
        let (data, w, h, alpha) = encode::crop_upright(&f, crop);
        let rgba = if alpha { data } else { data.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
        Some((rgba, w, h))
    }

    fn scroll_run(self: Arc<Self>, state: Arc<AtomicU8>) {
        let (source, window, area, logical) = match self.scroll.lock().unwrap().as_ref() {
            Some(s) => (s.source.clone(), s.window.clone(), s.area, s.logical),
            None => return,
        };
        let mut stitcher: Option<Stitcher> = None;
        let mut size = (0, 0);
        let mut last_preview = Instant::now() - Duration::from_secs(1);
        let mut preview_n = 0u32;
        let (mut frames, mut failed) = (0u32, 0u32);
        loop {
            let t0 = Instant::now();
            let s = state.load(Ordering::Relaxed);
            if s == CANCEL || (s == DONE && (stitcher.is_some() || failed > 2)) {
                break;
            }
            let Some((rgba, w, h)) = self.scroll_frame(&source, window.as_ref(), area, logical) else {
                failed += 1;
                continue;
            };
            let st = stitcher.get_or_insert_with(|| {
                size = (w, h);
                Stitcher::new(w, h)
            });
            if (w, h) != size {
                continue; // the window was resized: that frame can't be matched
            }
            let step = st.push(&rgba);
            frames += 1;
            let name = match step {
                Step::First | Step::Moved { .. } => "moved",
                Step::Still => "still",
                Step::Unsure => "unsure",
                Step::Lost => "lost",
            };
            let mut j = json!({ "phase": "capturing", "height": st.height(), "step": name });
            if last_preview.elapsed() > Duration::from_millis(300) && !matches!(step, Step::Still) {
                last_preview = Instant::now();
                preview_n += 1;
                if let Some(p) = scroll_preview(st, preview_n) {
                    j["preview"] = json!(format!("file://{}", p.display()));
                }
            }
            let j = j.to_string();
            self.ui(move |e| e.scroll_status(&QString::from(j.as_str())));
            // about 12 frames a second: scrolling by hand is slower than that
            std::thread::sleep(Duration::from_millis(80).saturating_sub(t0.elapsed()));
        }
        let cancelled_ = state.load(Ordering::Relaxed) == CANCEL;
        let Some(s) = self.scroll_close() else { return };
        let _ = std::fs::remove_file(preview_path(preview_n));
        trace::trace("scroll_end", &format!("\"frames\":{frames},\"height\":{},\"cancelled\":{cancelled_}", stitcher.as_ref().map_or(0, |s| s.height())));
        let Some(st) = stitcher.filter(|_| !cancelled_) else {
            return reply(s.waiter, cancelled());
        };
        self.play("shutter");
        let (w, h, rgba) = st.image();
        let alpha = rgba.chunks_exact(4).any(|p| p[3] != 255);
        let data = if alpha { rgba } else { rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect() };
        let png = encode::encode_png(&data, w, h, alpha);
        let meta = CaptureMeta { kind: "scroll".into(), output: s.output.clone(), app: s.app, title: s.title };
        self.ingest(png, w, h, meta, &s.output, s.waiter);
    }
}

fn preview_path(n: u32) -> std::path::PathBuf {
    snip_core::paths::runtime_dir().join(format!("scroll-preview-{n}.png"))
}

/// The panel's picture: the bottom of the image so far, small. A new file
/// each time (QML caches by name); the last one goes.
fn scroll_preview(st: &Stitcher, n: u32) -> Option<std::path::PathBuf> {
    let (w, h, rgba) = st.image();
    let rows = h.min(w * 2);
    let tail = &rgba[((h - rows) * w * 4) as usize..];
    let nw = w.min(440);
    let nh = (rows as u64 * nw as u64 / w as u64).max(1) as u32;
    let small = encode::resize(tail, w, rows, 4, nw, nh);
    let p = preview_path(n);
    std::fs::write(&p, encode::encode_png(&small, nw, nh, true)).ok()?;
    let _ = std::fs::remove_file(preview_path(n.wrapping_sub(1)));
    Some(p)
}
