//! A studio take's input log (the design's §4.7): what the pointer, its
//! buttons and the keys did while the take recorded, one JSON line each,
//! read back when the editor opens. Times are CLOCK_MONOTONIC µs, places
//! global logical px, as Hyprland gives them.

use std::io::{self, Write};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The log's first line: the take it belongs to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Header {
    pub format: String,
    pub version: u32,
    pub clock: String,
    /// gsr's first frame reaching its muxer: [monotonic µs, realtime µs]
    pub gsr_ts: Option<[i64; 2]>,
    pub fps: f64,
    pub encoder: String,
    pub output: String,
    pub scale: f64,
    /// what was recorded: x, y, w, h in global logical px
    pub area: [i32; 4],
    /// the video's size, px
    pub video: [u32; 2],
    pub cursor: Cursor,
    /// which keys are logged: "commands" (the hook's filter)
    pub keys: String,
    /// the pointer was in the picture from the start (a software cursor)
    pub baked: bool,
    /// a studio take: recorded without the pointer, for the editor to redraw
    /// (an ordinary take's pointer is the video's own)
    pub studio: bool,
    /// why a studio take's pointer is in the picture (`baked`): the outputs
    /// that kept Snip from moving it to the hardware plane ("HEADLESS-1"), or
    /// "software" (the user's own Hyprland setting)
    pub baked_why: String,
    /// the panel's Clicks and Keys as the take began: an ordinary take has
    /// them in its picture; a studio take's editor draws them after the fact,
    /// these its defaults (D60)
    pub clicks: bool,
    pub keystrokes: bool,
    /// a window's take (its area was a window's, as the panel's Space picks
    /// it): the window's corner radius, logical px
    pub window: Option<u32>,
    /// a studio take's camera, recorded apart (D61): as the panel showed it
    pub camera: Option<CameraLook>,
}

/// The camera as the recording panel showed it: where the editor starts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraLook {
    /// circle | square | rectangle | vertical
    pub shape: String,
    /// small | medium | large | huge
    pub size: String,
    pub mirror: bool,
}

impl Default for Header {
    fn default() -> Self {
        Header {
            format: "snip-input".into(),
            version: 1,
            clock: "monotonic_us".into(),
            gsr_ts: None,
            fps: 0.0,
            encoder: String::new(),
            output: String::new(),
            scale: 1.0,
            area: [0, 0, 0, 0],
            video: [0, 0],
            cursor: Cursor::default(),
            keys: "commands".into(),
            baked: false,
            studio: false,
            baked_why: String::new(),
            clicks: false,
            keystrokes: false,
            window: None,
            camera: None,
        }
    }
}

impl Header {
    /// A place (global logical px) in the video's px.
    pub fn video_px(&self, at: [f64; 2]) -> [f64; 2] {
        let k = |i: usize| self.video[i] as f64 / self.area[i + 2].max(1) as f64;
        [(at[0] - self.area[0] as f64) * k(0), (at[1] - self.area[1] as f64) * k(1)]
    }
}

/// The user's pointer as the take began: its theme and size (logical px).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cursor {
    pub theme: String,
    pub size: u32,
    pub hide_on_key_press: bool,
}

/// One thing that happened, at `t`.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// the pointer moved (Hyprland floors it to whole px)
    Move { t: i64, at: [i32; 2] },
    /// a button went down or up (Linux codes: 272 left), exactly where
    Button { t: i64, button: u32, down: bool, at: [f64; 2] },
    /// the wheel turned (a marker: at most once per 300 ms)
    Wheel { t: i64, down: bool },
    /// a command key went down or up (a keycode)
    Key { t: i64, code: u32, down: bool },
    /// some typing happened (no content)
    Typing { t: i64 },
    /// gsr paused or went on
    Pause { t: i64, paused: bool },
    /// the pointer went into the picture (a software cursor) or out
    Baked { t: i64, baked: bool },
    /// the camera's first frame, recorded apart (D61)
    Camera { t: i64 },
    /// the pointer's hotspot changed: its shape did (spike S6: the cursor
    /// session tells no shape, but each shape's hotspot differs)
    Shape { t: i64, hot: [i32; 2] },
}

impl Event {
    /// When it happened (CLOCK_MONOTONIC µs).
    pub fn t(&self) -> i64 {
        match *self {
            Event::Move { t, .. }
            | Event::Button { t, .. }
            | Event::Wheel { t, .. }
            | Event::Key { t, .. }
            | Event::Typing { t }
            | Event::Pause { t, .. }
            | Event::Baked { t, .. }
            | Event::Camera { t }
            | Event::Shape { t, .. } => t,
        }
    }

    /// Where the pointer was (global logical px): a move's middle (Hyprland
    /// floors it to whole px), a button's exact place; None for the rest.
    pub fn place(&self) -> Option<[f64; 2]> {
        match self {
            Event::Move { at, .. } => Some([at[0] as f64 + 0.5, at[1] as f64 + 0.5]),
            Event::Button { at, .. } => Some(*at),
            _ => None,
        }
    }

    /// Its line in the log.
    pub(crate) fn line(&self) -> String {
        let n = |b: bool| b as u8;
        match self {
            Event::Move { t, at } => format!(r#"{{"t":{t},"m":[{},{}]}}"#, at[0], at[1]),
            Event::Button { t, button, down, at } => format!(r#"{{"t":{t},"b":{button},"d":{},"p":[{},{}]}}"#, n(*down), at[0], at[1]),
            Event::Wheel { t, down } => format!(r#"{{"t":{t},"w":"{}"}}"#, if *down { "down" } else { "up" }),
            Event::Key { t, code, down } => format!(r#"{{"t":{t},"k":{code},"d":{}}}"#, n(*down)),
            Event::Typing { t } => format!(r#"{{"t":{t},"ty":1}}"#),
            Event::Pause { t, paused } => format!(r#"{{"t":{t},"pause":{}}}"#, n(*paused)),
            Event::Baked { t, baked } => format!(r#"{{"t":{t},"baked":{}}}"#, n(*baked)),
            Event::Camera { t } => format!(r#"{{"t":{t},"cam":1}}"#),
            Event::Shape { t, hot } => format!(r#"{{"t":{t},"hs":[{},{}]}}"#, hot[0], hot[1]),
        }
    }

    /// The event a line holds; None for one this Snip doesn't know.
    fn parse(line: &str) -> Option<Event> {
        let v: Value = serde_json::from_str(line).ok()?;
        let t = v["t"].as_i64()?;
        let on = |k: &str| v[k].as_i64().map(|d| d != 0);
        let pair = |k: &str| Some([v[k][0].as_f64()?, v[k][1].as_f64()?]);
        Some(if let Some(m) = pair("m") {
            Event::Move { t, at: [m[0] as i32, m[1] as i32] }
        } else if let Some(button) = v["b"].as_u64() {
            Event::Button { t, button: button as u32, down: on("d")?, at: pair("p")? }
        } else if let Some(w) = v["w"].as_str() {
            Event::Wheel { t, down: w == "down" }
        } else if let Some(code) = v["k"].as_u64() {
            Event::Key { t, code: code as u32, down: on("d")? }
        } else if v.get("ty").is_some() {
            Event::Typing { t }
        } else if let Some(paused) = on("pause") {
            Event::Pause { t, paused }
        } else if v.get("cam").is_some() {
            Event::Camera { t }
        } else if let Some(hot) = pair("hs") {
            Event::Shape { t, hot: [hot[0] as i32, hot[1] as i32] }
        } else {
            Event::Baked { t, baked: on("baked")? }
        })
    }
}

/// Writes a take's log as it records: on disk at least once a second (as
/// gsr's fragments are), so a crash loses a second at most.
pub struct Writer<W: Write> {
    out: io::BufWriter<W>,
    flushed: Instant,
}

impl<W: Write> Writer<W> {
    pub fn new(out: W, header: &Header) -> io::Result<Self> {
        let mut w = Writer { out: io::BufWriter::new(out), flushed: Instant::now() };
        writeln!(w.out, "{}", serde_json::to_string(header)?)?;
        w.out.flush()?;
        Ok(w)
    }

    pub fn push(&mut self, e: &Event) -> io::Result<()> {
        writeln!(self.out, "{}", e.line())?;
        if self.flushed.elapsed().as_secs() >= 1 {
            self.flushed = Instant::now();
            self.out.flush()?;
        }
        Ok(())
    }

    /// Everything written, and what it was written to.
    pub fn into_inner(self) -> io::Result<W> {
        self.out.into_inner().map_err(|e| e.into_error())
    }
}

/// A take's log, read back.
#[derive(Clone, Debug, PartialEq)]
pub struct Log {
    pub header: Header,
    pub events: Vec<Event>,
}

/// A take's camera, recorded apart (D61), on the take's clock.
pub struct CameraClock {
    clock: Clock,
    /// its first frame (CLOCK_MONOTONIC µs)
    first: i64,
}

impl CameraClock {
    /// None without a camera (or a clock).
    pub fn of(log: &Log) -> Option<CameraClock> {
        let first = log.events.iter().find_map(|e| if let Event::Camera { t } = *e { Some(t) } else { None })?;
        Some(CameraClock { clock: Clock::of(log)?, first })
    }

    /// Where the camera is at `ms` of the video: ms of its own file
    /// (negative before its first frame). It went on through the pauses.
    pub fn ms(&self, ms: f64) -> f64 {
        (self.clock.moment(ms) - self.first as f64) / 1000.0
    }
}

/// Frames from a take's start to gsr's first frame reaching its muxer
/// (`-write-first-frame-ts`): measured (spike S3, 2026-10-12: a pointer
/// moved at a known moment into a recorded area, gsr 6.1 on an RTX 4080),
/// 3 with NVENC and 7 with x264 (`-encoder cpu`, its lookahead). The test
/// pattern recorder ("fake") has none.
fn first_frame_lag(encoder: &str) -> f64 {
    match encoder {
        "fake" => 0.0,
        "x264" => 7.0,
        _ => 3.0,
    }
}

/// A take's clock (the design's §4.6): when it began, how long a frame is,
/// and when it was paused, which gsr leaves out of the video.
#[derive(Clone, Debug)]
pub struct Clock {
    start: f64,
    frame: f64,
    /// [paused, went on): `i64::MAX` when it never went on
    pauses: Vec<(i64, i64)>,
}

impl Clock {
    /// None without a first frame's time (gsr never wrote one) or a rate.
    pub fn of(log: &Log) -> Option<Clock> {
        if log.header.fps <= 0.0 {
            return None;
        }
        let frame = 1e6 / log.header.fps;
        let start = log.header.gsr_ts?[0] as f64 - first_frame_lag(&log.header.encoder) * frame;
        let (mut pauses, mut from) = (vec![], None);
        for e in &log.events {
            match (e, from) {
                (Event::Pause { t, paused: true }, None) => from = Some(*t),
                (Event::Pause { t, paused: false }, Some(p)) => {
                    pauses.push((p, *t));
                    from = None;
                }
                _ => {}
            }
        }
        pauses.extend(from.map(|p| (p, i64::MAX)));
        Some(Clock { start, frame, pauses })
    }

    /// The moment (CLOCK_MONOTONIC µs) at `ms` of the video: `video_ms`
    /// backwards, its pauses skipped.
    pub fn moment(&self, ms: f64) -> f64 {
        let mut m = self.start + self.frame + ms * 1000.0;
        for &(p, r) in &self.pauses {
            if p as f64 <= m {
                m += r.saturating_sub(p) as f64;
            }
        }
        m
    }

    /// Where moment `t` is in the video, ms: frame j shows the screen as it
    /// was a frame after the start and j frames. Negative before the first
    /// frame; None while paused.
    pub fn video_ms(&self, t: i64) -> Option<f64> {
        let mut gone = 0.0;
        for &(p, r) in &self.pauses {
            if (p..r).contains(&t) {
                return None;
            }
            if r <= t {
                gone += r as f64 - p as f64;
            }
        }
        Some((t as f64 - self.start - self.frame - gone) / 1000.0)
    }
}

/// The log in `text`; None if it isn't one, or a newer Snip's.
pub fn read(text: &str) -> Option<Log> {
    let mut lines = text.lines();
    let header: Header = serde_json::from_str(lines.next()?).ok()?;
    if header.format != "snip-input" || header.version > 1 {
        return None;
    }
    Some(Log { header, events: lines.filter_map(Event::parse).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            gsr_ts: Some([10_000_000, 1_760_000_000_000_000]),
            fps: 60.0,
            encoder: "nvenc".into(),
            output: "DP-2".into(),
            scale: 1.25,
            area: [0, 0, 2752, 1152],
            video: [3440, 1440],
            ..Header::default()
        }
    }

    #[test]
    fn a_log_reads_back_as_it_was_written() {
        let events = vec![
            Event::Move { t: 10_000_100, at: [1201, 644] },
            Event::Button { t: 10_000_200, button: 272, down: true, at: [1201.4, 644.9] },
            Event::Button { t: 10_000_300, button: 272, down: false, at: [1203.0, 646.2] },
            Event::Wheel { t: 10_000_400, down: true },
            Event::Key { t: 10_000_500, code: 54, down: true },
            Event::Typing { t: 10_000_600 },
            Event::Pause { t: 10_000_700, paused: true },
            Event::Pause { t: 10_000_800, paused: false },
            Event::Baked { t: 10_000_900, baked: true },
            Event::Camera { t: 10_001_000 },
            Event::Shape { t: 10_001_100, hot: [11, 12] },
        ];
        let mut w = Writer::new(Vec::new(), &header()).unwrap();
        for e in &events {
            w.push(e).unwrap();
        }
        let text = String::from_utf8(w.into_inner().unwrap()).unwrap();
        let log = read(&text).unwrap();
        assert_eq!(log.header, header());
        assert_eq!(log.events, events);
        // the design's lines: the time first, then what happened
        assert_eq!(text.lines().nth(1), Some(r#"{"t":10000100,"m":[1201,644]}"#));
        assert_eq!(text.lines().nth(2), Some(r#"{"t":10000200,"b":272,"d":1,"p":[1201.4,644.9]}"#));
    }

    fn written(events: &[Event]) -> String {
        let mut w = Writer::new(Vec::new(), &header()).unwrap();
        for e in events {
            w.push(e).unwrap();
        }
        String::from_utf8(w.into_inner().unwrap()).unwrap()
    }

    #[test]
    fn a_torn_last_line_and_records_this_snip_doesnt_know_are_skipped() {
        let mut text = written(&[Event::Move { t: 10_000_100, at: [5, 6] }]);
        text += "{\"t\":10000150,\"zz\":1}\n";          // a newer Snip's record
        text += "{\"t\":10000200,\"m\":[7,";             // a crash mid-line
        assert_eq!(read(&text).unwrap().events, vec![Event::Move { t: 10_000_100, at: [5, 6] }]);
    }

    const FRAME: f64 = 1e6 / 60.0;
    // the take began 3 frames before gsr's first frame reached its muxer (header's gsr_ts)
    const START: f64 = 10_000_000.0 - 3.0 * FRAME;
    // the moment `s` seconds into the video (before any pause)
    fn at(s: f64) -> i64 {
        (START + FRAME + s * 1e6).round() as i64
    }
    fn close(got: Option<f64>, want: f64) -> bool {
        got.is_some_and(|g| (g - want).abs() < 0.1)
    }

    #[test]
    fn a_moment_is_in_the_frame_that_shows_it() {
        let c = Clock::of(&Log { header: header(), events: vec![] }).unwrap();
        // frame j shows the screen as it was a frame after the start and j frames
        assert!(close(c.video_ms(at(0.0)), 0.0), "frame 0: {:?}", c.video_ms(at(0.0)));
        assert!(close(c.video_ms(at(1.0)), 1000.0), "a second in: {:?}", c.video_ms(at(1.0)));
        assert!(close(c.video_ms(at(-0.5)), -500.0), "before the first frame, where the pointer starts");
        assert!(Clock::of(&Log { header: Header { gsr_ts: None, ..header() }, events: vec![] }).is_none(), "no first frame: no clock");
    }

    #[test]
    fn a_pause_as_long_as_time_is_no_overflow() {
        // a log's times are what its file says: a pause from the first moment there is to the last
        let events = vec![Event::Pause { t: i64::MIN, paused: true }, Event::Pause { t: i64::MAX - 1, paused: false }];
        let c = Clock::of(&Log { header: header(), events }).unwrap();
        assert!(c.video_ms(i64::MAX).is_some_and(f64::is_finite));
    }

    #[test]
    fn a_test_takes_clock_starts_at_its_first_frame() {
        // the test pattern recorder (SNIP_RECORDER=fake) has no encoder lag: its first frame is its start
        let c = Clock::of(&Log { header: Header { encoder: "fake".into(), ..header() }, events: vec![] }).unwrap();
        assert!(close(c.video_ms(10_000_000 + (FRAME as i64)), 0.0), "{:?}", c.video_ms(10_000_000 + FRAME as i64));
    }

    #[test]
    fn a_cpu_takes_first_frame_comes_later_than_an_nvenc_ones() {
        // spike S3 (2026-10-12): gsr's first frame reaches its muxer 3 frames after
        // the take's start with NVENC, 7 with x264: the same moment, the same frame
        let (nvenc, x264) = (Header { encoder: "nvenc".into(), ..header() }, Header { encoder: "x264".into(), ..header() });
        let c = Clock::of(&Log { header: nvenc, events: vec![] }).unwrap();
        let cpu = Clock::of(&Log { header: Header { gsr_ts: Some([(START + 7.0 * FRAME) as i64, 0]), ..x264 }, events: vec![] }).unwrap();
        assert!(close(cpu.video_ms(at(1.0)), 1000.0) && close(c.video_ms(at(1.0)), 1000.0), "{:?}", cpu.video_ms(at(1.0)));
    }

    #[test]
    fn a_moment_is_found_again_from_its_place_in_the_video() {
        // paused from 2 s to 5 s: 3 s later, the same place in the video
        let events = vec![Event::Pause { t: at(2.0), paused: true }, Event::Pause { t: at(5.0), paused: false }];
        let c = Clock::of(&Log { header: header(), events }).unwrap();
        assert!((c.moment(1000.0) - at(1.0) as f64).abs() < 1.0, "before the pause");
        assert!((c.moment(2500.0) - at(5.5) as f64).abs() < 1.0, "after it, the pause skipped: {}", c.moment(2500.0) - at(5.5) as f64);
        assert!((c.moment(-500.0) - at(-0.5) as f64).abs() < 1.0, "before the first frame");
    }

    #[test]
    fn the_camera_is_where_the_video_is_through_its_pauses() {
        // the camera's first frame 0.2 s after the video's; a pause from 2 s to 5 s (the camera went on)
        let events = vec![Event::Pause { t: at(2.0), paused: true }, Event::Pause { t: at(5.0), paused: false }, Event::Camera { t: at(0.2) }];
        let c = CameraClock::of(&Log { header: header(), events }).unwrap();
        assert!((c.ms(1000.0) - 800.0).abs() < 1.0, "{}", c.ms(1000.0));
        assert!((c.ms(2500.0) - 5300.0).abs() < 1.0, "after the pause: {}", c.ms(2500.0));
        assert!(CameraClock::of(&Log { header: header(), events: vec![] }).is_none(), "no camera");
    }

    #[test]
    fn a_pause_is_cut_out_of_the_video() {
        let events = vec![Event::Pause { t: at(2.0), paused: true }, Event::Pause { t: at(5.0), paused: false }, Event::Pause { t: at(8.0), paused: true }];
        let c = Clock::of(&Log { header: header(), events }).unwrap();
        assert!(close(c.video_ms(at(1.0)), 1000.0));
        assert_eq!(c.video_ms(at(3.0)), None, "paused: not in the video");
        assert!(close(c.video_ms(at(6.0)), 3000.0), "the 3 s paused are left out: {:?}", c.video_ms(at(6.0)));
        assert_eq!(c.video_ms(at(9.0)), None, "paused at the end: nothing more was recorded");
    }

    #[test]
    fn a_place_is_in_the_videos_pixels() {
        // recorded at 1.25: 800 x 600 logical px are 1000 x 750
        let h = Header { area: [100, 50, 800, 600], video: [1000, 750], ..header() };
        // Hyprland floors a move: its middle is half a px on
        let moved = Event::Move { t: 0, at: [500, 350] };
        assert_eq!(h.video_px(moved.place().unwrap()), [500.625, 375.625]);
        // a press is exact
        let pressed = Event::Button { t: 0, button: 272, down: true, at: [500.0, 350.0] };
        assert_eq!(h.video_px(pressed.place().unwrap()), [500.0, 375.0]);
        assert_eq!(Event::Typing { t: 0 }.place(), None);
    }

    #[test]
    fn a_newer_snips_log_is_left_unread() {
        let text = written(&[Event::Move { t: 10_000_100, at: [5, 6] }]).replacen("\"version\":1", "\"version\":2", 1);
        assert_eq!(read(&text), None);
        assert_eq!(read("not a log"), None);
    }
}
