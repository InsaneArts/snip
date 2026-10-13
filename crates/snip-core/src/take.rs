//! A take (REC-01…05, 13, 14): one run of the screen recorder, from its
//! start to a finished file. gpu-screen-recorder records (architecture §4.3),
//! driven over its control socket; ffmpeg's test pattern stands in for it
//! where KMS capture can't see the screen (the e2e suite's nested Hyprland).
//! Which recorder is chosen once, when the take starts; the engine sees the
//! take, never the recorder.
//!
//! A take blocks only where it says so: `pause` asks gsr (up to 3 s), `stop`
//! waits for it to save (up to 30 s). Call those off the UI thread. A take is
//! shared (`Arc`): the watcher polls it while others pause or stop it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::record::{self, Spec};
use crate::{media, paths};

/// What records a take: the adapters at the take's seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recorder {
    /// gpu-screen-recorder, by its full path
    Gsr,
    /// ffmpeg's test pattern, made at stop for the time recorded
    TestPattern,
}

impl Recorder {
    /// gsr, unless SNIP_RECORDER=fake asks for the test pattern.
    pub fn from_env() -> Recorder {
        if std::env::var("SNIP_RECORDER").is_ok_and(|v| v == "fake") {
            Recorder::TestPattern
        } else {
            Recorder::Gsr
        }
    }
}

/// What a take has to say since it was last polled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// its first frame is written: the clock starts now
    Rolling,
    /// the recorder stopped by itself (an error, or something else sending
    /// it SIGINT): `saved` when it has frames to keep; else why, for a person
    Ended { saved: bool, why: String },
}

/// How a stop ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stopped {
    /// the finished file
    Kept(PathBuf),
    /// asked to keep it, but the first frame never came: nothing to keep
    BeforeFirstFrame,
    /// thrown away (or nothing was there)
    Thrown,
}

/// Where a take is written: on disk beside history (a long take would fill
/// the runtime dir's tmpfs, and history takes it with a rename), and its
/// control socket. A take still here when Snip starts was cut short.
pub fn stage(stamp: &str) -> (PathBuf, PathBuf) {
    let _ = paths::ensure_private_dir(&stage_dir());
    let _ = paths::ensure_private_dir(&sock_dir());
    let file = stage_dir().join(format!("{stamp}.mp4"));
    let ipc = sock_for(&file);
    (file, ipc)
}

fn stage_dir() -> PathBuf {
    paths::data_dir().join("recording")
}

/// gsr's control sockets (a socket's path can't be over 107 bytes).
fn sock_dir() -> PathBuf {
    paths::runtime_dir().join("recording")
}

fn sock_for(take: &Path) -> PathBuf {
    sock_dir().join(take.with_extension("sock").file_name().unwrap_or_default())
}

/// The disk takes are written to has less free space than a recording may
/// leave (REC-14): gsr would go on as if recording, saving nothing.
/// SNIP_RECORD_MIN_FREE (bytes) lets tests reach it; 1 GiB otherwise.
pub fn disk_low() -> bool {
    let min = std::env::var("SNIP_RECORD_MIN_FREE").ok().and_then(|v| v.parse().ok()).unwrap_or(1u64 << 30);
    rustix::fs::statvfs(stage_dir()).is_ok_and(|s| s.f_bavail.saturating_mul(s.f_frsize) < min)
}

pub struct Take {
    recorder: Recorder,
    file: PathBuf,
    ipc: PathBuf,
    audio: bool,
    /// the video's size in physical px (the test pattern is made so)
    size: (u32, u32),
    run: Mutex<Run>,
}

struct Run {
    /// gsr; None for the test pattern, and once stopped
    child: Option<Child>,
    started: Instant,
    paused_at: Option<Instant>,
    paused: Duration,
    /// the first frame is written: a stop before that leaves nothing to keep
    rolling: bool,
    /// told: Rolling was reported, Ended too
    announced: bool,
    ended: bool,
}

impl Take {
    /// Start recording `spec` (its file and socket from `stage`), `size`
    /// physical px, with sound when `audio`. Err: why, for a person.
    pub fn start(spec: &Spec, size: (u32, u32), audio: bool, recorder: Recorder) -> Result<Take, String> {
        let child = match recorder {
            Recorder::TestPattern => None,
            Recorder::Gsr => {
                let args = record::gsr_args(spec);
                crate::trace::trace("gsr", &format!("\"args\":{}", crate::trace::js(&args.join(" "))));
                // its own process group: a terminal's Ctrl+C never reaches it. By
                // its full path: Omarchy's bar and keys look for a bare
                // "gpu-screen-recorder" and would stop it (and toast a bogus "saved").
                // Its messages go to a file: a pipe nobody reads fills up and stalls
                // it, and if Snip died, gsr's next line would kill it (SIGPIPE) mid-save
                let log = std::fs::File::create(spec.file.with_extension("log")).map_err(|e| format!("gpu-screen-recorder: {e}"))?;
                let child = Command::new(gsr_path())
                    .args(&args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(log)
                    .process_group(0)
                    .spawn()
                    .map_err(|e| format!("gpu-screen-recorder: {e}"))?;
                Some(child)
            }
        };
        let rolling = recorder == Recorder::TestPattern;
        Ok(Take {
            recorder,
            file: spec.file.clone(),
            ipc: spec.ipc.clone(),
            audio,
            size,
            run: Mutex::new(Run { child, started: Instant::now(), paused_at: None, paused: Duration::ZERO, rolling, announced: rolling, ended: false }),
        })
    }

    /// Its file (still being written until it's stopped).
    pub fn file(&self) -> &Path {
        &self.file
    }

    fn run(&self) -> std::sync::MutexGuard<'_, Run> {
        self.run.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What happened since the last poll: the first frame (gsr writes
    /// `<file>.ts`), or the recorder ending by itself. Each is said once.
    pub fn poll(&self) -> Option<Event> {
        let mut run = self.run();
        if run.ended {
            return None;
        }
        if !run.announced && Path::new(&format!("{}.ts", self.file.display())).exists() {
            run.announced = true;
            // the clock starts at gsr's first frame
            run.started = Instant::now();
            run.rolling = true;
            return Some(Event::Rolling);
        }
        let status = run.child.as_mut().and_then(|c| c.try_wait().ok().flatten())?;
        run.ended = true;
        let log = std::fs::read_to_string(self.file.with_extension("log")).unwrap_or_default();
        let tail = &log[log.floor_char_boundary(log.len().saturating_sub(2000))..];
        let saved = run.rolling && self.file.metadata().is_ok_and(|m| m.len() > 0);
        crate::trace::warn("gsr_exited", &format!("\"status\":{},\"saved\":{saved},\"stderr\":{}", crate::trace::js(&status.to_string()), crate::trace::js(tail.trim())));
        Some(Event::Ended { saved, why: gsr_failure(status.code(), tail) })
    }

    /// The first frame is written.
    pub fn rolling(&self) -> bool {
        self.run().rolling
    }

    /// Time recorded, pauses left out.
    pub fn elapsed(&self) -> Duration {
        let run = self.run();
        run.paused_at.unwrap_or_else(Instant::now).duration_since(run.started).saturating_sub(run.paused)
    }

    /// Pause or resume. Asks gsr (up to 3 s): never on the UI thread.
    pub fn pause(&self, on: bool) -> Result<(), String> {
        if self.recorder == Recorder::Gsr {
            gsr_request(&self.ipc, "set-paused", json!(on))?;
        }
        let mut run = self.run();
        if on {
            run.paused_at.get_or_insert_with(Instant::now);
        } else if let Some(at) = run.paused_at.take() {
            run.paused += at.elapsed();
        }
        Ok(())
    }

    /// Stop: keep its file (gsr saves it, up to 30 s), or throw it away.
    /// Nothing of the take is left on disk but a kept file.
    pub fn stop(&self, keep: bool) -> Stopped {
        let (child, rolling, elapsed) = {
            let mut run = self.run();
            run.ended = true;
            let elapsed = run.paused_at.unwrap_or_else(Instant::now).duration_since(run.started).saturating_sub(run.paused);
            (run.child.take(), run.rolling, elapsed)
        };
        match (self.recorder, child) {
            (Recorder::Gsr, Some(mut child)) => {
                if keep {
                    if let Err(e) = gsr_request(&self.ipc, "stop", Value::Null) {
                        crate::trace::warn("gsr_stop", &format!("\"msg\":{}", crate::trace::js(&e)));
                        interrupt(&child);
                    }
                } else {
                    interrupt(&child);
                }
                reap(&mut child, Duration::from_secs(10));
            }
            (Recorder::TestPattern, _) if keep => {
                if let Err(e) = media::test_clip(&self.file, self.size.0, self.size.1, elapsed.as_millis() as u64, self.audio) {
                    crate::trace::warn("fake_recording", &format!("\"msg\":{}", crate::trace::js(&e)));
                }
            }
            _ => {}
        }
        for f in [PathBuf::from(format!("{}.ts", self.file.display())), self.ipc.clone(), self.file.with_extension("log")] {
            let _ = std::fs::remove_file(f);
        }
        if keep && rolling && self.file.metadata().is_ok_and(|m| m.len() > 0) {
            return Stopped::Kept(self.file.clone());
        }
        let _ = std::fs::remove_file(&self.file);
        if keep && !rolling { Stopped::BeforeFirstFrame } else { Stopped::Thrown }
    }
}

/// Takes left behind when Snip stopped mid-recording (it crashed, or its
/// service was stopped), oldest first.
pub fn leftovers() -> Vec<PathBuf> {
    let mut takes: Vec<PathBuf> = std::fs::read_dir(stage_dir())
        .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "mp4") && !p.to_string_lossy().ends_with(".recovered.mp4")).collect())
        .unwrap_or_default();
    takes.sort();
    takes
}

/// A take left behind, as far as it got: a gsr that outlived Snip is
/// stopped (so its file is finished), and fragments are joined into a plain
/// MP4. Then `forget` it once its file is kept.
pub fn recover(take: &Path) -> Result<PathBuf, String> {
    let _ = gsr_request(&sock_for(take), "stop", Value::Null);
    if media::fragmented(take) {
        let fixed = take.with_extension("recovered.mp4");
        media::recover(take, &fixed).map(|_| fixed)
    } else {
        Ok(take.to_path_buf())
    }
}

/// Everything a left-behind take wrote.
pub fn forget(take: &Path) {
    for f in [take.to_path_buf(), take.with_extension("recovered.mp4"), take.with_extension("log"), take.with_extension("input.jsonl"),
              take.with_extension("camera.mkv"), take.with_extension("camera.mp4"), take.with_extension("camera.log"), PathBuf::from(format!("{}.ts", take.display())), sock_for(take)] {
        let _ = std::fs::remove_file(f);
    }
}

/// gpu-screen-recorder's full path (PATH searched), so its argv[0] isn't the
/// bare name Omarchy matches (D44).
fn gsr_path() -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join("gpu-screen-recorder")).find(|f| f.is_file()))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/gpu-screen-recorder"))
}

pub(crate) fn interrupt(child: &Child) {
    let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
}

/// One request on gsr's control socket: JSON lines, each answered (its man
/// page, "IPC"). `stop` is answered once the file is saved (it may take a
/// while: no less than 30 s); others at once (3 s).
fn gsr_request(sock: &Path, name: &str, data: Value) -> Result<Value, String> {
    let mut s = UnixStream::connect(sock).map_err(|e| format!("gsr: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(if name == "stop" { 30 } else { 3 }))).ok();
    writeln!(s, "{}", json!({ "id": 1, "name": name, "data": data })).map_err(|e| format!("gsr: {e}"))?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).map_err(|e| format!("gsr: {e}"))?;
    let v: Value = serde_json::from_str(&line).map_err(|e| format!("gsr: {e}: {line}"))?;
    if v["result"] == "ok" {
        Ok(v["data"].clone())
    } else {
        Err(format!("gsr: {}", v["data"].as_str().unwrap_or("error")))
    }
}

/// Why gsr stopped before recording anything, for a person: its exit code
/// says (its own last line is often a device name), else that last line.
fn gsr_failure(code: Option<i32>, log: &str) -> String {
    match code {
        Some(50) => "the microphone isn't connected: choose another".into(),
        Some(51) => "this display can't be recorded".into(),
        Some(52..=54) => "the graphics card can't encode it".into(),
        // exit 0 too: the capture never started (a virtual display, say)
        _ if log.contains("gsr_capture_start failed") => "this display can't be recorded".into(),
        _ => log.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("gpu-screen-recorder stopped").trim().to_string(),
    }
}

/// Wait for a child to exit, then kill it if it takes longer than `limit`.
pub(crate) fn reap(child: &mut Child, limit: Duration) {
    let t0 = Instant::now();
    while t0.elapsed() < limit {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{Audio, Target};
    use crate::geometry::Rect;

    fn spec(dir: &Path) -> Spec {
        Spec {
            target: Target::Area(Rect::new(0, 0, 320, 200)),
            fps: 30,
            cursor: true,
            audio: Audio::default(),
            codec: "auto".into(),
            quality: "very_high".into(),
            max_size: None,
            cpu: false,
            file: dir.join("take.mp4"),
            ipc: dir.join("take.sock"),
        }
    }

    fn ffmpeg() -> bool {
        Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
    }

    /// The take's clock and stops, through the test-pattern recorder: paused
    /// time is left out of what's kept, and a thrown take leaves nothing.
    #[test]
    fn a_take_keeps_its_time_without_the_pauses() {
        if !ffmpeg() {
            return eprintln!("ffmpeg isn't available: skipped");
        }
        let dir = std::env::temp_dir().join(format!("snip-take-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let take = Take::start(&spec(&dir), (320, 200), false, Recorder::TestPattern).unwrap();
        assert!(take.rolling(), "the test pattern rolls at once");
        assert_eq!(take.poll(), None, "nothing to say");
        std::thread::sleep(Duration::from_millis(400));
        take.pause(true).unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let paused = take.elapsed();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(take.elapsed(), paused, "paused: the clock stands still");
        take.pause(false).unwrap();
        std::thread::sleep(Duration::from_millis(300));
        let Stopped::Kept(file) = take.stop(true) else { panic!("kept") };
        let ms = media::probe(&file).unwrap().duration_ms;
        assert!((600..1000).contains(&ms), "about 0.7 s, not 1.4 s: {ms}");
        let thrown = Take::start(&spec(&dir), (320, 200), false, Recorder::TestPattern).unwrap();
        assert_eq!(thrown.stop(false), Stopped::Thrown);
        assert!(!dir.join("take.mp4").exists(), "nothing left");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gsr_failures_read_as_reasons() {
        assert_eq!(gsr_failure(Some(50), "alsa_output.x.monitor|Monitor of HDMI"), "the microphone isn't connected: choose another");
        assert_eq!(gsr_failure(Some(0), "gsr error: no monitor\ngsr_capture_start failed\n"), "this display can't be recorded");
        assert_eq!(gsr_failure(Some(54), ""), "the graphics card can't encode it");
        assert_eq!(gsr_failure(Some(1), "first\nthe last line\n\n"), "the last line");
    }

    /// gsr's control socket, as its man page describes it: a JSON request
    /// line, one JSON reply line.
    #[test]
    fn gsr_requests_are_json_lines_answered_once() {
        let dir = std::env::temp_dir().join(format!("snip-gsr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("gsr.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (s, _) = listener.accept().unwrap();
                let mut line = String::new();
                BufReader::new(&s).read_line(&mut line).unwrap();
                let v: Value = serde_json::from_str(&line).unwrap();
                let reply = if v["name"] == "stop" && v["data"].is_null() {
                    json!({ "id": v["id"], "result": "ok", "data": "/run/rec/take.mp4" })
                } else {
                    json!({ "id": v["id"], "result": "error", "data": "not recording" })
                };
                writeln!(&s, "{reply}").unwrap();
            }
        });
        assert_eq!(gsr_request(&sock, "stop", Value::Null).unwrap(), json!("/run/rec/take.mp4"), "stop answers with the file");
        assert_eq!(gsr_request(&sock, "set-paused", json!(true)).unwrap_err(), "gsr: not recording", "errors come through");
        server.join().unwrap();
        assert!(gsr_request(&sock, "stop", Value::Null).is_err(), "no gsr: an error, not a hang");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
