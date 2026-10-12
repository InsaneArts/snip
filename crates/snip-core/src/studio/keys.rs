//! A studio take's keystrokes, drawn after the fact (the design's §6.5): the
//! live HUD's rules (RecordHud.qml) as a function of the video's time. Only
//! command keys are in the log (the hooks' filter), never typing.

use super::log::{Clock, Event, Log};

/// What the keystrokes show at one moment, and how much (0–1).
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub text: String,
    pub opacity: f64,
}

/// A take's keystrokes, ready to be asked about any moment.
pub struct Keys {
    items: Vec<Item>,
}

/// One text shown from `from` to `to` (ms of the video).
#[derive(Clone, Debug, PartialEq)]
struct Item {
    from: f64,
    to: f64,
    text: String,
}

/// A combo stays this long after its last press; modifiers held alone show
/// after this (a quick tap doesn't flicker); fades in and out (ms).
const HOLD: f64 = 1500.0;
const MODS_AFTER: f64 = 150.0;
const FADE_IN: f64 = 120.0;
const FADE_OUT: f64 = 200.0;
/// The modifiers, in the order they're shown.
const MODS: [&str; 4] = ["Super", "Ctrl", "Alt", "Shift"];

impl Keys {
    /// `label`: a key's label by its keycode (the HUD's: US positions).
    pub fn new(log: &Log, label: impl Fn(u32) -> Option<&'static str>) -> Keys {
        let Some(clock) = Clock::of(log) else { return Keys { items: vec![] } };
        let keys: Vec<(f64, &str, bool)> = log
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Key { t, code, down } => Some((clock.video_ms(t)?, label(code)?, down)),
                _ => None,
            })
            .collect();
        // what shows is the same between a key and the next, but for a combo
        // running out and modifiers held long enough to show
        let (mut state, mut items) = (Held::default(), Vec::<Item>::new());
        for (i, &(t, l, down)) in keys.iter().enumerate() {
            state.apply(t, l, down);
            let next = keys.get(i + 1).map_or(f64::INFINITY, |k| k.0);
            let mut at = vec![t, next];
            at.extend(state.changes().into_iter().filter(|&c| c > t && c < next));
            at.sort_by(f64::total_cmp);
            for w in at.windows(2) {
                let Some(text) = state.shows(w[0]) else { continue };
                match items.last_mut() {
                    Some(last) if last.text == text && last.to == w[0] => last.to = w[1],
                    _ => items.push(Item { from: w[0], to: w[1], text }),
                }
            }
        }
        Keys { items }
    }

    /// What shows at `ms` of the video, if anything: fading in, unless it
    /// takes over from another; fading out once it's over. Only what began
    /// at `since` or later: there a part of the video was cut out (Cap's
    /// #1725: keys pressed there don't show after it).
    pub fn at(&self, ms: f64, since: f64) -> Option<Shown> {
        let i = self.items.partition_point(|it| it.from <= ms).checked_sub(1)?;
        let it = self.items.get(i).filter(|it| it.from >= since)?;
        let opacity = if ms < it.to {
            let took_over = i > 0 && self.items[i - 1].to > it.from - FADE_OUT;
            if took_over { 1.0 } else { smooth((ms - it.from) / FADE_IN) }
        } else if ms < it.to + FADE_OUT {
            1.0 - smooth((ms - it.to) / FADE_OUT)
        } else {
            return None;
        };
        Some(Shown { text: it.text.clone(), opacity })
    }
}

/// The keys as they are: the modifiers held (since when), and the last combo
/// (its text, how many times in a row, when last pressed).
#[derive(Default)]
struct Held {
    mods: Vec<&'static str>,
    since: f64,
    combo: Option<(String, u32, f64)>,
}

impl Held {
    fn apply(&mut self, t: f64, label: &'static str, down: bool) {
        if MODS.contains(&label) {
            if !down {
                self.mods.retain(|m| *m != label);
            } else if !self.mods.contains(&label) {
                if self.mods.is_empty() {
                    self.since = t;
                }
                self.mods.push(label);
            }
        } else if down {
            let text = MODS.iter().filter(|m| self.mods.contains(m)).copied().chain([label]).collect::<Vec<_>>().join(" + ");
            let n = match &self.combo {
                Some((last, n, at)) if *last == text && t < at + HOLD => n + 1,
                _ => 1,
            };
            self.combo = Some((text, n, t));
        }
    }

    /// The last combo for HOLD after its press, else the modifiers held
    /// MODS_AFTER or longer ("Ctrl +").
    fn shows(&self, t: f64) -> Option<String> {
        match &self.combo {
            Some((text, n, at)) if t < at + HOLD => Some(if *n > 1 { format!("{text}  ×{n}") } else { text.clone() }),
            _ if !self.mods.is_empty() && t >= self.since + MODS_AFTER => {
                Some(MODS.iter().filter(|m| self.mods.contains(m)).copied().collect::<Vec<_>>().join(" + ") + " +")
            }
            _ => None,
        }
    }

    /// When what it shows changes by itself: the combo running out, the
    /// modifiers showing.
    fn changes(&self) -> Vec<f64> {
        let mut out: Vec<f64> = self.combo.iter().map(|&(_, _, at)| at + HOLD).collect();
        if !self.mods.is_empty() {
            out.push(self.since + MODS_AFTER);
        }
        out
    }
}

/// 0 → 1 between 0 and 1, eased at both ends.
fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
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
    fn take(events: Vec<Event>) -> Log {
        let header = Header { gsr_ts: Some([10_000_000, 0]), fps: 50.0, encoder: "fake".into(), area: [0, 0, 1000, 500], video: [1000, 500], ..Header::default() };
        Log { header, events }
    }
    // a few of the HUD's labels, by xkb keycode
    fn label(code: u32) -> Option<&'static str> {
        match code {
            37 => Some("Ctrl"),
            50 => Some("Shift"),
            54 => Some("C"),
            55 => Some("V"),
            9 => Some("Esc"),
            _ => None,
        }
    }
    fn key(s: f64, code: u32, down: bool) -> Event {
        Event::Key { t: at(s), code, down }
    }
    // Ctrl held, `code` pressed at `s` seconds, all up 0.1 s later
    fn with_ctrl(s: f64, code: u32) -> Vec<Event> {
        vec![key(s - 0.05, 37, true), key(s, code, true), key(s + 0.05, code, false), key(s + 0.1, 37, false)]
    }
    fn text(k: &Keys, ms: f64) -> Option<String> {
        k.at(ms, 0.0).map(|s| s.text)
    }

    #[test]
    fn a_combo_shows_from_its_press_for_a_moment_and_a_half() {
        let k = Keys::new(&take(with_ctrl(1.0, 54)), label);
        assert_eq!(text(&k, 990.0), None, "not before its press (Ctrl alone for 50 ms: too soon to show)");
        assert_eq!(k.at(1000.0, 0.0), Some(Shown { text: "Ctrl + C".into(), opacity: 0.0 }), "from its press, fading in");
        assert_eq!(k.at(1120.0, 0.0), Some(Shown { text: "Ctrl + C".into(), opacity: 1.0 }), "in after 120 ms");
        assert_eq!(k.at(2400.0, 0.0), Some(Shown { text: "Ctrl + C".into(), opacity: 1.0 }), "shown 1.5 s");
        assert!(k.at(2600.0, 0.0).is_some_and(|s| s.opacity < 1.0 && s.opacity > 0.0), "then fading out over 200 ms");
        assert_eq!(text(&k, 2700.0), None);
    }

    #[test]
    fn the_same_combo_again_counts_and_another_replaces_it() {
        let mut events = with_ctrl(1.0, 54);
        events.extend(with_ctrl(1.8, 54));
        events.extend(with_ctrl(2.6, 55));
        let k = Keys::new(&take(events), label);
        assert_eq!(text(&k, 1500.0).as_deref(), Some("Ctrl + C"));
        assert_eq!(k.at(1800.0, 0.0), Some(Shown { text: "Ctrl + C  ×2".into(), opacity: 1.0 }), "at once: no fade between");
        assert_eq!(k.at(2600.0, 0.0), Some(Shown { text: "Ctrl + V".into(), opacity: 1.0 }));
        assert_eq!(text(&k, 4300.0), None, "gone 1.5 s after the last, and its fade");
    }

    #[test]
    fn modifiers_held_alone_show_after_a_moment() {
        // Ctrl and Shift held for a second: shown after 150 ms, "Ctrl + Shift +"
        let k = Keys::new(&take(vec![key(1.0, 37, true), key(1.1, 50, true), key(2.0, 50, false), key(2.0, 37, false)]), label);
        assert_eq!(text(&k, 1100.0), None, "a quick tap doesn't flicker");
        assert_eq!(text(&k, 1300.0).as_deref(), Some("Ctrl + Shift +"));
        assert!(k.at(2100.0, 0.0).is_some_and(|s| s.opacity < 1.0), "let go: fading out");
        assert_eq!(text(&k, 2300.0), None);
    }

    #[test]
    fn a_special_key_shows_on_its_own() {
        let k = Keys::new(&take(vec![key(1.0, 9, true), key(1.1, 9, false)]), label);
        assert_eq!(text(&k, 1500.0).as_deref(), Some("Esc"));
    }

    #[test]
    fn keys_end_at_a_cut() {
        // Ctrl+C at 1 s, and the video cut from 1.5 s to 2 s: after the cut it doesn't show
        let k = Keys::new(&take(with_ctrl(1.0, 54)), label);
        assert_eq!(k.at(1400.0, 0.0).map(|s| s.text).as_deref(), Some("Ctrl + C"));
        assert_eq!(k.at(2100.0, 2000.0), None, "pressed before the part that shows now");
    }
}
