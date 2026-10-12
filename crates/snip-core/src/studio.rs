//! Studio Mode (D55): what a recording's export shows at any moment, from
//! the zooms placed on it (the design's §6.3). Pure, so the editor's preview
//! and the export ask the same thing and show the same frame.

use serde::{Deserialize, Serialize};

pub mod cursor;
pub mod log;
pub mod pointer;

/// A zoom on the timeline: from `from` to `to` (the recording's own ms), at
/// `level` (1–4), framing `focus`, a point of the recording (0–1 across and
/// down), or following the pointer. `auto`: placed by auto zoom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zoom {
    pub from: f64,
    pub to: f64,
    pub level: f64,
    pub focus: [f64; 2],
    #[serde(default)]
    pub follow: bool,
    #[serde(default)]
    pub auto: bool,
}

/// How zooms move in and out, one setting for the whole edit (CleanShot's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    Smooth,
    Dynamic,
}

/// The Studio part of a recording's edit, as its `edit.json` keeps it (the
/// design's §5.2): the zooms, and how they move.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edit {
    pub zooms: Vec<Zoom>,
    #[serde(rename = "zoom_style")]
    pub motion: Motion,
}

/// What the export shows at one moment: the recording drawn `scale` times
/// its size, with its point `center` (0–1) in the middle of the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub scale: f64,
    pub center: [f64; 2],
}

/// The whole recording, unzoomed.
const WHOLE: View = View { scale: 1.0, center: [0.5, 0.5] };

/// Following the pointer (Cap's, the design's §6.3): the dead zone, the share
/// of the view (across, down) the pointer moves in before the view follows;
/// the spring the view follows with (stiffness, mass, damping: settled in
/// about 0.45 s), in steps of STEP ms; and how near an edge (a share of the
/// recording) the view goes flush against it.
const DEAD: [f64; 2] = [0.5, 0.7];
const FOLLOW: (f64, f64, f64) = (200.0, 2.25, 40.0);
const STEP: f64 = 1000.0 / 240.0;
const SNAP: f64 = 0.25;

/// What the export shows all through an edit, worked out once: where follow
/// zooms look comes from the pointer's path, step by step from each one's
/// start, so any moment is the same in the preview and the export.
pub struct Views {
    zooms: Vec<Zoom>,
    motion: Motion,
    /// a follow zoom's centre every STEP from its start; None: it stays on its focus
    follow: Vec<Option<Vec<[f64; 2]>>>,
}

impl Views {
    /// `pointer`: where the take's pointer went (none: follow zooms keep their focus).
    pub fn new(edit: &Edit, pointer: Option<&pointer::Track>) -> Views {
        let mut zooms = edit.zooms.clone();
        zooms.sort_by(|a, b| a.from.total_cmp(&b.from));
        let follow = (0..zooms.len())
            .map(|i| {
                let z = &zooms[i];
                let until = zooms.get(i + 1).filter(|n| touch(z, n, edit.motion)).map_or(z.to, |n| n.from);
                pointer.filter(|_| z.follow).map(|p| centres(z, until, p))
            })
            .collect();
        Views { zooms, motion: edit.motion, follow }
    }

    /// What the export shows at `t` (ms of the recording).
    pub fn at(&self, t: f64) -> View {
        let (zooms, motion) = (&self.zooms, self.motion);
        for (i, z) in zooms.iter().enumerate() {
            let to_next = zooms.get(i + 1).filter(|n| touch(z, n, motion));
            if t < z.from || t >= to_next.map_or(z.to, |n| n.from) {
                continue;
            }
            let r = ramp(z, motion);
            let (in_, out) = ((t - z.from) / r, (z.to - t) / r);
            if in_ < 1.0 {
                let from = i.checked_sub(1).filter(|&p| touch(&zooms[p], z, motion)).map_or(WHOLE, |p| self.framed(p, z.from));
                return between(from, self.framed(i, t), eased(in_, motion));
            }
            if out < 1.0 && to_next.is_none() {
                // where it looks holds still on the way out
                return between(WHOLE, self.framed(i, z.to - r), eased(out, motion));
            }
            return self.framed(i, t);
        }
        WHOLE
    }

    /// Zoom `i`'s view at `t` once it's in: its level (held to 1–4), centred
    /// on its focus, or where it follows the pointer to, as far as the
    /// recording allows (the view never shows past its edges).
    fn framed(&self, i: usize, t: f64) -> View {
        let z = &self.zooms[i];
        let level = z.level.clamp(1.0, 4.0);
        let half = 0.5 / level;
        let center = match &self.follow[i] {
            Some(c) => {
                let f = ((t - z.from) / STEP).clamp(0.0, (c.len() - 1) as f64);
                let (a, b, k) = (c[f.floor() as usize], c[(f.floor() as usize + 1).min(c.len() - 1)], f.fract());
                [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]
            }
            None => z.focus.map(|f| f.clamp(half, 1.0 - half)),
        };
        View { scale: level, center }
    }
}

/// A zoom that touches the next one doesn't go out: it holds until the next
/// one comes in from it, so close zooms pan instead of out and in again.
fn touch(a: &Zoom, b: &Zoom, motion: Motion) -> bool {
    b.from - a.to < ramp(a, motion) + ramp(b, motion)
}

/// Where a follow zoom looks, every STEP from its start until `until`. It
/// starts on the pointer (so it zooms in toward it); after that it heads for
/// where the pointer is only once it leaves the dead zone, and just far
/// enough to have it back in; near an edge it heads flush against it.
fn centres(z: &Zoom, until: f64, pointer: &pointer::Track) -> Vec<[f64; 2]> {
    let level = z.level.clamp(1.0, 4.0);
    let half = 0.5 / level;
    let video = pointer.video();
    let place = |t: f64| {
        let at = pointer.at(t).at;
        [at[0] / video[0].max(1.0), at[1] / video[1].max(1.0)]
    };
    let goal = |aim: [f64; 2]| {
        aim.map(|a| match a.clamp(half, 1.0 - half) {
            a if a < SNAP => half,
            a if a > 1.0 - SNAP => 1.0 - half,
            a => a,
        })
    };
    let (k, m, c) = FOLLOW;
    let mut aim = place(z.from);
    let (mut x, mut v, dt) = (goal(aim), [0.0; 2], STEP / 1000.0);
    let n = ((until - z.from) / STEP).ceil().max(0.0) as usize + 1;
    (0..n)
        .map(|i| {
            if i > 0 {
                let p = place(z.from + i as f64 * STEP);
                for d in 0..2 {
                    let reach = DEAD[d] * half;
                    aim[d] = aim[d].clamp(p[d] - reach, p[d] + reach);
                }
                let g = goal(aim);
                for d in 0..2 {
                    v[d] += dt * (k * (g[d] - x[d]) - c * v[d]) / m;
                    x[d] += dt * v[d];
                }
            }
            x
        })
        .collect()
}

/// How long a zoom takes to move in, and out: CleanShot's 0.75 s (Smooth) or
/// 0.4 s (Dynamic), or half the zoom when it's shorter than both.
fn ramp(z: &Zoom, motion: Motion) -> f64 {
    let most = match motion {
        Motion::Smooth => 750.0,
        Motion::Dynamic => 400.0,
    };
    ((z.to - z.from) / 2.0).min(most)
}

/// How far along a move is at `u` (0–1) of its time, from rest: smootherstep
/// for Smooth; for Dynamic a critically damped spring's curve (ωT = 7), most
/// of the way by half its time.
fn eased(u: f64, motion: Motion) -> f64 {
    match motion {
        Motion::Smooth => u * u * u * (u * (u * 6.0 - 15.0) + 10.0),
        Motion::Dynamic => {
            let spring = |x: f64| 1.0 - (1.0 + 7.0 * x) * (-7.0 * x).exp();
            spring(u) / spring(1.0)
        }
    }
}

/// The view `e` (0–1) of the way from `a` to `b`. The scale moves evenly in
/// log space, so 1x to 2x feels like 2x to 4x, and one point of the
/// recording stays put on screen: the zoom heads straight for where it's
/// going (between two views of one size, it pans).
fn between(a: View, b: View, e: f64) -> View {
    if e <= 0.0 {
        return a;
    }
    if e >= 1.0 {
        return b;
    }
    let scale = (a.scale.ln() + (b.scale.ln() - a.scale.ln()) * e).exp();
    let center = [0, 1].map(|i| {
        if (a.scale - b.scale).abs() < 1e-9 {
            a.center[i] + (b.center[i] - a.center[i]) * e
        } else {
            let still = (a.center[i] * a.scale - b.center[i] * b.scale) / (a.scale - b.scale);
            still - (still - a.center[i]) * a.scale / scale
        }
    });
    View { scale, center }
}

/// Where the recording's card is drawn for `view`, in the export's px: `at1`
/// is where it sits unzoomed (inside the background's padding, or the whole
/// `canvas`). The whole card grows (D56), and the point it zooms on takes the
/// place of its middle. Bigger than the canvas, it covers it, so no
/// background shows at an edge; smaller, it stays on it.
pub fn card(view: View, canvas: [f64; 2], at1: [f64; 4]) -> [f64; 4] {
    let size = [at1[2] * view.scale, at1[3] * view.scale];
    let [x, y] = [0, 1].map(|i| {
        let at = at1[i] + at1[i + 2] / 2.0 - view.center[i] * size[i];
        let room = canvas[i] - size[i];
        at.clamp(room.min(0.0), room.max(0.0))
    });
    [x, y, size[0], size[1]]
}


#[cfg(test)]
mod tests {
    use super::*;

    fn view(zooms: &[Zoom], motion: Motion, t: f64) -> View {
        Views::new(&Edit { zooms: zooms.to_vec(), motion }, None).at(t)
    }

    #[test]
    fn with_no_zoom_the_whole_recording_shows() {
        assert_eq!(view(&[], Motion::Smooth, 0.0), WHOLE);
        assert_eq!(view(&[], Motion::Dynamic, 12_345.0), WHOLE);
    }

    fn zoom(from: f64, to: f64, level: f64, focus: [f64; 2]) -> Zoom {
        Zoom { from, to, level, focus, follow: false, auto: false }
    }

    #[test]
    fn inside_a_zoom_the_view_is_its_level_at_its_focus() {
        let z = [zoom(1000.0, 5000.0, 2.0, [0.3, 0.6])];
        assert_eq!(view(&z, Motion::Smooth, 3000.0), View { scale: 2.0, center: [0.3, 0.6] });
        assert_eq!(view(&z, Motion::Smooth, 500.0), WHOLE, "before it");
        assert_eq!(view(&z, Motion::Smooth, 6000.0), WHOLE, "after it");
    }

    #[test]
    fn a_focus_near_an_edge_keeps_the_view_inside_the_recording() {
        // at 2x the view is half as wide and high: its centre stays 0.25 from each edge
        let z = [zoom(0.0, 4000.0, 2.0, [0.05, 0.9])];
        assert_eq!(view(&z, Motion::Smooth, 2000.0), View { scale: 2.0, center: [0.25, 0.75] });
    }

    #[test]
    fn a_zoom_moves_in_at_its_start_and_out_at_its_end_inside_its_span() {
        let z = [zoom(1000.0, 5000.0, 2.0, [0.5, 0.5])];
        let at = |motion, t| view(&z, motion, t).scale;
        // Smooth: 0.75 s each way
        assert_eq!(at(Motion::Smooth, 1000.0), 1.0, "it starts from the whole recording");
        assert!(at(Motion::Smooth, 1375.0) > 1.0 && at(Motion::Smooth, 1375.0) < 2.0, "on its way in");
        assert_eq!(at(Motion::Smooth, 1750.0), 2.0, "in");
        assert_eq!(at(Motion::Smooth, 4250.0), 2.0, "still in");
        assert!(at(Motion::Smooth, 4625.0) > 1.0 && at(Motion::Smooth, 4625.0) < 2.0, "on its way out");
        // Dynamic: 0.4 s
        assert_eq!(at(Motion::Dynamic, 1400.0), 2.0, "in sooner");
        assert!(at(Motion::Dynamic, 1200.0) < 2.0);
        // a zoom shorter than two moves spends half of itself on each
        let short = [zoom(1000.0, 2000.0, 2.0, [0.5, 0.5])];
        assert_eq!(view(&short, Motion::Smooth, 1500.0).scale, 2.0, "in at its middle");
        assert!(view(&short, Motion::Smooth, 1250.0).scale < 2.0);
    }

    #[test]
    fn a_zoom_moves_from_rest_and_evenly_at_every_level() {
        let z = [zoom(0.0, 4000.0, 2.0, [0.5, 0.5])];
        // how far in (0-1), in log space: as far from 1x to 2x as from 2x to 4x
        let into = |m, t| view(&z, m, t).scale.log2();
        assert!(into(Motion::Smooth, 0.05 * 750.0) < 0.01, "it starts from rest: 5 % of the time, under 1 % of the way");
        assert!((into(Motion::Smooth, 375.0) - 0.5).abs() < 1e-9, "halfway: the middle in log space, so 1.41x, not 1.5x");
        assert!(into(Motion::Dynamic, 8.0) < 0.01, "Dynamic starts from rest too");
        assert!(into(Motion::Dynamic, 200.0) > 0.8, "and is most of the way in by half its time");
    }

    #[test]
    fn while_a_zoom_moves_one_point_of_the_recording_stays_still() {
        // 1x at the middle to 2x at (0.3, 0.6): the point that stays put is (0.1, 0.7)
        let z = [zoom(0.0, 4000.0, 2.0, [0.3, 0.6])];
        let still = [0.1, 0.7];
        for t in [0.0, 100.0, 300.0, 500.0, 700.0, 3300.0, 3700.0, 3999.0] {
            let v = view(&z, Motion::Smooth, t);
            // where it is on screen, from the view's middle, in recordings
            let on_screen = [(still[0] - v.center[0]) * v.scale, (still[1] - v.center[1]) * v.scale];
            assert!((on_screen[0] + 0.4).abs() < 1e-9 && (on_screen[1] - 0.2).abs() < 1e-9, "at {t} ms: {on_screen:?}");
        }
    }

    #[test]
    fn zooms_close_together_pan_from_one_to_the_next_without_zooming_out() {
        // 0.2 s apart, less than their two moves (0.75 s each): one zoom to the next
        let z = [zoom(0.0, 3000.0, 2.0, [0.3, 0.3]), zoom(3200.0, 6000.0, 2.0, [0.7, 0.7])];
        for t in (750..5250).step_by(50) {
            assert_eq!(view(&z, Motion::Smooth, t as f64).scale, 2.0, "never out at {t} ms");
        }
        assert_eq!(view(&z, Motion::Smooth, 3100.0).center, [0.3, 0.3], "the first holds until the next");
        assert_eq!(view(&z, Motion::Smooth, 3575.0).center, [0.5, 0.5], "halfway across");
        assert_eq!(view(&z, Motion::Smooth, 3950.0).center, [0.7, 0.7], "there");
        assert_eq!(view(&z, Motion::Smooth, 6000.0), WHOLE, "out after the last");
    }

    #[test]
    fn a_zoom_grows_the_card_around_its_focus() {
        let canvas = [1920.0, 1080.0];
        let padded = [192.0, 108.0, 1536.0, 864.0];
        assert_eq!(card(WHOLE, canvas, padded), padded, "at 1x, where the background put it");
        // no background: 2x at (0.3, 0.6) puts that point at the canvas's middle
        let whole = [0.0, 0.0, 1920.0, 1080.0];
        let r = card(View { scale: 2.0, center: [0.3, 0.6] }, canvas, whole);
        assert_eq!(r, [-192.0, -756.0, 3840.0, 2160.0]);
        assert_eq!([r[0] + 0.3 * r[2], r[1] + 0.6 * r[3]], [960.0, 540.0]);
    }

    #[test]
    fn a_zoomed_card_covers_the_canvas_and_never_slides_off_it() {
        let canvas = [1920.0, 1080.0];
        // 2x near the top right: bigger than the canvas, flush with that corner, no background showing
        let r = card(View { scale: 2.0, center: [0.75, 0.25] }, canvas, [192.0, 108.0, 1536.0, 864.0]);
        assert_eq!(r, [1920.0 - 3072.0, 0.0, 3072.0, 1728.0]);
        // still smaller than the canvas (the background aligned it left): it stays on the canvas
        let r = card(View { scale: 1.5, center: [2.0 / 3.0, 0.5] }, canvas, [0.0, 270.0, 960.0, 540.0]);
        assert_eq!(r[0], 0.0, "not off the left edge: {r:?}");
    }

    #[test]
    fn a_level_outside_1_to_4_is_held_to_it() {
        // edit.json could say anything: a broken level never takes the export down
        let low = [zoom(0.0, 4000.0, 0.5, [0.2, 0.2])];
        assert_eq!(view(&low, Motion::Smooth, 2000.0), WHOLE);
        let high = [zoom(0.0, 4000.0, 9.0, [0.5, 0.5])];
        assert_eq!(view(&high, Motion::Smooth, 2000.0).scale, 4.0);
    }

    #[test]
    fn an_edit_reads_from_its_json_as_the_editor_writes_it() {
        let e: Edit = serde_json::from_str(r#"{"zooms":[{"from":1000,"to":5000,"level":2,"focus":[0.3,0.6]}],"zoom_style":"dynamic"}"#).unwrap();
        assert_eq!(e, Edit { zooms: vec![zoom(1000.0, 5000.0, 2.0, [0.3, 0.6])], motion: Motion::Dynamic });
        let none: Edit = serde_json::from_str("{}").unwrap();
        assert_eq!(none, Edit::default(), "no zooms yet, Smooth");
    }

    #[test]
    fn zooms_far_apart_go_out_between() {
        let z = [zoom(0.0, 3000.0, 2.0, [0.3, 0.3]), zoom(5000.0, 8000.0, 2.0, [0.7, 0.7])];
        assert_eq!(view(&z, Motion::Smooth, 4000.0), WHOLE);
    }

    // a take where the pointer went to these places (px of a 1000×500 video,
    // at s seconds of it), unsmoothed: the test pattern recorder's clock
    fn pointer(to: &[(f64, i32, i32)]) -> pointer::Track {
        use log::{Event, Header, Log};
        let header = Header { gsr_ts: Some([10_000_000, 0]), fps: 60.0, encoder: "fake".into(), area: [0, 0, 1000, 500], video: [1000, 500], ..Header::default() };
        let events = to.iter().map(|&(s, x, y)| Event::Move { t: (10_000_000.0 + 1e6 / 60.0 + s * 1e6).round() as i64, at: [x, y] }).collect();
        pointer::Track::new(&Log { header, events }, &pointer::Settings { motion: pointer::Motion::Off, ..pointer::Settings::DEFAULT }).unwrap()
    }
    fn follow(from: f64, to: f64, level: f64) -> Zoom {
        Zoom { follow: true, ..zoom(from, to, level, [0.5, 0.5]) }
    }
    fn views(zooms: Vec<Zoom>, p: Option<&pointer::Track>) -> Views {
        Views::new(&Edit { zooms, motion: Motion::Smooth }, p)
    }
    fn close(a: [f64; 2], b: [f64; 2]) -> bool {
        (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
    }

    #[test]
    fn a_follow_zoom_zooms_in_on_the_pointer_from_its_start() {
        // the pointer rests at (700, 200): a fixed zoom on that spot, all the way in
        let p = pointer(&[(0.0, 700, 200)]);
        let there = [700.5 / 1000.0, 200.5 / 500.0];
        let (f, fixed) = (views(vec![follow(1000.0, 5000.0, 2.0)], Some(&p)), views(vec![zoom(1000.0, 5000.0, 2.0, there)], None));
        for t in [1100.0, 1375.0, 3000.0, 4600.0] {
            let (a, b) = (f.at(t), fixed.at(t));
            assert!((a.scale - b.scale).abs() < 1e-9 && close(a.center, b.center), "at {t} ms: {a:?}, not {b:?}");
        }
    }

    #[test]
    fn a_follow_zoom_stays_still_while_the_pointer_moves_inside_its_dead_zone() {
        // at 2x the view is 500×250 px: the dead zone 250×175 around its middle
        let p = pointer(&[(0.0, 500, 250), (2.0, 600, 320)]);
        let v = views(vec![follow(1000.0, 5000.0, 2.0)], Some(&p));
        assert!(close(v.at(1800.0).center, [0.5005, 0.501]));
        assert!(close(v.at(3500.0).center, [0.5005, 0.501]), "{:?}", v.at(3500.0));
    }

    #[test]
    fn a_follow_zoom_follows_the_pointer_out_of_its_dead_zone_just_far_enough() {
        // at 2 s it moves to (800, 250): the view follows until the pointer is
        // back on the dead zone's edge, 125 px right of the middle
        let p = pointer(&[(0.0, 500, 250), (2.0, 800, 250)]);
        let v = views(vec![follow(1000.0, 6000.0, 2.0)], Some(&p));
        let middle = (800.5 - 125.0) / 1000.0;
        let way = |t: f64| (v.at(t).center[0] - 0.5005) / (middle - 0.5005);
        assert!(way(2200.0) > 0.0 && way(2200.0) < 0.9, "on its way: {}", way(2200.0));
        assert!(way(2450.0) > 0.9, "most of the way in 0.45 s: {}", way(2450.0));
        assert!(close(v.at(3000.0).center, [middle, 0.501]), "{:?}", v.at(3000.0));
        assert!(close(v.at(5000.0).center, [middle, 0.501]), "{:?}", v.at(5000.0));
    }

    #[test]
    fn a_follow_zoom_holds_still_on_its_way_out() {
        // the pointer moves off after the zoom began to go out (4.25 s): the view doesn't follow
        let p = pointer(&[(0.0, 500, 250), (4.3, 900, 250)]);
        let (f, fixed) = (views(vec![follow(1000.0, 5000.0, 2.0)], Some(&p)), views(vec![zoom(1000.0, 5000.0, 2.0, [0.5005, 0.501])], None));
        for t in [4400.0, 4600.0, 4900.0] {
            let (a, b) = (f.at(t), fixed.at(t));
            assert!((a.scale - b.scale).abs() < 1e-9 && close(a.center, b.center), "at {t} ms: {a:?}, not {b:?}");
        }
    }

    #[test]
    fn a_follow_zoom_near_an_edge_sits_flush_against_it() {
        // at 4x the view is 250 px wide: the pointer 200 px from the left edge puts it flush left
        let p = pointer(&[(0.0, 200, 250)]);
        let v = views(vec![follow(1000.0, 5000.0, 4.0)], Some(&p));
        assert!(close(v.at(3000.0).center, [0.125, 0.501]), "{:?}", v.at(3000.0));
    }

    #[test]
    fn a_follow_zoom_with_no_pointer_to_follow_keeps_its_focus() {
        // a recording from before logs were kept
        let v = views(vec![Zoom { focus: [0.3, 0.6], ..follow(1000.0, 5000.0, 2.0) }], None);
        assert_eq!(v.at(3000.0), View { scale: 2.0, center: [0.3, 0.6] });
    }

    #[test]
    fn a_zoom_reads_follow_and_auto_from_its_json() {
        let e: Edit = serde_json::from_str(r#"{"zooms":[{"from":0,"to":3000,"level":1.5,"focus":[0.5,0.5],"follow":true,"auto":true}]}"#).unwrap();
        assert_eq!(e.zooms[0], Zoom { auto: true, ..follow(0.0, 3000.0, 1.5) });
    }
}
