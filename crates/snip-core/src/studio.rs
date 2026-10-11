//! Studio Mode (D55): what a recording's export shows at any moment, from
//! the zooms placed on it (the design's §6.3). Pure, so the editor's preview
//! and the export ask the same function and show the same frame.

use serde::{Deserialize, Serialize};

/// A zoom on the timeline: from `from` to `to` (the recording's own ms), at
/// `level` (1–4), framing `focus`, a point of the recording (0–1 across and
/// down).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zoom {
    pub from: f64,
    pub to: f64,
    pub level: f64,
    pub focus: [f64; 2],
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

/// What the export shows at `t` (ms of the recording).
pub fn view(zooms: &[Zoom], motion: Motion, t: f64) -> View {
    let mut zooms: Vec<&Zoom> = zooms.iter().collect();
    zooms.sort_by(|a, b| a.from.total_cmp(&b.from));
    // a zoom that touches the next one doesn't go out: it holds until the next
    // one comes in from it, so close zooms pan instead of out and in again
    let touch = |a: &Zoom, b: &Zoom| b.from - a.to < ramp(a, motion) + ramp(b, motion);
    for (i, z) in zooms.iter().enumerate() {
        let (prev, next) = (i.checked_sub(1).map(|p| zooms[p]), zooms.get(i + 1).copied());
        let to_next = next.filter(|n| touch(z, n));
        if t < z.from || t >= to_next.map_or(z.to, |n| n.from) {
            continue;
        }
        let r = ramp(z, motion);
        let (in_, out) = ((t - z.from) / r, (z.to - t) / r);
        if in_ < 1.0 {
            let from = prev.filter(|p| touch(p, z)).map_or(WHOLE, framed);
            return between(from, framed(z), eased(in_, motion));
        }
        if out < 1.0 && to_next.is_none() {
            return between(WHOLE, framed(z), eased(out, motion));
        }
        return framed(z);
    }
    WHOLE
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

/// A zoom's view once it's in: its level (held to 1–4), centred on its focus
/// as far as the recording allows (the view never shows past its edges).
fn framed(z: &Zoom) -> View {
    let level = z.level.clamp(1.0, 4.0);
    let half = 0.5 / level;
    View { scale: level, center: z.focus.map(|f| f.clamp(half, 1.0 - half)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_zoom_the_whole_recording_shows() {
        assert_eq!(view(&[], Motion::Smooth, 0.0), WHOLE);
        assert_eq!(view(&[], Motion::Dynamic, 12_345.0), WHOLE);
    }

    fn zoom(from: f64, to: f64, level: f64, focus: [f64; 2]) -> Zoom {
        Zoom { from, to, level, focus }
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
}
