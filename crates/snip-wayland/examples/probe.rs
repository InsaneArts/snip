//! Headless check of the Wayland service: outputs, an all-outputs capture,
//! and a window capture by Hyprland address (argv[1], hex, optional).
use std::sync::mpsc;
use std::time::Duration;

use snip_wayland::{Event, Request, Source};

fn main() {
    snip_core::trace::init();
    let (tx, rx) = mpsc::channel();
    let h = snip_wayland::spawn("snip-probe", Box::new(move |ev| {
        let _ = tx.send(ev);
    }));
    let wait = |rx: &mpsc::Receiver<Event>| loop {
        match rx.recv_timeout(Duration::from_secs(3)).expect("timeout") {
            Event::Ready { outputs, features } => {
                println!("ready: features {features:?}");
                for o in outputs {
                    println!("  output {} logical {:?} transform {}", o.name, o.logical, o.transform);
                }
            }
            Event::Captured(r) => return r,
            Event::Error(e) => panic!("{e}"),
            _ => {}
        }
    };
    h.send(Request::Capture { request: 1, source: Source::Outputs(None), cursor: false });
    let r = wait(&rx);
    println!("outputs capture: {:.1} ms, failed {:?}", (r.t_done_ns - r.t_start_ns) as f64 / 1e6, r.failed);
    for f in &r.frames {
        println!("  {} {}x{} {:?} scale {:.3} transform {:?}", f.output, f.buf.width, f.buf.height, f.buf.format, f.scale, f.transform);
    }
    if let Some(addr) = std::env::args().nth(1) {
        let a = u64::from_str_radix(addr.trim_start_matches("0x"), 16).expect("hex address");
        h.send(Request::Capture { request: 2, source: Source::Window(a), cursor: false });
        let r = wait(&rx);
        println!("window capture: {:.1} ms, failed {:?}", (r.t_done_ns - r.t_start_ns) as f64 / 1e6, r.failed);
        for f in &r.frames {
            let alpha = f.buf.bytes().chunks_exact(4).filter(|p| p[3] != 255).count();
            println!("  {} {}x{} {:?} transparent_px {}", f.output, f.buf.width, f.buf.height, f.buf.format, alpha);
            let e = snip_core::encode::crop_and_encode(f, snip_core::geometry::Rect::new(0, 0, f.buf.width as i32, f.buf.height as i32));
            std::fs::write("/tmp/claude-1000/snip-probe-window.png", &e.png).unwrap();
            println!("  encoded {}x{} in {:.1} ms -> /tmp/claude-1000/snip-probe-window.png", e.width, e.height, e.encode_ms);
        }
    }
}
