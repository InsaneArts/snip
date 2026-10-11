//! The microphone's live level for the recording panel and its pill (D53):
//! how loud the chosen microphone is right now, as the mic cell's fill shows
//! it, and whether its source is muted. PipeWire measures the peaks itself
//! (`pw-record`, 30 a second), so nothing is recorded. Tests hear a made-up
//! voice and never open a microphone.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::take::Recorder;
use crate::trace;

/// What the meter reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reading {
    /// 0–1, as the fill shows it
    Level(f32),
    /// the source's mute switch: first, then when it changes
    Muted(bool),
}

/// Listening to one microphone, until dropped.
pub struct Meter {
    stop: Arc<AtomicBool>,
    /// pw-record, while it runs
    child: Arc<Mutex<Option<Child>>>,
}

impl Meter {
    /// Listen to `mic` (gsr's name: "default_input" or a PipeWire node):
    /// the microphone itself, or the test voice. `on` gets each reading, on
    /// the meter's own thread.
    pub fn start(mic: &str, recorder: Recorder, mut on: impl FnMut(Reading) + Send + 'static) -> Meter {
        let stop = Arc::new(AtomicBool::new(false));
        let child = Arc::new(Mutex::new(None));
        let (stopped, slot, mic) = (stop.clone(), child.clone(), mic.to_string());
        let _ = std::thread::Builder::new().name("snip-meter".into()).spawn(move || match recorder {
            Recorder::Gsr => listen(&mic, &stopped, &slot, &mut on),
            Recorder::TestPattern => {
                let (t0, mut fill) = (Instant::now(), Fill::default());
                loop {
                    std::thread::sleep(Duration::from_millis(33));
                    if stopped.load(Ordering::Relaxed) {
                        return;
                    }
                    if let Some(level) = fill.feed(voice(t0.elapsed().as_secs_f32())) {
                        on(Reading::Level(level));
                    }
                }
            }
        });
        Meter { stop, child }
    }
}

impl Drop for Meter {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        end(&self.child);
    }
}

/// pw-record killed and reaped, if it runs.
fn end(slot: &Mutex<Option<Child>>) {
    if let Some(mut c) = slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = c.kill();
        let _ = c.wait();
    }
}

/// The microphone's peaks from pw-record (stdbuf: unbuffered, or they come
/// in 128 ms bursts), and its mute switch each second. A microphone that's
/// gone reads empty and is tried again each second: plugged back in, it
/// comes back.
fn listen(mic: &str, stop: &AtomicBool, slot: &Mutex<Option<Child>>, on: &mut impl FnMut(Reading)) {
    let source = if mic == "default_input" { "@DEFAULT_SOURCE@" } else { mic };
    let mut was_muted = None;
    while !stop.load(Ordering::Relaxed) {
        let spawned = Command::new("stdbuf")
            .args(["-o0", "pw-record"])
            .args(pw_record_args(mic))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => return trace::warn("meter", &format!("\"msg\":{}", trace::js(&e.to_string()))),
        };
        let Some(mut peaks) = child.stdout.take() else { return };
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        // dropped while it started: Drop found nothing to end
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let (mut fill, mut checked, mut block) = (Fill::default(), None::<Instant>, [0u8; 4]);
        while peaks.read_exact(&mut block).is_ok() && !stop.load(Ordering::Relaxed) {
            if let Some(level) = fill.feed(f32::from_le_bytes(block)) {
                on(Reading::Level(level));
            }
            if checked.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
                checked = Some(Instant::now());
                let answer = Command::new("pactl").args(["get-source-mute", source]).env("LC_ALL", "C").stdin(Stdio::null()).stderr(Stdio::null()).output();
                let now = answer.ok().and_then(|o| muted(&String::from_utf8_lossy(&o.stdout)));
                if now.is_some() && now != was_muted {
                    was_muted = now;
                    on(Reading::Muted(now.unwrap_or_default()));
                }
            }
        }
        end(slot);
        if fill.showing() && !stop.load(Ordering::Relaxed) {
            on(Reading::Level(0.0));
        }
        for _ in 0..20 {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    end(slot);
}

/// The fill as drawn, sent only when it moves a step (1/64 of the cell, under a
/// pixel of it): silence decayed by 12 % a block without ever reaching zero,
/// so the full-screen recording HUD redrew 30 times a second for a whole take.
#[derive(Default)]
struct Fill {
    level: f32,
    sent: Option<u32>,
}

impl Fill {
    /// The level to show for a block whose loudest sample is `peak`, when it
    /// moved a step.
    fn feed(&mut self, peak: f32) -> Option<f32> {
        self.level = shown(self.level, peak);
        let step = (self.level * 64.0).round() as u32;
        if self.sent == Some(step) {
            return None;
        }
        self.sent = Some(step);
        Some(step as f32 / 64.0)
    }

    fn showing(&self) -> bool {
        self.sent.is_some_and(|s| s > 0)
    }
}

/// The fill for a block whose loudest sample is `peak` (0–1), after `prev`:
/// its level over the last 50 dB (Talkify's scale: speech fills the upper
/// half, a quiet room stays near empty); at once when louder, 12 % less a
/// block when quieter, so it falls over about half a second.
fn shown(prev: f32, peak: f32) -> f32 {
    let db = 20.0 * peak.max(f32::MIN_POSITIVE).log10();
    ((db + 50.0) / 50.0).clamp(0.0, 1.0).max(prev * 0.88)
}

/// `pw-record`'s arguments for `mic`: PipeWire's own peaks, 30 a second,
/// as raw f32 on stdout. A named microphone that's gone ends it, rather
/// than the default one standing in under its name; "default_input" (gsr's
/// name) follows the default source, as gsr does.
fn pw_record_args(mic: &str) -> Vec<String> {
    let mut args: Vec<String> = ["--raw", "--format", "f32", "--rate", "30", "--channels", "1", "-P"].map(String::from).into();
    if mic == "default_input" {
        args.push(r#"{ "resample.peaks": true, "channelmix.normalize": true }"#.into());
    } else {
        args.push(r#"{ "resample.peaks": true, "channelmix.normalize": true, "node.dont-fallback": true }"#.into());
        args.extend(["--target".into(), mic.into()]);
    }
    args.push("-".into());
    args
}

/// `pactl get-source-mute`'s answer: muted or not; None when there's no
/// such source.
fn muted(answer: &str) -> Option<bool> {
    match answer.trim().strip_prefix("Mute: ")? {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// The test voice's peaks: half a second of speech, then half a second's
/// pause.
fn voice(t: f32) -> f32 {
    if t % 1.0 < 0.5 {
        0.05 + 0.65 * (t * 5.0 * std::f32::consts::TAU).sin().abs()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// The levels heard for `d`.
    fn levels(rx: &mpsc::Receiver<Reading>, d: Duration) -> Vec<f32> {
        let t0 = Instant::now();
        let mut out = vec![];
        while let Some(left) = d.checked_sub(t0.elapsed()) {
            match rx.recv_timeout(left) {
                Ok(Reading::Level(l)) => out.push(l),
                Ok(Reading::Muted(_)) => {}
                Err(_) => break,
            }
        }
        out
    }

    #[test]
    fn the_test_voice_moves_the_level() {
        let (tx, rx) = mpsc::channel();
        let _meter = Meter::start("default_input", Recorder::TestPattern, move |r| {
            let _ = tx.send(r);
        });
        let heard = levels(&rx, Duration::from_millis(1200));
        assert!(heard.len() >= 20, "about 30 a second: {heard:?}");
        let (lo, hi) = heard.iter().fold((1f32, 0f32), |(lo, hi), &l| (lo.min(l), hi.max(l)));
        assert!(hi > 0.6 && lo < 0.3, "it speaks, then pauses: {lo}..{hi}");
    }

    #[test]
    fn nothing_is_heard_once_it_is_dropped() {
        let (tx, rx) = mpsc::channel();
        let meter = Meter::start("default_input", Recorder::TestPattern, move |r| {
            let _ = tx.send(r);
        });
        assert!(rx.recv_timeout(Duration::from_secs(1)).is_ok(), "it listens");
        drop(meter);
        std::thread::sleep(Duration::from_millis(50)); // one reading may be on its way
        while rx.try_recv().is_ok() {}
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "and stops once dropped");
    }

    #[test]
    fn silence_empties_the_fill_once_then_sends_nothing() {
        let mut fill = Fill::default();
        assert!(fill.feed(0.5).is_some_and(|l| l > 0.8), "a loud block shows");
        let sent: Vec<f32> = (0..300).filter_map(|_| fill.feed(0.0)).collect();
        assert_eq!(sent.last(), Some(&0.0), "it empties: {sent:?}");
        assert!(sent.len() < 40, "falling, a step at a time: {sent:?}");
        assert_eq!((0..300).filter_map(|_| fill.feed(0.0)).count(), 0, "then nothing, however long the silence");
    }

    #[test]
    fn a_steady_voice_is_sent_once() {
        let mut fill = Fill::default();
        assert!(fill.feed(0.1).is_some());
        assert_eq!((0..30).filter_map(|_| fill.feed(0.1)).count(), 0, "the same fill: nothing to redraw");
    }

    #[test]
    fn a_peak_fills_the_last_50_db_rising_at_once_and_falling_slowly() {
        assert_eq!(shown(0.0, 1.0), 1.0, "0 dB: full");
        assert!((shown(0.0, 0.056_234) - 0.5).abs() < 1e-3, "-25 dB: half");
        assert!(shown(0.0, 0.003_163) < 1e-3, "-50 dB: empty");
        assert_eq!(shown(0.0, 0.0), 0.0, "digital silence");
        assert_eq!(shown(0.2, 1.0), 1.0, "louder: at once");
        assert!((shown(1.0, 0.0) - 0.88).abs() < 1e-6, "quieter: 12 % a block");
    }

    #[test]
    fn a_named_mic_never_falls_back_to_another_and_the_default_follows_the_default() {
        assert_eq!(
            pw_record_args("alsa_input.usb-Sony_INZONE_H5-00.mono-fallback"),
            [
                "--raw", "--format", "f32", "--rate", "30", "--channels", "1",
                "-P", r#"{ "resample.peaks": true, "channelmix.normalize": true, "node.dont-fallback": true }"#,
                "--target", "alsa_input.usb-Sony_INZONE_H5-00.mono-fallback", "-",
            ]
        );
        assert_eq!(
            pw_record_args("default_input"),
            ["--raw", "--format", "f32", "--rate", "30", "--channels", "1", "-P", r#"{ "resample.peaks": true, "channelmix.normalize": true }"#, "-"]
        );
    }

    #[test]
    fn pactl_says_whether_the_mic_is_muted() {
        assert_eq!(muted("Mute: yes\n"), Some(true));
        assert_eq!(muted("Mute: no\n"), Some(false));
        assert_eq!(muted(""), None, "no such source");
    }
}
