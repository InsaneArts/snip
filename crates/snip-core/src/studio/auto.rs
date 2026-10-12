//! Auto zoom (the design's §6.4; D54: on every take): zooms placed from
//! where a take's clicks and typing were, as CleanShot groups them, with
//! OpenScreen's guards. Deterministic, so a take always gets the same zooms.

use super::log::{Clock, Event, Log};
use super::{Motion, Zoom};

/// Auto zoom's rules, in one place to tune with the owner (the design's §6.4:
/// CleanShot's grouping and levels, OpenScreen's guards). Times are ms.
const NEAR_MS: f64 = 3000.0; // a press joins a group this soon after its last press,
const NEAR: f64 = 0.25; // and this near its middle (a share of the frame's diagonal)
const TYPED_MS: f64 = 2000.0; // a lone press zooms when typing follows this soon
const AHEAD_MS: f64 = 300.0; // a zoom is in this long before its first press,
const HOLD_MS: f64 = 1000.0; // and holds this long after its last (or after typing in it)
const LAST_MS: f64 = 1000.0; // presses this near the end don't count (the one that stopped the take)
const OPEN_MS: f64 = 2000.0; // the video opens wide this long,
const END_MS: f64 = 1000.0; // and ends wide this long
const SHORTEST_MS: f64 = 2000.0; // a zoom no shorter,
const LONGEST_MS: f64 = 60_000.0; // and no longer: a longer one is split into zooms that touch
const MERGE_MS: f64 = 3000.0; // zooms closer than this merge (or touch)
const STRETCH_MS: f64 = 2000.0; // typing this long with no zoom open zooms in at the last press,
const STRETCH_GAP_MS: f64 = 1000.0; // (typing with no gap longer than this)
const STRETCH_PRESS_MS: f64 = 10_000.0; // if it came this soon before,
const STRETCH_STILL: f64 = 0.1; // and the pointer stayed this near it (a share of the width)
const DENSE: usize = 3; // presses for the deeper level
const DEEP: f64 = 2.0;
const SHALLOW: f64 = 1.5;
const LEAST: f64 = 1.25; // lowered no further to fit its presses,
const MARGIN: f64 = 0.12; // with this much of the view around them

/// The buttons that zoom: left and right (Linux codes), not the middle one;
/// a press that goes further than DRAG_PX (px) before its release is a drag.
const LEFT: u32 = 272;
const RIGHT: u32 = 273;
const DRAG_PX: f64 = 8.0;

/// A take's zooms with auto zoom run again: the user's kept as they are,
/// auto zooms placed around them. `duration`: the video's, ms.
pub fn place(log: &Log, duration: f64, motion: Motion, zooms: &[Zoom]) -> Vec<Zoom> {
    let mut out: Vec<Zoom> = zooms.iter().filter(|z| !z.auto).cloned().collect();
    let Some(clock) = Clock::of(log) else { return out };
    let video = log.header.video.map(|v| f64::from(v.max(1)));
    let presses: Vec<(f64, [f64; 2], bool)> = pressed(log, &clock).into_iter().filter(|&(t, _, _)| t <= duration - LAST_MS).collect();
    let typed: Vec<f64> = log.events.iter().filter_map(|e| if let Event::Typing { t } = *e { clock.video_ms(t) } else { None }).collect();
    // each group that earns a zoom, in fully AHEAD_MS before its first press
    // and until HOLD_MS after its last
    let mut wanted: Vec<(f64, f64, Group)> = groups(presses.iter().copied(), video[0].hypot(video[1])).into_iter().filter_map(|g| {
        // a lone press doesn't zoom (long takes would get dozens), unless typing follows
        if g.n < 2 && !typed.iter().any(|&t| t > g.last && t <= g.last + TYPED_MS) {
            return None;
        }
        // typing in it keeps it in
        let mut hold = g.last + HOLD_MS;
        for &t in typed.iter().filter(|&&t| t > g.first) {
            if t > hold {
                break;
            }
            hold = hold.max(t + HOLD_MS);
        }
        Some((g.first - AHEAD_MS - motion.ramp(), hold + motion.ramp(), g))
    })
    .collect();
    // a stretch of typing with no zoom open: in at the last press before it, if
    // that was recent and the pointer stayed there (the caret is probably there)
    let moves: Vec<(f64, [f64; 2])> = log.events.iter().filter(|e| matches!(e, Event::Move { .. })).filter_map(|e| Some((clock.video_ms(e.t())?, log.header.video_px(e.place()?)))).collect();
    for (a, b) in stretches(&typed) {
        let open = wanted.iter().any(|&(from, to, _)| a >= from && a < to);
        let Some(&(t, p, _)) = presses.iter().rev().find(|&&(t, _, press)| press && t <= a) else { continue };
        let since = &moves[moves.partition_point(|&(m, _)| m <= t)..moves.partition_point(|&(m, _)| m <= a)];
        let stayed = !since.iter().any(|&(_, q)| (q[0] - p[0]).hypot(q[1] - p[1]) > STRETCH_STILL * video[0]);
        if b - a >= STRETCH_MS && !open && a - t <= STRETCH_PRESS_MS && stayed {
            let g = Group { first: a, last: b, n: 1, points: 1, sum: p, lo: p, hi: p };
            wanted.push((a - AHEAD_MS - motion.ramp(), b + HOLD_MS + motion.ramp(), g));
        }
    }
    wanted.sort_by(|x, y| x.0.total_cmp(&y.0));
    // close ones merge, or, when one view can't show both, touch: the view pans
    let mut placed: Vec<(f64, f64, Group)> = vec![];
    for (from, to, g) in wanted {
        if let Some((_, last_to, last)) = placed.last_mut().filter(|(_, last_to, _)| from - *last_to < MERGE_MS) {
            let both = last.and(&g);
            if level(&both, video).is_some() {
                (*last, *last_to) = (both, last_to.max(to));
                continue;
            }
            *last_to = from;
        }
        placed.push((from, to, g));
    }
    for (from, to, g) in placed {
        let (from, to) = (from.max(OPEN_MS), to.min(duration - END_MS));
        if to - from < SHORTEST_MS {
            continue;
        }
        let middle = [0, 1].map(|i| ((g.lo[i] + g.hi[i]) / 2.0 / video[i] * 1e4).round() / 1e4);
        let parts = ((to - from) / LONGEST_MS).ceil();
        let at = |k: f64| (from + (to - from) * k / parts).round();
        for k in 0..parts as usize {
            let k = k as f64;
            let mut z = Zoom { from: at(k), to: at(k + 1.0), level: level(&g, video).unwrap_or(LEAST), focus: middle, follow: true, auto: true };
            // never over one the user placed: the longer part beside it
            for u in zooms.iter().filter(|u| !u.auto) {
                if z.from >= u.to || u.from >= z.to {
                    continue;
                }
                if u.from - z.from >= z.to - u.to {
                    z.to = u.from;
                } else {
                    z.from = u.to;
                }
            }
            if z.to - z.from >= SHORTEST_MS {
                out.push(z);
            }
        }
    }
    out.sort_by(|a, b| a.from.total_cmp(&b.from));
    out
}

/// A group's level: DEEP for DENSE presses, else SHALLOW; lowered until the
/// box around them fits in the view with MARGIN at each side. None when that
/// would take it below LEAST.
fn level(g: &Group, video: [f64; 2]) -> Option<f64> {
    let fits = [0, 1].map(|i| (1.0 - 2.0 * MARGIN) * video[i] / (g.hi[i] - g.lo[i]).max(1e-9));
    let base = if g.n >= DENSE { DEEP } else { SHALLOW };
    let level = base.min(fits[0]).min(fits[1]);
    (level >= LEAST).then(|| (level * 100.0).floor() / 100.0)
}

/// Presses near each other in time and place: their first and last time,
/// how many, the box around them and their drags' ends (px), and its
/// running middle (`sum` of its `points`).
#[derive(Clone)]
struct Group {
    first: f64,
    last: f64,
    n: usize,
    points: usize,
    sum: [f64; 2],
    lo: [f64; 2],
    hi: [f64; 2],
}

impl Group {
    /// This group and `other` as one.
    fn and(&self, other: &Group) -> Group {
        Group {
            first: self.first.min(other.first),
            last: self.last.max(other.last),
            n: self.n + other.n,
            points: self.points + other.points,
            sum: [self.sum[0] + other.sum[0], self.sum[1] + other.sum[1]],
            lo: [self.lo[0].min(other.lo[0]), self.lo[1].min(other.lo[1])],
            hi: [self.hi[0].max(other.hi[0]), self.hi[1].max(other.hi[1])],
        }
    }
}

/// Typing's stretches (ms of the video): markers with no gap longer than STRETCH_GAP_MS.
fn stretches(typed: &[f64]) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = vec![];
    for &t in typed {
        match out.last_mut() {
            Some((_, b)) if t - *b <= STRETCH_GAP_MS => *b = t,
            _ => out.push((t, t)),
        }
    }
    out
}

/// What auto zoom looks at, in the video (ms, px): left and right presses
/// (true), and where each drag ended (false).
fn pressed(log: &Log, clock: &Clock) -> Vec<(f64, [f64; 2], bool)> {
    let (mut out, mut held) = (vec![], None);
    for e in &log.events {
        let Event::Button { t, button, down, at } = *e else { continue };
        let (Some(ms), true) = (clock.video_ms(t), button == LEFT || button == RIGHT) else { continue };
        let p = log.header.video_px(at);
        if down {
            out.push((ms, p, true));
            held = Some((button, p));
        } else if let Some((_, from)) = held.take().filter(|&(b, _)| b == button) {
            if (p[0] - from[0]).hypot(p[1] - from[1]) > DRAG_PX {
                out.push((ms, p, false));
            }
        }
    }
    out
}

/// The presses (ms, px) in groups: one joins the group before it when it
/// comes within NEAR_MS of its last press and NEAR of the frame's diagonal
/// (`diagonal`, px) of its middle. A drag's end is its press's.
fn groups(presses: impl Iterator<Item = (f64, [f64; 2], bool)>, diagonal: f64) -> Vec<Group> {
    let mut out: Vec<Group> = vec![];
    for (t, p, press) in presses {
        let one = Group { first: t, last: t, n: press as usize, points: 1, sum: p, lo: p, hi: p };
        match out.last_mut() {
            Some(g) if !press => *g = g.and(&one),
            Some(g) if t - g.last <= NEAR_MS && (g.sum[0] / g.points as f64 - p[0]).hypot(g.sum[1] / g.points as f64 - p[1]) <= NEAR * diagonal => *g = g.and(&one),
            _ if press => out.push(one),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::log::Header;

    // the moment `s` seconds into the video (the test pattern recorder's clock)
    fn at(s: f64) -> i64 {
        (10_000_000.0 + 1e6 / 60.0 + s * 1e6).round() as i64
    }
    fn take(events: Vec<Event>) -> Log {
        let header = Header { gsr_ts: Some([10_000_000, 0]), fps: 60.0, encoder: "fake".into(), area: [0, 0, 1000, 500], video: [1000, 500], ..Header::default() };
        Log { header, events }
    }
    // a left click at `s` seconds, at x, y (px of the 1000×500 video)
    fn click(s: f64, x: f64, y: f64) -> [Event; 2] {
        [Event::Button { t: at(s), button: 272, down: true, at: [x, y] }, Event::Button { t: at(s + 0.08), button: 272, down: false, at: [x, y] }]
    }
    fn clicks(list: &[(f64, f64, f64)]) -> Vec<Event> {
        list.iter().flat_map(|&(s, x, y)| click(s, x, y)).collect()
    }
    // auto zoom on a take `seconds` long, nothing placed by hand
    fn auto(events: Vec<Event>, seconds: f64) -> Vec<Zoom> {
        place(&take(events), seconds * 1000.0, Motion::Smooth, &[])
    }

    #[test]
    fn a_lone_click_doesnt_zoom() {
        assert_eq!(auto(clicks(&[(5.0, 300.0, 200.0)]), 20.0), vec![]);
    }

    #[test]
    fn a_burst_of_clicks_zooms_in_on_them_in_time() {
        let z = auto(clicks(&[(5.0, 300.0, 200.0), (5.8, 320.0, 210.0), (6.5, 310.0, 190.0)]), 20.0);
        assert_eq!(z.len(), 1, "{z:?}");
        // fully in 0.3 s before the first click (Smooth: in over 0.75 s), held 1 s after the last
        assert_eq!((z[0].from, z[0].to), (5000.0 - 300.0 - 750.0, 6500.0 + 1000.0 + 750.0));
        assert_eq!(z[0].level, 2.0, "three clicks: 2x");
        assert!(z[0].follow && z[0].auto);
        assert_eq!(z[0].focus, [0.31, 0.4], "the clicks' middle");
    }

    #[test]
    fn a_zoom_is_shallower_until_its_clicks_fit_in_it() {
        // two clicks: 1.5x
        assert_eq!(auto(clicks(&[(5.0, 300.0, 200.0), (5.5, 310.0, 200.0)]), 20.0)[0].level, 1.5);
        // three clicks 300 px apart down a 500 px frame: 2x would show 250 px, so
        // 1.26x (rounded down), where 300 px is the view less 12 % at each side
        let z = auto(clicks(&[(5.0, 500.0, 100.0), (5.5, 500.0, 300.0), (6.0, 500.0, 400.0)]), 20.0);
        assert_eq!(z[0].level, 1.26, "{z:?}");
        // never below 1.25x: further apart, the view follows the pointer instead
        let z = auto(clicks(&[(5.0, 300.0, 100.0), (5.5, 300.0, 300.0), (6.0, 300.0, 420.0)]), 20.0);
        assert_eq!(z[0].level, 1.25, "{z:?}");
    }

    #[test]
    fn the_click_that_stops_the_take_doesnt_count() {
        // a whole-screen take stops at a click on the bar (D52): the video never shows what it did
        assert_eq!(auto(clicks(&[(17.0, 300.0, 200.0), (19.3, 310.0, 200.0)]), 20.0), vec![]);
    }

    #[test]
    fn the_video_opens_wide_for_2_s_and_ends_wide_for_1_s() {
        let z = auto(clicks(&[(1.5, 300.0, 200.0), (2.0, 300.0, 200.0), (2.5, 300.0, 200.0), (3.0, 300.0, 200.0)]), 20.0);
        assert_eq!(z[0].from, 2000.0, "{z:?}");
        let z = auto(clicks(&[(16.5, 300.0, 200.0), (17.0, 300.0, 200.0), (17.4, 300.0, 200.0)]), 20.0);
        assert_eq!(z[0].to, 19000.0, "{z:?}");
        // clicks too early for a zoom of 2 s after that: none
        assert_eq!(auto(clicks(&[(0.5, 300.0, 200.0), (1.0, 300.0, 200.0)]), 20.0), vec![]);
    }

    // typing (the hooks' marker: at most every 500 ms) from `a` to `b` seconds
    fn typing(a: f64, b: f64) -> Vec<Event> {
        (0..).map(|i| a + i as f64 * 0.5).take_while(|&s| s <= b + 1e-9).map(|s| Event::Typing { t: at(s) }).collect()
    }

    #[test]
    fn a_click_then_typing_zooms_in_until_the_typing_ends() {
        // a click in a field, typing from 1 s after it until 9 s
        let mut events = clicks(&[(5.0, 300.0, 200.0)]);
        events.extend(typing(6.0, 9.0));
        let z = auto(events, 20.0);
        assert_eq!(z.len(), 1, "{z:?}");
        assert_eq!((z[0].from, z[0].to, z[0].level), (5000.0 - 300.0 - 750.0, 9000.0 + 1000.0 + 750.0, 1.5));
        // typing long after a lone click: not this rule
        let mut events = clicks(&[(5.0, 300.0, 200.0)]);
        events.extend(typing(8.0, 8.0));
        assert_eq!(auto(events, 20.0), vec![]);
    }

    #[test]
    fn zooms_close_together_merge_into_one() {
        // two pairs of clicks 3.5 s apart in one place: their zooms would be 0.7 s apart
        let z = auto(clicks(&[(5.0, 300.0, 200.0), (5.5, 300.0, 200.0), (9.0, 320.0, 220.0), (9.5, 320.0, 220.0)]), 20.0);
        assert_eq!(z.len(), 1, "{z:?}");
        assert_eq!((z[0].from, z[0].to), (3950.0, 11250.0));
        assert_eq!(z[0].level, 2.0, "four clicks: 2x");
    }

    #[test]
    fn close_zooms_too_far_apart_for_one_view_touch_and_pan() {
        let z = auto(clicks(&[(5.0, 100.0, 100.0), (5.5, 100.0, 100.0), (9.0, 900.0, 400.0), (9.5, 900.0, 400.0)]), 20.0);
        assert_eq!(z.len(), 2, "{z:?}");
        assert_eq!(z[0].to, z[1].from, "the first holds until the next comes in: {z:?}");
        assert_eq!((z[0].from, z[1].from, z[1].to), (3950.0, 7950.0, 11250.0));
    }

    #[test]
    fn zooms_far_apart_stay_apart() {
        let z = auto(clicks(&[(5.0, 100.0, 100.0), (5.5, 100.0, 100.0), (12.0, 900.0, 400.0), (12.5, 900.0, 400.0)]), 20.0);
        assert_eq!(z.iter().map(|z| (z.from, z.to)).collect::<Vec<_>>(), vec![(3950.0, 7250.0), (10950.0, 14250.0)]);
    }

    #[test]
    fn a_zoom_lasts_a_minute_at_most() {
        // a click every 2 s for two minutes, in one place: zooms that touch, none longer than 60 s
        let every: Vec<(f64, f64, f64)> = (0..=60).map(|i| (5.0 + i as f64 * 2.0, 300.0, 200.0)).collect();
        let z = auto(clicks(&every), 140.0);
        assert_eq!(z.len(), 3, "{z:?}");
        assert_eq!((z[0].from, z[2].to), (3950.0, 125_000.0 + 1750.0));
        assert!(z.windows(2).all(|w| w[0].to == w[1].from), "{z:?}");
        assert!(z.iter().all(|z| z.to - z.from <= 60_000.0), "{z:?}");
    }

    #[test]
    fn what_happens_while_paused_isnt_in_the_video() {
        // paused from 4 s to 9 s: the clicks then don't count, and later ones come 5 s sooner
        let mut events = vec![Event::Pause { t: at(4.0), paused: true }, Event::Pause { t: at(9.0), paused: false }];
        events.extend(clicks(&[(6.0, 300.0, 200.0), (6.5, 300.0, 200.0), (12.0, 700.0, 300.0), (12.5, 700.0, 300.0)]));
        let z = auto(events, 20.0);
        assert_eq!(z.iter().map(|z| (z.from, z.to, z.focus)).collect::<Vec<_>>(), vec![(5950.0, 9250.0, [0.7, 0.6])]);
    }

    #[test]
    fn the_middle_button_doesnt_zoom() {
        let middle = |s: f64| [Event::Button { t: at(s), button: 274, down: true, at: [300.0, 200.0] }, Event::Button { t: at(s + 0.1), button: 274, down: false, at: [300.0, 200.0] }];
        assert_eq!(auto([middle(5.0), middle(5.5), middle(6.0)].concat(), 20.0), vec![]);
    }

    #[test]
    fn a_drag_keeps_its_zoom_in_until_it_ends() {
        // a click, then a press dragged 200 px right for 2.5 s (a selection, a slider)
        let mut events = clicks(&[(5.0, 300.0, 200.0)]);
        events.push(Event::Button { t: at(5.5), button: 272, down: true, at: [300.0, 200.0] });
        events.push(Event::Button { t: at(8.0), button: 272, down: false, at: [500.0, 260.0] });
        let z = auto(events, 20.0);
        assert_eq!(z.iter().map(|z| (z.from, z.to, z.focus)).collect::<Vec<_>>(), vec![(3950.0, 8000.0 + 1750.0, [0.4, 0.46])]);
    }

    #[test]
    fn the_users_own_zooms_stay_where_they_are() {
        let mine = Zoom { from: 4000.0, to: 9000.0, level: 3.0, focus: [0.2, 0.2], follow: false, auto: false };
        let old = Zoom { from: 12_000.0, to: 15_000.0, level: 2.0, focus: [0.5, 0.5], follow: true, auto: true };
        let events = clicks(&[(5.0, 300.0, 200.0), (5.5, 300.0, 200.0), (6.0, 300.0, 200.0), (15.0, 700.0, 300.0), (15.5, 700.0, 300.0)]);
        let z = place(&take(events), 20_000.0, Motion::Smooth, &[old, mine.clone()]);
        // the auto zoom placed before is gone; the new one over the user's isn't placed
        assert_eq!(z.len(), 2, "{z:?}");
        assert_eq!(z[0], mine);
        assert_eq!((z[1].from, z[1].to, z[1].auto), (13_950.0, 17_250.0, true));
    }

    #[test]
    fn a_stretch_of_typing_zooms_in_where_the_last_click_was() {
        // a click in a field, then typing: the caret is probably there
        let typed = |mut events: Vec<Event>, a: f64, b: f64| {
            events.extend(typing(a, b));
            events.sort_by_key(Event::t);
            auto(events, 30.0)
        };
        // 3 s of it, starting 3 s after the click
        let z = typed(clicks(&[(5.0, 300.0, 200.0)]), 8.0, 11.0);
        assert_eq!(z.iter().map(|z| (z.from, z.to, z.level, z.focus)).collect::<Vec<_>>(), vec![(6950.0, 12_750.0, 1.5, [0.3, 0.4])]);
        // not when the pointer went elsewhere since (more than 10 % of the width)
        let mut moved = clicks(&[(5.0, 300.0, 200.0)]);
        moved.push(Event::Move { t: at(6.0), at: [450, 200] });
        assert_eq!(typed(moved, 8.0, 11.0), vec![]);
        // nor when the click was more than 10 s before
        assert_eq!(typed(clicks(&[(5.0, 300.0, 200.0)]), 16.0, 19.0), vec![]);
        // nor for a moment of typing (under 2 s)
        assert_eq!(typed(clicks(&[(5.0, 300.0, 200.0)]), 8.0, 9.5), vec![]);
    }

    #[test]
    fn an_auto_zoom_into_one_the_user_placed_stops_short_of_it() {
        // the user moved the zoom before it a little later: it would overlap by 0.4 s
        let mine = Zoom { from: 3000.0, to: 7600.0, level: 2.0, focus: [0.3, 0.4], follow: true, auto: false };
        let events = clicks(&[(8.0, 700.0, 300.0), (8.5, 700.0, 300.0)]);
        let z = place(&take(events), 20_000.0, Motion::Smooth, &[mine.clone()]);
        assert_eq!(z.iter().map(|z| (z.from, z.to)).collect::<Vec<_>>(), vec![(3000.0, 7600.0), (7600.0, 10_250.0)]);
    }
}
