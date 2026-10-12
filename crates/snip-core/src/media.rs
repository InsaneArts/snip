//! Videos and GIFs (REC, VED): what a file holds (ffprobe), and its first
//! frame as a poster (ffmpeg). Both run as child processes, which keeps
//! libav* out of Snip's binary (architecture §1, "GIF / video post").

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
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
    /// constant quality on x264's scale, 0 (best) to 51; NVENC looks the
    /// same 9 steps higher (equal SSIM, ffmpeg research)
    pub quality: u32,
    /// a GIF (frames a second, the largest width) instead of an MP4
    pub gif: Option<(u32, u32)>,
    /// "h264_nvenc" or "libx264"
    pub encoder: String,
    /// the video's frame rate: cuts land on its frames
    pub fps: f64,
    /// a background around it (D50), or none
    pub backdrop: Option<Backdrop>,
}

/// A background around the video (D50): an image the size of the result,
/// the background with its shadow and inset, and a hole where the video
/// shows through, its corners rounded. The editor draws it as the Background
/// tool draws a screenshot's.
#[derive(Clone, Debug, PartialEq)]
pub struct Backdrop {
    pub cover: PathBuf,
    /// the result's size
    pub size: (u32, u32),
    /// where the video goes in it: x, y, width, height
    pub video: (u32, u32, u32, u32),
}

/// `[vc]` in its background, as `[vb]`: padded to the background's size
/// (in 4:4:4, so the hole is exact to the pixel), the cover (input 1) on
/// top. The size is made even, as 4:2:0 needs.
fn backdrop_graph(b: &Backdrop) -> String {
    let (w, h) = (b.size.0 & !1, b.size.1 & !1);
    let (vw, vh) = (b.video.2.clamp(2, w), b.video.3.clamp(2, h));
    let (x, y) = (b.video.0.min(w - vw), b.video.1.min(h - vh));
    format!(";[vc]scale={vw}:{vh}:flags=lanczos,format=yuv444p,pad={w}:{h}:{x}:{y}[vp];[vp][1:v]overlay=0:0:format=yuv444[vb]")
}

/// The joined video's label: `vb` in a background, else `vc`.
fn video_label(edit: &Edit) -> &'static str {
    if edit.backdrop.is_some() { "vb" } else { "vc" }
}

/// The inputs after the video: the cover, if any.
fn cover_input(edit: &Edit) -> Vec<String> {
    edit.backdrop.as_ref().map_or(vec![], |b| vec!["-i".into(), b.cover.to_string_lossy().into_owned()])
}

/// The parts kept, joined into `[vc]` (and `[ac]`, the tracks mixed, when
/// `sound`). The frames are made regular first and each cut lands on one:
/// `select` had left frames over at each cut, and the sound up to 43 ms
/// off the picture (ffmpeg research: this is exact).
fn cut_graph(edit: &Edit, sound: bool) -> String {
    parts_graph(edit, 0, true, sound)
}

/// The parts kept, each cut on a frame (`edit.fps`), in seconds.
fn kept_frames(edit: &Edit) -> Vec<(f64, f64)> {
    let fps = if edit.fps > 0.0 { edit.fps } else { 30.0 };
    let snap = |t: f64| (t * fps).round() / fps;
    edit.keep.iter().map(|&(a, b)| (snap(a), snap(b))).filter(|(a, b)| b > a).collect()
}

/// `cut_graph` for input `input`: its picture joined as `[vc]` when
/// `video`, its tracks mixed as `[ac]` when `sound`.
fn parts_graph(edit: &Edit, input: usize, video: bool, sound: bool) -> String {
    let fps = if edit.fps > 0.0 { edit.fps } else { 30.0 };
    let keep = kept_frames(edit);
    let k = keep.len().max(1);
    let fan = |name: &str| (0..k).map(|i| format!("[{name}{i}]")).collect::<String>();
    let mut g = vec![];
    if video {
        g.push(format!("[{input}:v]fps={fps},split={k}{}", fan("vin")));
    }
    if sound {
        let live: Vec<(usize, f64)> = edit.tracks.iter().copied().enumerate().filter(|&(_, v)| v > 0.0).collect();
        let mut parts: Vec<String> = live.iter().map(|(i, v)| format!("[{input}:a:{i}]volume={v:.3}[t{i}]")).collect();
        let ins: String = live.iter().map(|(i, _)| format!("[t{i}]")).collect();
        let mix = if live.len() > 1 { format!("amix=inputs={}:normalize=0:duration=first,", live.len()) } else { "anull,".into() };
        let mono = if edit.mono { "pan=mono|c0=0.5*c0+0.5*c1," } else { "" };
        parts.push(format!("{ins}{mix}{mono}asplit={k}{}", fan("ain")));
        g.push(parts.join(";"));
    }
    let mut joined = String::new();
    for (i, (a, b)) in keep.iter().enumerate() {
        if video {
            g.push(format!("[vin{i}]trim=start={a:.6}:end={b:.6},setpts=PTS-STARTPTS[v{i}]"));
            joined += &format!("[v{i}]");
        }
        if sound {
            g.push(format!("[ain{i}]atrim=start={a:.6}:end={b:.6},asetpts=PTS-STARTPTS[a{i}]"));
            joined += &format!("[a{i}]");
        }
    }
    let outs = format!("{}{}", if video { "[vc]" } else { "" }, if sound { "[ac]" } else { "" });
    g.push(format!("{joined}concat=n={k}:v={}:a={}{outs}", video as u8, sound as u8));
    g.join(";")
}

/// A Studio export (D55, the design's §8.2): ffmpeg decodes the kept parts
/// (`decode`: raw I420 frames of the recording's size on stdout) for
/// snipd's renderer, which draws each frame (the background still, the card
/// zoomed) and hands it (raw I420 again, BT.709 at video range, `size`) to a
/// second ffmpeg (`encode`) that adds the sound and writes `dst`. `keep` is where each part starts
/// and ends in the recording, cut on frames, so the renderer knows each
/// frame's time.
pub struct Render {
    pub decode: Vec<String>,
    pub encode: Vec<String>,
    pub keep: Vec<(f64, f64)>,
    pub fps: f64,
}

pub fn render_args(src: &Path, dst: &Path, edit: &Edit, size: (u32, u32)) -> Render {
    let fps = if edit.fps > 0.0 { edit.fps } else { 30.0 };
    let src_s = src.to_string_lossy().into_owned();
    let mut decode: Vec<String> = ["-v", "error", "-nostdin", "-i"].map(String::from).to_vec();
    decode.extend([src_s.clone(), "-filter_complex".into(), format!("{};[vc]format=yuv420p[v]", parts_graph(edit, 0, true, false))]);
    decode.extend(["-map", "[v]", "-f", "rawvideo", "-pix_fmt", "yuv420p", "pipe:1"].map(String::from));
    let sound = edit.tracks.iter().any(|&v| v > 0.0);
    let mut encode: Vec<String> = ["-v", "error", "-nostdin", "-y", "-f", "rawvideo", "-pix_fmt", "yuv420p", "-s"].map(String::from).to_vec();
    encode.extend([format!("{}x{}", size.0, size.1), "-r".into(), format!("{fps}"), "-i".into(), "pipe:0".into()]);
    // BT.709, as gsr records it (the renderer made it so), and tagged so
    let picture = "[0:v]setparams=color_primaries=bt709:color_trc=bt709:colorspace=bt709:range=tv[v]";
    if sound {
        encode.extend(["-i".into(), src_s, "-filter_complex".into(), format!("{picture};{}", parts_graph(edit, 1, false, true))]);
        encode.extend(["-map", "[v]", "-map", "[ac]", "-c:a", "aac", "-b:a", "160k"].map(String::from));
    } else {
        encode.extend(["-filter_complex".into(), picture.into(), "-map".into(), "[v]".into()]);
    }
    let q = edit.quality.min(51);
    if edit.encoder == "h264_nvenc" {
        encode.extend(["-c:v", "h264_nvenc", "-preset", "p4", "-rc", "vbr", "-cq", &(q + 9).min(51).to_string()].map(String::from));
    } else {
        encode.extend(["-c:v", "libx264", "-preset", "medium", "-crf", &q.to_string()].map(String::from));
    }
    encode.extend(["-movflags".into(), "+faststart".into()]);
    encode.push(dst.to_string_lossy().into_owned());
    Render { decode, encode, keep: kept_frames(edit), fps }
}

/// ffmpeg's arguments for the one frame of `src` at `ms`, raw I420 on its
/// stdout (Export Current Frame: the renderer draws it).
pub fn frame_args(src: &Path, ms: f64) -> Vec<String> {
    let mut a: Vec<String> = ["-v", "error", "-nostdin", "-ss"].map(String::from).to_vec();
    a.extend([format!("{:.3}", ms.max(0.0) / 1000.0), "-i".into(), src.to_string_lossy().into_owned()]);
    a.extend(["-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "yuv420p", "pipe:1"].map(String::from));
    a
}

/// A GIF's frames from the joined video: its rate, at most `width` wide
/// (never wider).
fn gif_frames(edit: &Edit, fps: u32, width: u32) -> String {
    format!("[{}]fps={fps},scale='min({width},iw)':-1:flags=lanczos", video_label(edit))
}

/// ffmpeg's arguments for a GIF's palette (the first of its two passes: in
/// one, ffmpeg held every frame in memory, 1.5 GB for a minute of 1080p).
fn palette_args(src: &Path, palette: &Path, edit: &Edit) -> Vec<String> {
    let (fps, width) = edit.gif.unwrap_or((15, 800));
    let back = edit.backdrop.as_ref().map_or(String::new(), backdrop_graph);
    let graph = format!("{}{back};{},palettegen=stats_mode=diff[p]", cut_graph(edit, false), gif_frames(edit, fps, width));
    let mut a: Vec<String> = ["-v", "error", "-y", "-i"].map(String::from).to_vec();
    a.push(src.to_string_lossy().into_owned());
    a.extend(cover_input(edit));
    a.extend(["-filter_complex".into(), graph, "-map".into(), "[p]".into(), "-update".into(), "1".into()]);
    a.push(palette.to_string_lossy().into_owned());
    a
}

/// ffmpeg's arguments for `edit` from `src` to `dst`, with progress on
/// stdout. A GIF takes the palette `palette_args` made, beside `dst`.
pub fn export_args(src: &Path, dst: &Path, edit: &Edit) -> Vec<String> {
    let sound = edit.gif.is_none() && edit.tracks.iter().any(|&v| v > 0.0);
    let back = edit.backdrop.as_ref().map_or(String::new(), backdrop_graph);
    let graph = match edit.gif {
        Some((fps, width)) => format!(
            "{}{back};{}[x];[x][{}:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle[v]",
            cut_graph(edit, false),
            gif_frames(edit, fps, width),
            1 + edit.backdrop.is_some() as usize
        ),
        None => {
            let scale = edit.height.map_or(String::new(), |h| format!(",scale=-2:'min({h},ih)':flags=lanczos"));
            format!("{}{back};[{}]format=yuv420p{scale}[v]", cut_graph(edit, sound), video_label(edit))
        }
    };
    let mut a: Vec<String> = ["-v", "error", "-nostats", "-progress", "pipe:1", "-y", "-i"].map(String::from).to_vec();
    a.push(src.to_string_lossy().into_owned());
    a.extend(cover_input(edit));
    if edit.gif.is_some() {
        a.extend(["-i".into(), palette_for(dst).to_string_lossy().into_owned()]);
    }
    a.extend(["-filter_complex".into(), graph, "-map".into(), "[v]".into()]);
    if sound {
        a.extend(["-map".into(), "[ac]".into(), "-c:a".into(), "aac".into(), "-b:a".into(), "160k".into()]);
    }
    let q = edit.quality.min(51);
    match edit.gif {
        Some(_) => a.extend(["-loop".into(), "0".into()]),
        None if edit.encoder == "h264_nvenc" => {
            a.extend(["-c:v", "h264_nvenc", "-preset", "p4", "-rc", "vbr", "-cq", &(q + 9).min(51).to_string()].map(String::from));
        }
        None => a.extend(["-c:v", "libx264", "-preset", "medium", "-crf", &q.to_string()].map(String::from)),
    }
    if edit.gif.is_none() {
        a.extend(["-movflags".into(), "+faststart".into()]);
    }
    a.push(dst.to_string_lossy().into_owned());
    a
}

fn palette_for(dst: &Path) -> std::path::PathBuf {
    dst.with_extension("palette.png")
}

/// Run an export, calling `progress` with 0…1 (ffmpeg's out_time over the
/// kept length). Returns ffmpeg's error, if any.
pub fn export(src: &Path, dst: &Path, edit: &Edit, mut progress: impl FnMut(f64)) -> Result<(), String> {
    if edit.gif.is_some() {
        let args = palette_args(src, &palette_for(dst), edit);
        run_ffmpeg(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    }
    let done = export_frames(src, dst, edit, &mut progress);
    let _ = std::fs::remove_file(palette_for(dst));
    done
}

fn export_frames(src: &Path, dst: &Path, edit: &Edit, progress: &mut impl FnMut(f64)) -> Result<(), String> {
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

/// `n` frames spread over a video (the editor's timeline, VED-01), `height`
/// px high, as JPEGs in `dir`: one ffmpeg, the keyframe at or before each
/// point (gsr writes one every 2 s). 1.4 s of CPU for a 3-minute 3440×1440
/// recording; accurate seeks took 9 s.
pub fn thumbnails(src: &Path, duration_ms: u64, n: u32, height: u32, dir: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let n = n.max(1);
    let mut args: Vec<String> = ["-v", "error", "-y"].map(String::from).to_vec();
    for i in 0..n {
        // the middle of each slice of the timeline
        let t = (duration_ms as f64 / 1000.0) * (i as f64 + 0.5) / n as f64;
        // only keyframes are decoded, and the one before `t` is kept
        args.extend(["-noaccurate_seek".into(), "-skip_frame".into(), "nokey".into(), "-ss".into(), format!("{t:.3}"), "-i".into(), src.to_string_lossy().into_owned()]);
    }
    let mut out = vec![];
    for i in 0..n {
        let f = dir.join(format!("thumb-{i:02}.jpg"));
        args.extend(["-map".into(), format!("{i}:v:0"), "-frames:v".into(), "1".into(), "-fps_mode".into(), "passthrough".into(), "-vf".into(), format!("scale=-2:{height}"), "-q:v".into(), "4".into()]);
        args.push(f.to_string_lossy().into_owned());
        out.push(f);
    }
    run_ffmpeg(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
    Ok(out.into_iter().filter(|f| f.exists()).collect())
}

/// A recording as a GIF (REC-13): `fps` frames a second, at most `width` px
/// wide (never enlarged), one palette fitted to the clip (architecture §1,
/// "GIF / video post").
pub fn gif(src: &Path, dst: &Path, fps: u32, width: u32) -> Result<(), String> {
    let p = probe(src)?;
    let whole = Edit { keep: vec![(0.0, p.duration_ms as f64 / 1000.0)], tracks: vec![], mono: false, height: None, quality: 0, gif: Some((fps, width)), encoder: String::new(), fps: p.fps, backdrop: None };
    export(src, dst, &whole, |_| {})
}

/// Whether an MP4 was never finished: its recorder died while writing
/// fragments (a stopped gsr turns them into a plain MP4).
pub fn fragmented(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let (mut at, mut head) = (0u64, [0u8; 16]);
    // the top-level boxes: a `moof` is a fragment
    while f.seek(SeekFrom::Start(at)).is_ok() && f.read_exact(&mut head[..8]).is_ok() {
        if &head[4..8] == b"moof" {
            return true;
        }
        let size = match u32::from_be_bytes([head[0], head[1], head[2], head[3]]) {
            1 if f.read_exact(&mut head[8..]).is_ok() => u64::from_be_bytes(head[8..].try_into().unwrap()),
            n if n >= 8 => n as u64,
            // to the end of the file, or broken
            _ => return false,
        };
        at += size;
    }
    false
}

/// What a recording cut short holds, as a plain MP4 (REC-14). The edit list
/// gsr writes up front would hide the first fragment, so it's ignored.
pub fn recover(src: &Path, dst: &Path) -> Result<Probe, String> {
    let (s, d) = (src.to_string_lossy(), dst.to_string_lossy());
    // every stream: a recording's tracks of sound may be two
    let ran = run_ffmpeg(&["-v", "error", "-y", "-ignore_editlist", "1", "-fflags", "+discardcorrupt", "-i", &s, "-map", "0", "-c", "copy", "-movflags", "+faststart", &d]);
    // a cut-off last fragment is an error, though what came before is kept
    match probe(dst) {
        Ok(p) if p.duration_ms > 0 => Ok(p),
        _ => Err(ran.err().unwrap_or_else(|| "nothing to recover".into())),
    }
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
        Edit { keep, tracks, mono: false, height: None, quality: 23, gif: None, encoder: "libx264".into(), fps: 30.0, backdrop: None }
    }

    #[test]
    fn export_keeps_the_parts_and_mixes_the_tracks() {
        let graph_of = |a: &[String]| a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1].clone();
        // cuts land on frames (at 30 fps, 4.51 s is frame 135: 4.5 s)
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 2.0), (4.51, 6.0)], vec![1.0, 0.5]));
        let graph = graph_of(&a);
        assert!(graph.starts_with("[0:v]fps=30,split=2[vin0][vin1]"), "regular frames first: {graph}");
        assert!(graph.contains("[vin1]trim=start=4.500000:end=6.000000,setpts=PTS-STARTPTS[v1]"), "{graph}");
        assert!(graph.contains("[ain1]atrim=start=4.500000:end=6.000000,asetpts=PTS-STARTPTS[a1]"), "{graph}");
        assert!(graph.contains("[0:a:0]volume=1.000[t0];[0:a:1]volume=0.500[t1];[t0][t1]amix=inputs=2:normalize=0:duration=first,asplit=2[ain0][ain1]"), "{graph}");
        assert!(graph.contains("[v0][a0][v1][a1]concat=n=2:v=1:a=1[vc][ac]"), "{graph}");
        assert!(a.windows(2).any(|w| w == ["-crf", "23"]) && a.windows(2).any(|w| w == ["-map", "[ac]"]) && a.last().unwrap() == "out.mp4");
        // NVENC: the same look 9 steps higher
        let mut e = edit(vec![(0.0, 1.0)], vec![1.0]);
        e.encoder = "h264_nvenc".into();
        assert!(export_args(Path::new("in.mp4"), Path::new("out.mp4"), &e).windows(2).any(|w| w == ["-cq", "32"]));
        // muted: no audio at all; a muted track drops out of the mix
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 1.0)], vec![0.0]));
        assert!(!a.iter().any(|x| x == "[ac]") && graph_of(&a).ends_with("[v0]concat=n=1:v=1:a=0[vc];[vc]format=yuv420p[v]"), "{a:?}");
        let graph = graph_of(&export_args(Path::new("in.mp4"), Path::new("out.mp4"), &edit(vec![(0.0, 1.0)], vec![0.0, 1.0])));
        assert!(graph.contains("[0:a:1]volume=1.000[t1];[t1]anull,asplit=1[ain0]") && !graph.contains("amix") && !graph.contains("[0:a:0]"), "{graph}");
        // a GIF: its frames from the parts, through the palette made beside it
        let mut g = edit(vec![(0.0, 1.0)], vec![1.0]);
        g.gif = Some((15, 800));
        let a = export_args(Path::new("in.mp4"), Path::new("/x/out.gif"), &g);
        assert!(a.windows(2).any(|w| w == ["-i", "/x/out.palette.png"]) && !a.iter().any(|x| x == "[ac]"), "{a:?}");
        assert!(graph_of(&a).ends_with("[vc]fps=15,scale='min(800,iw)':-1:flags=lanczos[x];[x][1:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle[v]"), "{a:?}");
    }

    #[test]
    fn export_puts_the_video_in_its_background() {
        let graph_of = |a: &[String]| a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1].clone();
        let mut e = edit(vec![(0.0, 1.0)], vec![1.0]);
        e.backdrop = Some(Backdrop { cover: "/x/cover.png".into(), size: (769, 489), video: (64, 64, 640, 360) });
        let a = export_args(Path::new("in.mp4"), Path::new("out.mp4"), &e);
        assert!(a.windows(2).any(|w| w == ["-i", "/x/cover.png"]), "{a:?}");
        let graph = graph_of(&a);
        assert!(
            graph.ends_with("[vc]scale=640:360:flags=lanczos,format=yuv444p,pad=768:488:64:64[vp];[vp][1:v]overlay=0:0:format=yuv444[vb];[vb]format=yuv420p[v]"),
            "padded to an even size, the cover on top: {graph}"
        );
        // a GIF: the palette is input 2, after the cover; its first pass sees the background too
        e.gif = Some((15, 800));
        let graph = graph_of(&export_args(Path::new("in.mp4"), Path::new("/x/out.gif"), &e));
        assert!(graph.ends_with("[vb]fps=15,scale='min(800,iw)':-1:flags=lanczos[x];[x][2:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle[v]"), "{graph}");
        let p = palette_args(Path::new("in.mp4"), Path::new("/x/p.png"), &e);
        assert!(p.windows(2).any(|w| w == ["-i", "/x/cover.png"]) && graph_of(&p).contains("[vp][1:v]overlay"), "{p:?}");
    }

    /// A background around a real clip (D50): the result is the background's
    /// size, the cover around the video, the video in its hole.
    #[test]
    fn a_real_clip_exports_in_its_background() {
        let dir = std::env::temp_dir().join(format!("snip-backdrop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let clip = dir.join("blue.mp4");
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=blue:size=320x200:rate=30:duration=1", "-c:v", "libx264", "-pix_fmt", "yuv420p"])
            .arg(&clip)
            .status()
            .is_ok_and(|s| s.success());
        if !made {
            eprintln!("ffmpeg with libx264 isn't available: skipped");
            return;
        }
        // red, with a hole for the video at (40, 30)
        let (w, h) = (400u32, 260u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let hole = (40..360).contains(&x) && (30..230).contains(&y);
                let i = ((y * w + x) * 4) as usize;
                rgba[i..i + 4].copy_from_slice(if hole { &[0, 0, 0, 0] } else { &[255, 0, 0, 255] });
            }
        }
        let cover = dir.join("cover.png");
        std::fs::write(&cover, crate::encode::encode_png(&rgba, w, h, true)).unwrap();
        let mut e = edit(vec![(0.0, 1.0)], vec![]);
        e.backdrop = Some(Backdrop { cover, size: (w, h), video: (40, 30, 320, 200) });
        for name in ["out.mp4", "out.gif"] {
            e.gif = name.ends_with(".gif").then_some((10, 800));
            let out = dir.join(name);
            export(&clip, &out, &e, |_| {}).unwrap();
            let p = probe(&out).unwrap();
            assert_eq!((p.width, p.height), (w, h), "{name}: the background's size");
            let (px, pw, _, alpha) = crate::encode::decode_png(&poster(&out).unwrap()).unwrap();
            let at = |x: u32, y: u32| {
                let i = ((y * pw + x) * if alpha { 4 } else { 3 }) as usize;
                (px[i], px[i + 1], px[i + 2])
            };
            let (r, g, b) = at(10, 10);
            assert!(r > 200 && g < 60 && b < 60, "{name}: the background around it: {:?}", (r, g, b));
            let (r, g, b) = at(200, 130);
            assert!(r < 60 && g < 60 && b > 200, "{name}: the video in its hole: {:?}", (r, g, b));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_recording_cut_short_is_recovered_from_its_fragments() {
        let dir = std::env::temp_dir().join(format!("snip-recover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let whole = dir.join("whole.mp4");
        test_clip(&whole, 160, 120, 2000, true).unwrap();
        assert!(!fragmented(&whole), "a finished recording is a plain MP4");
        // a recorder killed while it wrote fragments: three quarters of a file
        let cut = dir.join("cut.mp4");
        let (w, c) = (whole.to_string_lossy(), cut.to_string_lossy());
        run_ffmpeg(&["-v", "error", "-y", "-i", &w, "-c", "copy", "-movflags", "+empty_moov+default_base_moof", "-frag_duration", "250000", &c]).unwrap();
        let bytes = std::fs::read(&cut).unwrap();
        std::fs::write(&cut, &bytes[..bytes.len() * 3 / 4]).unwrap();
        assert!(fragmented(&cut));
        let back = dir.join("back.mp4");
        let p = recover(&cut, &back).unwrap();
        assert!((500..1900).contains(&p.duration_ms) && p.audio_tracks == 1, "most of it: {p:?}");
        assert!(!fragmented(&back) && recover(&dir.join("none.mp4"), &dir.join("x.mp4")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
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
        assert!((500..=530).contains(&p.duration_ms), "0.5 s kept, exactly (the sound ends on its last packet): {p:?}");
        assert_eq!(seen.last(), Some(&1.0), "progress ends at 1");
        e.gif = Some((10, 120));
        let g = dir.join("cut.gif");
        export(&clip, &g, &e, |_| {}).unwrap();
        assert_eq!((probe(&g).unwrap().width, probe(&g).unwrap().audio_tracks), (120, 0));
        // the timeline's frames
        let thumbs = thumbnails(&clip, p.duration_ms, 4, 40, &dir.join("thumbs")).unwrap();
        assert_eq!(thumbs.len(), 4);
        assert_eq!(crate::encode::decode_png(&std::fs::read(&thumbs[0]).unwrap()).ok().map(|_| ()), None, "JPEGs");
        // the tests' stand-in recording
        let t = dir.join("take.mp4");
        test_clip(&t, 401, 301, 1500, true).unwrap();
        let p = probe(&t).unwrap();
        assert_eq!((p.width, p.height, p.audio_tracks), (400, 300, 1), "even sizes, as an encoder needs");
        assert!((1400..=1600).contains(&p.duration_ms), "{p:?}");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
