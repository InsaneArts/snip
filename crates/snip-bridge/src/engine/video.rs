//! The video editor (VED-01…03): a recording's frames for the timeline, and
//! its export through ffmpeg, which replaces the recording or becomes a new
//! capture (CleanShot's "Replace" and "Save as New").

use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use snip_core::history::Media;
use snip_core::encode;
use snip_core::media::{self, Backdrop, Edit};
use snip_core::store::{CaptureMeta, Store};
use snip_core::trace::{self, js};
use snip_core::paths;

use super::{card_json, Inner};
use crate::ui::Ui;

impl Inner {
    /// Open a recording in the video editor: what it holds at once, then its
    /// frames along the timeline once they're made (about a second).
    pub fn video_open(self: &Arc<Self>, id: &str) {
        let (me, id) = (self.clone(), id.to_string());
        std::thread::spawn(move || {
            let Ok(rec) = me.store.get(&id) else { return };
            if rec.media() != Media::Video {
                return me.toast("Only videos open in the video editor");
            }
            let p = match media::probe(&rec.source) {
                Ok(p) => p,
                Err(e) => return me.toast(format!("Can't open the video: {e}")),
            };
            let j = json!({
                "id": id, "path": rec.source, "poster": Store::poster(&rec), "title": rec.title.clone().unwrap_or_else(|| "Recording".into()),
                "duration": p.duration_ms, "fps": if p.fps > 0.0 { p.fps } else { 30.0 }, "width": p.width, "height": p.height,
                "tracks": p.audio_tracks, "bytes": rec.source.metadata().map_or(0, |m| m.len()),
                // pixels per point: a background's sizes are in points
                "scale": rec.scale, "output": me.cursor_output(),
            })
            .to_string();
            trace::trace("video_open", &format!("\"id\":\"{id}\",\"tracks\":{}", p.audio_tracks));
            me.ui(Ui::VideoEditorOpen(j));
            let dir = paths::cache_dir().join("video").join(&id);
            let _ = std::fs::remove_dir_all(&dir);
            let thumbs = media::thumbnails(&rec.source, p.duration_ms, 12, 72, &dir).unwrap_or_default();
            trace::trace("video_thumbs", &format!("\"id\":\"{id}\",\"thumbs\":{}", thumbs.len()));
            let t = json!(thumbs).to_string();
            me.ui(Ui::VideoThumbs { id, json: t });
        });
    }

    /// Export what the editor kept ({keep: [[a, b], …] s, tracks: [volume…],
    /// mono, height, quality, gif: {fps, width} | null, background: {token,
    /// video: [x, y, w, h]} | null}): `replace` the recording, else a new
    /// capture with its card. A background (D50) is the editor's grab
    /// `token`: around the video, with a hole where it shows.
    pub fn video_export(self: &Arc<Self>, id: &str, spec: &str, replace: bool) {
        let (me, id) = (self.clone(), id.to_string());
        let spec: Value = serde_json::from_str(spec).unwrap_or(Value::Null);
        std::thread::spawn(move || {
            let status = |j: Value| {
                let (id, s) = (id.clone(), j.to_string());
                me.ui(Ui::VideoExportStatus { id, json: s });
            };
            let Ok(rec) = me.store.get(&id) else { return status(json!({ "done": true, "ok": false, "error": "the recording is gone" })) };
            let keep: Vec<(f64, f64)> = spec["keep"].as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).filter(|(a, b)| b > a).collect()).unwrap_or_default();
            if keep.is_empty() {
                return status(json!({ "done": true, "ok": false, "error": "nothing is kept" }));
            }
            let gif = spec["gif"].as_object().map(|g| (g.get("fps").and_then(Value::as_u64).unwrap_or(15) as u32, g.get("width").and_then(Value::as_u64).unwrap_or(800) as u32));
            let dir = paths::runtime_dir().join("export");
            let _ = paths::ensure_private_dir(&dir);
            let stamp = chrono::Local::now().format("%H%M%S%3f").to_string();
            let backdrop = match spec["background"].as_object() {
                None => None,
                Some(b) => {
                    let Some(r) = b.get("token").and_then(Value::as_u64).and_then(|t| crate::edits::take(t as u32)) else {
                        return status(json!({ "done": true, "ok": false, "error": "the background wasn't drawn" }));
                    };
                    let cover = dir.join(format!("{id}-{stamp}-cover.png"));
                    if let Err(e) = std::fs::write(&cover, encode::encode_png(&r.rgba, r.width, r.height, true)) {
                        return status(json!({ "done": true, "ok": false, "error": e.to_string() }));
                    }
                    let at = |i: usize| b.get("video").and_then(|v| v[i].as_u64()).unwrap_or(0) as u32;
                    Some(Backdrop { cover, size: (r.width, r.height), video: (at(0), at(1), at(2), at(3)) })
                }
            };
            let cover = backdrop.as_ref().map(|b| b.cover.clone());
            let mut edit = Edit {
                keep,
                tracks: spec["tracks"].as_array().map(|a| a.iter().map(|v| v.as_f64().unwrap_or(1.0)).collect()).unwrap_or_default(),
                mono: spec["mono"].as_bool().unwrap_or(false),
                height: spec["height"].as_u64().map(|h| h as u32),
                quality: spec["quality"].as_u64().unwrap_or(23) as u32,
                gif,
                encoder: "h264_nvenc".into(),
                fps: media::probe(&rec.source).map_or(0.0, |p| p.fps),
                backdrop,
            };
            let out = dir.join(format!("{id}-{stamp}.{}", if gif.is_some() { "gif" } else { "mp4" }));
            let t0 = Instant::now();
            let mut last = Instant::now();
            let mut report = |f: f64| {
                if last.elapsed().as_millis() >= 100 || f >= 1.0 {
                    last = Instant::now();
                    status(json!({ "progress": f }));
                }
            };
            // Studio Mode (D55): with zooms, snipd draws every frame itself
            let scene = spec.get("scene").filter(|s| s["studio"]["zooms"].as_array().is_some_and(|z| !z.is_empty()));
            let run = |edit: &Edit, report: &mut dyn FnMut(f64)| match scene {
                Some(scene) => render_studio(&rec.source, &out, edit, scene, report),
                None => media::export(&rec.source, &out, edit, report),
            };
            let mut done = run(&edit, &mut report);
            if done.is_err() && gif.is_none() {
                // no NVENC (or it refused): x264 on the CPU
                edit.encoder = "libx264".into();
                done = run(&edit, &mut report);
            }
            if let Some(c) = cover {
                let _ = std::fs::remove_file(c);
            }
            if let Err(e) = done {
                let _ = std::fs::remove_file(&out);
                trace::warn("video_export", &format!("\"msg\":{}", js(&e)));
                return status(json!({ "done": true, "ok": false, "error": e }));
            }
            trace::trace("video_exported", &format!("\"id\":\"{id}\",\"replace\":{replace},\"ms\":{}", t0.elapsed().as_millis()));
            // Replace: the card stays on its output; Save as New: a card where the pointer is
            let card_output = me.cards.lock().unwrap().iter().find(|(i, _)| i == &id).map(|(_, o)| o.clone());
            let output = card_output.filter(|_| replace).or_else(|| me.cursor_output()).unwrap_or_default();
            let result = if replace {
                media::probe(&out)
                    .and_then(|p| media::poster(&out).map(|poster| (p, poster)))
                    .and_then(|(p, poster)| me.store.replace_media(&id, &out, &poster, p.width, p.height, p.duration_ms).map_err(|e| e.to_string()))
                    .map(|r| {
                        let _ = std::fs::remove_dir_all(paths::drag_dir().join(&id));
                        let j = card_json(&r, &output).to_string();
                        let rid = r.id.clone();
                        me.ui(Ui::CardUpdate { id: rid, json: j });
                        r
                    })
            } else {
                let meta = CaptureMeta { kind: "recording".into(), scale: rec.scale, output: output.clone(), app: rec.app.clone(), title: rec.title.clone() };
                me.ingest_media(&out, true, meta).inspect(|r| me.show_card(r, &output))
            };
            match result {
                Ok(r) => status(json!({ "done": true, "ok": true, "id": r.id, "media": r.media() })),
                Err(e) => status(json!({ "done": true, "ok": false, "error": e })),
            }
        });
    }
}

/// A Studio export (D55, the design's §8.2): snipd draws each frame of the
/// kept parts with the scene the editor shows (StudioFrame.qml: the
/// background still, the card zoomed), between an ffmpeg that decodes and
/// one that encodes, in a child (`snipd --render-video`), so its GPU work and
/// a crash stay out of the daemon. A GIF is drawn as an MP4 first, then made
/// a GIF as any recording is.
fn render_studio(src: &Path, dst: &Path, edit: &Edit, scene: &Value, report: &mut dyn FnMut(f64)) -> Result<(), String> {
    let probe = media::probe(src)?;
    let side = |k: &str, full: u32| scene["outer"][k].as_f64().unwrap_or(full as f64);
    let (ow, oh) = (side("w", probe.width), side("h", probe.height));
    let (mp4, drawn) = match edit.gif {
        // as any GIF: no sound, and its own width, not a height
        Some(_) => (dst.with_extension("studio.mp4"), Edit { gif: None, tracks: vec![], height: None, encoder: "libx264".into(), quality: 16, ..edit.clone() }),
        None => (dst.to_path_buf(), edit.clone()),
    };
    let k = drawn.height.map_or(1.0, |h| (h as f64 / oh).min(1.0));
    let size = (((ow * k).round() as u32).max(2) & !1, ((oh * k).round() as u32).max(2) & !1);
    let r = media::render_args(src, &mp4, &drawn, size);
    let frames: f64 = r.keep.iter().map(|(a, b)| ((b - a) * r.fps).round()).sum::<f64>().max(1.0);
    let spec = json!({
        "decode": r.decode, "encode": r.encode, "keep": r.keep, "fps": r.fps,
        "source": [probe.width, probe.height], "size": [size.0, size.1], "scale": size.0 as f64 / ow, "scene": scene,
    });
    let spec_path = dst.with_extension("render.json");
    std::fs::write(&spec_path, spec.to_string()).map_err(|e| e.to_string())?;
    // snipd's own file, even once a rebuild or an upgrade has replaced it
    // (current_exe() then names a file that's gone)
    let child = Command::new("/proc/self/exe").arg("--render-video").arg(&spec_path).stdin(Stdio::null()).stdout(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let _ = std::fs::remove_file(&spec_path);
            return Err(format!("render: {e}"));
        }
    };
    // a GIF's last fifth is the GIF itself
    let share = if edit.gif.is_some() { 0.8 } else { 1.0 };
    let mut why = vec![];
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            if let Some(n) = line.strip_prefix("frame ").and_then(|v| v.trim().parse::<f64>().ok()) {
                report((n / frames).clamp(0.0, 1.0) * share);
            } else if let Some(e) = line.strip_prefix("error ") {
                why.push(e.to_string());
            }
        }
    }
    let status = child.wait().map_err(|e| format!("render: {e}"));
    let _ = std::fs::remove_file(&spec_path);
    if !status?.success() {
        let _ = std::fs::remove_file(&mp4);
        return Err(format!("render: {}", why.join("\n")));
    }
    if let Some((fps, width)) = edit.gif {
        let made = media::gif(&mp4, dst, fps, width);
        let _ = std::fs::remove_file(&mp4);
        made?;
    }
    report(1.0);
    Ok(())
}
