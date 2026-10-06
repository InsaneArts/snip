# Performance budgets and measurement

Status: **DRAFT**, 2026-10-06.

Every number here is a **budget we test against**, not a hope. The "Evidence" column says where the number comes from:
- **M**: measured on the reference machine during research.
- **P**: published by a cited source.
- **E**: estimate, to be confirmed by a spike.

**Reference machine:** i5-13600KF, RTX 4080 (nvidia-open 610), 32 GB, Hyprland 0.56.2, DP-2 3440×1440@144 at scale 1.25.
**Second reference to add:** an Intel Xe or AMD 780M laptop on battery at 1.5× or 2× scale. Every budget must also hold there, or carry its own laptop figure.

## 1. Latency budgets (p95, warm daemon, unless marked cold)

| Interaction | Budget | Floor / evidence | How it is achieved |
|---|---|---|---|
| Hotkey to frozen overlay presented | **≤ 35 ms** (cold ≤ 300 ms) | One commit (≤ 7 ms at 144 Hz, 16.7 at 60) plus compositor readback 4–12 ms plus upload 2–5 ms (E). Cold: exec 35–50 ms (M) plus EGL 19–25 ms (M) plus GL context 6–7 ms (M) plus QML | global-shortcuts protocol (no spawn); preallocated memfd buffers; fresh capture session; content attached on the first commit; no layer animation |
| Pointer motion to updated selection chrome | ≤ 1 frame (6.9 ms at 144 Hz) | Qt Quick: one full-screen textured quad plus a few nodes, about 0.2–1 ms per frame on a GPU (E) | only the chrome changes; no CPU raster of the full frame (Omasnap needed 60 Hz caps and damage tracking on a CPU path) |
| Release to overlay gone and QAO card visible | ≤ 2 frames | — | unmap first; thumbnail made by a GPU or box downscale (1–4 ms, M) |
| Release to capture persisted in history (fast PNG, thumbnail, DB row) | **≤ 30 ms** at 3440×1440, ≤ 40 ms at 4K | crop < 5 ms (E) plus fdeflate PNG 6.5–8.6 ms (3440×1440) or 11–16 ms (4K) (M) plus a tmpfs/SSD write | encode on a worker at once |
| Copy action to `image/png` on the clipboard | **≤ 5 ms** (bytes warm), ≤ 40 ms (cold, from history) | the bytes were already encoded at capture | the warm tier keeps the fast-PNG bytes |
| Save action to file in the export folder and clipboard | ≤ 60 ms | write the encoded bytes plus temp/fsync/rename plus clipboard offer | a background re-encode (libdeflate L6) replaces the file later, atomically |
| OCR, 1000×600 region | ≤ 300 ms | 0.10–0.26 s (M, tessdata_fast) | warm `TessBaseAPI`; `OMP_THREAD_LIMIT=1`; tile parallelism |
| OCR, full 3440×1440 | ≤ 500 ms | 0.31 s with 6 tiles (M) | — |
| QR scan (always on, after capture) | ≤ 60 ms, off the UI thread | 35–50 ms full frame (M, zxing-cpp) | — |
| Editor open (warm) | ≤ 120 ms to first interactive frame | E | the window component is pre-compiled (QML AOT / qmlcachegen); the texture is shared with the capture |
| Editor stroke latency | input to photon ≤ 1 frame + vblank | E | scene graph; no full-canvas repaint |
| Editor export (3440×1440 with 30 objects) | ≤ 150 ms | offscreen render (E) plus fast PNG (M) | — |
| Background tool live preview | 144 fps while dragging sliders | GPU SDF + erf shadow < 1 ms (E) | one shader; no CPU blur in the preview |
| Recording start (hotkey to first frame encoded) | ≤ 400 ms, countdown excluded | gsr start-up plus CUDA init 150–250 ms (P/E) | gsr pre-flight check off the hot path |
| Recording stop to QAO card | ≤ 1.5 s for a 1-minute recording | IPC `stop` plus finalise; remux trim 46 ms (M) | thumbnail from a keyframe |
| Trimmer open, 20 s 1440p60 | ≤ 300 ms to an interactive scrub bar | keyframe thumbnail strip 140 ms (M) | in-process decoder; strip refined later |
| Trimmer seek | ≤ 50 ms | all-intra proxy 26–38 ms per seek (M); accurate CPU seek with a 2 s GOP: median 190 ms (M) | proxy built in the background after recording |
| Trim export (stream copy) | ≤ 200 ms | 46 ms (M) | MP4 edit lists; smart-cut only with matching encoder settings |
| GIF export, 10 s 720p | ≤ 2 s | 0.8–1.4 s (M, ffmpeg diff/bayer) | `fps=15` first (gsr records VFR) |
| Scroll stitch, per frame at 1500×1200 | ≤ 5 ms desktop, ≤ 12 ms laptop | 2–3 ms (M); laptop 5–10 ms (E) | row signatures plus SAD refine; capture readback dominates |
| History strip open (200 items) | ≤ 100 ms | E | thumbnails from disk, cached; lazy delegates |

## 2. Throughput budgets: recording

| Scenario | Budget | Evidence |
|---|---|---|
| 3440×1440@60, NVENC (gsr KMS), 10 minutes | **0 dropped frames**, gsr CPU ≤ 5 % of one core, our daemon ≤ 2 % | gsr zero-copy KMS path (P). Spike S7 measures it. |
| 3440×1440@144 native | best effort; warn if the encoder falls behind | NVENC Ada h264 p4: 182 fps at 3440×1440 (M) |
| 4K60 on an Intel/AMD iGPU (VAAPI) | 0 drops at 1440p60; warn above that | E; needs the laptop reference |
| Keystroke/click overlay while recording | ≤ 0.5 ms per event on the compositor thread (Lua hook) | E; spike S9 |
| Webcam overlay | 30 fps, ≤ 3 % CPU | QtMultimedia with hardware decode of MJPEG/YUYV (E) |

## 3. Resource budgets

| Resource | Budget | Notes |
|---|---|---|
| Daemon RSS, idle (no windows) | **≤ 40 MB** | No GL context while idle. The NVIDIA driver maps about 65 MB per context (M), so we keep only `EGLDisplay` warm, or nothing in "low memory" mode. |
| Daemon RSS, overlay up, 1 × 3440×1440 | ≤ 160 MB | frame 19.8 MB plus texture plus driver |
| Daemon RSS, 3 × 4K overlays | ≤ 320 MB transient | 95 MiB raw (M) plus textures |
| Pixel cache budget | 384 MiB default (configurable) | hot/warm/cold tiers (architecture §6.11) |
| Idle CPU | 0 %; **no periodic wake-ups** | event-driven only: no polling of hyprctl or cursor position; ONNX Runtime `allow_spinning=0` |
| Idle disk I/O | none | history sweep once a day |
| OCR model memory | load on use, unload after 60 s idle | Tesseract 50–150 MB, ORT 100–200 MB (E) |
| File descriptors | ≤ 64 idle; the watchdog restarts the daemon when idle if they grow | guard against NVIDIA sync_file leaks (Quickshell #723) |
| Wayland round-trips on the hot path | ≤ 2 | configure plus the first commit |

## 4. Things we will not do on the hot path
- Spawn a process (grim, slurp, wl-copy, hyprctl, bash).
- Call `hyprctl`. Window geometry comes from a cache kept current by socket2 events.
- Encode PNG at zlib level 6 or higher, or encode in Qt's encoder.
- Allocate frame buffers. They are preallocated per output and remade only on hotplug.
- Touch disk before the clipboard offer. History and save are asynchronous.
- Reuse a capture session within 500 ms (it can hang on a static screen).
- Run anything that can block the GUI thread. All I/O and compute go to workers.

## 5. How we measure

1. **End-to-end latency with presentation feedback.**
   - `wp_presentation` feedback on the overlay's first commit gives a CLOCK_MONOTONIC presented time.
   - The hotkey event carries a timestamp (hyprland-global-shortcuts `pressed` has `tv_sec`/`tv_nsec`).
   - The difference is the true hotkey-to-photon latency, minus scan-out.
2. **Tracing:** built-in spans (capture issued, frame ready, texture uploaded, first commit, presented, encode done, clipboard offered) with the `tracy` profiler or Perfetto in dev builds. `snip debug timings` prints the last 50 captures.
3. **Benchmark harness** (`bench/`):
   - fixed images (UI, mixed, photo) at 1080p, 1440p ultrawide, 4K and 5K, for encode, OCR, stitch and redaction;
   - a recorded scroll corpus for stitching;
   - a 20 s recording corpus for the trimmer.
   
   Results go to JSON, and CI compares them with the last release on the same runner (tolerance ±10 %).
4. **Soak test (spike S5, then nightly):**
   - 72 h on NVIDIA with a capture every 30 s, lock/unlock every 15 min, suspend/resume every 2 h, and DPMS cycles.
   - Track RSS, fd count, GL context errors and crash count.
   - Pass: RSS drift ≤ 10 MB, fd drift 0, no crashes.
5. **Recording drop check:** gsr stats plus ffprobe frame timestamps. Gaps over 1.5 × the frame interval count as drops.
6. **Pixel-exactness test:**
   - capture, show the overlay, capture again, diff == 0;
   - at scales 1, 1.25, 1.5, 1.6̅ and 2, and with transforms 0, 1 and 4;
   - on a nested or headless Hyprland in CI, and on real hardware before each release.
