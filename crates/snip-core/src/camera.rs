//! A studio take's camera, recorded apart (D61): ffmpeg reads the camera
//! (V4L2) beside gsr, its frames keeping the kernel's timestamps, so the
//! first one says when it started on the take's clock; the editor places it.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A camera recorded beside a take, from the take's start to its stop.
pub struct Recording {
    /// ffmpeg, recording into `file` (Matroska); none for the tests' camera
    child: Option<Child>,
    file: PathBuf,
    /// the tests' camera's start (µs, CLOCK_MONOTONIC)
    started: i64,
}

/// How long a camera still held (the panel's bubble letting go of it as the
/// take starts) is tried again before it's given up.
const HELD: Duration = Duration::from_secs(3);

impl Recording {
    /// Record the camera called `name` beside the take at `take`, into
    /// `<take>.camera.mkv`; `fake`: the tests' camera, its bars made at the
    /// stop. None: there's no such camera, or it won't record.
    pub fn start(name: &str, take: &Path, fake: bool) -> Option<Recording> {
        let file = take.with_extension("camera.mkv");
        let started = crate::trace::now_ns() as i64 / 1000;
        if fake {
            return Some(Recording { child: None, file, started });
        }
        let device = device(name)?;
        let log = take.with_extension("camera.log");
        let (t0, mut plain) = (Instant::now(), false);
        loop {
            let err = std::fs::File::create(&log).ok()?;
            let mut child = Command::new("ffmpeg").args(record_args(&device, &file, plain)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(err).spawn().ok()?;
            // still going after a moment: it records
            std::thread::sleep(Duration::from_millis(400));
            if let Ok(None) = child.try_wait() {
                return Some(Recording { child: Some(child), file, started });
            }
            let why = std::fs::read_to_string(&log).unwrap_or_default();
            if why.contains("busy") && t0.elapsed() < HELD {
                std::thread::sleep(Duration::from_millis(200));
            } else if !plain {
                // not that format: whatever the camera gives
                plain = true;
            } else {
                crate::trace::warn("camera", &format!("\"msg\":{}", crate::trace::js(why.trim())));
                let _ = std::fs::remove_file(&log);
                return None;
            }
        }
    }

    /// Stop it: the camera file beside the take (`<take>.camera.mp4`), its
    /// time from 0, and when its first frame was (µs, CLOCK_MONOTONIC); None
    /// when nothing was recorded, or for a take not kept.
    pub fn stop(mut self, keep: bool) -> Option<(PathBuf, i64)> {
        let out = self.file.with_extension("mp4");
        let fake = self.child.is_none();
        if let Some(child) = self.child.as_mut() {
            crate::take::interrupt(child);
            crate::take::reap(child, Duration::from_secs(10));
        }
        let made: Result<i64, String> = match (keep, fake) {
            (false, _) => Err("not kept".into()),
            (true, true) => fake_recording(&out, ((crate::trace::now_ns() as i64 / 1000 - self.started) / 1000).max(0) as u64).map(|_| self.started),
            (true, false) => finish(&self.file, &out),
        };
        for f in [self.file.clone(), self.file.with_extension("log")] {
            let _ = std::fs::remove_file(f);
        }
        match made {
            Ok(first) => Some((out, first)),
            Err(e) => {
                if keep {
                    crate::trace::warn("camera", &format!("\"msg\":{}", crate::trace::js(&e)));
                }
                let _ = std::fs::remove_file(&out);
                None
            }
        }
    }
}

/// The capture device of the camera called `name` (as QtMultimedia names
/// it: the driver's card name), e.g. /dev/video0.
pub fn device(name: &str) -> Option<PathBuf> {
    device_in(Path::new("/sys/class/video4linux"), name)
}

fn device_in(sys: &Path, name: &str) -> Option<PathBuf> {
    let read = |d: &Path, f: &str| std::fs::read_to_string(d.join(f)).ok().map(|v| v.trim().to_string());
    // a camera's nodes share its name: the capture one is its first (index 0)
    let mut nodes: Vec<(u32, String)> = std::fs::read_dir(sys)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let node = e.file_name().to_string_lossy().into_owned();
            let n = node.strip_prefix("video")?.parse().ok()?;
            (read(&e.path(), "name")? == name && read(&e.path(), "index").as_deref() == Some("0")).then_some((n, node))
        })
        .collect();
    nodes.sort();
    nodes.first().map(|(_, node)| Path::new("/dev").join(node))
}

/// ffmpeg's arguments to record the camera at `device` into `out`
/// (Matroska, so a stop at any moment leaves a file), its frames keeping the
/// kernel's timestamps (`-copyts`). MJPEG at 1280×720 and 30 fps first (UVC
/// cameras have it, and it's cheap to read); `plain`: whatever the camera
/// gives by default, for one that hasn't.
fn record_args(device: &Path, out: &Path, plain: bool) -> Vec<String> {
    let mut a: Vec<String> = ["-v", "error", "-nostdin", "-y", "-f", "v4l2"].map(String::from).to_vec();
    if !plain {
        a.extend(["-input_format", "mjpeg", "-video_size", "1280x720", "-framerate", "30"].map(String::from));
    }
    a.extend(["-i".into(), device.to_string_lossy().into_owned()]);
    a.extend(["-copyts", "-an", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "22", "-pix_fmt", "yuv420p"].map(String::from));
    a.push(out.to_string_lossy().into_owned());
    a
}

/// Tests' stand-in for a camera recording (SNIP_CAMERA=fake): SMPTE bars,
/// `ms` long, into `out`, its time from 0.
fn fake_recording(out: &Path, ms: u64) -> Result<(), String> {
    let bars = format!("smptebars=size=640x480:rate=30:duration={:.3}", ms.max(100) as f64 / 1000.0);
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", &bars, "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p"])
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if made.status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg: {}", String::from_utf8_lossy(&made.stderr).trim()))
    }
}

/// The time of the first frame (µs) of the recording at `file`, as its
/// timestamps have it.
fn first_frame(file: &Path) -> Result<i64, String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=start_time", "-of", "csv=p=0"])
        .arg(file)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let s: f64 = text.trim().parse().map_err(|_| format!("ffprobe: no start in {:?}", text.trim()))?;
    Ok((s * 1e6).round() as i64)
}

/// A camera recording made the take's camera file: `recorded` (Matroska,
/// the kernel's timestamps) into `out` (MP4, its time from 0). Answers when
/// its first frame was (µs, CLOCK_MONOTONIC for a UVC camera).
fn finish(recorded: &Path, out: &Path) -> Result<i64, String> {
    let first = first_frame(recorded)?;
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(recorded)
        .args(["-map", "0:v", "-c", "copy", "-movflags", "+faststart"])
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    if !made.status.success() {
        return Err(format!("ffmpeg: {}", String::from_utf8_lossy(&made.stderr).trim()));
    }
    let _ = std::fs::remove_file(recorded);
    Ok(first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("snip-camera-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn sys(dir: &Path, nodes: &[(&str, &str, u32)]) {
        for (node, name, index) in nodes {
            let d = dir.join(node);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("name"), format!("{name}\n")).unwrap();
            std::fs::write(d.join("index"), format!("{index}\n")).unwrap();
        }
    }

    #[test]
    fn a_camera_is_found_by_its_name_at_its_capture_node() {
        let dir = scratch("sys");
        // a webcam has two nodes (capture, then metadata), another camera one
        sys(&dir, &[("video2", "HD Pro Webcam C920", 1), ("video1", "HD Pro Webcam C920", 0), ("video0", "Integrated Camera", 0)]);
        assert_eq!(device_in(&dir, "HD Pro Webcam C920"), Some(PathBuf::from("/dev/video1")));
        assert_eq!(device_in(&dir, "Integrated Camera"), Some(PathBuf::from("/dev/video0")));
        assert_eq!(device_in(&dir, "Gone"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_recording_is_made_the_takes_camera_file_from_its_first_frame() {
        // a camera's frames carry the kernel's clock: their time starts at
        // 382000 s (the machine's uptime), not 0
        let dir = scratch("finish");
        let (mkv, mp4) = (dir.join("cam.mkv"), dir.join("cam.mp4"));
        let made = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "smptebars=size=320x240:rate=30:duration=1", "-output_ts_offset", "382000.25"])
            .args(["-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p"])
            .arg(&mkv)
            .stdin(Stdio::null())
            .status()
            .unwrap();
        assert!(made.success());
        let first = finish(&mkv, &mp4).unwrap();
        assert!((first - 382_000_250_000).abs() < 1000, "{first}");
        assert!(first_frame(&mp4).unwrap().abs() < 1000, "its time from 0: {:?}", first_frame(&mp4));
        assert!(!mkv.exists(), "the recording is gone once made");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_test_cameras_recording_is_its_bars_from_when_it_started() {
        let dir = scratch("fake");
        let take = dir.join("take.mp4");
        let t0 = crate::trace::now_ns() as i64 / 1000;
        let r = Recording::start("Test camera", &take, true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let (file, first) = r.stop(true).unwrap();
        assert_eq!(file, dir.join("take.camera.mp4"));
        assert!(first >= t0 && first - t0 < 100_000, "it started then: {}", first - t0);
        assert!(file.metadata().unwrap().len() > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_camera_thats_gone_isnt_recorded() {
        let dir = scratch("gone");
        assert!(Recording::start("No such camera", &dir.join("take.mp4"), false).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
