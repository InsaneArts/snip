//! A studio take's click ripples, drawn after the fact (the design's §6.2,
//! step 7): a ring at each press, where it was, growing and fading as the
//! live HUD's do (accent for the left button, warning for the right).

use super::log::{Clock, Event, Log};

/// A ring at one moment: its middle and size across (px of the video), how
/// much it shows (0–1), and whether the right button made it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    pub at: [f64; 2],
    pub size: f64,
    pub alpha: f64,
    pub right: bool,
}

/// How a ring goes: from SMALL to BIG px across (the user's px, as the live
/// HUD draws it), over LIFE ms.
const SMALL: f64 = 16.0;
const BIG: f64 = 56.0;
const LIFE: f64 = 600.0;

/// The buttons with rings (Linux codes).
const LEFT: u32 = 272;
const RIGHT: u32 = 273;

/// A take's ripples, ready to be asked about any moment.
pub struct Ripples {
    /// each press: when (ms of the video), where (px), the right button's
    presses: Vec<(f64, [f64; 2], bool)>,
    /// the video's px per the user's
    px: f64,
}

impl Ripples {
    pub fn new(log: &Log) -> Ripples {
        let presses = Clock::of(log).map_or(vec![], |clock| {
            log.events
                .iter()
                .filter_map(|e| match *e {
                    Event::Button { t, button, down: true, at } if button == LEFT || button == RIGHT => Some((clock.video_ms(t)?, log.header.video_px(at), button == RIGHT)),
                    _ => None,
                })
                .collect()
        });
        Ripples { presses, px: f64::from(log.header.video[0]) / f64::from(log.header.area[2].max(1)) }
    }

    /// The rings showing at `ms` of the video: each grows quickly, then
    /// slower (1 − (1 − p)³), and fades ((1 − p)^1.5). Only those of
    /// presses at `since` or later: there a part of the video was cut out.
    pub fn at(&self, ms: f64, since: f64) -> Vec<Ring> {
        let from = self.presses.partition_point(|&(t, _, _)| t < (ms - LIFE).max(since));
        self.presses[from..]
            .iter()
            .take_while(|&&(t, _, _)| t <= ms)
            .map(|&(t, at, right)| {
                let p = (ms - t) / LIFE;
                Ring { at, size: (SMALL + (BIG - SMALL) * (1.0 - (1.0 - p).powi(3))) * self.px, alpha: (1.0 - p).powf(1.5), right }
            })
            .filter(|r| r.alpha > 0.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::studio::log::Header;

    // the moment `s` seconds into the video (the test pattern recorder's
    // clock, at 50 fps: a frame is 20 ms exactly)
    fn at(s: f64) -> i64 {
        (10_000_000.0 + 20_000.0 + s * 1e6).round() as i64
    }
    // a take of a 1000×500 area recorded at scale 1.25: 1250×625 px
    fn take(events: Vec<Event>) -> Log {
        let header = Header { gsr_ts: Some([10_000_000, 0]), fps: 50.0, encoder: "fake".into(), scale: 1.25, area: [0, 0, 1000, 500], video: [1250, 625], ..Header::default() };
        Log { header, events }
    }
    fn press(s: f64, button: u32, x: f64, y: f64) -> Vec<Event> {
        vec![Event::Button { t: at(s), button, down: true, at: [x, y] }, Event::Button { t: at(s + 0.1), button, down: false, at: [x, y] }]
    }

    #[test]
    fn a_press_makes_a_ring_that_grows_and_fades_where_it_was() {
        let r = Ripples::new(&take(press(1.0, 272, 300.0, 200.0)));
        assert_eq!(r.at(900.0, 0.0), vec![], "not before it");
        // where it was, 16 px across (×1.25: the video's px), fully shown
        assert_eq!(r.at(1000.0, 0.0), vec![Ring { at: [375.0, 250.0], size: 20.0, alpha: 1.0, right: false }]);
        // halfway through its 0.6 s: 16 + 40 × (1 − 0.5³) px across, (1 − 0.5)^1.5 shown
        let half = r.at(1300.0, 0.0);
        assert!((half[0].size - 51.0 * 1.25).abs() < 1e-9 && (half[0].alpha - 0.5f64.powf(1.5)).abs() < 1e-9, "{half:?}");
        assert_eq!(r.at(1600.0, 0.0), vec![], "gone after 0.6 s");
    }

    #[test]
    fn the_right_button_has_its_own_ring_and_the_middle_none() {
        let mut events = press(1.0, 273, 300.0, 200.0);
        events.extend(press(1.2, 274, 300.0, 200.0));
        let rings = Ripples::new(&take(events)).at(1250.0, 0.0);
        assert_eq!(rings.len(), 1, "{rings:?}");
        assert!(rings[0].right);
    }

    #[test]
    fn a_ring_ends_at_a_cut() {
        let r = Ripples::new(&take(press(1.0, 272, 300.0, 200.0)));
        assert_eq!(r.at(1300.0, 0.0).len(), 1);
        assert_eq!(r.at(1300.0, 1200.0), vec![], "its click was in a part cut out");
    }
}
