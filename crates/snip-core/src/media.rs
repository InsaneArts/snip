//! Videos and GIFs (REC, VED): what a file holds (ffprobe), and its first
//! frame as a poster (ffmpeg). Both run as child processes, which keeps
//! libav* out of Snip's binary (architecture §1, "GIF / video post").

use std::path::Path;
use std::process::{Command, Stdio};

/// What `probe` reads from a video or GIF.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
    /// frames per second of the video stream (0 if unknown)
    pub fps: f64,
    /// how many audio streams it has (gsr records one per `-a`)
    pub audio_tracks: u32,
}

/// Read a video's size, length, frame rate and audio streams with ffprobe.
pub fn probe(path: &Path) -> Result<Probe, String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-print_format", "json", "-show_entries", "format=duration:stream=codec_type,width,height,avg_frame_rate,duration"])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    if !out.status.success() {
        return Err(format!("ffprobe: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    parse_probe(&String::from_utf8_lossy(&out.stdout))
}

fn parse_probe(json: &str) -> Result<Probe, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s["codec_type"] == "video").ok_or("no video stream")?;
    let secs = |x: &serde_json::Value| x.as_str().and_then(|s| s.parse::<f64>().ok());
    let duration = secs(&v["format"]["duration"]).or_else(|| secs(&video["duration"])).unwrap_or(0.0);
    let fps = video["avg_frame_rate"]
        .as_str()
        .and_then(|r| r.split_once('/'))
        .and_then(|(n, d)| Some((n.parse::<f64>().ok()?, d.parse::<f64>().ok()?)))
        .filter(|&(_, d)| d > 0.0)
        .map_or(0.0, |(n, d)| n / d);
    Ok(Probe {
        width: video["width"].as_u64().unwrap_or(0) as u32,
        height: video["height"].as_u64().unwrap_or(0) as u32,
        duration_ms: (duration * 1000.0).round() as u64,
        fps,
        audio_tracks: streams.iter().filter(|s| s["codec_type"] == "audio").count() as u32,
    })
}

/// The first frame, as PNG bytes: a recording's poster for cards and history.
pub fn poster(path: &Path) -> Result<Vec<u8>, String> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(format!("ffmpeg: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(out.stdout)
}

/// What the video editor exports (VED-01…03): the parts kept, the sound,
/// the size and quality, MP4 or GIF.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    /// the parts kept, in seconds, in order (a trim is one, cuts make more)
    pub keep: Vec<(f64, f64)>,
    /// each audio track's volume (1 as recorded, 0 muted); several are
    /// mixed into one track. Empty: no sound.
    pub tracks: Vec<f64>,
    pub mono: bool,
    /// the largest height (None: as recorded); the width follows
    pub height: Option<u32>,
    /// constant quality, 0 (best) to 51 (H.264's scale)
    pub quality: u32,
    /// a GIF (frames a second, the largest width) instead of an MP4
    pub gif: Option<(u32, u32)>,
    /// "h264_nvenc" or "libx264"
    pub encoder: String,
}

/// ffmpeg's arguments for `edit` from `src` to `dst`, with progress on stdout.
pub fn export_args(src: &Path, dst: &Path, edit: &Edit) -> Vec<String> {
    let between = edit.keep.iter().map(|(a, b)| format!("between(t\\,{a:.3}\\,{b:.3})")).collect::<Vec<_>>().join("+");
    let select = if edit.keep.is_empty() { String::new() } else { format!("select='{between}',setpts=N/FRAME_RATE/TB,") };
    let aselect = if edit.keep.is_empty() { String::new() } else { format!("aselect='{between}',asetpts=N/SR/TB,") };
    let mut graph = vec![];
    match edit.gif {
        Some((fps, width)) => graph.push(format!(
            "[0:v]{select}fps={fps},scale='min({width},iw)':-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle[v]"
        )),
        None => {
            let scale = edit.height.map_or(String::new(), |h| format!(",scale=-2:'min({h},ih)':flags=lanczos"));
            graph.push(format!("[0:v]{select}format=yuv420p{scale}[v]"));
        }
    }
    let sound = edit.gif.is_none() && edit.tracks.iter().any(|&v| v > 0.0);
    if sound {
        let live: Vec<(usize, f64)> = edit.tracks.iter().copied().enumerate().filter(|&(_, v)| v > 0.0).collect();
        let mut parts = vec![];
        for (i, v) in &live {
            parts.push(format!("[0:a:{i}]volume={v:.3}[t{i}]"));
        }
        let ins: String = live.iter().map(|(i, _)| format!("[t{i}]")).collect();
        let mixed = if live.len() > 1 { format!("{ins}amix=inputs={}:normalize=0:duration=longest,", live.len()) } else { ins.clone() };
        let mono = if edit.mono { "pan=mono|c0=0.5*c0+0.5*c1," } else { "" };
        let chain = format!("{aselect}{mono}aresample=async=1");
        parts.push(if live.len() > 1 { format!("{mixed}{chain}[a]") } else { format!("{ins}{chain}[a]") });
        graph.push(parts.join(";"));
    }
    let mut a: Vec<String> = ["-v", "error", "-nostats", "-progress", "pipe:1", "-y", "-i"].map(String::from).to_vec();
    a.push(src.to_string_lossy().into_owned());
    a.extend(["-filter_complex".into(), graph.join(";"), "-map".into(), "[v]".into()]);
    if sound {
        a.extend(["-map".into(), "[a]".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "160k".into()]);
    }
    match edit.gif {
        Some(_) => a.extend(["-loop".into(), "0".into()]),
        None if edit.encoder == "h264_nvenc" => {
            a.extend(["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", &edit.quality.min(51).to_string(), "-b:v", "0"].map(String::from));
        }
        None => a.extend(["-c:v", "libx264", "-preset", "medium", "-crf", &edit.quality.min(51).to_string()].map(String::from)),
    }
    if edit.gif.is_none() {
        a.extend(["-movflags".into(), "+faststart".into()]);
    }
    a.push(dst.to_string_lossy().into_owned());
    a
}

/// Run an export, calling `progress` with 0…1 (ffmpeg's out_time over the
/// kept length). Returns ffmpeg's error, if any.
pub fn export(src: &Path, dst: &Path, edit: &Edit, mut progress: impl FnMut(f64)) -> Result<(), String> {
    use std::io::BufRead;
    let total: f64 = edit.keep.iter().map(|(a, b)| (b - a).max(0.0)).sum();
    let mut child = Command::new("ffmpeg")
        .args(export_args(src, dst, edit))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok()) {
                if total > 0.0 {
                    progress((us / 1e6 / total).clamp(0.0, 1.0));
                }
            }
        }
    }
    let out = child.wait_with_output().map_err(|e| format!("ffmpeg: {e}"))?;
    if out.status.success() {
        progress(1.0);
        Ok(())
    } else {
        Err(format!("ffmpeg: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

/// A recording as a GIF (REC-13): `fps` frames a second, at most `width` px
/// wide (never enlarged), one palette fitted to the clip (architecture §1,
/// "GIF / video post").
pub fn gif(src: &Path, dst: &Path, fps: u32, width: u32) -> Result<(), String> {
    let filter = format!(
        "fps={fps},scale='min({width},iw)':-1:flags=lanczos,split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"
    );
    run_ffmpeg(&["-v", "error", "-y", "-i", &src.to_string_lossy(), "-filter_complex", &filter, "-loop", "0", &dst.to_string_lossy()])
}

/// Tests' stand-in for a recording (SNIP_RECORDER=fake): ffmpeg's test
/// pattern, `w`×`h` for `ms`, with a tone when `audio`.
pub fn test_clip(dst: &Path, w: u32, h: u32, ms: u64, audio: bool) -> Result<(), String> {
    let secs = format!("{:.3}", ms.max(100) as f64 / 1000.0);
    let video = format!("testsrc2=size={}x{}:rate=30:duration={secs}", w & !1, h & !1);
    let mut args: Vec<String> = ["-v", "error", "-y", "-f", "lavfi", "-i", &video].map(String::from).to_vec();
    if audio {
        args.extend(["-f", "lavfi", "-i", &format!("sine=duration={secs}"), "-c:a", "aac", "-shortest"].map(String::from));
    }
    args.extend(["-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", &dst.to_string_lossy()].map(|s| s.to_string()));
    run_ffmpeg(&args.iter().map(String::as_str).collect::<Vec<_>>())
}

fn run_ffmpeg(args: &[&str]) -> Result<(), String> {
    let out = Command::new("ffmpeg").args(args).stdin(Stdio::null()).output().map_err(|e| format!("ffmpeg: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg: {}", String::from_utf8_lossy(&out.stderr).trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(keep: Vec<(f64, f64)>, tracks: Vec<f64>) -> Edit {
        Edit { keep, tracks, mono: false, height: None, quality: 23, gif: None, encoder: "libx264".into() }
    }

    #[test]
    fn export_keeps_the_parts_and_mixes_the_tracks() {
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 2.0), (4.5, 6.0)], vec![1.0, 0.5]));
        let graph = &a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1];
        assert!(graph.contains("select='between(t\\,0.000\\,2.000)+between(t\\,4.500\\,6.000)'"), "{graph}");
        assert!(graph.contains("[0:a:0]volume=1.000[t0];[0:a:1]volume=0.500[t1];[t0][t1]amix=inputs=2:normalize=0"), "{graph}");
        assert!(a.windows(2).any(|w| w == ["-crf", "23"]) && a.last().unwrap() == "out.mp4");
        // muted: no audio at all; a muted track drops out of the mix
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 1.0)], vec![0.0]));
        assert!(!a.iter().any(|x| x == "[a]"), "{a:?}");
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 1.0)], vec![0.0, 1.0]));
        let graph = &a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1];
        assert!(graph.contains("[0:a:1]volume=1.000[t1];[t1]aselect") && !graph.contains("amix") && !graph.contains("[0:a:0]"), "{graph}");
    }

    #[test]
    fn probe_reads_size_length_rate_and_tracks() {
        let json = r#"{"streams": [
            {"codec_type": "video", "width": 1920, "height": 1080, "avg_frame_rate": "60/1", "duration": "4.000000"},
            {"codec_type": "audio", "duration": "4.010000"},
            {"codec_type": "audio"}],
          "format": {"duration": "4.021000"}}"#;
        assert_eq!(parse_probe(json).unwrap(), Probe { width: 1920, height: 1080, duration_ms: 4021, fps: 60.0, audio_tracks: 2 });
        assert!(parse_probe(r#"{"streams": [{"codec_type": "audio"}], "format": {}}"#).is_err(), "no video, no probe");
        // a GIF: no rate when ffprobe gives 0/0
        let gif = parse_probe(r#"{"streams": [{"codec_type": "video", "width": 8, "height": 6, "avg_frame_rate": "0/0"}], "format": {"duration": "1.5"}}"#).unwrap();
        assert_eq!((gif.fps, gif.duration_ms, gif.audio_tracks), (0.0, 1500, 0));
    }

    /// A real file through ffprobe and ffmpeg (skipped where ffmpeg isn't installed).
    #[test]
    fn a_real_clip_probes_and_gives_a_poster() {
        let dir = std::env::temp_dir().join(format!("snip-media-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("clip.mp4");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=320x200:rate=30:duration=1", "-f", "lavfi", "-i", "sine=duration=1"])
            .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
            .arg(&clip)
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            eprintln!("ffmpeg with libx264 isn't available: skipped");
            return;
        }
        let p = probe(&clip).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (320, 200, 1));
        assert!((900..=1100).contains(&p.duration_ms) && (p.fps - 30.0).abs() < 0.01, "{p:?}");
        let png = poster(&clip).unwrap();
        assert_eq!(crate::encode::decode_png(&png).map(|(_, w, h, _)| (w, h)).unwrap(), (320, 200));
        // as a GIF: narrower than asked is kept as it is; wider is scaled down
        let g = dir.join("clip.gif");
        gif(&clip, &g, 10, 800).unwrap();
        let p = probe(&g).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (320, 200, 0), "never enlarged");
        gif(&clip, &g, 10, 160).unwrap();
        assert_eq!((probe(&g).unwrap().width, probe(&g).unwrap().height), (160, 100));
        // exported: two parts kept (0.2–0.5 s and 0.7–0.9 s), smaller, as an MP4 and a GIF
        let out = dir.join("cut.mp4");
        let mut e = edit(vec![(0.2, 0.5), (0.7, 0.9)], vec![1.0]);
        e.height = Some(100);
        let mut seen = vec![];
        export(&clip, &out, &e, |f| seen.push(f)).unwrap();
        let p = probe(&out).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (160, 100, 1), "{p:?}");
        assert!((450..=600).contains(&p.duration_ms), "0.5 s kept: {p:?}");
        assert_eq!(seen.last(), Some(&1.0), "progress ends at 1");
        e.gif = Some((10, 120));
        let g = dir.join("cut.gif");
        export(&clip, &g, &e, |_| {}).unwrap();
        assert_eq!((probe(&g).unwrap().width, probe(&g).unwrap().audio_tracks), (120, 0));
        // the tests' stand-in recording
        let t = dir.join("take.mp4");
        test_clip(&t, 401, 301, 1500, true).unwrap();
        let p = probe(&t).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (400, 300, 1), "even sizes, as an encoder needs");
        assert!((1400..=1600).contains(&p.duration_ms), "{p:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
