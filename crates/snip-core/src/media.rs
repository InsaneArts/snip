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
        // the tests' stand-in recording
        let t = dir.join("take.mp4");
        test_clip(&t, 401, 301, 1500, true).unwrap();
        let p = probe(&t).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (400, 300, 1), "even sizes, as an encoder needs");
        assert!((1400..=1600).contains(&p.duration_ms), "{p:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
