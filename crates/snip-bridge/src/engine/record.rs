//! Screen recording (REC-01…07, 11, 13): from the chosen area to the video,
//! or GIF, in history. A take (snip_core::take) runs the recorder; this
//! module holds the recording's state, the HUD, and what a take turns on
//! around it (Do Not Disturb, the desktop cover, the click and key hooks,
//! keep-awake), and turns it off again once the recorder has stopped.
//!
//! The HUD (QML) draws only outside the area: KMS capture records whatever
//! is on screen there, Snip's own layers included.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use snip_core::geometry::{self, Rect};
use snip_core::meter::{Meter, Reading};
use snip_core::record::{self, Audio, Spec, Target};
use snip_core::store::CaptureMeta;
use snip_core::studio::log::{Cursor, Event as Input, Header};
use snip_core::take::{self, Event, Recorder, Stopped, Take};
use snip_core::trace::{self, js};
use snip_core::media;

use super::take_log::{now_us, TakeLog};
use super::{cancelled, card_json, reply, Inner, Waiter};
use crate::ui::Ui;
use snip_ipc::Response;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum State {
    /// the panel is open: the area is chosen on it, Record waits
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
    /// listening to the chosen microphone (D53), and which
    meter: Option<(String, Meter)>,
}

struct Run {
    /// the recorder's take: its file, its clock
    take: Arc<Take>,
    /// the Hyprland hooks for clicks and keys (REC-09/10), by this take's id
    sid: String,
    hooks: Option<(bool, bool)>,
    /// what the HUD shows of them as it records: keys, clicks
    hud: (bool, bool),
    /// a studio take (D54): its command keys and typing logged, the editor
    /// opened at its end (D58)
    studio: bool,
    /// the `cursor:no_hardware_cursors` a studio take put the pointer on the
    /// hardware plane from (D57), to put back
    cursor: Option<i64>,
    /// where the pointer went and the clicks, for the editor (D54)
    log: Option<TakeLog>,
    /// this take put the desktop cover up (CAP-12): it takes it down
    covered: bool,
    /// this take turned Do Not Disturb on (REC-11): it turns it off, whatever
    /// the setting says by then
    dnd: bool,
}

/// Which sources are muted, by name, from `pactl list sources` (in English).
fn parse_mutes(text: &str) -> std::collections::HashMap<String, bool> {
    let mut out = std::collections::HashMap::new();
    let mut name = None;
    for line in text.lines().map(str::trim) {
        if let Some(n) = line.strip_prefix("Name: ") {
            name = Some(n.to_string());
        } else if let (Some(m), Some(n)) = (line.strip_prefix("Mute: "), name.take()) {
            out.insert(n, m == "yes");
        }
    }
    out
}

fn pactl(args: &[&str]) -> String {
    Command::new("pactl").args(args).env("LC_ALL", "C").stdin(Stdio::null()).stderr(Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

impl Inner {
    /// The microphones gsr can record (REC-02's picker): its own names, and
    /// whether each is muted (REC-14: the panel says so before recording).
    pub fn audio_inputs(&self) -> Vec<Value> {
        let mut out = vec![json!({ "id": "default_input", "label": "Default microphone" })];
        if Recorder::from_env() == Recorder::TestPattern {
            return out;
        }
        let mutes = parse_mutes(&pactl(&["list", "sources"]));
        let default = pactl(&["get-default-source"]);
        out[0]["muted"] = json!(mutes.get(default.trim()).copied().unwrap_or(false));
        let Ok(o) = Command::new("gpu-screen-recorder").arg("--list-audio-devices").stdin(Stdio::null()).stderr(Stdio::null()).output() else { return out };
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let Some((id, label)) = line.split_once('|') else { continue };
            // outputs' monitors are "system audio", not microphones
            if id.starts_with("default_") || id.ends_with(".monitor") {
                continue;
            }
            out.push(json!({ "id": id, "label": label, "muted": mutes.get(id).copied().unwrap_or(false) }));
        }
        out
    }

    /// Show the panel (D49) over `area` (physical px of `output`), ready to
    /// record. `now` (scripts) skips the panel.
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
        *self.recording.lock().unwrap() = Some(Rec { output: output.clone(), logical, local, scale: s, gif, waiter, app, title, state: State::Ready, run: None, meter: None });
        let cfg = self.cfg().recording;
        // the windows on this output, front-most first, for Space (local px)
        let windows: Vec<Value> = if now {
            vec![]
        } else {
            snip_hypr::visible_windows()
                .into_iter()
                .map(|c| (Rect::new(c.x - o.x, c.y - o.y, c.w, c.h), c.title))
                .filter(|(r, _)| r.x < o.w && r.y < o.h && r.x + r.w > 0 && r.y + r.h > 0)
                .map(|(r, title)| json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h, "title": title }))
                .collect()
        };
        let j = json!({
            // `gif`: what Enter records
            "output": output, "x": local.x, "y": local.y, "w": local.w, "h": local.h, "ow": o.w, "oh": o.h, "scale": s, "gif": gif,
            "mics": if now { vec![] } else { self.audio_inputs() }, "mic": cfg.mic, "system": cfg.system_audio, "dim": cfg.dim, "now": now,
            "camera": cfg.camera, "cameraShape": cfg.camera_shape, "cameraSize": cfg.camera_size, "cameraMirror": cfg.camera_mirror,
            "clicks": cfg.highlight_clicks, "keys": cfg.show_keystrokes, "studio": cfg.studio, "windows": windows,
            // tests: a test card stands in for a camera (the suite's machine has none)
            "fakeCamera": std::env::var("SNIP_CAMERA").is_ok_and(|v| v == "fake"),
        })
        .to_string();
        self.ui(Ui::RecordOpen(j));
        // scripts: no panel and no countdown, the frame and controls only
        if now {
            self.record_go();
        } else {
            self.record_listen();
        }
    }

    /// D53: listen to the chosen microphone while the panel shows it, and
    /// while an area take records it (its pill shows the level); not for a
    /// GIF, a whole-output take, or with no microphone. Called whenever one
    /// of those changes.
    pub(super) fn record_listen(self: &Arc<Self>) {
        let mic = self.cfg().recording.mic;
        let outputs = self.outputs.lock().unwrap().clone();
        let old = {
            let mut r = self.recording.lock().unwrap();
            let Some(rec) = r.as_mut() else { return };
            let o = outputs.iter().find(|o| o.name == rec.output).map(|o| o.logical);
            let whole = o.is_none_or(|o| rec.local.x <= 0 && rec.local.y <= 0 && rec.local.w >= o.w && rec.local.h >= o.h);
            let listen = !mic.is_empty()
                && !rec.gif
                && match rec.state {
                    State::Ready => true,
                    State::Countdown | State::Recording | State::Paused => !whole,
                    State::Finishing => false,
                };
            if rec.meter.as_ref().map(|(m, _)| m.as_str()) == listen.then_some(mic.as_str()) {
                return;
            }
            let (me, id) = (Arc::downgrade(self), mic.clone());
            let heard = move |reading| {
                let Some(me) = me.upgrade() else { return };
                me.ui(match reading {
                    Reading::Level(l) => Ui::RecordLevel(l as f64),
                    Reading::Muted(muted) => Ui::RecordMic { mic: id.clone(), muted },
                });
            };
            let new = listen.then(|| (mic.clone(), Meter::start(&mic, Recorder::from_env(), heard)));
            std::mem::replace(&mut rec.meter, new)
        };
        // stopped outside the lock; what it showed goes with it
        if old.is_some() {
            drop(old);
            self.ui(Ui::RecordLevel(0.0));
        }
    }

    /// Record, for `area` (logical px from the output's corner: chosen on
    /// the HUD), as a GIF or not: the countdown first, if set.
    pub fn record_start(self: &Arc<Self>, gif: Option<bool>, area: Option<Rect>) {
        let seconds = self.cfg().recording.countdown.min(10);
        {
            let mut r = self.recording.lock().unwrap();
            let Some(rec) = r.as_mut().filter(|r| r.state == State::Ready) else { return };
            if let Some(g) = gif {
                rec.gif = g;
            }
            let o = self.outputs.lock().unwrap().iter().find(|o| o.name == rec.output).map(|o| o.logical);
            if let (Some(a), Some(o)) = (area, o) {
                let (x, y) = (a.x.clamp(0, o.w - 2), a.y.clamp(0, o.h - 2));
                rec.local = Rect::new(x, y, a.w.clamp(2, o.w - x), a.h.clamp(2, o.h - y));
                rec.logical = Rect::new(o.x + rec.local.x, o.y + rec.local.y, rec.local.w, rec.local.h);
                let s = rec.scale;
                (rec.app, rec.title) = self.window_meta_at(&rec.output, geometry::logical_to_physical(rec.local, s), s);
                trace::trace("record_area", &format!("\"area\":[{},{},{},{}]", rec.local.x, rec.local.y, rec.local.w, rec.local.h));
            }
            // with or without a countdown, the HUD calls record_go once its
            // panel is off screen: the panel is never in the first frame (D49)
            rec.state = State::Countdown;
        }
        self.record_status(json!({ "seconds": seconds }));
        self.record_listen();
    }

    /// The countdown is over and off screen (QML): record.
    pub fn record_go(self: &Arc<Self>) {
        let cfg = self.cfg().recording;
        let mut r = self.recording.lock().unwrap();
        let Some(rec) = r.as_mut().filter(|r| matches!(r.state, State::Ready | State::Countdown)) else { return };
        let stamp = format!("{}", chrono::Local::now().format("%Y%m%d-%H%M%S%3f"));
        let (file, ipc) = take::stage(&stamp);
        // a GIF is silent (REC-13)
        let audio = Audio {
            mic: (!rec.gif && !cfg.mic.is_empty()).then(|| cfg.mic.clone()),
            system: !rec.gif && cfg.system_audio,
            separate: cfg.separate_tracks,
        };
        let has_audio = audio.mic.is_some() || audio.system;
        let whole = self.outputs.lock().unwrap().iter().find(|o| o.name == rec.output).is_some_and(|o| rec.local == Rect::new(0, 0, o.logical.w, o.logical.h));
        let target = if whole { Target::Output(rec.output.clone()) } else { Target::Area(rec.logical) };
        // Studio (D54; never a GIF): the pointer left out, to be redrawn
        let studio = cfg.studio && !rec.gif;
        let spec = Spec {
            target,
            fps: cfg.fps.clamp(1, 240),
            cursor: cfg.show_cursor && !studio,
            audio,
            codec: "auto".into(),
            quality: cfg.quality.as_str().into(),
            max_size: cfg.max_box(),
            cpu: false,
            file: file.clone(),
            ipc: ipc.clone(),
        };
        let size = ((rec.local.w as f64 * rec.scale).round() as u32, (rec.local.h as f64 * rec.scale).round() as u32);
        // the largest size only to shrink it: gsr scales up to fit too
        let max_size = spec.max_size.filter(|&(w, h)| size.0 > w || size.1 > h);
        let spec = Spec { cpu: !record::gpu_can_encode(size.0, size.1), max_size, ..spec };
        let recorder = Recorder::from_env();
        let fake = matches!(recorder, Recorder::TestPattern);
        // D57: a studio take's pointer on the hardware plane, so gsr can leave it
        // out (Hyprland's auto mode draws it into the screen on some setups,
        // spike S2); the user's own choice of software is kept, and the pointer
        // is then in the picture, never redrawn
        let hw = studio.then(|| snip_hypr::option_int("cursor:no_hardware_cursors")).flatten();
        // only with DRM outputs alone: forced hardware cursors crash Hyprland
        // 0.56 at the next pointer move on a nested session's WAYLAND- output
        // (measured), and maybe on a HEADLESS- one
        let others: Vec<String> = self.outputs.lock().unwrap().iter().filter(|o| o.name.starts_with("WAYLAND-") || o.name.starts_with("HEADLESS-")).map(|o| o.name.clone()).collect();
        let cursor = (hw == Some(2) && others.is_empty() && !fake).then_some(2);
        if cursor.is_some() {
            super::hypr("hardware_pointer", snip_hypr::run("hl.config({ cursor = { no_hardware_cursors = 0 } })"));
            self.apply_pointer_setting();
        }
        let take = match Take::start(&spec, size, has_audio, recorder) {
            Ok(t) => Arc::new(t),
            Err(e) => {
                drop(r);
                self.toast(format!("Can't record: {e}"));
                return self.record_close(false);
            }
        };
        rec.state = State::Recording;
        // every take logs its clicks (D54), a studio take its command keys too;
        // the HUD shows what it's asked to, nothing in a studio take (it would
        // be in the video, and redrawn: twice)
        let hooks = Some((cfg.show_keystrokes || studio, true));
        let hud = if studio { (false, false) } else { (cfg.show_keystrokes, cfg.highlight_clicks) };
        let sid = stamp.replace('-', "");
        let header = Header {
            fps: spec.fps as f64,
            encoder: if fake { "fake" } else if spec.cpu { "x264" } else { "nvenc" }.into(),
            output: rec.output.clone(),
            scale: rec.scale,
            area: [rec.logical.x, rec.logical.y, rec.logical.w, rec.logical.h],
            video: [size.0, size.1],
            cursor: Cursor {
                theme: ["HYPRCURSOR_THEME", "XCURSOR_THEME"].iter().filter_map(|v| std::env::var(v).ok()).find(|v| !v.is_empty()).unwrap_or_else(|| "default".into()),
                size: ["HYPRCURSOR_SIZE", "XCURSOR_SIZE"].iter().filter_map(|v| std::env::var(v).ok()?.parse().ok()).next().unwrap_or(24),
                hide_on_key_press: snip_hypr::option_bool("cursor:hide_on_key_press").unwrap_or(false),
            },
            keys: if studio { "commands" } else { "none" }.into(),
            studio,
            // not surely out of the video (no hardware plane for it): never
            // redrawn over it (the test pattern never has it)
            baked: studio && !fake && hw != Some(0) && cursor.is_none(),
            baked_why: if hw == Some(1) { "software".into() } else { others.join(", ") },
            clicks: cfg.highlight_clicks,
            keystrokes: cfg.show_keystrokes,
            ..Header::default()
        };
        // the pointer twice a frame, as Hyprland has it, at most 120 times a
        // second (spike S4: that costs Hyprland 0.4 % of a core; 240, 0.7 %)
        let log = TakeLog::start(file.with_extension("input.jsonl"), header, take.clone(), (2.0 * spec.fps as f64).min(120.0));
        // CAP-12: with Hide desktop clutter on, the bars show the wallpaper and
        // pins step aside while it records (unless they're hidden already)
        let covered = self.cfg().capture.hide_desktop && !self.desktop_is_hidden();
        rec.run = Some(Run { take: take.clone(), sid: sid.clone(), hooks, hud, studio, cursor, log: Some(log), covered, dnd: cfg.dnd });
        drop(r);
        if covered {
            self.desktop("hide");
        }
        if cfg.dnd {
            self.dnd(true);
        }
        // the screen doesn't idle into the lock screen mid-recording
        self.wl(snip_wayland::Request::KeepAwake(true));
        if let Some((keys, clicks)) = hooks {
            self.record_hooks(&sid, keys, clicks, studio, cursor);
        }
        self.record_status(json!({ "elapsed": 0 }));
        self.record_listen();
        let me = self.clone();
        std::thread::spawn(move || me.record_watch(take));
    }

    /// While a take records: its first frame (the clock starts), or its
    /// recorder ending by itself (an error, or something else sending it
    /// SIGINT, such as Omarchy's bar button): then the recording ends as if
    /// stopped. And the disk isn't full yet.
    fn record_watch(self: Arc<Self>, take: Arc<Take>) {
        let mut checked = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(150));
            let current = self.recording.lock().unwrap().as_ref().and_then(|r| r.run.as_ref()).is_some_and(|run| Arc::ptr_eq(&run.take, &take));
            if !current {
                return;
            }
            if checked.elapsed() >= Duration::from_secs(2) {
                checked = Instant::now();
                if take::disk_low() {
                    trace::warn("record_disk_low", "");
                    self.record_finish(true);
                    return self.toast("The disk is almost full: the recording stopped");
                }
            }
            match take.poll() {
                Some(Event::Rolling) => self.record_status(json!({ "elapsed": 0 })),
                Some(Event::Ended { saved: true, .. }) => return self.record_finish(true),
                Some(Event::Ended { why, .. }) => {
                    self.toast(format!("Can't record: {why}"));
                    return self.record_finish(false);
                }
                None => {}
            }
        }
    }

    /// Pause or resume (REC-05). gsr is asked outside the lock: the
    /// Hyprland event thread takes it for each click and key.
    pub fn record_pause(&self, pause: bool) {
        let (take, hooks) = {
            let r = self.recording.lock().unwrap();
            let Some(rec) = r.as_ref().filter(|r| matches!(r.state, State::Recording | State::Paused)) else { return };
            let Some(run) = rec.run.as_ref().filter(|_| (rec.state == State::Paused) != pause) else { return };
            (run.take.clone(), run.hooks.is_some())
        };
        if let Err(e) = take.pause(pause) {
            trace::warn("gsr_pause", &format!("\"msg\":{}", js(&e)));
            return;
        }
        if hooks {
            super::hypr("recording_mute", snip_hypr::keys::recording_mute(pause));
        }
        let mut r = self.recording.lock().unwrap();
        // still this take: it may have been stopped meanwhile
        let Some(rec) = r.as_mut().filter(|rec| rec.run.as_ref().is_some_and(|run| Arc::ptr_eq(&run.take, &take))) else { return };
        rec.state = if pause { State::Paused } else { State::Recording };
        if let Some(log) = rec.run.as_ref().and_then(|run| run.log.as_ref()) {
            log.pause(pause);
        }
        drop(r);
        self.record_status(json!({ "elapsed": take.elapsed().as_millis() as u64 }));
    }

    /// Stop and keep it (REC-05's stop; the Record hotkey again).
    pub fn record_stop(self: &Arc<Self>) {
        let me = self.clone();
        std::thread::spawn(move || me.record_finish(true));
    }

    /// Throw this take away (REC-05's trash), or close the panel.
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
                let dnd = run.dnd;
                me.record_end_run(run, false);
                if dnd {
                    me.dnd(false);
                }
            }
            me.record_go();
        });
    }

    /// `snip record area|fullscreen` (API-01): record this at once, no
    /// panel. `rect` is logical px on the monitor.
    pub fn record_area(self: &Arc<Self>, monitor: Option<String>, rect: Option<[i32; 4]>, gif: bool, waiter: Waiter) -> Result<(), String> {
        let output = monitor.filter(|m| self.output_names().contains(m)).or_else(|| self.cursor_output()).ok_or("no output")?;
        let o = self.outputs.lock().unwrap().iter().find(|o| o.name == output).map(|o| o.logical).ok_or("no such output")?;
        let scale = self.output_scale(&output);
        let [x, y, w, h] = rect.unwrap_or([0, 0, o.w, o.h]);
        if w < 2 || h < 2 || x < 0 || y < 0 || x + w > o.w || y + h > o.h {
            return Err(format!("the area must lie inside {output} ({}x{} logical)", o.w, o.h));
        }
        self.record_open(output, geometry::logical_to_physical(Rect::new(x, y, w, h), scale), scale, Some(waiter), gif, true);
        Ok(())
    }

    /// The Record hotkey (or `snip record`): stop the recording that runs,
    /// close the panel, or open it.
    pub fn record_toggle(self: &Arc<Self>, gif: bool, waiter: Option<Waiter>) {
        let state = self.recording.lock().unwrap().as_ref().map(|r| r.state);
        match state {
            Some(State::Recording | State::Paused) => {
                reply(waiter, Response::ok(json!({ "stopping": true })));
                self.record_stop();
            }
            // SEL-09: pressed again while the panel is up, it closes it
            Some(State::Ready) => {
                reply(waiter, cancelled());
                self.record_cancel();
            }
            Some(_) => reply(waiter, Response::err("a recording is already open")),
            None => self.record_setup(gif, waiter),
        }
    }

    /// Record Screen (D49, CleanShot's): its panel at once, over the whole
    /// output under the pointer; the area (or a window) is chosen on it.
    fn record_setup(self: &Arc<Self>, gif: bool, waiter: Option<Waiter>) {
        // tests: the test output only, as for the overlays
        let output = std::env::var("SNIP_OUTPUTS").ok().filter(|s| !s.is_empty()).or_else(|| self.cursor_output());
        let o = output.as_ref().and_then(|n| self.outputs.lock().unwrap().iter().find(|o| &o.name == n).map(|o| o.logical));
        let (Some(output), Some(o)) = (output, o) else { return reply(waiter, Response::err("no output")) };
        let scale = self.output_scale(&output);
        self.record_open(output, geometry::logical_to_physical(Rect::new(0, 0, o.w, o.h), scale), scale, waiter, gif, false);
    }

    /// Where things stand, for `snip status` and tests.
    pub fn record_state(&self) -> Value {
        let r = self.recording.lock().unwrap();
        match r.as_ref() {
            None => Value::Null,
            Some(rec) => json!({
                "state": rec.state.name(), "output": rec.output, "gif": rec.gif,
                "area": [rec.local.x, rec.local.y, rec.local.w, rec.local.h],
                "elapsed": rec.run.as_ref().map_or(0, |r| r.take.elapsed().as_millis() as u64),
                "file": rec.run.as_ref().map(|r| r.take.file().to_path_buf()),
            }),
        }
    }

    fn record_status(&self, mut extra: Value) {
        // gsr takes about 0.4 s to start: until its first frame, not recording yet
        let state = self.recording.lock().unwrap().as_ref().map(|r| match (r.state, r.run.as_ref()) {
            (State::Recording, Some(run)) if !run.take.rolling() => "launching",
            (s, _) => s.name(),
        });
        let Some(state) = state else { return };
        extra["state"] = json!(state);
        let j = extra.to_string();
        trace::trace("record_status", &j[1..j.len() - 1]);
        self.ui(Ui::RecordStatus(j));
    }

    /// Hyprland applies a pointer setting at the pointer's next move: one
    /// (Snip's virtual pointer), then a moment for it to take.
    fn apply_pointer_setting(&self) {
        self.wl(snip_wayland::Request::PointerNudge);
        std::thread::sleep(Duration::from_millis(50));
    }

    /// Clicks and keys while `sid` records (REC-09/10): Hyprland's hooks send
    /// them as socket2 events; a heartbeat keeps the hooks, which remove
    /// themselves once it stops (Snip gone).
    fn record_hooks(self: &Arc<Self>, sid: &str, keys: bool, clicks: bool, typing: bool, cursor: Option<i64>) {
        if let Err(e) = snip_hypr::keys::recording_hooks(sid, keys, clicks, typing, cursor) {
            return trace::warn("record_hooks", &format!("\"reply\":{}", js(&e)));
        }
        trace::trace("record_hooks", &format!("\"sid\":\"{sid}\",\"keys\":{keys},\"clicks\":{clicks}"));
        let (me, sid) = (self.clone(), sid.to_string());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(1));
            let live = me.recording.lock().unwrap().as_ref().and_then(|r| r.run.as_ref()).is_some_and(|run| run.sid == sid);
            if !live {
                return;
            }
            let _ = snip_hypr::keys::recording_beat(&sid);
        });
    }

    /// A click, its release or a key from the hooks (`snip:<sid>:c:<button>:
    /// <x>:<y>` in global logical px, `...:u:...` at the release, or
    /// `snip:<sid>:k:<code>:<0|1>`): into the take's log, and to the HUD
    /// what it shows.
    pub fn record_input(&self, data: &str) {
        let t = now_us();
        let Some(rest) = data.strip_prefix("snip:") else { return };
        let parts: Vec<&str> = rest.split(':').collect();
        let r = self.recording.lock().unwrap();
        let Some(rec) = r.as_ref().filter(|r| r.run.as_ref().is_some_and(|run| Some(&run.sid.as_str()) == parts.first())) else { return };
        let Some(run) = rec.run.as_ref() else { return };
        let origin = self.outputs.lock().unwrap().iter().find(|o| o.name == rec.output).map(|o| (o.logical.x as f64, o.logical.y as f64));
        let j = match parts.as_slice() {
            [_, "k", code, st] => {
                let Ok(code) = code.parse::<u32>() else { return };
                if let Some(log) = run.log.as_ref().filter(|_| run.studio) {
                    log.push(Input::Key { t, code, down: *st == "1" });
                }
                if !run.hud.0 {
                    return;
                }
                json!({ "kind": "key", "code": code, "down": *st == "1", "label": snip_hypr::keys::key_label(code) })
            }
            [_, "t"] => {
                if let Some(log) = run.log.as_ref().filter(|_| run.studio) {
                    log.push(Input::Typing { t });
                }
                return;
            }
            [_, c @ ("c" | "u"), button, x, y] => {
                let (Ok(x), Ok(y), Some((ox, oy))) = (x.parse::<f64>(), y.parse::<f64>(), origin) else { return };
                let button = button.parse::<u32>().unwrap_or(272);
                if let Some(log) = run.log.as_ref() {
                    log.push(Input::Button { t, button, down: *c == "c", at: [x, y] });
                }
                if *c == "u" || !run.hud.1 {
                    return;
                }
                json!({ "kind": "click", "button": button, "x": x - ox, "y": y - oy })
            }
            _ => return,
        };
        drop(r);
        let j = j.to_string();
        self.ui(Ui::RecordInput(j));
    }

    /// Stop a take's recorder (its file is finished, or deleted), then what
    /// the take turned on around it: none of that may end up in the video.
    fn record_end_run(&self, mut run: Run, keep: bool) -> Option<PathBuf> {
        let stopped = run.take.stop(keep);
        // the log, all of it, beside the take (`<take>.input.jsonl`); none for a take not kept
        let log = run.log.take().and_then(TakeLog::finish);
        if let (Some(log), false) = (log, matches!(stopped, Stopped::Kept(_))) {
            let _ = std::fs::remove_file(log);
        }
        self.wl(snip_wayland::Request::KeepAwake(false));
        if run.covered {
            self.desktop("show");
        }
        if run.hooks.is_some() {
            super::hypr("recording_hooks", snip_hypr::keys::recording_hooks_off());
        }
        // the user's pointer setting again (the hooks put it back too, even
        // without snipd), applied at once
        if let Some(v) = run.cursor {
            super::hypr("hardware_pointer", snip_hypr::run(&format!("hl.config({{ cursor = {{ no_hardware_cursors = {v} }} }})")));
            self.apply_pointer_setting();
        }
        match stopped {
            Stopped::Kept(file) => Some(file),
            Stopped::BeforeFirstFrame => {
                self.toast("Stopped before the first frame: nothing was recorded");
                None
            }
            Stopped::Thrown => None,
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
        // the microphone isn't listened to while gsr finishes
        rec.meter = None;
        let dnd = rec.run.as_ref().is_some_and(|run| run.dnd);
        let studio = rec.run.as_ref().is_some_and(|run| run.studio);
        // gsr first: the HUD (the camera, the keys) stays until the file is finished
        let file = rec.run.take().and_then(|run| self.record_end_run(run, keep));
        self.ui(Ui::RecordClose);
        if dnd {
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
        let log = file.with_extension("input.jsonl");
        let ingested = self.ingest_media(&file, true, meta);
        // its input log goes with it (D54)
        match ingested.as_ref().map(|r| self.store.keep_input_log(&r.id, &log)) {
            Ok(Err(e)) if log.exists() => trace::warn("input_log", &format!("\"msg\":{}", js(&e.to_string()))),
            Err(_) => {
                let _ = std::fs::remove_file(&log);
            }
            _ => {}
        }
        match ingested {
            Ok(r) => {
                self.show_card(&r, &rec.output);
                // a studio take goes on in the video editor (D58)
                if studio {
                    self.video_open(&r.id);
                }
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
        let takes = take::leftovers();
        if takes.is_empty() {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            let mut kept = 0;
            for t in takes {
                let output = me.cursor_output().unwrap_or_default();
                let meta = CaptureMeta { kind: "recording".into(), scale: 1.0, output: output.clone(), app: None, title: None };
                match take::recover(&t).and_then(|f| me.ingest_media(&f, true, meta)) {
                    Ok(r) => {
                        // its input log, as far as it got
                        let _ = me.store.keep_input_log(&r.id, &t.with_extension("input.jsonl"));
                        trace::trace("record_recovered", &format!("\"id\":\"{}\",\"ms\":{}", r.id, r.duration_ms));
                        me.show_card(&r, &output);
                        kept += 1;
                    }
                    // nothing in it (stopped before its first frame)
                    Err(e) => trace::warn("record_recover", &format!("\"file\":{},\"msg\":{}", js(&t.to_string_lossy()), js(&e))),
                }
                take::forget(&t);
            }
            if kept > 0 {
                me.toast(if kept == 1 { "Recovered a recording that was cut short".to_string() } else { format!("Recovered {kept} recordings that were cut short") });
            }
        });
    }

    /// The focus went nowhere, as it does when the session locks (REC-14): a
    /// recording pauses, so the lock screen isn't in it. It stays paused
    /// until resumed.
    pub fn record_check_lock(self: &Arc<Self>) {
        let recording = self.recording.lock().unwrap().as_ref().is_some_and(|r| r.state == State::Recording);
        if !recording {
            return;
        }
        let me = self.clone();
        std::thread::spawn(move || {
            // the lock surface comes a moment after the focus goes
            std::thread::sleep(Duration::from_millis(150));
            if snip_hypr::locked() {
                trace::trace("record_locked", "");
                me.record_pause(true);
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

    /// Close without recording (Esc on the panel).
    pub fn record_cancel(self: &Arc<Self>) {
        let ready = self.recording.lock().unwrap().as_ref().is_some_and(|r| matches!(r.state, State::Ready | State::Countdown));
        if ready {
            self.record_close(false);
        }
    }

    fn record_close(self: &Arc<Self>, _keep: bool) {
        let rec = self.recording.lock().unwrap().take();
        self.ui(Ui::RecordClose);
        if let Some(rec) = rec {
            reply(rec.waiter, cancelled());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muted_sources_are_read_from_pactl() {
        let text = "Source #61\n\tState: SUSPENDED\n\tName: alsa_input.usb-mic\n\tMute: yes\nSource #62\n\tName: alsa_output.x.monitor\n\tMute: no\n";
        let m = parse_mutes(text);
        assert_eq!((m.get("alsa_input.usb-mic"), m.get("alsa_output.x.monitor"), m.len()), (Some(&true), Some(&false), 2));
    }
}
