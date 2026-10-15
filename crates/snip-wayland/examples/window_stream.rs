//! Spike S7 (D68): how fast does Hyprland hand Snip one window's own frames
//! (its toplevel capture, never covered), and can they go to an encoder at
//! 60 fps? Captures the window at Hyprland address `ADDR` (hex) as fast as
//! frames come for `SECONDS`, and with `--encode` pipes them into ffmpeg's
//! NVENC (BGRA in, nothing written: `-f null`), so no frame is kept.
//!
//!   cargo run --release -p snip-wayland --example window_stream -- ADDR [SECONDS] [--encode]
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use snip_wayland::{Event, Request, Source};

/// CPU time used by this process so far (user + system), in seconds.
fn cpu_self() -> f64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    // after the ")" of the command name: fields 14 and 15 (utime, stime) in ticks
    let rest = stat.rsplit(')').next().unwrap_or("");
    let f: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (f.get(11).unwrap_or(&0.0) + f.get(12).unwrap_or(&0.0)) / 100.0
}

/// CPU time a child used (user + system), in seconds, while it runs.
fn cpu_of(pid: u32) -> f64 {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
    let rest = stat.rsplit(')').next().unwrap_or("");
    let f: Vec<f64> = rest.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    (f.get(11).unwrap_or(&0.0) + f.get(12).unwrap_or(&0.0)) / 100.0
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let addr = u64::from_str_radix(args.get(1).expect("a window's address (hex)").trim_start_matches("0x"), 16).expect("hex address");
    let secs: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10.0);
    let encode = args.iter().any(|a| a == "--encode");

    let (tx, rx) = mpsc::channel();
    let h = snip_wayland::spawn("snip-s7", Box::new(move |ev| {
        let _ = tx.send(ev);
    }));
    let next = |rx: &mpsc::Receiver<Event>| loop {
        match rx.recv_timeout(Duration::from_secs(3)).expect("no answer from the compositor") {
            Event::Captured(r) => return r,
            Event::Error(e) => panic!("{e}"),
            _ => {}
        }
    };

    let mut ffmpeg = None;
    let (mut frames, mut ms, mut size) = (0u32, Vec::new(), (0, 0));
    let (t0, cpu0) = (Instant::now(), cpu_self());
    let mut req = 0;
    while t0.elapsed().as_secs_f64() < secs {
        req += 1;
        h.send(Request::Capture { request: req, source: Source::Window(addr), cursor: false });
        let r = next(&rx);
        let Some(f) = r.frames.first() else { panic!("no frame: {:?}", r.failed) };
        ms.push((r.t_done_ns - r.t_start_ns) as f64 / 1e6);
        let (w, hgt, stride) = (f.buf.width as usize, f.buf.height as usize, f.buf.stride as usize);
        size = (w, hgt);
        if encode {
            let ff = ffmpeg.get_or_insert_with(|| {
                Command::new("ffmpeg")
                    .args(["-v", "error", "-f", "rawvideo", "-pix_fmt", "bgra", "-s", &format!("{w}x{hgt}"), "-r", "60", "-i", "-"])
                    .args(["-c:v", "h264_nvenc", "-preset", "p4", "-f", "null", "-"])
                    .stdin(Stdio::piped())
                    .spawn()
                    .expect("ffmpeg")
            });
            let stdin = ff.stdin.as_mut().unwrap();
            let bytes = f.buf.bytes();
            for row in 0..hgt {
                stdin.write_all(&bytes[row * stride..row * stride + w * 4]).expect("ffmpeg's stdin");
            }
        }
        frames += 1;
    }
    let wall = t0.elapsed().as_secs_f64();
    let cpu = cpu_self() - cpu0;
    let ff_cpu = ffmpeg.as_ref().map(|c| cpu_of(c.id())).unwrap_or(0.0);
    if let Some(mut c) = ffmpeg {
        drop(c.stdin.take());
        let _ = c.wait();
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |p: f64| ms[((ms.len() - 1) as f64 * p) as usize];
    println!(
        "{}x{}: {frames} frames in {wall:.1} s = {:.1} fps; capture ms p50 {:.1}, p95 {:.1}, max {:.1}; CPU: probe {:.0} %, ffmpeg {:.0} % of a core",
        size.0,
        size.1,
        frames as f64 / wall,
        pct(0.5),
        pct(0.95),
        pct(1.0),
        100.0 * cpu / wall,
        100.0 * ff_cpu / wall
    );
}
