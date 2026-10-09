//! Screen recording (REC-01…07, 11, 13): from the chosen area to the video,
//! or GIF, in history. gpu-screen-recorder records (architecture §4.3),
//! driven over its IPC socket. With SNIP_RECORDER=fake, ffmpeg's test pattern
//! stands in for it: KMS capture can't see the nested Hyprland the e2e suite
//! runs in.
//!
//! The HUD (QML) draws only outside the area: KMS capture records whatever
//! is on screen there, Snip's own layers included.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use snip_core::geometry::Rect;
use snip_core::record::{self, Audio, Spec, Target};
use snip_core::store::CaptureMeta;
use snip_core::trace::{self, js};
use snip_core::{media, paths};

use super::{cancelled, card_json, reply, Inner, Waiter};
use cxx_qt_lib::QString;
use snip_ipc::Response;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    /// the area is chosen; the toolbar waits for Record
    Ready,
    /// the countdown before it starts (REC-03)
    Countdown,
    Recording,
    Paused,
    /// stopped: the file is being finished (and made a GIF)
    Finishing,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            State::Ready => "ready",
            State::Countdown => "countdown",
            State::Recording => "recording",
            State::Paused => "paused",
            State::Finishing => "finishing",
        }
    }
}

pub(super) struct Rec {
    output: String,
    /// global logical px (what gsr records), and from the output's corner
    /// (where the HUD draws)
    logical: Rect,
    local: Rect,
    scale: f64,
    gif: bool,
    waiter: Option<Waiter>,
    app: Option<String>,
    title: Option<String>,
    state: State,
    run: Option<Run>,
}

struct Run {
    /// gpu-screen-recorder; None for the test stand-in
    child: Option<Child>,
    file: PathBuf,
    ipc: PathBuf,
    started: Instant,
    paused_at: Option<Instant>,
    paused: Duration,
    audio: bool,
    /// the video's size in physical px (the test stand-in makes it so)
    size: (u32, u32),
    /// gsr wrote its first frame: a stop before that leaves no usable file
    rolling: bool,
}

impl Run {
    fn elapsed(&self) -> Duration {
        self.paused_at.unwrap_or_else(Instant::now).duration_since(self.started).saturating_sub(self.paused)
    }
}

fn fake() -> bool {
    std::env::var("SNIP_RECORDER").is_ok_and(|v| v == "fake")
}

/// Where takes are written: on disk beside history (a long take would fill
/// the runtime dir's tmpfs, and history takes it with a rename). A take still
/// here when Snip starts was cut short, and is recovered.
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

/// A recording stops when its disk has less free space than this (REC-14):
/// gsr would go on as if recording, saving nothing. SNIP_RECORD_MIN_FREE
/// (bytes) lets tests reach it.
fn disk_low() -> bool {
    let min = std::env::var("SNIP_RECORD_MIN_FREE").ok().and_then(|v| v.parse().ok()).unwrap_or(1u64 << 30);
    rustix::fs::statvfs(stage_dir()).is_ok_and(|s| s.f_bavail.saturating_mul(s.f_frsize) < min)
}

/// gpu-screen-recorder's full path (PATH searched), so its argv[0] isn't the
/// bare name Omarchy matches.
fn gsr_path() -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join("gpu-screen-recorder")).find(|f| f.is_file()))
        .unwrap_or_else(|| PathBuf::from("/usr/bin/gpu-screen-recorder"))
}

/// One request on gsr's control socket: JSON lines, each answered (its man
/// page, "IPC"). `stop` is answered once the file is saved.
fn gsr_request(sock: &Path, name: &str, data: Value) -> Result<Value, String> {
    let mut s = UnixStream::connect(sock).map_err(|e| format!("gsr: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(30))).ok();
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

/// Wait for a child to exit, then kill it if it takes longer than `limit`.
fn reap(child: &mut Child, limit: Duration) {
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

impl Inner {
    /// The microphones gsr can record (REC-02's picker): its own names.
    pub fn audio_inputs(&self) -> Vec<Value> {
        let mut out = vec![json!({ "id": "default_input", "label": "Default microphone" })];
        if fake() {
            return out;
        }
        let Ok(o) = Command::new("gpu-screen-recorder").arg("--list-audio-devices").stdin(Stdio::null()).stderr(Stdio::null()).output() else { return out };
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let Some((id, label)) = line.split_once('|') else { continue };
            // outputs' monitors are "system audio", not microphones
            if id.starts_with("default_") || id.ends_with(".monitor") {
                continue;
            }
            out.push(json!({ "id": id, "label": label }));
        }
        out
    }

    /// The area is chosen (physical px of `output`): show the frame and the
    /// toolbar, ready to record. `now` (scripts) skips the toolbar.
    pub(super) fn record_open(self: &Arc<Self>, output: String, area: Rect, scale: f64, waiter: Option<Waiter>, gif: bool, now: bool) {
        if self.recording.lock().unwrap().is_some() {
            return reply(waiter, Response::err("a recording is already open"));
        }
        let Some(o) = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| o.logical) else {
            return reply(waiter, Response::err("no such output"));
        };
        let s = if scale > 0.0 { scale } else { 1.0 };
        let logical = record::area_from_physical(area, (o.x, o.y), s);
        let local = Rect::new(logical.x - o.x, logical.y - o.y, logical.w, logical.h);
        let (app, title) = self.window_meta_at(&output, area, s);
        trace::trace("record_open", &format!("\"output\":{},\"area\":[{},{},{},{}],\"gif\":{gif}", js(&output), local.x, local.y, local.w, local.h));
        *self.recording.lock().unwrap() = Some(Rec { output: output.clone(), logical, local, scale: s, gif, waiter, app, title, state: State::Ready, run: None });
        if !now {
            let _ = snip_hypr::eval(&snip_hypr::keys::session_lua(&crate::bridge::app_id(), true, "record"));
        }
        let cfg = self.cfg().recording;
        let whole = local == Rect::new(0, 0, o.w, o.h);
        let j = json!({
            // a GIF if asked, else what was chosen last (REC-02)
            "output": output, "x": local.x, "y": local.y, "w": local.w, "h": local.h, "whole": whole, "gif": gif || cfg.kind == "gif",
            "mics": if now { vec![] } else { self.audio_inputs() }, "mic": cfg.mic, "system": cfg.system_audio, "dim": cfg.dim, "now": now,
            "camera": cfg.camera, "cameraShape": cfg.camera_shape, "cameraSize": cfg.camera_size, "cameraMirror": cfg.camera_mirror,
            // tests: a test card stands in for a camera (the suite's machine has none)
            "fakeCamera": std::env::var("SNIP_CAMERA").is_ok_and(|v| v == "fake"),
        })
        .to_string();
        self.ui(move |e| e.record_open(&QString::from(j.as_str())));
        // scripts: no toolbar and no countdown, the frame and controls only
        if now {
            self.record_go();
        }
    }

    /// Record (the toolbar's button, or Enter): the countdown first, if set.
    pub fn record_start(self: &Arc<Self>, gif: Option<bool>) {
        let seconds = self.cfg().recording.countdown.min(10);
        {
            let mut r = self.recording.lock().unwrap();
            let Some(rec) = r.as_mut().filter(|r| r.state == State::Ready) else { return };
            if let Some(g) = gif {
                rec.gif = g;
            }
            rec.state = if seconds > 0 { State::Countdown } else { State::Ready };
        }
        let _ = snip_hypr::eval(&snip_hypr::keys::session_lua("", false, ""));
        if seconds > 0 {
            self.record_status(json!({ "seconds": seconds }));
        } else {
            self.record_go();
        }
    }

    /// The countdown is over and off screen (QML): record.
    pub fn record_go(self: &Arc<Self>) {
        let cfg = self.cfg().recording;
        let mut r = self.recording.lock().unwrap();
        let Some(rec) = r.as_mut().filter(|r| matches!(r.state, State::Ready | State::Countdown)) else { return };
        let _ = paths::ensure_private_dir(&stage_dir());
        let _ = paths::ensure_private_dir(&sock_dir());
        let stamp = format!("{}", chrono::Local::now().format("%Y%m%d-%H%M%S%3f"));
        let file = stage_dir().join(format!("{stamp}.mp4"));
        let ipc = sock_for(&file);
        // a GIF is silent (REC-13)
        let audio = Audio {
            mic: (!rec.gif && !cfg.mic.is_empty()).then(|| cfg.mic.clone()),
            system: !rec.gif && cfg.system_audio,
            separate: cfg.separate_tracks,
        };
        let has_audio = audio.mic.is_some() || audio.system;
        let whole = self.outputs.lock().unwrap().iter().find(|o| o.name == rec.output).is_some_and(|o| rec.local == Rect::new(0, 0, o.logical.w, o.logical.h));
        let target = if whole { Target::Output(rec.output.clone()) } else { Target::Area(rec.logical) };
        let spec = Spec {
            target,
            fps: cfg.fps.clamp(1, 240),
            cursor: cfg.show_cursor,
            audio,
            codec: "auto".into(),
            quality: cfg.quality.clone(),
            max_size: cfg.max_box(),
            cpu: false,
            file: file.clone(),
            ipc: ipc.clone(),
        };
        let size = ((rec.local.w as f64 * rec.scale).round() as u32, (rec.local.h as f64 * rec.scale).round() as u32);
        let spec = Spec { cpu: !record::gpu_can_encode(size.0, size.1), ..spec };
        let child = if fake() {
            None
        } else {
            let args = record::gsr_args(&spec);
            trace::trace("gsr", &format!("\"args\":{}", js(&args.join(" "))));
            // its own process group: a terminal's Ctrl+C never reaches it. By
            // its full path: Omarchy's bar and keys look for a bare
            // "gpu-screen-recorder" and would stop it (and toast a bogus "saved").
            // Its messages go to a file: a pipe nobody reads fills up and stalls
            // it, and if Snip died, gsr's next line would kill it (SIGPIPE) mid-save
            let spawned = std::fs::File::create(file.with_extension("log"))
                .and_then(|log| Command::new(gsr_path()).args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(log).process_group(0).spawn());
            match spawned {
                Ok(c) => Some(c),
                Err(e) => {
                    drop(r);
                    self.toast(format!("Can't record: gpu-screen-recorder: {e}"));
                    return self.record_close(false);
                }
            }
        };
        rec.state = State::Recording;
        let rolling = fake();
        rec.run = Some(Run { child, file: file.clone(), ipc, started: Instant::now(), paused_at: None, paused: Duration::ZERO, audio: has_audio, size, rolling });
        drop(r);
        if cfg.dnd {
            self.dnd(true);
        }
        self.record_status(json!({ "elapsed": 0 }));
        let me = self.clone();
        std::thread::spawn(move || me.record_watch(file));
    }

    /// While gsr records: it started (its first-frame file), or it stopped by
    /// itself (an error, or something else sending it SIGINT, such as
    /// Omarchy's bar button): then the recording ends as if stopped. And the
    /// disk isn't full yet.
    fn record_watch(self: Arc<Self>, file: PathBuf) {
        let ts = PathBuf::from(format!("{}.ts", file.display()));
        let mut announced = fake();
        let mut checked = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(150));
            let mut r = self.recording.lock().unwrap();
            let Some(rec) = r.as_mut() else { return };
            let Some(run) = rec.run.as_mut().filter(|run| run.file == file) else { return };
            if checked.elapsed() >= Duration::from_secs(2) {
                checked = Instant::now();
                if disk_low() {
                    drop(r);
                    trace::warn("record_disk_low", "");
                    self.record_finish(true);
                    return self.toast("The disk is almost full: the recording stopped");
                }
            }
            if !announced && ts.exists() {
                announced = true;
                // the clock starts at gsr's first frame
                run.started = Instant::now();
                run.rolling = true;
                drop(r);
                self.record_status(json!({ "elapsed": 0 }));
                continue;
            }
            let exited = run.child.as_mut().and_then(|c| c.try_wait().ok().flatten());
            if let Some(status) = exited {
                let log = std::fs::read_to_string(file.with_extension("log")).unwrap_or_default();
                let err = &log[log.floor_char_boundary(log.len().saturating_sub(2000))..];
                let saved = file.metadata().is_ok_and(|m| m.len() > 0);
                drop(r);
                trace::warn("gsr_exited", &format!("\"status\":{},\"saved\":{saved},\"stderr\":{}", js(&status.to_string()), js(err.trim())));
                if saved && announced {
                    self.record_finish(true);
                } else {
                    let why = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("gpu-screen-recorder stopped").trim().to_string();
                    self.toast(format!("Can't record: {why}"));
                    self.record_finish(false);
                }
                return;
            }
        }
    }

    /// Pause or resume (REC-05).
    pub fn record_pause(&self, pause: bool) {
        let mut r = self.recording.lock().unwrap();
        let Some(rec) = r.as_mut().filter(|r| matches!(r.state, State::Recording | State::Paused)) else { return };
        if (rec.state == State::Paused) == pause {
            return;
        }
        let Some(run) = rec.run.as_mut() else { return };
        if run.child.is_some() {
            if let Err(e) = gsr_request(&run.ipc, "set-paused", json!(pause)) {
                trace::warn("gsr_pause", &format!("\"msg\":{}", js(&e)));
                return;
            }
        }
        if pause {
            run.paused_at = Some(Instant::now());
        } else if let Some(at) = run.paused_at.take() {
            run.paused += at.elapsed();
        }
        rec.state = if pause { State::Paused } else { State::Recording };
        let elapsed = run.elapsed().as_millis() as u64;
        drop(r);
        self.record_status(json!({ "elapsed": elapsed }));
    }

    /// Stop and keep it (REC-05's stop; the Record hotkey again).
    pub fn record_stop(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || me.record_finish(true));
    }

    /// Throw this take away (REC-05's trash), or close the toolbar.
    pub fn record_discard(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || me.record_finish(false));
    }

    /// Start over in the same area (REC-05's restart): this take is thrown away.
    pub fn record_restart(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || {
            let run = {
                let mut r = me.recording.lock().unwrap();
                let Some(rec) = r.as_mut().filter(|r| matches!(r.state, State::Recording | State::Paused)) else { return };
                rec.state = State::Ready;
                rec.run.take()
            };
            if let Some(run) = run {
                me.record_end_run(run, false);
            }
            if me.cfg().recording.dnd {
                me.dnd(false);
            }
            me.record_go();
        });
    }

    /// `snip record area|fullscreen` (API-01): record this at once, no
    /// overlay or toolbar. `rect` is logical px on the monitor.
    pub fn record_area(self: &Arc<Self>, monitor: Option<String>, rect: Option<[i32; 4]>, gif: bool, waiter: Waiter) -> Result<(), String> {
        let output = monitor.filter(|m| self.output_names().contains(m)).or_else(|| self.cursor_output()).ok_or("no output")?;
        let o = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| o.logical).ok_or("no such output")?;
        let scale = self.output_scale(&output);
        let [x, y, w, h] = rect.unwrap_or([0, 0, o.w, o.h]);
        if w < 2 || h < 2 || x < 0 || y < 0 || x + w > o.w || y + h > o.h {
            return Err(format!("the area must lie inside {output} ({}x{} logical)", o.w, o.h));
        }
        let phys = |v: i32| (v as f64 * scale).round() as i32;
        self.record_open(output, Rect::new(phys(x), phys(y), phys(w), phys(h)), scale, Some(waiter), gif, true);
        Ok(())
    }

    /// The Record hotkey (or `snip record`): stop the recording that runs,
    /// else choose an area to record.
    pub fn record_toggle(self: &Arc<Self>, gif: bool, waiter: Option<Waiter>) {
        let state = self.recording.lock().unwrap().as_ref().map(|r| r.state);
        match state {
            Some(State::Recording | State::Paused) => {
                reply(waiter, Response::ok(json!({ "stopping": true })));
                self.record_stop();
            }
            Some(_) => reply(waiter, Response::err("a recording is already open")),
            None => self.begin(if gif { "record-gif" } else { "record" }, None, None, waiter, 0, None),
        }
    }

    /// Where things stand, for `snip status` and tests.
    pub fn record_state(&self) -> Value {
        let r = self.recording.lock().unwrap();
        match r.as_ref() {
            None => Value::Null,
            Some(rec) => json!({
                "state": rec.state.name(), "output": rec.output, "gif": rec.gif,
                "area": [rec.local.x, rec.local.y, rec.local.w, rec.local.h],
                "elapsed": rec.run.as_ref().map_or(0, |r| r.elapsed().as_millis() as u64),
                "file": rec.run.as_ref().map(|r| r.file.clone()),
            }),
        }
    }

    fn record_status(&self, mut extra: Value) {
        let state = self.recording.lock().unwrap().as_ref().map(|r| r.state.name());
        let Some(state) = state else { return };
        extra["state"] = json!(state);
        let j = extra.to_string();
        trace::trace("record_status", &j[1..j.len() - 1]);
        self.ui(move |e| e.record_status(&QString::from(j.as_str())));
    }

    /// Stop a take's recorder; its file is finished (kept) or deleted.
    fn record_end_run(&self, mut run: Run, keep: bool) -> Option<PathBuf> {
        match run.child.as_mut() {
            Some(child) => {
                if keep {
                    if let Err(e) = gsr_request(&run.ipc, "stop", Value::Null) {
                        trace::warn("gsr_stop", &format!("\"msg\":{}", js(&e)));
                        let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
                    }
                } else {
                    let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
                }
                reap(child, Duration::from_secs(10));
            }
            None if keep => {
                if let Err(e) = media::test_clip(&run.file, run.size.0, run.size.1, run.elapsed().as_millis() as u64, run.audio) {
                    trace::warn("fake_recording", &format!("\"msg\":{}", js(&e)));
                }
            }
            None => {}
        }
        let ts = PathBuf::from(format!("{}.ts", run.file.display()));
        let _ = std::fs::remove_file(ts);
        let _ = std::fs::remove_file(&run.ipc);
        let _ = std::fs::remove_file(run.file.with_extension("log"));
        if keep && !run.rolling {
            self.toast("Stopped before the first frame: nothing was recorded");
        }
        if keep && run.rolling && run.file.metadata().is_ok_and(|m| m.len() > 0) {
            Some(run.file)
        } else {
            let _ = std::fs::remove_file(&run.file);
            None
        }
    }

    /// The end of a recording: kept (into history, as a GIF if asked) or
    /// thrown away; the HUD closes either way.
    fn record_finish(self: &Arc<Self>, keep: bool) {
        let rec = {
            let mut r = self.recording.lock().unwrap();
            match r.as_mut() {
                Some(rec) if rec.state != State::Finishing => {
                    rec.state = State::Finishing;
                    r.take()
                }
                _ => return,
            }
        };
        let Some(mut rec) = rec else { return };
        let recording = rec.run.is_some();
        self.ui(|e| e.record_close());
        let _ = snip_hypr::eval(&snip_hypr::keys::session_lua("", false, ""));
        let file = rec.run.take().and_then(|run| self.record_end_run(run, keep));
        if recording && self.cfg().recording.dnd {
            self.dnd(false);
        }
        let Some(file) = file else {
            return reply(rec.waiter, cancelled());
        };
        let file = if rec.gif {
            self.toast("Creating GIF…");
            let cfg = self.cfg().recording;
            let gif = file.with_extension("gif");
            let made = media::gif(&file, &gif, cfg.gif_fps.clamp(1, 50), cfg.gif_width.max(16));
            let _ = std::fs::remove_file(&file);
            if let Err(e) = made {
                self.toast("Couldn't make the GIF");
                trace::warn("gif", &format!("\"msg\":{}", js(&e)));
                return reply(rec.waiter, Response::err(e));
            }
            gif
        } else {
            file
        };
        self.play("shutter");
        let meta = CaptureMeta { kind: "recording".into(), scale: rec.scale, output: rec.output.clone(), app: rec.app, title: rec.title };
        match self.ingest_media(&file, true, meta) {
            Ok(r) => {
                self.show_card(&r, &rec.output);
                reply(rec.waiter, Response::ok(card_json(&r, &rec.output)));
            }
            Err(e) => {
                self.toast(format!("Couldn't keep the recording: {e}"));
                reply(rec.waiter, Response::err(e));
            }
        }
    }

    /// Takes left behind when Snip stopped mid-recording (it crashed, or its
    /// service was stopped): into history, as far as they got (REC-14).
    pub fn record_recover(self: &Arc<Self>) {
        // listed now, before a new take can start
        let takes: Vec<PathBuf> = std::fs::read_dir(stage_dir())
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "mp4")).collect())
            .unwrap_or_default();
        if takes.is_empty() {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let mut kept = 0;
            for take in takes {
                // a gsr that outlived Snip: stopped, so its file is finished
                let _ = gsr_request(&sock_for(&take), "stop", Value::Null);
                let fixed = take.with_extension("recovered.mp4");
                let file = if media::fragmented(&take) { media::recover(&take, &fixed).map(|_| fixed.clone()) } else { Ok(take.clone()) };
                let output = me.cursor_output().unwrap_or_default();
                let meta = CaptureMeta { kind: "recording".into(), scale: 1.0, output: output.clone(), app: None, title: None };
                match file.and_then(|f| me.ingest_media(&f, true, meta)) {
                    Ok(r) => {
                        trace::trace("record_recovered", &format!("\"id\":\"{}\",\"ms\":{}", r.id, r.duration_ms));
                        me.show_card(&r, &output);
                        kept += 1;
                    }
                    // nothing in it (stopped before its first frame)
                    Err(e) => trace::warn("record_recover", &format!("\"file\":{},\"msg\":{}", js(&take.to_string_lossy()), js(&e))),
                }
                for f in [take.clone(), fixed, take.with_extension("log"), PathBuf::from(format!("{}.ts", take.display())), sock_for(&take)] {
                    let _ = std::fs::remove_file(f);
                }
            }
            if kept > 0 {
                me.toast(if kept == 1 { "Recovered a recording that was cut short".to_string() } else { format!("Recovered {kept} recordings that were cut short") });
            }
        });
    }

    /// A monitor went away (REC-14): gsr would go on encoding its last frame,
    /// so a recording of it stops, and is kept.
    pub fn record_output_removed(self: &Arc<Self>, name: &str) {
        let state = self.recording.lock().unwrap().as_ref().filter(|r| r.output == name).map(|r| r.state);
        match state {
            Some(State::Recording | State::Paused) => {
                let me = self.clone();
                std::thread::spawn(move || {
                    me.record_finish(true);
                    me.toast("The display was disconnected: the recording is saved");
                });
            }
            Some(State::Ready | State::Countdown) => self.record_close(false),
            _ => {}
        }
    }

    /// Close without recording (Esc, or Cancel in the toolbar).
    pub fn record_cancel(self: &Arc<Self>) {
        let ready = self.recording.lock().unwrap().as_ref().is_some_and(|r| matches!(r.state, State::Ready | State::Countdown));
        if ready {
            self.record_close(false);
        }
    }

    fn record_close(self: &Arc<Self>, _keep: bool) {
        let rec = self.recording.lock().unwrap().take();
        let _ = snip_hypr::eval(&snip_hypr::keys::session_lua("", false, ""));
        self.ui(|e| e.record_close());
        if let Some(rec) = rec {
            reply(rec.waiter, cancelled());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
