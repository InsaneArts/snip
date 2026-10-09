//! The video editor (VED-01…03): a recording's frames for the timeline, and
//! its export through ffmpeg, which replaces the recording or becomes a new
//! capture (CleanShot's "Replace" and "Save as New").

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use snip_core::history::Media;
use snip_core::media::{self, Edit};
use snip_core::store::{CaptureMeta, Store};
use snip_core::trace::{self, js};
use snip_core::paths;

use super::{card_json, Inner};
use cxx_qt_lib::QString;

impl Inner {
    /// Open a recording in the video editor: what it holds, and its frames
    /// along the timeline (off the UI thread: about a second).
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
            let dir = paths::cache_dir().join("video").join(&id);
            let _ = std::fs::remove_dir_all(&dir);
            let thumbs = media::thumbnails(&rec.source, p.duration_ms, 12, 72, &dir).unwrap_or_default();
            let j = json!({
                "id": id, "path": rec.source, "poster": Store::poster(&rec), "title": rec.title.clone().unwrap_or_else(|| "Recording".into()),
                "duration": p.duration_ms, "fps": if p.fps > 0.0 { p.fps } else { 30.0 }, "width": p.width, "height": p.height,
                "tracks": p.audio_tracks, "thumbs": thumbs, "bytes": rec.source.metadata().map_or(0, |m| m.len()),
                "output": me.cursor_output(),
            })
            .to_string();
            trace::trace("video_open", &format!("\"id\":\"{id}\",\"tracks\":{},\"thumbs\":{}", p.audio_tracks, thumbs.len()));
            me.ui(move |e| e.video_editor_open(&QString::from(j.as_str())));
        });
    }

    /// Export what the editor kept ({keep: [[a, b], …] s, tracks: [volume…],
    /// mono, height, quality, gif: {fps, width} | null}): `replace` the
    /// recording, else a new capture with its card.
    pub fn video_export(self: &Arc<Self>, id: &str, spec: &str, replace: bool) {
        let (me, id) = (self.clone(), id.to_string());
        let spec: Value = serde_json::from_str(spec).unwrap_or(Value::Null);
        std::thread::spawn(move || {
            let status = |j: Value| {
                let (id, s) = (id.clone(), j.to_string());
                me.ui(move |e| e.video_export_status(&QString::from(id.as_str()), &QString::from(s.as_str())));
            };
            let Ok(rec) = me.store.get(&id) else { return status(json!({ "done": true, "ok": false, "error": "the recording is gone" })) };
            let keep: Vec<(f64, f64)> = spec["keep"].as_array().map(|a| a.iter().filter_map(|p| Some((p[0].as_f64()?, p[1].as_f64()?))).filter(|(a, b)| b > a).collect()).unwrap_or_default();
            if keep.is_empty() {
                return status(json!({ "done": true, "ok": false, "error": "nothing is kept" }));
            }
            let gif = spec["gif"].as_object().map(|g| (g.get("fps").and_then(Value::as_u64).unwrap_or(15) as u32, g.get("width").and_then(Value::as_u64).unwrap_or(800) as u32));
            let mut edit = Edit {
                keep,
                tracks: spec["tracks"].as_array().map(|a| a.iter().map(|v| v.as_f64().unwrap_or(1.0)).collect()).unwrap_or_default(),
                mono: spec["mono"].as_bool().unwrap_or(false),
                height: spec["height"].as_u64().map(|h| h as u32),
                quality: spec["quality"].as_u64().unwrap_or(23) as u32,
                gif,
                encoder: "h264_nvenc".into(),
                fps: media::probe(&rec.source).map_or(0.0, |p| p.fps),
            };
            let dir = paths::runtime_dir().join("export");
            let _ = paths::ensure_private_dir(&dir);
            let out = dir.join(format!("{id}-{}.{}", chrono::Local::now().format("%H%M%S%3f"), if gif.is_some() { "gif" } else { "mp4" }));
            let t0 = Instant::now();
            let mut last = Instant::now();
            let mut report = |f: f64| {
                if last.elapsed().as_millis() >= 100 || f >= 1.0 {
                    last = Instant::now();
                    status(json!({ "progress": f }));
                }
            };
            let mut done = media::export(&rec.source, &out, &edit, &mut report);
            if done.is_err() && gif.is_none() {
                // no NVENC (or it refused): x264 on the CPU
                edit.encoder = "libx264".into();
                done = media::export(&rec.source, &out, &edit, &mut report);
            }
            if let Err(e) = done {
                let _ = std::fs::remove_file(&out);
                trace::warn("video_export", &format!("\"msg\":{}", js(&e)));
                return status(json!({ "done": true, "ok": false, "error": e }));
            }
            trace::trace("video_exported", &format!("\"id\":\"{id}\",\"replace\":{replace},\"ms\":{}", t0.elapsed().as_millis()));
            let output = me.cursor_output().unwrap_or_default();
            let result = if replace {
                media::probe(&out)
                    .and_then(|p| media::poster(&out).map(|poster| (p, poster)))
                    .and_then(|(p, poster)| me.store.replace_media(&id, &out, &poster, p.width, p.height, p.duration_ms).map_err(|e| e.to_string()))
                    .map(|r| {
                        let _ = std::fs::remove_dir_all(paths::drag_dir().join(&id));
                        let j = card_json(&r, &output).to_string();
                        let rid = r.id.clone();
                        me.ui(move |e| e.card_update(&QString::from(rid.as_str()), &QString::from(j.as_str())));
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
