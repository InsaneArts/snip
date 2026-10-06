//! "Connection A" (docs/02-architecture.md §3): Snip's own Wayland
//! connection on its own thread. Everything that needs no UI surface lives
//! here: capture (ext-image-copy-capture, outputs and toplevels), clipboard
//! ownership (ext-data-control), global shortcuts (hyprland), virtual pointer
//! (wlr), toplevel <-> Hyprland window mapping, and output hot-plug.
//!
//! The UI talks to it through [`Handle::send`] and receives [`Event`]s on a
//! callback that runs on the Wayland thread (forward them to the UI thread).

mod state;

use std::sync::Arc;

pub use snip_core::frame::Frame;
pub use snip_core::geometry::Rect;

#[derive(Clone, Debug, PartialEq)]
pub struct OutputInfo {
    pub name: String,
    /// Logical position and size (xdg-output)
    pub logical: Rect,
    pub transform: u32,
}

#[derive(Clone, Debug)]
pub enum Source {
    /// All outputs, or only the named ones
    Outputs(Option<Vec<String>>),
    /// A Hyprland window by address (`hyprctl clients` "address")
    Window(u64),
}

#[derive(Clone)]
pub struct CaptureResult {
    pub request: u64,
    pub frames: Vec<Frame>,
    /// Outputs (or "window") that failed or timed out
    pub failed: Vec<String>,
    pub t_start_ns: u64,
    pub t_done_ns: u64,
}

pub enum Event {
    Ready { outputs: Vec<OutputInfo>, features: Features },
    OutputsChanged { outputs: Vec<OutputInfo> },
    Shortcut { id: String, pressed: bool, t_ns: u64 },
    Captured(CaptureResult),
    /// Another client took the clipboard
    ClipboardLost,
    Error(String),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Features {
    pub data_control: bool,
    pub global_shortcuts: bool,
    pub virtual_pointer: bool,
    pub toplevel_capture: bool,
}

pub enum Request {
    Capture { request: u64, source: Source, cursor: bool },
    /// Offer these MIME types; replaces any current selection.
    SetClipboard { offers: Vec<(String, Arc<Vec<u8>>)> },
    RegisterShortcut { id: String, description: String },
    PointerMove { output: String, x: u32, y: u32, w: u32, h: u32 },
    /// Linux button code: 0x110 left, 0x111 right, 0x112 middle
    PointerButton { down: bool, button: u32 },
    PointerAxis { dy: f64 },
}

pub type Sink = Box<dyn Fn(Event) + Send>;

#[derive(Clone)]
pub struct Handle {
    tx: calloop::channel::Sender<Request>,
}

impl Handle {
    pub fn send(&self, r: Request) {
        let _ = self.tx.send(r);
    }
}

/// Start the Wayland thread. `app_id` scopes global shortcuts
/// (`hl.dsp.global("<app_id>:<id>")`).
pub fn spawn(app_id: &str, sink: Sink) -> Handle {
    let (tx, rx) = calloop::channel::channel::<Request>();
    let app_id = app_id.to_owned();
    std::thread::Builder::new()
        .name("snip-wayland".into())
        .spawn(move || {
            if let Err(e) = state::run(app_id, rx, &sink) {
                sink(Event::Error(e.to_string()));
            }
        })
        .expect("spawn wayland thread");
    Handle { tx }
}
