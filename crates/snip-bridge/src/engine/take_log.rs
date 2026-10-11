//! A take's input log, written as it records (the design's §4.2, §4.7; D54:
//! every take keeps where the pointer went and its clicks): the pointer as
//! Hyprland has it, polled; the hooks' presses and releases (and a studio
//! take's command keys and typing); the take's pauses. Beside the take's
//! video until it goes into history with it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::time::Duration;

use snip_core::studio::log::{Event, Header, Writer};
use snip_core::take::Take;
use snip_core::trace;

/// CLOCK_MONOTONIC, µs: gsr's clock, and the log's.
pub(super) fn now_us() -> i64 {
    (trace::now_ns() / 1000) as i64
}

pub(super) struct TakeLog {
    tx: mpsc::Sender<Event>,
    paused: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
    writer: JoinHandle<Option<PathBuf>>,
}

impl TakeLog {
    /// Log `take` into `path`, the pointer polled `hz` times a second.
    /// `header` comes without gsr's first frame: the log is written from
    /// then on (what happens before waits for it); a test pattern take
    /// ("fake") starts with its take.
    pub(super) fn start(path: PathBuf, mut header: Header, take: Arc<Take>, hz: f64) -> TakeLog {
        let (tx, rx) = mpsc::channel();
        let paused = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(false));
        let started = now_us();
        let (poll_tx, poll_paused, poll_done) = (tx.clone(), paused.clone(), done.clone());
        std::thread::spawn(move || {
            let mut last = None;
            while !poll_done.load(Ordering::Relaxed) {
                if !poll_paused.load(Ordering::Relaxed) {
                    let t0 = now_us();
                    // only a change: an idle pointer costs nothing on disk
                    if let Some(at) = snip_hypr::cursor_pos().filter(|&at| last != Some(at)) {
                        last = Some(at);
                        let _ = poll_tx.send(Event::Move { t: (t0 + now_us()) / 2, at: [at.0, at.1] });
                    }
                }
                std::thread::sleep(Duration::from_secs_f64(1.0 / hz));
            }
        });
        let (wait_done, wait_take) = (done.clone(), take);
        let writer = std::thread::spawn(move || {
            while !wait_take.rolling() {
                if wait_done.load(Ordering::Relaxed) {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            header.gsr_ts = if header.encoder == "fake" { Some([started, 0]) } else { first_frame(wait_take.file()) };
            let mut log = Writer::new(std::fs::File::create(&path).ok()?, &header).ok()?;
            // until the take is over and the poller has stopped
            for e in rx {
                if log.push(&e).is_err() {
                    break;
                }
            }
            log.into_inner().ok()?;
            Some(path)
        });
        TakeLog { tx, paused, done, writer }
    }

    pub(super) fn push(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    /// Paused (gsr leaves it out of the video), or going on.
    pub(super) fn pause(&self, on: bool) {
        self.paused.store(on, Ordering::Relaxed);
        self.push(Event::Pause { t: now_us(), paused: on });
    }

    /// The take is over: the log's file, all of it written; None when the
    /// take stopped before its first frame.
    pub(super) fn finish(self) -> Option<PathBuf> {
        self.done.store(true, Ordering::Relaxed);
        drop(self.tx);
        self.writer.join().ok().flatten()
    }
}

/// gsr's first frame reaching its muxer (`-write-first-frame-ts`, beside
/// the take as `<file>.ts`): its monotonic and real time, µs.
fn first_frame(take: &Path) -> Option<[i64; 2]> {
    let text = std::fs::read_to_string(format!("{}.ts", take.display())).ok()?;
    let mut n = text.split_whitespace().filter_map(|v| v.parse().ok());
    Some([n.next()?, n.next().unwrap_or(0)])
}
