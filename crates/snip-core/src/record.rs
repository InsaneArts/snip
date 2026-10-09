//! Screen recording (REC): what a recording covers and the gpu-screen-recorder
//! command that records it (architecture §4.3). Pure; the engine runs it.

use std::path::PathBuf;

use crate::geometry::Rect;

/// What a recording covers.
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// a whole monitor, by name
    Output(String),
    /// a rectangle in global logical px (Hyprland's layout), as slurp prints
    /// it: gsr maps it to physical px itself
    Area(Rect),
}

/// Where the sound comes from (REC-07).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    /// "default_input", a device's name, or None: no microphone
    pub mic: Option<String>,
    /// what the computer plays (the default output's monitor)
    pub system: bool,
    /// one track per source, instead of one mixed track
    pub separate: bool,
}

impl Audio {
    /// gsr's `-a` values: one mixed source list, or one per track.
    fn sources(&self) -> Vec<String> {
        let mic = self.mic.as_ref().map(|m| if m == "default_input" || m.contains(':') { m.clone() } else { format!("device:{m}") });
        let all: Vec<String> = [self.system.then(|| "default_output".to_string()), mic].into_iter().flatten().collect();
        if self.separate || all.len() < 2 {
            all
        } else {
            vec![all.join("|")]
        }
    }
}

/// A recording's settings, as gsr takes them.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub target: Target,
    pub fps: u32,
    pub cursor: bool,
    pub audio: Audio,
    /// "auto" (H.264 on NVENC here), "h264", "hevc" or "av1"
    pub codec: String,
    /// gsr's quality: medium, high, very_high or ultra
    pub quality: String,
    /// the largest video size (gsr `-s`); None: as captured
    pub max_size: Option<(u32, u32)>,
    /// encode on the CPU: NVENC refuses H.264 narrower than 146 px and pads
    /// anything shorter than 128 px (measured), so small areas need it
    pub cpu: bool,
    /// the MP4 gsr writes
    pub file: PathBuf,
    /// gsr's control socket (stop, set-paused)
    pub ipc: PathBuf,
}

/// The gpu-screen-recorder command line (without the program) for `s`.
pub fn gsr_args(s: &Spec) -> Vec<String> {
    let mut a: Vec<String> = vec![];
    let mut push = |k: &str, v: String| {
        a.push(k.into());
        a.push(v);
    };
    match &s.target {
        Target::Output(name) => push("-w", name.clone()),
        // gsr 6.1 takes an area as the capture itself (`-region` is deprecated)
        Target::Area(r) => push("-w", format!("{}x{}+{}+{}", r.w, r.h, r.x, r.y)),
    }
    push("-c", "mp4".into());
    if s.cpu {
        push("-encoder", "cpu".into());
        push("-k", "h264".into());
    } else {
        push("-k", s.codec.clone());
        // no CUDA (after a suspend, a driver mismatch): x264 instead of failing
        push("-fallback-cpu-encoding", "yes".into());
    }
    push("-q", s.quality.clone());
    push("-f", s.fps.to_string());
    // constant frame rate: what video editors (and Snip's own) expect
    push("-fm", "cfr".into());
    push("-cursor", if s.cursor { "yes" } else { "no" }.into());
    if let Some((w, h)) = s.max_size {
        push("-s", format!("{w}x{h}"));
    }
    let sources = s.audio.sources();
    if !sources.is_empty() {
        for src in sources {
            push("-a", src);
        }
        // AAC: every player and browser takes it in an MP4 (opus is gsr's default)
        push("-ac", "aac".into());
    }
    // a recorder killed mid-take leaves a file Snip can recover (REC-14):
    // each packet goes to disk, and a fragment closes every second
    push("-ffmpeg-opts", "flush_packets=1;frag_duration=1000000".into());
    // no device names in the tracks' titles
    push("-exclude-metadata", "yes".into());
    push("-write-first-frame-ts", "yes".into());
    push("-ipc", s.ipc.to_string_lossy().into_owned());
    push("-v", "no".into());
    push("-o", s.file.to_string_lossy().into_owned());
    a
}

/// Whether NVENC can encode a `w`×`h` (physical px) video (measured with gsr
/// 6.1 on an RTX 4080: H.264 needs 146 px wide and pads below 128 px tall).
pub fn gpu_can_encode(w: u32, h: u32) -> bool {
    w >= 160 && h >= 128
}

/// A selection (physical px of an output whose layout position is `origin`,
/// at `scale`) as the area gsr records: global logical px, whole, at least
/// 2×2. gsr can only take whole logical px, so the recording may be up to a
/// pixel off the selection at a fractional scale.
pub fn area_from_physical(rect: Rect, origin: (i32, i32), scale: f64) -> Rect {
    let s = if scale > 0.0 { scale } else { 1.0 };
    let x0 = (rect.x as f64 / s).round() as i32;
    let y0 = (rect.y as f64 / s).round() as i32;
    let x1 = (rect.right() as f64 / s).round() as i32;
    let y1 = (rect.bottom() as f64 / s).round() as i32;
    Rect::new(origin.0 + x0, origin.1 + y0, (x1 - x0).max(2), (y1 - y0).max(2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(target: Target, audio: Audio) -> Spec {
        Spec { target, fps: 60, cursor: true, audio, codec: "auto".into(), quality: "very_high".into(), max_size: None, cpu: false, file: "/run/rec/take.mp4".into(), ipc: "/run/rec/gsr.sock".into() }
    }

    fn pairs(a: &[String]) -> Vec<(String, String)> {
        a.chunks(2).map(|c| (c[0].clone(), c[1].clone())).collect()
    }

    #[test]
    fn an_area_with_both_sounds_mixed() {
        let a = gsr_args(&spec(Target::Area(Rect::new(100, 50, 800, 600)), Audio { mic: Some("default_input".into()), system: true, separate: false }));
        let p = pairs(&a);
        assert_eq!(p[0], ("-w".into(), "800x600+100+50".into()), "an area is the capture itself");
        assert!(p.contains(&("-a".into(), "default_output|default_input".into())) && p.contains(&("-ac".into(), "aac".into())), "{a:?}");
        assert!(p.contains(&("-fm".into(), "cfr".into())) && p.contains(&("-write-first-frame-ts".into(), "yes".into())));
        assert_eq!(p.last().unwrap(), &("-o".into(), "/run/rec/take.mp4".into()), "the output goes last");
        assert!(p.contains(&("-ipc".into(), "/run/rec/gsr.sock".into())));
        assert!(p.contains(&("-ffmpeg-opts".into(), "flush_packets=1;frag_duration=1000000".into())), "recoverable if killed");
        assert!(p.contains(&("-fallback-cpu-encoding".into(), "yes".into())), "x264 when CUDA can't start");
    }

    #[test]
    fn a_monitor_with_separate_tracks_or_none() {
        let a = gsr_args(&spec(Target::Output("DP-2".into()), Audio { mic: Some("alsa_input.usb-mic".into()), system: true, separate: true }));
        let p = pairs(&a);
        assert_eq!(p[0], ("-w".into(), "DP-2".into()));
        let tracks: Vec<_> = p.iter().filter(|(k, _)| k == "-a").map(|(_, v)| v.as_str()).collect();
        assert_eq!(tracks, ["default_output", "device:alsa_input.usb-mic"], "one track each");
        let silent = gsr_args(&spec(Target::Output("DP-2".into()), Audio::default()));
        assert!(!silent.iter().any(|x| x == "-a" || x == "-ac"), "no sound, no audio track");
        let mut s = spec(Target::Output("DP-2".into()), Audio::default());
        s.max_size = Some((1920, 1080));
        s.cursor = false;
        let p = pairs(&gsr_args(&s));
        assert!(p.contains(&("-s".into(), "1920x1080".into())) && p.contains(&("-cursor".into(), "no".into())));
        // a small area: the CPU's H.264
        s.cpu = true;
        let p = pairs(&gsr_args(&s));
        assert!(p.contains(&("-encoder".into(), "cpu".into())) && p.contains(&("-k".into(), "h264".into())));
        assert!(!gpu_can_encode(140, 300) && !gpu_can_encode(400, 100) && gpu_can_encode(160, 128));
    }

    #[test]
    fn a_selection_becomes_whole_logical_px() {
        // DP-2 at 1.25, at the layout's origin: 1000×500 physical from (125, 250)
        assert_eq!(area_from_physical(Rect::new(125, 250, 1000, 500), (0, 0), 1.25), Rect::new(100, 200, 800, 400));
        // a second monitor to its right: its layout position is added
        assert_eq!(area_from_physical(Rect::new(0, 0, 1920, 1080), (2752, 0), 1.0), Rect::new(2752, 0, 1920, 1080));
        // a fractional edge rounds to the nearest logical px; never smaller than 2×2
        assert_eq!(area_from_physical(Rect::new(1, 1, 999, 501), (0, 0), 1.25), Rect::new(1, 1, 799, 401));
        assert_eq!(area_from_physical(Rect::new(10, 10, 1, 1), (0, 0), 1.25), Rect::new(8, 8, 2, 2));
    }
}
