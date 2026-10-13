//! A studio take's pointer, redrawn (the design's §6.2): where it is at any
//! moment of the video, from its input log. Built once into a 240 Hz table,
//! the same for the preview and the export.

use serde::{Deserialize, Serialize};

use super::log::{Clock, Event, Log};

/// How the pointer's path is smoothed (CleanShot's choices).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    Natural,
    Smooth,
    Off,
}

/// The edit's pointer, as edit.json keeps it (the design's §5.2), with
/// CleanShot's defaults (D60).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show: bool,
    /// 1–5, times the theme's size
    pub size: f64,
    pub motion: Motion,
    /// smaller while a button is down
    pub press: bool,
    pub hide_typing: bool,
    pub hide_idle: bool,
}

impl Settings {
    pub const DEFAULT: Settings = Settings { show: true, size: 2.0, motion: Motion::Natural, press: true, hide_typing: true, hide_idle: false };
}

impl Default for Settings {
    fn default() -> Self {
        Settings::DEFAULT
    }
}

/// The pointer at one moment: where (video px), how much it shows (0–1),
/// how far a press has it down (0–1), and its hotspot as the take logged
/// it (its shape: spike S6; None, the arrow).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shown {
    pub at: [f64; 2],
    pub opacity: f64,
    pub press: f64,
    pub hot: Option<[i32; 2]>,
}

/// The table's step, and the poller's (ms).
const STEP: f64 = 1000.0 / 240.0;
const POLL: f64 = 1000.0 / 120.0;
/// A gap between moves longer than this (ms) is a stop, not a slow move.
const STILL: f64 = 67.0;
/// How long the table runs on after the last place (ms): the spring settles.
const SETTLE: f64 = 1000.0;
/// A track starts no earlier than BEFORE_MS before the video and covers no
/// more than TRACK_MS (ms): a log's times are its own word, and one broken
/// (or written to break Snip) mustn't ask for terabytes.
const BEFORE_MS: f64 = 60_000.0;
pub(crate) const TRACK_MS: f64 = 4.0 * 3600.0 * 1000.0;

/// A damped spring chasing the path: stiffness, mass, damping (the design's
/// presets, Cap's "Fast" and "Mellow").
#[derive(Clone, Copy)]
struct Spring {
    k: f64,
    m: f64,
    c: f64,
}
const NATURAL: Spring = Spring { k: 380.0, m: 1.0, c: 30.0 };
const SMOOTH: Spring = Spring { k: 470.0, m: 3.0, c: 70.0 };
/// The last moments before a press, and a drag (Cap's).
const STIFF: Spring = Spring { k: 530.0, m: 1.0, c: 40.0 };
const DRAG: Spring = Spring { k: 1000.0, m: 1.0, c: 40.0 };
/// Landing on a click: the spring heads for it this long before (ms), stiff
/// for the last of it; a press that travels further than this (px) is a drag.
const LAND: f64 = 500.0;
const LAND_STIFF: f64 = 175.0;
const DRAG_PX: f64 = 8.0;
/// Fades (ms): out while typing, and back; out when idle (after IDLE still);
/// a press's ease at each edge. Moving TYPING_PX (px) shows it again.
const TYPING_FADE: f64 = 150.0;
const TYPING_PX: f64 = 4.0;
const IDLE: f64 = 2000.0;
const IDLE_FADE: f64 = 400.0;
const PRESS_EASE: f64 = 130.0;
/// Motion blur reaches back this far at most (px).
const TRAIL: f64 = 480.0;

/// 0 → 1 between 0 and 1, eased at both ends (all of the pointer's fades).
fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Spring {
    /// How far ahead (ms) it chases the path: by c/k, a spring trails what it chases.
    fn lead(self) -> f64 {
        1000.0 * self.c / self.k
    }
}

/// A button held: when it went down and up (ms of the video), where (px),
/// and whether the pointer travelled with it (a drag).
struct Press {
    down: f64,
    up: f64,
    at: [f64; 2],
    drag: bool,
}

/// The presses in a log, each with its release (none: held to the end).
fn presses(log: &Log, clock: &Clock, path: &[(f64, [f64; 2])]) -> Vec<Press> {
    let mut out: Vec<Press> = vec![];
    let mut held: Option<(u32, f64, [f64; 2])> = None;
    for e in &log.events {
        let Event::Button { t, button, down, at } = *e else { continue };
        let Some(ms) = clock.video_ms(t) else { continue };
        match (down, held) {
            (true, None) => held = Some((button, ms, log.header.video_px(at))),
            (false, Some((b, from, place))) if b == button => {
                out.push(Press { down: from, up: ms, at: place, drag: false });
                held = None;
            }
            _ => {}
        }
    }
    out.extend(held.map(|(_, from, place)| Press { down: from, up: f64::INFINITY, at: place, drag: false }));
    for p in &mut out {
        p.drag = path.iter().any(|&(t, q)| t >= p.down && t <= p.up && (q[0] - p.at[0]).hypot(q[1] - p.at[1]) > DRAG_PX);
    }
    out
}

/// A take's pointer, ready to be asked about any moment.
#[derive(Clone, Debug)]
pub struct Track {
    from: f64,
    shown: Vec<Shown>,
    video: [f64; 2],
    /// when the pointer's hotspot changed (ms of the video), and to what
    hots: Vec<(f64, [i32; 2])>,
}

impl Track {
    /// None when the log can't be timed, or never says where the pointer was.
    pub fn new(log: &Log, s: &Settings) -> Option<Track> {
        let clock = Clock::of(log)?;
        let path: Vec<(f64, [f64; 2])> = log.events.iter().filter_map(|e| Some((clock.video_ms(e.t())?, log.header.video_px(e.place()?)))).collect();
        let first = path.first()?.0;
        let last = log.events.iter().filter_map(|e| clock.video_ms(e.t())).fold(first, f64::max);
        let from = first.clamp(-BEFORE_MS, 0.0);
        let last = last.min(from + TRACK_MS);
        let n = ((last + SETTLE - from) / STEP).ceil() as usize + 1;
        let spring = match s.motion {
            Motion::Natural => Some(NATURAL),
            Motion::Smooth => Some(SMOOTH),
            Motion::Off => None,
        };
        let presses = presses(log, &clock, &path);
        // when it was moved, typed over (and moved more than TYPING_PX from
        // there), and in the picture already (a software cursor)
        let moves: Vec<f64> = log.events.iter().filter(|e| matches!(e, Event::Move { .. })).filter_map(|e| clock.video_ms(e.t())).collect();
        let typed: Vec<(f64, f64)> = log
            .events
            .iter()
            .filter(|e| matches!(e, Event::Typing { .. }))
            .filter_map(|e| clock.video_ms(e.t()))
            .map(|m| {
                let there = raw(&path, m);
                let back = path.iter().find(|&&(t, q)| t > m && (q[0] - there[0]).hypot(q[1] - there[1]) > TYPING_PX).map_or(f64::INFINITY, |&(t, _)| t);
                (m, back)
            })
            .collect();
        let baked = baked_spans(log, &clock);
        // the spring, in fixed steps from the start: the same table every time.
        // It chases the path ahead (Spring::lead); before a press it heads for
        // the press, and sits on it until the release, unless it's a drag
        let (mut x, mut v, dt) = (raw(&path, from), [0.0; 2], STEP / 1000.0);
        let mut next = 0;
        let shown = (0..n)
            .map(|i| {
                let t = from + i as f64 * STEP;
                while next < presses.len() && presses[next].up < t {
                    next += 1;
                }
                let since = if next > 0 { presses[next - 1].up } else { f64::NEG_INFINITY };
                let at = match (spring, presses.get(next)) {
                    (None, _) => raw(&path, t),
                    // from a step before: on it when it happens, wherever between steps
                    (Some(_), Some(p)) if t >= p.down - STEP && !p.drag => {
                        (x, v) = (p.at, [0.0; 2]);
                        x
                    }
                    (Some(sp), press) => {
                        let (sp, target) = match press {
                            Some(p) if t >= p.down => (DRAG, raw(&path, t + DRAG.lead())),
                            Some(p) if t >= p.down - LAND && t >= since => (if p.down - t <= LAND_STIFF { STIFF } else { sp }, p.at),
                            _ => (sp, raw(&path, t + sp.lead())),
                        };
                        for d in 0..2 {
                            v[d] += dt * (sp.k * (target[d] - x[d]) - sp.c * v[d]) / sp.m;
                            x[d] += dt * v[d];
                        }
                        x
                    }
                };
                let press = if !s.press {
                    0.0
                } else {
                    let held = |p: &Press| smooth((t - p.down) / PRESS_EASE) - smooth((t - p.up) / PRESS_EASE);
                    presses.get(next).map_or(0.0, held).max(if next > 0 { held(&presses[next - 1]) } else { 0.0 })
                };
                let typing = if s.hide_typing { typing_shown(&typed, t) } else { 1.0 };
                let idle = if s.hide_idle { idle_shown(&moves, t) } else { 1.0 };
                let drawn = s.show && !baked.iter().any(|&(a, b)| t >= a && t < b);
                Shown { at, opacity: if drawn { typing.min(idle) } else { 0.0 }, press, hot: None }
            })
            .collect();
        let hots = log.events.iter().filter_map(|e| if let Event::Shape { t, hot } = *e { Some((clock.video_ms(t)?, hot)) } else { None }).collect();
        Some(Track { from, shown, video: log.header.video.map(f64::from), hots })
    }

    /// The size (px) of the video its places are in.
    pub fn video(&self) -> [f64; 2] {
        self.video
    }

    /// Its motion blur at `ms` (the design's §6.6, Cap's): where it was over
    /// the last `level`/60 s (Low 0.5, Medium 1, High 2), at most TRAIL px
    /// back, as eight places, the nearest first; none while it's still.
    pub fn trail(&self, ms: f64, level: f64) -> Vec<[f64; 2]> {
        let now = self.at(ms).at;
        let mut span = level * 1000.0 / 60.0;
        let far = self.at(ms - span).at;
        let back = (now[0] - far[0]).hypot(now[1] - far[1]);
        if back < 1.0 {
            return vec![];
        }
        if back > TRAIL {
            span *= TRAIL / back;
        }
        (1..=8).map(|k| self.at(ms - span * k as f64 / 8.0).at).collect()
    }

    /// The pointer at `ms` of the video.
    pub fn at(&self, ms: f64) -> Shown {
        let last = self.shown.len() - 1;
        let f = ((ms - self.from) / STEP).clamp(0.0, last as f64);
        let (i, k) = (f.floor() as usize, f.fract());
        let (a, b) = (self.shown[i], self.shown[(i + 1).min(last)]);
        let mix = |x: f64, y: f64| x + (y - x) * k;
        // the hotspot last logged by then (the first one holds before it)
        let hot = self.hots.partition_point(|&(t, _)| t <= ms).checked_sub(1).or((!self.hots.is_empty()).then_some(0)).map(|i| self.hots[i].1);
        Shown { at: [mix(a.at[0], b.at[0]), mix(a.at[1], b.at[1])], opacity: mix(a.opacity, b.opacity), press: mix(a.press, b.press), hot }
    }
}

/// How much the pointer shows at `t` for typing: gone TYPING_FADE after the
/// last typing before `t`, until it moved TYPING_PX from there (`typed`:
/// each typing's time and that move's).
fn typing_shown(typed: &[(f64, f64)], t: f64) -> f64 {
    let i = typed.partition_point(|&(m, _)| m <= t);
    let Some(&(m, back)) = i.checked_sub(1).map(|i| &typed[i]) else { return 1.0 };
    if t >= back {
        smooth((t - back) / TYPING_FADE)
    } else {
        1.0 - smooth((t - m) / TYPING_FADE)
    }
}

/// How much the pointer shows at `t` for stillness: gone IDLE_FADE after
/// IDLE without a move, and back by the next move.
fn idle_shown(moves: &[f64], t: f64) -> f64 {
    let i = moves.partition_point(|&m| m <= t);
    let Some(&last) = i.checked_sub(1).map(|i| &moves[i]) else { return 1.0 };
    let out = 1.0 - smooth((t - last - IDLE) / IDLE_FADE);
    let back = moves.get(i).map_or(0.0, |&next| smooth((t - next + TYPING_FADE) / TYPING_FADE));
    out.max(back)
}

/// When the pointer was in the picture already ([from, to) ms of the video):
/// a software cursor, from the start (the header) or from a `baked` event.
fn baked_spans(log: &Log, clock: &Clock) -> Vec<(f64, f64)> {
    let (mut out, mut from) = (vec![], log.header.baked.then_some(f64::NEG_INFINITY));
    for e in &log.events {
        let Event::Baked { t, baked } = *e else { continue };
        let Some(ms) = clock.video_ms(t) else { continue };
        match (baked, from) {
            (true, None) => from = Some(ms),
            (false, Some(a)) => {
                out.push((a, ms));
                from = None;
            }
            _ => {}
        }
    }
    out.extend(from.map(|a| (a, f64::INFINITY)));
    out
}

/// The logged path at `ms`: straight from place to place, but still through a
/// long gap (a stop) until a poll before the next place.
fn raw(path: &[(f64, [f64; 2])], ms: f64) -> [f64; 2] {
    let i = path.partition_point(|&(t, _)| t <= ms);
    if i == 0 || i == path.len() {
        return path[i.saturating_sub(1)].1;
    }
    let ((ta, a), (tb, b)) = (path[i - 1], path[i]);
    let ta = if tb - ta > STILL { tb - POLL } else { ta };
    if ms <= ta {
        return a;
    }
    let f = (ms - ta) / (tb - ta);
    [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::log::{Event, Header, Log};

    const FRAME: f64 = 1e6 / 60.0;
    // the moment `s` seconds into the video (see log's tests: gsr_ts 10 s, 60 fps, NVENC)
    fn at(s: f64) -> i64 {
        (10_000_000.0 - 3.0 * FRAME + FRAME + s * 1e6).round() as i64
    }
    fn take(events: Vec<Event>) -> Log {
        let header = Header { gsr_ts: Some([10_000_000, 0]), fps: 60.0, area: [0, 0, 1000, 500], video: [1000, 500], ..Header::default() };
        Log { header, events }
    }
    fn moved(s: f64, x: i32, y: i32) -> Event {
        Event::Move { t: at(s), at: [x, y] }
    }
    fn near(a: [f64; 2], b: [f64; 2], px: f64) -> bool {
        (a[0] - b[0]).hypot(a[1] - b[1]) <= px
    }
    const OFF: Settings = Settings { motion: Motion::Off, ..Settings::DEFAULT };

    #[test]
    fn unsmoothed_the_pointer_goes_straight_from_place_to_place() {
        let p = Track::new(&take(vec![moved(0.0, 100, 100), moved(0.05, 200, 100), moved(0.1, 300, 150)]), &OFF).unwrap();
        // a move's place is the middle of its pixel
        assert!(near(p.at(25.0).at, [150.5, 100.5], 0.5), "{:?}", p.at(25.0));
        assert!(near(p.at(75.0).at, [250.5, 125.5], 0.5), "{:?}", p.at(75.0));
        assert!(near(p.at(-100.0).at, [100.5, 100.5], 0.5), "before its first move: where it started");
        assert!(near(p.at(5000.0).at, [300.5, 150.5], 0.5), "after its last: where it stopped");
    }

    // across at 600 px/s for a second, polled at 120 Hz, wobbling ±12 px up and down
    fn wobbling() -> Vec<Event> {
        (0..=120).map(|i| moved(i as f64 / 120.0, 100 + i * 5, if i % 2 == 0 { 188 } else { 212 })).collect()
    }

    #[test]
    fn smoothed_a_wobbling_path_is_calm() {
        let raw = Track::new(&take(wobbling()), &OFF).unwrap();
        let p = Track::new(&take(wobbling()), &Settings::DEFAULT).unwrap();
        let swing = |t: &Track| (300..700).step_by(5).map(|ms| (t.at(ms as f64).at[1] - 200.5).abs()).fold(0.0, f64::max);
        assert!(swing(&raw) > 10.0, "the raw path wobbles: {}", swing(&raw));
        assert!(swing(&p) < 3.0, "Natural: {}", swing(&p));
    }

    #[test]
    fn smoothed_a_steady_move_doesnt_trail_behind() {
        // across at 600 px/s: a spring trails what it chases by c/k (79 ms for Natural, 47 px here)
        let moves: Vec<Event> = (0..=120).map(|i| moved(i as f64 / 120.0, 100 + i * 5, 200)).collect();
        let raw = Track::new(&take(moves.clone()), &OFF).unwrap();
        for s in [Settings::DEFAULT, Settings { motion: Motion::Smooth, ..Settings::DEFAULT }] {
            let p = Track::new(&take(moves.clone()), &s).unwrap();
            for ms in [400.0, 550.0, 700.0] {
                let (got, want) = (p.at(ms).at[0], raw.at(ms).at[0]);
                assert!((got - want).abs() < 3.0, "{:?} at {ms} ms: {got:.1}, the path {want:.1}", s.motion);
            }
        }
    }

    fn button(s: f64, down: bool, x: f64, y: f64) -> Event {
        Event::Button { t: at(s), button: 272, down, at: [x, y] }
    }

    #[test]
    fn smoothed_the_pointer_lands_on_each_click() {
        // a fast move (1200 px/s) that ends on a button, pressed for 120 ms
        let mut events: Vec<Event> = (0..=60).map(|i| moved(i as f64 / 120.0, 100 + i * 10, 200)).collect();
        events.push(button(0.5, true, 700.3, 200.4));
        events.push(button(0.62, false, 700.3, 200.4));
        for s in [Settings::DEFAULT, Settings { motion: Motion::Smooth, ..Settings::DEFAULT }] {
            let p = Track::new(&take(events.clone()), &s).unwrap();
            for ms in [500.0, 560.0, 615.0] {
                assert!(near(p.at(ms).at, [700.3, 200.4], 0.5), "{:?} at {ms} ms: {:?}", s.motion, p.at(ms).at);
            }
        }
    }

    #[test]
    fn smoothed_a_drag_follows_the_pointer_not_the_press() {
        // pressed at 300, dragged to 500 over 0.3 s (a selection, a slider), released there
        let mut events = vec![moved(0.0, 300, 200), button(0.2, true, 300.5, 200.5)];
        events.extend((1..=36).map(|i| moved(0.2 + i as f64 / 120.0, 300 + i * 200 / 36, 200)));
        events.push(button(0.5, false, 500.5, 200.5));
        let raw = Track::new(&take(events.clone()), &OFF).unwrap();
        let p = Track::new(&take(events), &Settings::DEFAULT).unwrap();
        for ms in [300.0, 400.0] {
            let (got, want) = (p.at(ms).at[0], raw.at(ms).at[0]);
            assert!((got - want).abs() < 4.0, "at {ms} ms: {got:.1}, the path {want:.1}");
        }
    }

    #[test]
    fn the_pointer_fades_while_typing_until_it_moves() {
        let events = vec![moved(0.0, 100, 100), Event::Typing { t: at(1.0) }, moved(2.0, 110, 100)];
        let p = Track::new(&take(events.clone()), &Settings::DEFAULT).unwrap();
        assert!(((p.at(900.0).opacity) - 1.0).abs() < 1e-3);
        assert!(((p.at(1200.0).opacity) - 0.0).abs() < 1e-3, "gone 150 ms after the typing");
        assert!(((p.at(1900.0).opacity) - 0.0).abs() < 1e-3, "still, it stays gone");
        assert!(((p.at(2200.0).opacity) - 1.0).abs() < 1e-3, "back once it moved");
        let shown = Track::new(&take(events), &Settings { hide_typing: false, ..Settings::DEFAULT }).unwrap();
        assert!(((shown.at(1500.0).opacity) - 1.0).abs() < 1e-3, "unless the edit keeps it");
    }

    #[test]
    fn a_still_pointer_fades_when_the_edit_asks() {
        let events = vec![moved(0.0, 100, 100), moved(5.0, 300, 100)];
        let p = Track::new(&take(events.clone()), &Settings { hide_idle: true, ..Settings::DEFAULT }).unwrap();
        assert!(((p.at(1900.0).opacity) - 1.0).abs() < 1e-3);
        assert!(((p.at(2500.0).opacity) - 0.0).abs() < 1e-3, "gone 400 ms after 2 s still");
        assert!(((p.at(5000.0).opacity) - 1.0).abs() < 1e-3, "back by the time it moves");
        let kept = Track::new(&take(events), &Settings::DEFAULT).unwrap();
        assert!(((kept.at(2500.0).opacity) - 1.0).abs() < 1e-3, "CleanShot's default keeps it");
    }

    #[test]
    fn a_press_shrinks_the_pointer_while_its_held() {
        let events = vec![moved(0.0, 100, 100), button(0.5, true, 100.5, 100.5), button(0.8, false, 100.5, 100.5)];
        let p = Track::new(&take(events.clone()), &Settings::DEFAULT).unwrap();
        assert!(((p.at(400.0).press) - 0.0).abs() < 1e-3);
        assert!(((p.at(700.0).press) - 1.0).abs() < 1e-3, "down after 130 ms");
        assert!(((p.at(1000.0).press) - 0.0).abs() < 1e-3, "up 130 ms after the release");
        let flat = Track::new(&take(events), &Settings { press: false, ..Settings::DEFAULT }).unwrap();
        assert!(((flat.at(700.0).press) - 0.0).abs() < 1e-3, "unless the edit turns it off");
    }

    #[test]
    fn the_pointer_isnt_drawn_where_the_video_has_it() {
        let events = vec![moved(0.0, 100, 100), Event::Baked { t: at(1.0), baked: true }, Event::Baked { t: at(2.0), baked: false }];
        let p = Track::new(&take(events.clone()), &Settings::DEFAULT).unwrap();
        assert!(((p.at(500.0).opacity) - 1.0).abs() < 1e-3);
        assert!(((p.at(1500.0).opacity) - 0.0).abs() < 1e-3, "in the picture already: not drawn twice");
        assert!(((p.at(2500.0).opacity) - 1.0).abs() < 1e-3);
        let off = Track::new(&take(events), &Settings { show: false, ..Settings::DEFAULT }).unwrap();
        assert!(((off.at(500.0).opacity) - 0.0).abs() < 1e-3, "the edit hides it");
    }

    #[test]
    fn a_long_still_moment_is_a_stop_not_a_slow_move() {
        // a second between two moves: it stood still, then moved
        let p = Track::new(&take(vec![moved(0.0, 100, 100), moved(1.0, 300, 100)]), &OFF).unwrap();
        assert!(near(p.at(500.0).at, [100.5, 100.5], 0.5), "{:?}", p.at(500.0));
        assert!(near(p.at(1000.0).at, [300.5, 100.5], 0.5), "{:?}", p.at(1000.0));
    }

    #[test]
    fn a_moving_pointer_leaves_a_trail_and_a_still_one_none() {
        // across at 600 px/s: at Medium (1) the trail reaches back 1/60 s, 10 px
        let p = Track::new(&take((0..=120).map(|i| moved(i as f64 / 120.0, 100 + i * 5, 200)).collect()), &OFF).unwrap();
        let trail = p.trail(500.0, 1.0);
        assert_eq!(trail.len(), 8, "eight copies");
        let back = p.at(500.0).at[0] - trail[7][0];
        assert!((back - 10.0).abs() < 0.5, "{back} px back");
        assert!(trail.windows(2).all(|w| w[1][0] < w[0][0]), "each further back: {trail:?}");
        assert!((p.at(500.0).at[0] - p.trail(500.0, 2.0)[7][0] - 20.0).abs() < 0.5, "High: twice as long");
        // still: none
        let still = Track::new(&take(vec![moved(0.0, 100, 100)]), &OFF).unwrap();
        assert!(still.trail(500.0, 1.0).is_empty());
    }

    #[test]
    fn a_trail_is_480_px_at_most() {
        // a flick at 12,000 px/s: 8/60 s of it would be 1600 px
        let p = Track::new(&take((0..=30).map(|i| moved(i as f64 / 120.0, 100 + i * 100, 200)).collect()), &OFF).unwrap();
        let trail = p.trail(200.0, 8.0);
        let back = p.at(200.0).at[0] - trail[7][0];
        assert!(back <= 480.0 + 1e-9 && back > 470.0, "{back}");
    }

    #[test]
    fn the_pointer_has_the_shape_its_hotspot_says() {
        // the cursor session's hotspot (spike S6): an arrow, an I-beam from 1 s, a hand from 2 s
        let shape = |s: f64, x: i32, y: i32| Event::Shape { t: at(s), hot: [x, y] };
        let events = vec![shape(-0.2, 3, 1), moved(0.0, 100, 100), shape(1.0, 11, 12), shape(2.0, 7, 5)];
        let p = Track::new(&take(events), &Settings::DEFAULT).unwrap();
        assert_eq!(p.at(500.0).hot, Some([3, 1]), "from before the take");
        assert_eq!(p.at(1500.0).hot, Some([11, 12]));
        assert_eq!(p.at(2500.0).hot, Some([7, 5]));
        let none = Track::new(&take(vec![moved(0.0, 100, 100)]), &Settings::DEFAULT).unwrap();
        assert_eq!(none.at(500.0).hot, None, "a log without them: the arrow");
    }

    #[test]
    fn a_log_with_an_absurd_time_makes_a_track_of_sane_length() {
        // one event a year later (a log broken, or written to break Snip):
        // without a bound, its table would want terabytes
        let p = Track::new(&take(vec![moved(0.0, 100, 100), moved(1.0, 200, 100), moved(3.2e7, 300, 100)]), &OFF).unwrap();
        assert!(near(p.at(1000.0).at, [200.5, 100.5], 0.5));
        assert!(p.shown.len() <= ((TRACK_MS + SETTLE) / STEP) as usize + 2, "{} samples", p.shown.len());
        let p = Track::new(&take(vec![moved(-3.2e7, 100, 100), moved(1.0, 200, 100)]), &OFF).unwrap();
        assert!(near(p.at(1000.0).at, [200.5, 100.5], 0.5), "nor a year before: {:?}", p.at(1000.0));
    }

    #[test]
    fn a_broken_log_makes_a_track_or_none_without_panicking() {
        let text = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n",
            serde_json::to_string(&take(vec![]).header).unwrap(),
            Event::Move { t: at(0.1), at: [10, 20] }.line(),
            Event::Button { t: at(0.5), button: 272, down: true, at: [11.0, 21.0] }.line(),
            Event::Button { t: at(0.6), button: 272, down: false, at: [11.0, 21.0] }.line(),
            Event::Typing { t: at(0.8) }.line(),
            Event::Shape { t: at(0.9), hot: [11, 12] }.line(),
        );
        crate::fuzz::run(&[text.as_bytes()], 3000, |b| {
            if let Some(log) = crate::studio::log::read(&String::from_utf8_lossy(b)) {
                if let Some(p) = Track::new(&log, &Settings::DEFAULT) {
                    let _ = (p.at(0.0), p.at(1e9), p.trail(500.0, 2.0));
                }
            }
        });
    }
}
