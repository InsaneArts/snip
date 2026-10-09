# Webcam bubble and trimmer player in Qt 6.11 QML (FFmpeg backend, Wayland, NVIDIA)

How confident each claim is:
- **[verified]**: run on this machine. Headless only, so the GPU rendering path itself was never exercised.
- **[source]**: read in the Qt 6.11.2 source (qtmultimedia / qtdeclarative tag `v6.11.2`).
- **[doc]**: Qt 6.11 docs, the Qt bug tracker, or the gsr man page or source.
- **[inferred]**: reasoning, not tested.

There was no webcam, so the camera claims are [source] or [doc] unless marked otherwise.

## Summary

1. **Camera backend.** On Linux, Qt 6.11 opens cameras through V4L2 directly (`QV4L2Camera`, FFmpeg plugin). There is no PipeWire or camera-portal path in either backend. The GStreamer backend even skips PipeWire camera devices on purpose [source].
   - This contradicts architecture §6.3, which says "PipeWire camera where available".
   - Only one process can stream a camera. If gsr, OBS or a browser is streaming it, Qt reports the error "Camera is in use" [source].
2. **Camera CPU cost.** Qt's default format choice is the highest resolution with at least 29 fps. For most webcams that is MJPEG 1080p. Qt decodes every MJPEG frame on the CPU with `QImage::fromData` on the render thread [source].
   - Measured here: 2.9–7.8 ms per 1080p frame, which is 9–23 % of one core at 30 fps [verified].
   - Qt's own bug report shows 60 % of a core for 1080p30 MJPEG versus 2 % for YUYV 640×360 (QTBUG-126695).
   - Choose YUYV or NV12 at 640×480 or smaller for the bubble. The 3 % CPU budget in `03-performance.md` holds only for that case.
3. **Bubble rendering.**
   - `VideoOutput` has had a `mirrored` property since Qt 6.9, so no `Scale` transform is needed. Combine it with `fillMode: PreserveAspectCrop` [verified API].
   - For the circle, use a `MultiEffect` mask, the same pattern as `Pin.qml` and `History.qml`.
   - Unplug and replug: Qt never resumes the stream on the same `Camera` object (QTBUG-136979) and `active` stays true (QTBUG-135869). Destroy the `Camera` and create a new one, and add a frame watchdog.
4. **Player seeking is exact but slow on originals.** Seeks decode from the previous keyframe up to the target frame, so they are frame-accurate [verified]. On 3440×1440 recordings with gsr's 2 s keyframe interval:
   - Every seek costs 55–310 ms, and a new `setPosition` cancels the seek in flight.
   - During a timeline drag at 125 Hz, the preview showed 1 frame in 0.5 s [verified].
   - A seek gate (one seek in flight at a time) raises that to 10–14 fps.
   - An **all-intra 1280 px proxy** gives 3 ms seeks and drag updates at pointer rate. It takes 1.5 s per 20 s of recording to build and keeps the original's timestamps exactly [verified].
5. **Biggest single fix: a GUI-thread stall on first use.** The first QtMultimedia use in a process probes every FFmpeg hardware device type (vdpau, cuda, vaapi, qsv, vulkan) on the GUI thread. That blocks it for **0.67–1.4 s**.
   - This happens even when only `MediaDevices` or `Camera` is used [verified].
   - Setting **both** `QT_FFMPEG_DECODING_HW_DEVICE_TYPES` and `QT_FFMPEG_ENCODING_HW_DEVICE_TYPES` skips the probe. Setting only one does not [verified].
6. **Seek boundary off-by-one.** If the position lands exactly on a frame boundary, Qt shows the frame *before* it (except at keyframes).
   - Naive `position += 1000/fps` also drifts by +⅓ ms per step at 60 fps, and skips a frame about every 50 steps.
   - Fix: seek to the middle of the frame, `(n + 0.5) × frameMs` [verified].
   - `position` only updates at 20 Hz [verified], so extrapolate the playhead between updates.
7. **Thumbnail strip.**
   - `ffmpeg -discard nokey` (keyframes only, file names are timestamps): 0.13–0.32 s.
   - The repo's current N-inputs `-ss` method: 1.05–1.6 s, and today the editor waits for it before opening.
   - In-process `QVideoSink` seeking: 1.4–4.1 s.
   - From the proxy: 0.12 s [verified].
8. **GIF playback.** Use `AnimatedImage` with `cache: false`. With the default `cache: true`, every decoded frame is kept: a 10 s, 1280-wide GIF went from 244 to 482 MB RSS. With `cache: false` memory stayed flat at 91 MB [verified]. `Preview.qml` uses the default today.
9. **Sparse-frame files plus audio output.** On a file with very few video frames per second, a paused seek **never** showed a frame while an `AudioOutput` was attached. Playback from load showed its first frame after 5.9 s [verified, same family as QTBUG-150596].
   - gsr's KMS capture emits a frame every tick in both `cfr` and `vfr` modes (gsr 6.1.3 source), and Snip already passes `-fm cfr`. So Snip's own recordings are not affected.
   - Cheap insurance for other files: attach the `AudioOutput` only while playing [verified].

---

## 0. Environment and method

- **Versions:** qt6-base / declarative / multimedia / multimedia-ffmpeg 6.11.2; FFmpeg n9.0.1; NVIDIA 610.57.04 with `libnvcuvid` present; `libva-nvidia-driver` 0.0.18; i5-13600KF (no iGPU); PipeWire 1.6.8. No `/dev/video*` and no `videodev` module loaded.
- **QtMultimedia files:** the plugin is `/usr/lib/qt6/plugins/multimedia/libffmpegmediaplugin.so` and contains `QV4L2Camera`, `QV4L2CameraDevices` and `QPipeWireCapture` (screen capture only). `libQt6Multimedia` contains PipeWire **audio** and screen capture, but no camera code.
- **Qt source:** copies of the 6.11.2 files used are in `$R/src/`, fetched from `github.com/qt/qtmultimedia` and `github.com/qt/qtdeclarative` at tag `v6.11.2`. The full qtmultimedia tree has no Wayland or subsurface code at all.
- **Test harnesses:**
  - `playerprobe.cpp`: `QMediaPlayer` plus `QVideoSink`, offscreen.
  - `jpegbench.cpp`, `gifbench.cpp`: micro-benchmarks.
  - QML probes run with `qml6` offscreen. Offscreen means the software scene graph, so GPU texture paths were not exercised.
- **Test media** (ffmpeg-generated):
  - `t1080.mp4`: 1920×1080, CFR 60 fps, x264, 2 s keyframe interval, no B-frames, AAC.
  - `screen3440.mp4`: screen-like content, 3440×1440, NVENC constant QP 24, 2 s keyframe interval, 1 µs timebase, `hybrid_fragmented` MP4, AAC, about 6.5 Mbit/s.
  - `gsr3440.mp4`: synthetic `testsrc2` pattern (heavy, about 44 Mbit/s), VFR with a 3 s gap.
  - `sparse_a.mp4`: 1 fps for 10 s, then 60 fps, with AAC.
- **Logging gotcha:** when stderr is not a TTY, Qt logs to journald. Harnesses need `QT_FORCE_STDERR_LOGGING=1` [verified].

`$R` = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research`

---

## 1. The webcam bubble

### 1.1 How Qt 6.11 gets camera frames on Linux

| Fact | Detail | Confidence |
|---|---|---|
| Backend | FFmpeg plugin → `QV4L2Camera` (raw V4L2 ioctls; FFmpeg is not used for cameras) | [source] `qv4l2camera.cpp` |
| PipeWire / portal | None for cameras. PipeWire appears in Qt 6.11 only for audio and screen capture. The GStreamer backend even skips PipeWire video sources ("infinite futex wait") | [source] `src/multimedia/pipewire/*`, `qgstreamervideodevices.cpp:152-155` |
| Enumeration | Scans `/dev/video*`; opens each one `O_RDONLY`; keeps devices with VIDEO_CAPTURE and STREAMING that are not META_CAPTURE (UVC metadata nodes are skipped). Formats come from `ENUM_FMT/FRAMESIZES/FRAMEINTERVALS`, collapsed to min/max fps per resolution | [source] `qv4l2cameradevices.cpp:55-170` |
| Default camera | The first `/dev/videoN` in name order (`video10` sorts before `video2`). A v4l2loopback node such as OBS's virtual camera can become the default | [source] `:167` |
| Hot-plug | `QFileSystemWatcher` on `/dev` triggers a rescan and `MediaDevices.videoInputsChanged` | [source] `:36-49` |
| Supported pixel formats | YUV420, YUV422P, YUYV, UYVY, the RGB32 variants, GREY, Y16, NV12, NV21, **MJPEG/JPEG → `Format_Jpeg`**. H.264 cameras are not supported | [source] `qv4l2camera.cpp:21-45` |
| Default format | Highest resolution with at least 29 fps; ties go to the higher fps. The pixel-format score is 0 for V4L2, so MJPEG is not penalised. Typical result: **MJPEG 1920×1080@30** | [source] `qplatformcamera.cpp:19-52` |
| Frame rate | Qt always requests the format's *maximum* fps (`VIDIOC_S_PARM`). The only way to get a lower rate is to pick another format | [source] `qv4l2camera.cpp:621-628` |
| Device is open while idle | Assigning a device opens it `O_RDWR` for its controls, even with `active: false`, and keeps it open. This does not block other apps from streaming, but it keeps the UVC device awake | [source] `:106-121, 400-416` [inferred: autosuspend] |
| Exclusive streaming | `S_FMT` or `REQBUFS` failing with EBUSY gives `errorOccurred(CameraError, "Camera is in use")` | [source] `:391-398, 586-588, 643-645` |
| Thread | Frames are dequeued by a `QSocketNotifier` on the **GUI thread**, 2 buffers. A busy GUI thread drops camera frames, and the recorded bubble stutters | [source] `:686-693`, `qv4l2memorytransfer.cpp:32` |
| No camera | The `Camera` gets `error = CameraError`, `errorString = "No camera detected"` in its constructor, and `active: true` stays `false`. `errorOccurred` fires before QML handlers connect, so check `videoInputs.length` instead | [verified] `qml/devices.qml` |
| Unplug while streaming | Dequeue fails with ENODEV; Qt stops and **closes the fd**. `active` stays `true`, no error is raised, and no null frame is sent | [source] `:359-373`; QTBUG-135869, QTBUG-137118 (open) |
| Replug | The same `QCamera` never streams again: toggling `active` is a no-op because the fd is gone, and setting an equal device is a no-op because devices compare by `id`. Workarounds: a fresh `Camera`, or switching format to force a reopen | [source] `qcameradevice.cpp:285-288`; QTBUG-136979 (open, 6.9/6.10) |
| Recreating via `Loader` | A second instance showed blank output in QTBUG-139209. Workaround from the thread: keep the Camera outside the Loader. For "replug" I recommend destroying first and creating again a tick later (§6.1) | [doc] [inferred] |

### 1.2 CPU cost and latency

How a `Format_Jpeg` frame reaches the screen: `QImage::fromData(jpeg, "JPG")`, then `convertTo(ARGB32)`, then a texture upload. This runs on the window's render thread with the threaded render loop [source: `qvideotexturehelper.cpp:752-770`, `qvideoframeconverter.cpp:169-215`]. YUYV, NV12 and YUV420P frames are uploaded as they are and converted by a shader (YUYV is uploaded as RGBA8 at half width with the `yuyv` shader) [source: `:133-137, 397-398`].

The decode cost below is the same code path, measured here (`cam/jpegbench.cpp`, frames made from a wallpaper plus noise) [verified]:

| MJPEG frame | Busy frame (decode + ARGB convert) | Light frame | At 30 fps, % of one core |
|---|---|---|---|
| 640×480 | 1.44 ms (78 KB) | 0.43 ms (4 KB) | 1.3–4.3 % |
| 1280×720 | 4.41 ms (237 KB) | 1.27 ms (11 KB) | 3.8–13 % |
| 1920×1080 | 7.79 ms (546 KB) | 2.85 ms (25 KB) | 8.5–23 % |

QTBUG-126695 (open), measured on a Xeon with a GTX 1060, whole app: MJPEG 1080p30 = 60 %, MJPEG 640×360@30 = 18 %, YUYV 640×360@30 = 2 % of a core.

**Conclusion:** for bubbles up to about 480 physical px, use YUYV 640×480@30. Switch to MJPEG 720p or 1080p only for the full-screen camera toggle.
- Changing `cameraFormat` while active stops the stream, closes the device, reopens it and restarts [source: `:126-141`]. With `KeepLastFrame`, expect a short freeze [inferred].
- This contradicts the "hardware decode of MJPEG" assumption in `03-performance.md`.

**Latency [inferred]:** the camera's own pipeline dominates (exposure, ISP and USB, typically 1–3 camera frames). Qt adds the GUI-thread dequeue, at most one display frame until the render thread picks the frame up, the JPEG decode if any, and the compositor. In recordings the face therefore lags the microphone by roughly 50–150 ms. CleanShot's standard (non-Studio) mode has the same property.

### 1.3 Drawing the bubble

- **Mirror:** `VideoOutput.mirrored: true`, available since 6.9 [doc: VideoOutput 6.11 page; source `qquickvideooutput.cpp:305-330, 597`]. It is applied in the node's texture coordinates after `orientation`, so it costs nothing extra. `transform: Scale { xScale: -1; origin.x: width/2 }` also works.
- **Crop:** `fillMode: VideoOutput.PreserveAspectCrop` crops through texture coordinates, so no extra pass [source `:514-540`].
- **Mask options:**
  1. **`MultiEffect` with `maskEnabled`**: the same pattern as `app/qml/Pin.qml`, `History.qml` and `Magnifier.qml`. Hide the source, put a white rounded `Rectangle` in an Item with `layer.enabled`, and set `maskThresholdMin: 0.5; maskSpreadAtMin: 1.0` for a soft edge. Recommended: it is house style and adds no shader build step.
  2. `QtQuick.Shapes` `ShapePath.fillItem` (since 6.8; present here). Gives an analytically antialiased circle with `CurveRenderer`, but the `VideoOutput` must be a texture provider (`layer.enabled`). The cost is the same as option 1.
  3. Qt5Compat `OpacityMask` (installed): legacy.
  4. A custom `ShaderEffect` with an SDF circle: one pass, but needs a `qsb` build step.
  All of these add one offscreen pass per camera frame at bubble size, which is negligible.
- `endOfStreamPolicy: VideoOutput.KeepLastFrame` (6.9+) and `clearOutput()` exist [doc]. Clear the content **before** hiding the bubble's window (Hyprland fades unmapped layers; see CLAUDE.md).

### 1.4 Where the bubble lives (decision belongs to you; architecture §6.3 says pinned floating toplevel)

| | Pinned floating toplevel (current plan) | Layer surface (overlay layer, bubble-sized, `keyboard_interactivity none`) |
|---|---|---|
| Drag | `startSystemMove()`: Hyprland moves it; snap afterwards via hyprctl | Update margins from QML; the QAO and pin lock mode already position layers this way [inferred: smoothness during drag untested] |
| Resize, size presets, full-screen toggle | Needs Hyprland (CLAUDE.md: "A floating window can't resize itself") | The client sets its own size |
| Always on top | Other floating or fullscreen windows can cover it [inferred] | The overlay layer sits above every window |
| Focus | Needs a `no_focus` rule | No keyboard focus by construction |
| Multi-monitor | The compositor moves it across outputs | A layer surface belongs to one output; recreate it on another (gsr records one monitor anyway) |

Either way the bubble is composited, so gsr KMS records it. gsr's own camera composite (`-w "screen|/dev/video0;hflip=…"`) is a fixed rectangle chosen at start: no circle, cannot move, and it takes the device exclusively [doc: gsr man page]. So the on-screen bubble is still the right design.

---

## 2. The trimmer player (MediaPlayer + VideoOutput)

### 2.1 Load and metadata [verified]

- Loading takes 6–22 ms.
- `duration` in ms, `seekable`, `hasAudio`, `hasVideo`, `videoTracks` and `audioTracks` are correct.
- `metaData` provides Resolution (a `QSize`; read `.width` and `.height` in QML), VideoFrameRate, Duration, VideoCodec, AudioCodec, the two bitrates and HasHdrContent.
- **VideoFrameRate is the average for VFR files** (50.9 for a nominally 60 fps VFR file). The core's `probe()` also uses `avg_frame_rate`; that is correct for Snip's CFR recordings.
- `pitchCompensationAvailability` is Available and `pitchCompensation` defaults to `true` (6.10+).
- `playbackRate` 0.5, 2 and 4 all work; every frame is delivered (240 fps into the sink at 4×), and the display drops what it cannot show.
- Qt plays **one** audio track at a time (`activeAudioTrack`) [doc]. Recordings with separate mic and system tracks preview track 0 only, unless the proxy carries a pre-mixed audio track [inferred].

### 2.2 How seeking works and how exact it is

- The demuxer calls `av_seek_frame(..., AVSEEK_FLAG_BACKWARD)`. The decoder then drops frames with `end < target` (`qffmpegdemuxer.cpp:272`, `qffmpegstreamdecoder.cpp:130`, `qffmpegrenderer.cpp:91`) [source]. So seeks are **exact, not keyframe-snapped**.
- The playback engine objects are rebuilt on every seek (QTBUG-122057, open).
- **Boundary rule [verified]:** the frame shown is the first one whose end is at or after the position.
  - At 60 fps, `setPosition(1000)` shows frame [983.33, 1000.00).
  - `setPosition(50)` shows [33.3, 50.0).
  - At a keyframe (4000) it shows [4000, 4016.7), because decoding starts at that keyframe.
  - `position` reports the *requested* value, not the frame's time.
- **Latency per paused seek [verified]**, in ms:

| Clip (2 s keyframe interval) | Software | CUDA (Qt default here) | VAAPI (nvidia-vaapi-driver) |
|---|---|---|---|
| 1080p60 x264 `t1080` | 4–28 (audio attached) | 10–118 | – |
| 3440×1440 screen-like `screen3440` | **55–111** | 31–274 | 34–276 |
| 3440×1440 heavy test pattern `gsr3440` | 20–300 | 27–310 | 35–317 |
| **all-intra 1280 proxy** | **2.6 mean / 4.0 max** | 4.0 / 4.4 | – |

  Latency grows linearly with the distance from the previous keyframe (CUDA: about 2 ms per frame at 3440×1440).
- QTBUG-140440 (open): under CPU load a paused seek can deliver a couple of extra frames.
- The black frame after EOS or after a paused seek is solved by `endOfStreamPolicy` (QTBUG-129764).

### 2.3 Frame stepping [verified]

- Assigning a fractional `position` in QML **rounds** (16.5 → 17, 2000.9 → 2001).
- `position += 1000/60` yields 17, 34, 51, … : a drift of +⅓ ms per step that skips one frame about every 50 steps. Combined with the boundary rule, a step can also show the previous frame.
- Fix for CFR files: keep a frame index and seek to `Math.round((n + 0.5) * frameMs)`. Tested: 5 rapid steps landed on frame 5.
- VFR files need real timestamps. QML cannot read them: `VideoSink.videoFrameChanged(frame)` delivers `frame === undefined` in JS [verified].
  - Option: a tiny C++ helper exposing `QVideoFrame::startTime()/endTime()`. Next frame = `floor(end/1000) + 1`; previous = `ceil(start/1000) - 1`. Both were tested and step exactly one frame.
  - Option: an ffprobe packet index from the Rust core.
- Stepping on the original costs one full seek each time (11–310 ms). On the proxy it costs 2.7 ms.

### 2.4 Scrubbing (simulated drag: 60 `setPosition` calls at 8 ms intervals) [verified]

| | Frames shown during a 0.5 s drag | Last frame after release |
|---|---|---|
| Original, a seek per pointer move (what `VideoEditor.qml` does) | **1** (software and CUDA) | +121 ms (software) / +281 ms (CUDA) |
| Original, seek gate (one in flight, keep only the newest) | 7 (14 fps, software) / 5 (10 fps, CUDA) | +207 / +313 ms |
| All-intra proxy, no gate | 60 (117 fps) | +0 ms |

### 2.5 Proxy for scrubbing [verified]

```
nice -n 10 ffmpeg -v error -y -i REC.mp4 -map 0:v:0 -map 0:a? \
  -vf scale=1280:-2:flags=fast_bilinear -fps_mode passthrough -enc_time_base demux \
  -c:v libx264 -preset ultrafast -tune fastdecode -g 1 -crf 28 -pix_fmt yuv420p \
  -video_track_timescale 1000000 -c:a copy PROXY.mp4
```

- Build time: 1.49 s for 20 s of 3440×1440 (about 7.5 % of real time). Size: 7.8 MB.
- Video and audio packet timestamps are **identical** to the original, so the playhead maps 1:1.
- Seeks: 3.0 ms mean with an audio output attached. Playback: 9 % of a core in software (2 % with CUDA).
- Without `-enc_time_base demux`, ffmpeg quantises VFR timestamps to 1/60 s; all three flags (`-fps_mode passthrough`, `-enc_time_base demux`, `-video_track_timescale 1000000`) are needed.
- NVENC builds are not faster (1.8–2.8 s): the job is limited by decoding.
- Export always from the original. Prior CLI measurements were 26–38 ms per seek on an 860 px proxy; in-process Qt is about 10× faster.

### 2.6 Position updates, playhead, mute

- `positionChanged` fires at **20 Hz** (a 50 ms timer, `qffmpegmediaplayer.cpp:41`) [source; measured 19.7–20 Hz].
  - A playhead bound to `position` moves in 20 Hz steps on a 144 Hz display. Extrapolate between updates with a `FrameAnimation`.
  - Skipping cut parts in `onPositionChanged` (as `VideoEditor.qml` does) overshoots into the cut by up to 50 ms, plus the seek latency.
- Mute: `AudioOutput.muted` and `volume` (volume is linear, 0..1) [doc]. Setting `audioOutput: null` stops audio rendering entirely.

### 2.7 Hardware decode on NVIDIA

- **Order:** on Linux Qt prefers CUDA, then VAAPI (VDPAU is only probed) [source `qffmpeghwaccel.cpp:41-50`]. CUDA is used only when `/proc/driver/nvidia/version` exists **and** `libnvcuvid.so` loads (QTBUG-122199).
- **Probe cost [verified]:** if *either* env var is unset, the first QtMultimedia use runs `av_hwdevice_ctx_create` for vdpau, cuda, vaapi, qsv and vulkan on the GUI thread.
  - Measured: 655–746 ms (C++) and 814–1382 ms (`qml6`).
  - Whole `qml6` process with just `MediaDevices` or a `Camera`: 950–1747 ms unset, versus 96–103 ms with both set to empty.
  - Setting only ENCODING moves the probe to the first decode. The docs say `,` means an empty list; an empty string also works [doc: "Advanced FFmpeg Configuration" 6.11].
  - With `cuda`, each newly loaded source also costs about 60 ms of GUI-thread time to create a CUDA context.
- **Texture path [source]:**
  - The only zero-copy converter on Linux is VAAPI (EGL dma-buf import). It is disabled on `EGL_BAD_MATCH` (QTBUG-112312). Its docs note concerns X11 (xcb), not Wayland.
  - **CUDA frames are copied GPU → CPU → GPU** (`av_hwframe_transfer_data` in `QFFmpegVideoBuffer::map`).
  - Qt devs report that VAAPI on NVIDIA "mostly could not be opened" (QTBUG-147731). It worked in the headless test here.
- **Playback CPU** (sink only; "+download" emulates the CUDA texture path) [verified]:

| Clip, 60 fps | Software | CUDA / CUDA+download | VAAPI / VAAPI+download |
|---|---|---|---|
| 1080p | 42 % | 4 % / 8 % | 4 % / 11 % |
| 3440×1440 screen-like | 51 % | – / 10 % | – / 11 % |
| 3440×1440 heavy pattern | 80 % | 5 % / 13 % | 6 % / 18 % |
| 1280 all-intra proxy | 9 % | 2 % | – |

  First frame after `pause()` at 3440×1440: software 60 ms, CUDA 105–127 ms, VAAPI 265–283 ms.
- **Recommendation:** in `main.cpp`, before the first QtMultimedia use, set `QT_FFMPEG_ENCODING_HW_DEVICE_TYPES=,` and `QT_FFMPEG_DECODING_HW_DEVICE_TYPES=,` (software).
  - With the proxy, software decode is cheap, seeks are fastest, there is no CUDA context and no CUDA teardown crash.
  - Use `cuda` only if full-resolution playback CPU matters (laptops); that is an engineering call to revisit.
- **Teardown crash [verified]:** a `QVideoFrame` that outlived the CUDA player caused a SIGSEGV in CUDA cleanup at process exit. Never keep video frames (for example in a cache) past the player's lifetime. The daemon crashing at exit would trigger the desktop crash toast.

### 2.8 Sparse-frame files plus an attached AudioOutput [verified, related to QTBUG-150596]

`sparse_a.mp4` (1 fps for 10 s, then 60 fps, with AAC):
- With a muted `AudioOutput` attached, `pause()` showed **no frame within 5 s**.
- A paused seek to 4000 **never** showed a frame (20 s timeout).
- `play()` from load showed its first frame at 5.9 s, then a burst of the missed frames.
- Without an audio output, everything showed within 4–15 ms.
- Detaching the output while paused and attaching it on `play()` fixed all of it; frames then arrived on time.

gsr does not produce such files: it marks every tick as changed unless `-fm content` is used, and `-fm content` has no effect on Wayland monitor capture [doc: gsr 6.1.3 `src/recorder/recorder.c:524-525, 833-857`]. Snip also passes `-fm cfr` (`crates/snip-core/src/record.rs:86`).

### 2.9 Known bugs that matter (Qt Jira)

| Bug | Status | Matters for |
|---|---|---|
| QTBUG-136979: QCamera stops working after the device is reconnected (Linux) | Open, 6.9/6.10 | Bubble: recreate the Camera |
| QTBUG-135869: `active` not updated on disconnect (Linux FFmpeg too) | Open | Bubble: frame watchdog |
| QTBUG-139682: camera already in use; QCamera still reports active and shows artifacts | Reported, 6.10 | Hide the bubble on error |
| QTBUG-139209: Camera recreated inside a Loader shows blank | Reported | Use `createObject`, destroy first |
| QTBUG-126695: high CPU with JPEG camera formats | Reported | Use YUYV |
| QTBUG-140440: a paused seek can produce several frames | Open, 6.8.3 | Frame stepping |
| QTBUG-150596: first frame gated by a frame count, not by time | Reported, 6.11.2 | §2.8 |
| QTBUG-122057: seek recreates playback objects | Reported | Per-seek fixed cost |
| QTBUG-129764: black screen after EOS | Fixed via `endOfStreamPolicy` (6.9) | Use KeepLastFrame |
| QTBUG-145575: the Slider pattern in Qt's examples makes seeking jumpy | Reported, 6.11.0 | Don't feed `position` back into the slider while it is pressed |
| QTBUG-139848: crash on graphics device loss | Reported, 6.8/6.9 | Suspend or GPU reset |
| QTBUG-140518: Wayland ghosting with alpha surfaces (HEVC 10-bit) | Open, 6.10 | Snip forces alpha buffers; keep the video backdrop opaque [inferred] |
| QTBUG-145825: no video, audio only, on Wayland (AMD) in 6.11.0 | Reported | Watch item |

No Wayland subsurface issues apply: `VideoOutput` renders through the scene graph only [source: no Wayland code anywhere in qtmultimedia 6.11.2].

---

## 3. Thumbnail strip

Measured on 3440×1440, N = 12, 120 px tall, best of 3 [verified]:

| Method | 20 s screen-like | 60 s | Notes |
|---|---|---|---|
| **`-discard nokey`** (keyframes only) | **0.25 s** (0.13 s on the pattern clip) | **0.32 s** | One thumbnail per 2 s keyframe interval |
| `-skip_frame nokey` | 0.64 s | 1.66 s | The decoder still parses every packet |
| N inputs with accurate `-ss`, one process (**the repo's `media::thumbnails`**) | 1.05–1.6 s | 1.1–1.3 s | Exact timestamps; `video.rs` opens the editor only after this ("about a second") |
| Full decode with `fps=N/dur` | 1.3–2.5 s | 3.1–7.2 s | Grows with duration |
| In-process `QMediaPlayer` + `QVideoSink` seeks, then `toImage` | 1.4 s (software) / 4.1 s (CUDA) | 1.4 / 2.3 s | Occupies a decoder and runs on the GUI thread |
| From the all-intra proxy (either method) | 0.12 s | – | Exact frames |

Recommended: open the editor right away, run the keyframe pass, and lay each thumbnail at its timestamp (or let each tile take the nearest keyframe). The file names carry the timestamp in ms:

```
ffmpeg -v error -y -discard nokey -i REC.mp4 -an -vf scale=-2:144 \
  -fps_mode passthrough -enc_time_base 1/1000 -frame_pts 1 -q:v 4 DIR/kf_%d.jpg
```

This produced `kf_0.jpg kf_2021.jpg kf_4021.jpg …` [verified]. Swap in exact proxy frames when the proxy exists. A demux-only keyframe index takes 126 ms:

```
ffprobe -select_streams v -show_entries packet=pts_time,flags -of csv=p=0
```

---

## 4. GIF playback

**`AnimatedImage` [verified]:**
- It uses `QMovie` on the GUI thread. With `cache: true` (the default) every frame is kept as a pixmap [source `qquickanimatedimage.cpp:20-49, 397`].
- Memory, 150-frame GIFs: 800×450 went 169 → 294 MB RSS; 1280×536 went 244 → 482 MB. With `cache: false`, RSS stayed flat at 90–91 MB.
- `QMovie` decode cost: 1.15–1.5 ms per frame without cache, 0.7–0.9 ms with cache. At 15 fps that is about 2 % of a core.
- `sourceSize.width: 400` added CPU (scaling on the CPU) without saving anything when the cache is off.
- Assigning `undefined` to `sourceSize.width` broke playback (frame count 0); the docs talk about resetting `sourceSize` as a whole.
- Frame timing was correct: 181 frames in 12 s.
- Use `cache: false; playing: visible`. For small thumbnails, a static poster plus animation on hover. Related open bugs: QTBUG-107895 (`asynchronous` is ignored), QTBUG-103710 (performance).

**`MediaPlayer` also plays GIFs [verified]:** duration 10000 ms, seekable, VideoFrameRate 16.67 (GIF delays are in 1/100 s). Seeks take 5–84 ms and grow with position, because decoding starts at frame 0. Prefer trimming the MP4 source and re-exporting the GIF.

---

## 5. Hosting a VideoOutput in a layer-shell or toplevel window (threaded loop, NVIDIA)

- **One render thread per window** [doc: Qt Quick Scene Graph]. The MJPEG decode for the bubble runs on the bubble window's render thread and does not stall other windows.
- **Animation timing:** with several Snip windows visible during a recording (pill, bubble, keystroke overlay), Qt falls back to a system timer for animations instead of vsync [doc]. Expect slightly less smooth animations.
- **Frame handoff:** `VideoOutput` gets frames through a direct connection and a mutex, and creates textures in the render phase [source `qsgvideonode_p.cpp:217-231`]. The camera frames themselves come in on the GUI thread.
- **Losing the scene graph:** on `sceneGraphInvalidated` the video textures are dropped [source `qquickvideooutput.cpp:480-485`]. After the window is shown again, a paused player shows nothing until a new frame arrives; reseek after showing [inferred].
- **Layer surfaces:**
  - Clear the content first, then unmap.
  - Never let `visible` go false for one frame (both from CLAUDE.md).
  - The bubble needs a transparent window; `QQuickWindow::setDefaultAlphaBuffer(true)` is already set in `main.cpp`.
- **No dedicated NVIDIA render path for video:** camera frames are CPU uploads; CUDA decode output is downloaded and uploaded (§2.7).
- **Headless testing cannot show rendering:** with `QT_QPA_PLATFORM=offscreen`, `GraphicsInfo.api` is Software [verified], so `VideoOutput` and `MultiEffect` rendering cannot be checked headless. Visual e2e cases must run in the nested Hyprland.
- **Re-entrant status handler [verified]:** calling `pause()` and then setting `position` inside `onMediaStatusChanged` re-entered the handler endlessly (52 calls before my guard stopped it; a RangeError without the guard). Defer with `Qt.callLater` and a one-shot flag.
- **QML API quirks [verified]:**
  - The docs say to check `MediaDevices.defaultVideoInput.isNull()`, but in QML that throws `TypeError: … is not a function`. Use `videoInputs.length`.
  - Device values compare by value with `===` (tested on `audioInput`), but `device.id` is an `ArrayBuffer`, so `id === id` is false.
  - `cameraFormat.pixelFormat` is a bare int: YUYV = 17, NV12 = 18, YUV420P = 13, Jpeg = 29.
- **Device-switch format reset:** `setCameraDevice` emits `cameraDeviceChanged` and *then* resets `cameraFormat` to the default [source `qcamera.cpp:490-494`]. A binding `cameraFormat: pick(cameraDevice)` is therefore overwritten. Set the format imperatively after the device.
- **Mic picker bonus [verified]:** `MediaDevices.audioInputs[i].id` strings are exactly gsr's `-a` names (for example `alsa_input.usb-Sony_INZONE_H5-00.mono-fallback`), so the picker can pass them straight through.

---

## 6. Recommended code

Both components passed `qmllint` and were run offscreen. `CameraBubble.qml` was only loaded with no camera present; its format picker was unit-tested against a C920-like format list (200→320×240 YUYV, 300/480→640×480 YUYV, 600→1280×720 MJPEG, 900/1440→1920×1080 MJPEG). `TrimmerPlayer.qml` was exercised on `t1080.mp4`: rapid steps, a drag and playback.

### 6.1 `CameraBubble.qml` (shape mask, mirror, format choice, hot-plug) [verified to load; camera behaviour unverified]

```qml
pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Effects
import QtMultimedia

// The live webcam bubble (REC-08). Recorded as seen, so it must never show a
// stale or garbled frame: it hides itself when the camera is gone or busy.
Item {
    id: bubble
    property string shape: "circle"         // "circle" | "square" | "rect"
    property bool mirror: true               // a selfie view, like CleanShot's "Flip"
    property var wanted: null                // a cameraDevice the user picked; null = the default
    readonly property real dpr: Screen.devicePixelRatio
    readonly property bool ready: cam !== null && frames > 0
    property var cam: null                   // created on demand: an idle Camera still holds /dev/videoN open
    property int frames: 0

    MediaDevices {
        id: devices
        // /dev changed: a camera came or went (the FFmpeg backend watches /dev)
        onVideoInputsChanged: bubble.reconcile()
    }

    function deviceToUse() {
        const list = devices.videoInputs
        if (list.length === 0) return null
        if (wanted !== null)
            for (const d of list) if (d === wanted) return d          // value types compare by value
        return devices.defaultVideoInput
    }

    // The smallest uncompressed format that still covers the bubble at >= 29 fps.
    // MJPEG (pixelFormat 29) costs a CPU JPEG decode per frame on the render thread;
    // YUYV (17), NV12 (18), YUV420P (13) are uploaded as is and converted by a shader.
    function pickFormat(device, needPx) {
        const JPEG = 29
        const edge = f => Math.min(f.resolution.width, f.resolution.height)
        let biggest = null, best = null
        for (const f of device.videoFormats) {
            if (f.maxFrameRate < 29) continue
            if (!biggest || edge(f) > edge(biggest)) biggest = f
            if (edge(f) < needPx) continue
            const better = !best
                || (f.pixelFormat !== JPEG && best.pixelFormat === JPEG)
                || ((f.pixelFormat === JPEG) === (best.pixelFormat === JPEG) && edge(f) < edge(best))
            if (better) best = f
        }
        return best || biggest               // nothing covers it: the biggest there is
    }
    function applyFormat() {
        if (!cam) return
        const f = pickFormat(cam.cameraDevice, Math.ceil(Math.min(width, height) * dpr))
        if (f) cam.cameraFormat = f
    }

    function start() {
        const d = deviceToUse()
        if (!d || cam) return
        frames = 0
        cam = cameraComponent.createObject(bubble, { cameraDevice: d })
        applyFormat()                        // after the device: setting a device resets the format
        session.camera = cam
        cam.active = true
    }
    // bigger bubble or camera full screen: a format with more pixels (restarts the stream)
    onWidthChanged: if (cam) Qt.callLater(applyFormat)
    function stop() {
        if (!cam) return
        session.camera = null
        cam.active = false
        cam.destroy()                        // closes /dev/videoN (deferred to the event loop)
        cam = null
        output.clearOutput()
    }
    // A reconnected camera never streams again on the same QCamera
    // (QTBUG-136979), so start over with a fresh one, one event loop later.
    function reconcile() {
        const d = deviceToUse()
        if (cam && (d === null || d !== cam.cameraDevice)) stop()
        if (!cam && d !== null && visible) restart.restart()
    }
    Timer { id: restart; interval: 250; onTriggered: bubble.start() }
    onVisibleChanged: visible ? start() : stop()
    Component.onCompleted: if (visible) start()

    Component {
        id: cameraComponent
        Camera {
            onErrorOccurred: (error, errorString) => {
                // "No camera detected", "Camera is in use" (another app streams it), "Cannot open camera"
                console.warn("camera:", errorString)
                bubble.stop()
            }
        }
    }
    CaptureSession { id: session; videoOutput: output }

    // frame watchdog: an unplugged camera stops sending frames but stays "active"
    Connections {
        target: output.videoSink
        function onVideoFrameChanged() { bubble.frames++; watchdog.restart() }
    }
    Timer { id: watchdog; interval: 1500; onTriggered: if (bubble.cam) { bubble.stop(); restart.restart() } }

    VideoOutput {
        id: output
        anchors.fill: parent
        visible: false                       // drawn once, through the mask
        fillMode: VideoOutput.PreserveAspectCrop
        mirrored: bubble.mirror              // Qt 6.9+: no extra pass
        endOfStreamPolicy: VideoOutput.KeepLastFrame
    }
    Item {
        id: mask
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Rectangle {
            anchors.fill: parent
            radius: bubble.shape === "circle" ? width / 2 : Math.min(width, height) * 0.12
            color: "white"
        }
    }
    MultiEffect {
        anchors.fill: parent
        visible: bubble.ready
        source: output
        maskEnabled: true
        maskSource: mask
        maskThresholdMin: 0.5
        maskSpreadAtMin: 1.0                 // soft 1-texel edge instead of jaggies
    }
}
```

Start the camera when the recorder toolbar shows a camera selection, not at record start. The first frame takes a moment, and the countdown hides it.

### 6.2 `TrimmerPlayer.qml` (mid-frame seeks, frame step, seek gate, smooth playhead) [verified offscreen]

```qml
import QtQuick
import QtMultimedia

// Preview for the trimmer (VED-01): exact-ish seeks, frame steps, a smooth
// playhead, and no seek pile-up while the timeline is dragged.
Item {
    id: tp
    property url source                      // the recording, or its all-intra proxy once built
    property real fps: 60                    // Snip records CFR (gsr -fm cfr)
    readonly property real frameMs: 1000 / fps
    readonly property bool playing: player.playing
    property real playheadMs: 0              // smooth: updated every display frame while playing
    property alias muted: audio.muted
    property alias volume: audio.volume

    // ---- transport
    function play() {
        player.audioOutput = audio          // only while playing: paused seeks on sparse (VFR) files
        player.play()                       // never present a frame with an audio output attached
    }
    function pause() {
        player.pause()
        player.audioOutput = null
    }
    // Frame n covers [n*frameMs, (n+1)*frameMs). Seek to its middle: Qt shows the
    // frame whose end >= position, so a position on a boundary shows the frame before.
    function frameAt(ms) { return Math.floor(ms / frameMs) }
    function showFrame(n) {
        n = Math.max(0, Math.min(n, Math.floor((player.duration - 1) / frameMs)))
        seek(Math.round((n + 0.5) * frameMs))
    }
    function step(k) { pause(); showFrame(frameAt(playheadMs) + k) }

    // One seek in flight; newer targets replace the pending one. Qt cancels an
    // in-flight seek on every setPosition, so unthrottled drags show nothing.
    property real pendingMs: -1
    property bool seeking: false
    function seek(ms) {
        playheadMs = ms
        if (seeking) { pendingMs = ms; return }
        seeking = true
        seekGuard.restart()
        player.position = ms
    }
    function seekDone() {
        seeking = false
        if (pendingMs >= 0) { const ms = pendingMs; pendingMs = -1; seek(ms) }
    }
    Timer { id: seekGuard; interval: 600; onTriggered: tp.seekDone() }   // never wedge the gate
    Connections {
        target: video.videoSink
        function onVideoFrameChanged() { if (tp.seeking) { seekGuard.stop(); tp.seekDone() } }
    }

    // position only updates at 20 Hz (a 50 ms timer in the FFmpeg backend):
    // extrapolate between updates so the playhead moves every frame
    property real baseMs: 0
    property real baseAt: 0
    Connections {
        target: player
        function onPositionChanged() { tp.baseMs = player.position; tp.baseAt = Date.now() }
    }
    FrameAnimation {
        running: player.playing
        onTriggered: tp.playheadMs = Math.min(player.duration, tp.baseMs + (Date.now() - tp.baseAt) * player.playbackRate)
    }

    MediaPlayer {
        id: player
        source: tp.source
        videoOutput: video
        // pause() + a seek inside this handler re-enters it forever: do it once, later
        property bool primed: false
        onSourceChanged: primed = false
        onMediaStatusChanged: if (!primed && mediaStatus === MediaPlayer.LoadedMedia) {
            primed = true
            Qt.callLater(() => { player.pause(); tp.showFrame(tp.frameAt(tp.playheadMs)) })   // also keeps the place when swapping to the proxy
        }
    }
    AudioOutput { id: audio }
    VideoOutput {
        id: video
        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectFit
        endOfStreamPolicy: VideoOutput.KeepLastFrame   // Qt 6.9+: no black frame at the end
    }
}
```

The offscreen run gave: 5 rapid steps landed on frame 5 (playhead 92 ms), a 40-seek drag ended at 6000 with the gate idle, and playback advanced the extrapolated playhead.

### 6.3 `app/src/main.cpp`, before `QGuiApplication` [verified effect]

```cpp
// QtMultimedia probes every FFmpeg hw device on the GUI thread at first use
// (0.7–1.4 s here) unless both lists are named. Snip never encodes through Qt.
if (!qEnvironmentVariableIsSet("QT_FFMPEG_ENCODING_HW_DEVICE_TYPES"))
  qputenv("QT_FFMPEG_ENCODING_HW_DEVICE_TYPES", ",");
if (!qEnvironmentVariableIsSet("QT_FFMPEG_DECODING_HW_DEVICE_TYPES"))
  qputenv("QT_FFMPEG_DECODING_HW_DEVICE_TYPES", ",");   // software; "cuda" trades seek speed for playback CPU (§2.7)
```

---

## 7. Pitfalls

1. MJPEG camera formats cost a CPU JPEG decode per frame, and Qt's default format choice is usually MJPEG 1080p. Pick the format yourself.
2. Setting `cameraDevice` resets `cameraFormat` after it emits the change signal, so a format binding does not stick.
3. An inactive `Camera` with a device still holds `/dev/videoN` open. Create it on demand and destroy it after recording.
4. Unplug: frames stop, `active` stays true, no error. Replug: the same `Camera` never resumes. Use a watchdog and recreate the Camera.
5. Another app streaming the camera produces "Camera is in use". The bubble must hide itself, because whatever it shows is recorded.
6. Camera frames are dequeued on the GUI thread with only 2 buffers. GUI-thread stalls during a recording show up as stutter in the recorded bubble.
7. `defaultVideoInput.isNull()` does not exist in QML, despite the docs. `pixelFormat` is a bare int. `device.id` is an `ArrayBuffer`.
8. The first QtMultimedia use blocks the GUI thread for 0.7–1.4 s unless both `QT_FFMPEG_*_HW_DEVICE_TYPES` variables are set.
9. Every seek re-decodes from the previous keyframe (55–310 ms at 3440×1440), and a new seek cancels the one in flight. Unthrottled drags show nothing.
10. A position on a frame boundary shows the previous frame, and `position` reports the requested value, not the frame time.
11. `position += 1000/fps` rounds and drifts, skipping a frame about every 50 steps at 60 fps.
12. `positionChanged` is 20 Hz. Playheads need extrapolation, and skipping cuts from that signal overshoots by up to 50 ms.
13. QML cannot see `QVideoFrame` timestamps (`frame` is `undefined` in JS). Exact VFR stepping needs a C++ helper or an ffprobe index.
14. Paused seeks on sparse-frame files never show a frame while an `AudioOutput` is attached. Attach it only while playing.
15. With CUDA decode there is no zero-copy texture path (download plus upload), each source load costs about 60 ms of GUI-thread time, and a `QVideoFrame` that outlives the player can crash the process at exit.
16. The VAAPI zero-copy path on NVIDIA is unreliable according to Qt devs. `QT_DISABLE_HW_TEXTURES_CONVERSION=1` is the escape hatch.
17. Calling `pause()` and setting `position` inside `onMediaStatusChanged` recurses forever.
18. `AnimatedImage` caches every frame by default (hundreds of MB). Set `cache: false`, and never assign `undefined` to `sourceSize.width`.
19. Qt plays one audio track at a time. Mic and system audio on separate tracks are not mixed in the preview.
20. `VideoFrameRate` metadata is the average fps; frame math only holds for CFR files.
21. Headless (offscreen) tests use the software scene graph, so `VideoOutput` and `MultiEffect` rendering can only be checked in the nested Hyprland.
22. With alpha windows on Wayland there is an open ghosting bug (QTBUG-140518). Keep the video backdrop opaque.

---

## 8. Where this meets the current repo (read only; nothing changed)

- `app/src/main.cpp`: neither `QT_FFMPEG_*` variable is set, so the GUI thread blocks on first use (§2.7).
- `app/qml/VideoEditor.qml`:
  - `step()` uses `position + frames * frameMs`: rounding drift and the boundary rule (§2.3).
  - The timeline `onPositionChanged` seeks on every mouse move, so the preview is frozen during drags (§2.4).
  - The playhead `x` is bound to `player.position`, so it moves at 20 Hz.
  - Cuts are skipped from `onPositionChanged` (overshoot up to 50 ms).
  - The `AudioOutput` is always attached.
- `app/qml/Preview.qml`: `AnimatedImage` uses the default `cache: true` (§4). Its `MediaPlayer` has an unmuted `AudioOutput`.
- `crates/snip-core/src/media.rs` `thumbnails()` (N accurate `-ss` inputs, about 1–1.6 s at 3440×1440) runs before the editor window opens (`crates/snip-bridge/src/engine/video.rs:21-45`). The 300 ms "Trimmer open" budget needs the keyframe pass or opening the editor first.
- `docs/03-performance.md` webcam row ("hardware decode of MJPEG/YUYV, ≤ 3 % CPU") only holds for YUYV at 640×480 or smaller.
- `docs/02-architecture.md` §6.3 "PipeWire camera where available": QtMultimedia 6.11 has no such path.
  - Sharing the camera through PipeWire would need our own libpipewire consumer pushing frames into `VideoOutput.videoSink`. That is not built; it is an option [inferred].
  - Also: V4L2 allows several processes to *open* a camera; only *streaming* is exclusive.

## 9. Open questions for the product owner (candidates for D-numbers)

1. Should the recording show the bubble mirrored (selfie view), and should Flip default to on?
2. Should the bubble window be a pinned toplevel (current plan) or an overlay layer surface (§1.4)?
3. Is a background proxy acceptable? It costs about 7.5 % of real time in CPU after each recording and about 3 Mbit/s of cache disk, kept for the history retention period. Should the proxy pre-mix mic and system audio for preview?
4. If the camera is unplugged or taken by another app mid-recording, should the bubble hide (recommended, since it is recorded) or freeze on the last frame?
5. Full-screen camera toggle: switch to a higher camera format (a short freeze) or always capture at 720p MJPEG (about 4–13 % of a core)?

## Sources

**Qt docs (6.11.2):**
- https://doc.qt.io/qt-6.11/qml-qtmultimedia-videooutput.html
- https://doc.qt.io/qt-6.11/qml-qtmultimedia-camera.html
- https://doc.qt.io/qt-6.11/qml-qtmultimedia-mediadevices.html
- https://doc.qt.io/qt-6.11/qml-qtmultimedia-mediaplayer.html
- https://doc.qt.io/qt-6.11/qml-qtmultimedia-capturesession.html
- https://doc.qt.io/qt-6.11/qvideosink.html
- https://doc.qt.io/qt-6.11/qvideoframe.html
- https://doc.qt.io/qt-6.11/advanced-ffmpeg-configuration.html
- https://doc.qt.io/qt-6.11/qtmultimedia-linux.html
- https://doc.qt.io/qt-6.11/qml-qtquick-animatedimage.html
- https://doc.qt.io/qt-6.11/qtquick-visualcanvas-scenegraph.html
- https://doc.qt.io/qt-6/qml-qtquick-effects-multieffect.html
- https://doc.qt.io/qt-6/qml-qtquick-shapes-shapepath.html

**Qt Jira** (`https://qt-project.atlassian.net/browse/<KEY>`): QTBUG-136979, -135869, -139682, -139209, -126695, -137118, -140440, -150596, -122057, -129764, -145575, -139848, -140518, -145825, -147731, -112312, -130591, -107895, -103710, -122199.

**Qt source:** https://github.com/qt/qtmultimedia/tree/v6.11.2 (files under `src/plugins/multimedia/ffmpeg/`, `src/multimedia/`, `src/multimediaquick/`) and https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquickanimatedimage.cpp. Local copies are in `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/src/`.

**gsr:** gpu-screen-recorder 6.1.3 source `src/recorder/recorder.c` (a copy another agent placed at `…/scratchpad/research/gsr-src/`); man page at `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/gsr-man.txt`.

**Local harnesses and results:**
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/player/playerprobe.cpp`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/cam/jpegbench.cpp`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/cam/gifbench.cpp`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/qml/` (devices, cam2, player, posround, recur, idcmp, gif probes)
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/snippets/CameraBubble.qml`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/snippets/TrimmerPlayer.qml`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/snippets/picktest.qml`
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/snippets/trimtest.qml` (now muted)
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/media/` (test clips, proxies, GIFs)
- `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/jira.py` and `jira_issue.py` (anonymous Jira Cloud `rest/api/3/search/jql`)

**Commands:** `pacman -Q`, `ldd` and `strings` on the FFmpeg media plugin; `gpu-screen-recorder --list-audio-devices`; `coredumpctl list`.