# Architecture and tech stack

Status: **DRAFT; stack decided 2026-10-06**: Qt 6 Quick UI, Rust core, GPL-3.0-or-later. §9 lists what is decided and what is open. §10 is the Omasnap reuse plan, §11 the code layout.

Sources:
- [research/tech-stack-and-prior-art.md](research/tech-stack-and-prior-art.md): toolkit comparison, measured startup costs, and lessons from Omasnap, Tensaku, Satty, Flameshot, Spectacle and Kooha.
- [research/platform-feasibility-hyprland.md](research/platform-feasibility-hyprland.md): protocols, Hyprland 0.56 source behaviour, recording pipelines, NVIDIA.
- [research/processing-pipelines.md](research/processing-pipelines.md): OCR, stitching, encoding, GIF, video.

---

## 1. Recommendation in one page

| Layer | Choice | Main reason |
|---|---|---|
| Process model | One **resident daemon** (`snipd`, systemd user service) plus a thin CLI. Heavy jobs run in child processes: gpu-screen-recorder and ffmpeg. | A cold start alone costs 150–400 ms (exec plus EGL plus toolkit init, measured on this machine). A warm daemon can show a frozen overlay in about 15–35 ms. |
| Language | **Decided: Rust core library plus C++/QML UI**, in one process, joined by **cxx-qt** and built with CMake plus **Corrosion** (§11) | Qt Quick custom items and scene-graph nodes must be C++. The compute and parsing paths get memory safety and the fastest options measured: `png`+fdeflate, fast_image_resize, `toml_edit` (keeps comments), wayland-rs, rayon. Spike S13 checks the build and integration early. |
| UI toolkit | **Qt 6.11 Qt Quick/QML** on the OpenGL RHI backend, **LayerShellQt** for overlays, xdg toplevels for windows | GPU scene graph at 144 Hz, the best animation system available, fcitx5 IME, QtMultimedia for the trimmer, and the same QML idiom as omarchy-shell, so theming matches pixel for pixel |
| Wayland | **Two connections** (§3). **Connection A**: wayland-rs in the Rust core on its own thread, carrying capture, clipboard, shortcuts, virtual pointer and toplevels. **Connection B**: Qt's own, carrying surfaces and DnD. A few raw calls use Qt's `wl_surface` through `QNativeInterface`. Only public Qt APIs. | Hotkeys and clipboard ownership never depend on UI lifetime. No QtWaylandClient private headers, so Arch Qt minor updates do not break AUR users. |
| Capture | Our own **ext-image-copy-capture-v1** client (shm by default, dmabuf behind a flag), fallback **wlr-screencopy**. Clean window capture through ext toplevel sources. | Capture happens in-process before any overlay maps. No grim, no hyprpicker race. |
| Hotkeys | **hyprland-global-shortcuts-v1**, bound by the user as `hl.dsp.global("snip:…")` | No process spawn, under 1 ms. Press and release events allow hold-to-record. |
| Clipboard | **ext-data-control-v1** owned by the daemon; the selection is offered at once and the PNG is encoded lazily | The clipboard works without focus and persists. Paste blocks only until the encode is done. |
| Recording (Phase 1) | **gpu-screen-recorder** child process driven over `-ipc` (KMS backend; NVENC on NVIDIA, VAAPI on Mesa) | The only proven NVENC path on Hyprland. NVIDIA has no VA-API encode. |
| Recording (Phase 2) | Our own ext-image-copy-capture dmabuf pipeline to FFmpeg (NVENC/VAAPI/Vulkan), with event logs | Needed only if gsr limits privacy-safe mode or Studio Mode. Studio Mode can start on gsr (§6.4). |
| Image I/O | **Hot path:** fdeflate-based PNG (Rust `png` `Compression::Fast`; in C++ use fpnge) **encoded at once on a worker**. **Background:** re-encode with libdeflate L6 + Paeth and replace the file atomically. JPEG through libjpeg-turbo 4:4:4, WebP z6, optional JPEG XL. | Measured at 3440×1440: fdeflate 6.5–8.6 ms, libdeflate L1 19–24 ms, libpng default 110–276 ms, Qt about 3 s at 6K (Omasnap #161) |
| OCR | **libtesseract in-process**, `tessdata_fast` models, `OMP_THREAD_LIMIT=1`, parallelised by tiles (about 6), upscaled by measured glyph height, dark regions inverted per region, layout and indentation rebuilt from word boxes. Optional "high accuracy" engine: PP-OCRv5 through ONNX Runtime, **with Georgian always sent to Tesseract `kat`**. | Measured: 0.10–0.26 s for 1000×600, 0.31 s for full 3440×1440 with 6 tiles. Indentation rebuilt from boxes cut the error rate on code from 14.2 % to 2.9 %. Paddle has **no Georgian model** (its `ka_*` models are Kannada). |
| QR | zxing-cpp, 2D formats only by default; results copied as *sensitive* | 7/7 codes in 47 ms on a full frame vs zbar 4/7. Omarchy disables 1D codes because they false-positive on dense screens. |
| GIF / video post | **ffmpeg CLI** (`-progress pipe:1`): GIF via `palettegen=stats_mode=diff` + `paletteuse=bayer:5:diff_mode=rectangle`; stream-copy trims using MP4 edit lists; NVENC "smart cut"; `gifski` CLI as an optional "HQ" choice | Process isolation also keeps GPL/AGPL code (libav*, gifski) out of our binary. A 10 s 720p GIF takes about 1 s and 7–11 MB; the same clip as MP4 is 0.3 MB, so **MP4 is offered first**. |
| Persistence | TOML config (live reload), SQLite (WAL) history index, a zip project format | |
| Omarchy integration | Lua module for binds and rules; theme tokens read from Omarchy's theme files; DND via `omarchy-shell`; an **optional QML-only bar-widget plugin** | Tight integration without sharing a crash domain with the desktop shell |

**Rejected:**
- Running inside omarchy-shell: it shares a crash domain with the bar, notifications, polkit and the lock screen, and has open screencopy crash bugs in Quickshell 0.3.1.
- Tauri, Electron and Flutter: integer scaling only on Linux and no layer shell.
- Pure Rust with iced, libcosmic, Slint or egui: there is no mature layer-shell, drag-and-drop or IME combination on Hyprland 0.56.
- Zig.

**Fallback stack (if the Qt spikes fail):** Rust + gtk4-rs + gtk4-layer-shell, with the same daemon architecture and GSK forced to GL. The Rust crates (§11) carry over unchanged; only `app/` and `snip-bridge` would be rewritten.

---

## 2. Constraints from research that shape the design

These are platform facts, not preferences. Each traces back to a research doc.

1. **Capture before showing anything.** Grab every output into memory first, then map overlays that display those buffers, and crop from memory. Re-capturing a frozen overlay causes races and duplicated frames (grimblast #148/#155, hyprpicker #101/#119).
2. **Hyprland cannot make our surfaces invisible in captures.** `no_screen_share` paints black boxes, and gsr KMS capture ignores it completely. So anything we must not record (controls, dimming) stays **outside the encoded rectangle**.
3. **An exclusive-keyboard layer surface takes all pointer input**, even through an empty input region (Hyprland InputManager "forced above all"). Consequences:
   - On multi-monitor, every per-output overlay must be exclusive.
   - During scrolling capture the overlay must drop exclusive focus over the live page. Omasnap's `setKeyboardGrab` dance is the reference.
   - The QAO and pins must **never** be exclusive.
4. **Drag-and-drop out of exclusive layer surfaces is broken in 0.56.2** (fixed by Hyprland #15780, expected in 0.57). The QAO uses keyboard interactivity `none`/`on_demand`. Pins are xdg toplevels.
5. **Hyprland always reports full damage**, so damage can't drive change detection. A reused capture session within 500 ms can hang on a static screen, so **use a fresh session for each screenshot**.
6. **NVIDIA has no VA-API encode.** Use NVENC through gsr (CUDA) or FFmpeg (`h264_nvenc`, `*_vulkan`).
7. **dmabuf capture is fragile on NVIDIA:** modifier import failures, gray screens, EGL_BAD_MATCH. **shm is the default** for stills; dmabuf only for continuous capture after per-driver probing.
8. **Fractional scale:**
   - Selections are held in **physical pixels**. Logical-integer APIs (gsr regions, screencopy regions) cannot express every physical rectangle at a scale of 5/3.
   - Overlays render with wp-fractional-scale plus viewporter and sample 1:1.
   - Qt must not be left to the `gtk3` platform theme. Override `QT_QPA_PLATFORMTHEME`: it costs about 90 ms at start-up and about 23 MB.
9. **Chromium and Electron accept file URIs from drags, not `image/png`.** For clipboard paste, `image/png` is the safe default.
10. **Omarchy owns the PRINT keys today** and is moving to Omasnap on the next release branch (`quattro`). We integrate by replacing binds through an opt-in Lua module, never by editing Omarchy's files.

---

## 3. Process model

```
                ┌──────────────────────────── Hyprland 0.56 ────────────────────────────┐
  key press ──▶ │ hl.dsp.global("snip:area") ─▶ hyprland_global_shortcuts_v1            │
                │ ext-image-copy-capture · ext-data-control · wlr-virtual-pointer       │
                │ toplevel list + mapping │ layer-shell · xdg · fractional-scale · DnD  │
                └────────▲────────────────────────────▲──────────────────────▲───────────┘
          wl connection A│ (Rust core, own thread)    │ wl connection B (Qt) │ socket2 / eval
┌────────────────────────┴────────────────────────────┴──────────────────────┴───────────┐
│ snipd   (one process · systemd --user · Restart=on-failure)                            │
│                                                                                        │
│  RUST CORE  (crates/*)                         │  QT UI  (app/: C++ + QML)              │
│  ├─ snip-wayland: capture (ext-icc/wlr),       │  ├─ SelectionOverlay ×N (LayerShellQt) │
│  │   clipboard (ext-data-control), global      │  ├─ QuickAccess stack (layer surface)  │
│  │   shortcuts, virtual pointer, toplevels,    │  ├─ Editor · Pins · History · Settings │
│  │   cursor session                            │  │   · Trimmer · Recorder HUD          │
│  ├─ snip-hypr: socket2 events, window cache,   │  ├─ C++ items: FrozenFrame, Canvas     │
│  │   eval (rules, input hooks), version probe  │  │   nodes, ShaderEffects, DnD (KF6)   │
│  ├─ snip-core: geometry, documents/op-log,     │  └─ Render thread (Qt Quick RHI / GL)  │
│  │   encode, resize, stitch, OCR, QR, history, │                                        │
│  │   config (toml_edit), naming, theme tokens  │   ▲ QObjects/models from Rust          │
│  ├─ snip-recorder: gsr + ffmpeg jobs           │   │ (cxx-qt): CaptureController,       │
│  └─ snip-ipc: ctl.sock server                  │   │ QuickAccessModel, HistoryModel,    │
│                                                │   │ Settings, Theme, Recorder          │
│  Threads: wayland (calloop) · workers (rayon)  │◀──┘ frames shared zero-copy (memfd)    │
└──────┬──────────────────────┬────────────────────────────┬─────────────────────────────┘
       │ -ipc socket          │ argv / -progress pipe      │ ctl.sock (JSON lines)
 gpu-screen-recorder     ffmpeg / gifski             `snip` CLI (tiny Rust binary, no Qt)
 (own process group)     (trim, GIF, remux)          omarchy-shell bar widget (QML plugin)
```

**Two Wayland connections, on purpose**
- **Connection A** belongs to the Rust core and runs on its own thread. It carries everything that needs no UI surface: capture, clipboard, global shortcuts, virtual pointer, the toplevel list and mapping, and cursor sessions. It stays alive while no UI exists, so hotkeys and clipboard ownership never depend on Qt.
- **Connection B** is Qt's. It carries surfaces: layer shell, xdg toplevels, fractional scale, cursor shape, DnD. A few raw protocol calls that need Qt's `wl_surface` (xdg-toplevel-tag, hyprland-surface opacity) go through `QNativeInterface::QWaylandApplication`.
- **Frames cross from A to B as memfd pixels** (zero copy into a `QImage`, then a texture upload). If the raw-subsurface optimisation (§4.1) is ever needed, the same memfd can be wrapped in a `wl_shm` pool on connection B at no cost.

**Why one process, not a split engine and UI**
- The editor's export renderer **must** be the same code as its on-screen renderer, for WYSIWYG (Omasnap also guarantees this). So the document rendering lives with the UI anyway.
- Recording, the expensive state to lose, runs in gsr's own process group. It survives a daemon crash, and we re-attach through gsr's `-ipc` socket.
- Rust gives memory safety in the parsing and compute paths without a second process.
- Revisit only if spike S5 (NVIDIA soak) shows Qt/EGL instability we cannot contain (Q-S2). In that case the cxx-qt seam becomes an IPC seam, and the Rust crates don't change.

**The CLI is a separate tiny Rust binary.** A Qt-linked binary pays 35–50 ms just to load and link (measured), and scripts and menus call `snip` often. The CLI links no Qt, talks to `ctl.sock`, and starts in about 1–2 ms.

**Daemon lifetime**
- Started by `exec-once` or a systemd user unit. Bound to `graphical-session.target` (uwsm is in use on Omarchy).
- `snip` (the CLI) socket-activates `snipd` if it is not running. The cold path is about 150–300 ms but works.
- **Idle footprint target: under 40 MB RSS.** The NVIDIA driver alone costs about 65 MB of RSS per GPU context. So **no QQuickWindow and no GL context exist while idle.**
  - `EGLDisplay` stays initialised (about 20 ms, paid once).
  - The context for each overlay is created on demand (about 6–7 ms) or kept warm for N seconds after use (setting: "keep warm" vs "low memory").
- Watchdog: the daemon tracks its own fd count and RSS and restarts itself when idle if they cross a threshold. This guards against NVIDIA/Qt Quick leaks (Quickshell #723) and EGL context loss after lock or DPMS (#1224).

---

## 4. Hot paths

The budgets are in [03-performance.md](03-performance.md).

### 4.1 Area capture (the path that matters most)

| # | Step | Where | Budget |
|---|---|---|---|
| 1 | `pressed` event from hyprland-global-shortcuts | platform thread | < 1 ms |
| 2 | Guards: session locked? overlay already up (cancel or cycle)? recording active? | GUI thread | < 0.1 ms |
| 3 | Start a **fresh** ext-image-copy-capture session for **every** output in parallel, into **preallocated** memfd shm buffers (allocated per output at start-up and on hotplug), cursor off | platform thread | issue < 0.5 ms |
| 4 | Each output's frame arrives after its next commit: ≤ 1 refresh plus Hyprland's `glReadPixels` (about 4–12 ms dGPU). **Per-output timeout of 150 ms**: a monitor that is DPMS-off or stalled gets an unfrozen overlay. | compositor | 7–25 ms |
| 5 | While waiting: create the GL context if cold, and instantiate per-output `SelectionOverlay` windows (not yet mapped) | GUI thread | overlaps step 4 |
| 6 | Upload the frame as a texture (about 20 MB for 3440×1440), then map the layer surface and commit with the content in the **first** commit (never an empty first frame). Layer rule `no_anim`. | render thread | 2–5 ms |
| 7 | Pointer motion redraws only the chrome: dim quads, border, label, loupe. Qt Quick redraws the whole window each frame, but that is one textured quad plus a few nodes, about 0.2–1 ms per frame on a GPU. | render thread | < 2 ms per frame at 144 Hz |
| 8 | Release: crop the **physical** rectangle from the shm buffer (row memcpy), convert XRGB to RGBA, build a `Capture` | worker | < 5 ms |
| 9 | Unmap overlays **at once**; show the QAO card with a GPU-downscaled thumbnail | GUI thread | next frame |
| 10 | **Persist to history** (capture lifecycle, product §5.0): encode a **fast PNG** at once (fdeflate, about 7–16 ms for 3440×1440 to 4K) and write it to `~/.local/share/snip/history/<id>/source.png` (temp, then fsync, then rename), plus `thumb.webp` and the SQLite row. **No clipboard offer at capture by default.** The fast-PNG bytes stay in the warm tier, so a later **Copy** is instant. | worker | about 15–30 ms after release |
| 11 | **Copy** (user action or After Capture rule): set the ext-data-control selection with `image/png` from the already-encoded bytes. We never encode inside a paste request: KDE bug 519651 shows paste timeouts at 1 s. **Save:** write to the export folder with the name template, **then** offer the clipboard (default "Save also copies"), with drags and file-mode copies pointing to the saved file. A background re-encode (libdeflate L6) replaces the history copy, and the saved file if one exists, atomically. | worker | Copy: ≤ 5 ms when the bytes are warm |

Total from hotkey to frozen overlay: **about 15–35 ms warm** (one refresh plus the readback plus one frame). The cold path adds process start and EGL, about 150–300 ms.

**Further optimisation if measurements disappoint:** attach the captured shm `wl_buffer` straight to a raw `wl_subsurface` under the Qt window, with a viewport, so we do no GPU upload at all. This is the latchshot/hyprpicker technique. It needs raw Wayland on Qt's `wl_surface`, so it is kept behind a flag.

### 4.2 Window capture
- **Hover** uses `hyprctl clients -j` geometry, cached and refreshed on socket2 `openwindow`/`closewindow`/`movewindow` events, never polled on the hot path. Geometry is mapped `(at − monitorPos) × scale` and rounded with Hyprland's `CBox::round()` semantics.
- **Capture**:
  - **Window mode (Space, then click; CAP-05)** is the CleanShot window shot.
    - It captures the window's **own buffer** through an ext toplevel source; the ext handle is mapped to the Hyprland address with hyprland-toplevel-mapping.
    - So it is never occluded, and it keeps alpha.
    - It gives no Hyprland border, shadow or rounding. We add the corner mask, shadow, and the "With wallpaper" or "Transparent" background ourselves (§7.3, BG-04).
  - **A click without dragging in area mode** (Omarchy's "smart" habit) **crops from the frozen frame**. This is instant, WYSIWYG, includes overlapping popups, and matches Omasnap's behaviour.
- **Edge cases:**
  - `no_screen_share` windows return a "denied" texture. Detect it and show "This window is private".
  - XWayland override-redirect menus are missing from toplevel capture, so fall back to the frozen crop.

### 4.3 Recording start (Phase 1)
1. The selection overlay in recording mode **snaps the rectangle to what gsr can express**: logical-integer origin and size whose truncated physical size is **even**. Snapped edges are shown live, so WYSIWYG still holds.
2. Pre-roll UI: toolbar (mic, system audio, camera, keys, clicks), countdown on a no-input layer *outside* the region, or centred and removed before frame 0.
3. Start gsr:
   ```
   gpu-screen-recorder -w <WxH+X+Y | monitor> -c mp4 -k auto -f <fps> -fm cfr|vfr
     -a "<sources>" -ac opus|aac -cursor yes|no -ipc <sock>
     -write-first-frame-ts yes -o <tmp>
   ```
   in its own process group. Wait for the first-frame timestamp, then start the timer.
4. Turn on DND (`omarchy-shell notifications setDnd on`, previous state saved). Install the keystroke and click Lua hooks if they are enabled (§6.3).
5. Stop: IPC `stop`, which replies after the file is finalised. Then post-process: trim the warm-up and the audio pop, and remux. Show the QAO card with a thumbnail; open the trimmer if After Capture says so.

### 4.4 Paste and drag compatibility rules
- **Copy (default):** offer `image/png`.
- **"Copy as file":** offer `text/uri-list` plus `x-special/gnome-copied-files`.
- **Optional "do not keep in clipboard history":** add `x-kde-passwordManagerHint` (Omarchy's clipboard manager honours it).
- **Drag:** offer `text/uri-list` (file in a persistent cache dir) plus `image/png` plus `application/vnd.portal.filetransfer` for Flatpak receivers (through KF6CoreAddons `KUrlMimeData` or a direct D-Bus call). Allowed action **COPY only**: Hyprland picks MOVE whenever it is allowed, and Nautilus would then *move* our file.
- **Lifetime:** keep drag files for 24 h or until saved. Keep the data source alive until `dnd_finished`/`cancelled`; handle repeated `send`.

---

## 5. Data model and storage

### 5.1 Core types
```
Capture      { id (ULID), created_at, kind: still|scroll|video|gif,
               source: { monitor{name, scale, transform, logical_geo}, rect_px, window{class,title,address}? },
               pixels: memfd (RGBA8, physical), format_in: XRGB8888|XBGR2101010…,
               encoded: { png? path? }, document: DocumentRef? }
Document     { schema_version, canvas{w,h,bg}, base_image_ref, layers: [Object], background: BgPreset?, crop, history: OpLog }
Object       { id, type: arrow|rect|ellipse|line|text|pencil|highlighter|counter|redact|spotlight|image, z, geom, style, … }
OpLog        append-only [{seq, op, args, inverse}]  → undo/redo, crash recovery, editable history
```

### 5.2 On-disk layout (XDG)
| Path | Contents |
|---|---|
| `~/.config/snip/config.toml` | User settings, the single source of truth, inotify live reload |
| `~/.local/share/snip/history.db` | SQLite (WAL): captures, tags, retention |
| `~/.local/share/snip/history/<ulid>/` | **The default home of every capture** (product §5.0): `source.png` (original physical pixels), `document.json`, `thumb.webp`. Recordings are written here while they record. Retention per HIS-02. |
| `~/.cache/snip/drag/` | Files offered for DnD and "copy as file" (cleaned after 24 h) |
| `$XDG_RUNTIME_DIR/snip/` | `ctl.sock`, gsr IPC socket, **working documents** (op log appended on every edit so a crash loses nothing), recording temp files |
| Export folder | Written **only on Save** (or an auto-save After Capture rule): `$XDG_PICTURES_DIR/Screenshots`, `$XDG_VIDEOS_DIR/Screencasts` (configurable). Save copies out of history; history keeps its copy until retention removes it. |

### 5.3 Project file (`.snip`, name TBD)
- A zip container, store-only for embedded PNGs: `manifest.json` (format version, app version), `source.png`, `document.json`, `preview.png`, `thumb.webp`.
- Forward compatibility: unknown object types are kept on save and drawn as a "newer version" placeholder.
- Video projects (Studio Mode, P2): a directory bundle with `screen.mp4`, `camera.mp4`, `audio-*.opus`, `events.jsonl` (cursor, clicks, keys with monotonic timestamps), and `project.json`.

---

## 6. Subsystems

### 6.1 Selection overlay
- One `LayerShellQt` window per output: layer `overlay`, anchored on all edges, `exclusive_zone=-1`, keyboard exclusive, namespace `snip-select`. The frozen texture is drawn by a custom `QQuickItem` sampling 1:1 (`smooth:false`, origin snapped to device pixels).
- All coordinates are in physical pixels internally. Logical↔physical conversion goes through one tested module. It covers fractional scales N/120, all 8 transforms, and negative global coordinates (floor division).
- Window and monitor hit-testing reuses Omarchy's semantics: the smallest containing rectangle wins; Tab and arrow keys cycle.
- Keyboard is routed centrally (focus lands on one overlay, so the overlay under the pointer forwards).

### 6.2 Quick Access Overlay
- One layer surface per active monitor: layer `overlay`, anchored bottom-left or bottom-right, keyboard `on_demand`, input region equal to the card stack only.
- Cards are QML delegates with spring animations. Swipe gestures come from touchpad axis events. A paused timer while hovered drives auto-close.
- State (the card list) lives in a C++ model, so restoring cards after a UI rebuild is free.

### 6.3 Recording extras
- **Controls pill:** a layer surface (no exclusivity) placed outside the region. On a full-monitor recording it goes on another monitor if there is one; otherwise it is hidden and we rely on the bar widget and hotkeys. Users are told why.
- **Webcam:** our own floating, pinned toplevel (QtMultimedia camera; PipeWire camera where available, otherwise V4L2). Shape mask, mirror, corner snapping, click to go full screen. Hyprland rule through `xdg_toplevel_tag` `snip-webcam`: float, pin, no border, no shadow, no anim, no focus, opacity override. Only one process can open a V4L2 device, so the camera is **ours**, not gsr's.
- **Keystrokes and clicks:**
  - Source: the **Hyprland Lua hook** `hl.on("input.keyboard.key")` plus a `non_consuming` `mouse:272/273` bind. It is installed with `hyprctl eval` at record start and removed at stop. A Lua `hl.timer` watchdog removes it if our heartbeat stops.
  - Transport: socket2 `custom>>` events, or a 0600 file in the runtime dir for privacy (Q-S5).
  - Keycodes become keysyms through xkbcommon, using our own keymap. Output is suppressed while a `no_screen_share` window is focused or the session is locked.
  - Rendering: a no-input overlay layer, so the KMS capture records it.
- **DND and bar:** DND through omarchy-shell IPC (idempotent `setDnd`). Never hide the bar during region recordings, because tiled windows would re-layout.

### 6.4 Studio Mode (P2) without our own capture pipeline
- gsr records with `-cursor no`.
- We log cursor positions from the **ext-image-copy-capture cursor session** (position plus image, cheap, per commit; it reports logical coordinates on Hyprland, so multiply by scale), plus clicks and keys from the Lua hook. Everything is timestamped on CLOCK_MONOTONIC and aligned with gsr's `-write-first-frame-ts`.
- The video editor renders zooms, the smoothed cursor (Xcursor theme), click ripples and keystrokes with Qt Quick offscreen, then encodes with FFmpeg (NVENC/VAAPI).
- That avoids a Phase-2 capture pipeline unless privacy-safe mode needs one.

### 6.5 Scrolling capture
- The region is selected on the frozen overlay. During the capture the overlay **unmaps**, and a no-input HUD plus the live preview are placed outside the region.
- Frames come from an **output** capture session cropped to the region, or from a **toplevel** session when the region lies inside one window. The toplevel source is preferred: it never contains our UI.
- Esc and Enter come through temporary `hl.bind` handles added by eval and removed afterwards.
- Auto-scroll uses `zwlr_virtual_pointer_v1` (`axis_source(wheel)`, `axis_discrete`). It pre-compensates `input:natural_scroll` (Omasnap's `scroll-inject.cpp` is the reference).
- Stitching algorithm: see the processing doc. It runs incrementally on the worker pool, so the live preview updates as you go.

### 6.6 Editor
- An xdg toplevel. Hyprland rule: float, centre, size to the image plus chrome. An optional "always on top" uses pin.
- **Canvas rendering:**
  - Each object is a scene-graph node: Shapes CurveRenderer for vector paths, a text node for text.
  - Effects are `ShaderEffect`s: pixelate with randomisation, secure blur, spotlight, shadows.
  - **Export = an offscreen render of the same scene** (`QQuickRenderControl`) at native pixel size, tiled when it is larger than `GL_MAX_TEXTURE_SIZE` (scroll captures). This gives a single renderer, so preview equals output.
  - Qt Canvas Painter (6.11 tech preview) is assessed in spike S8 as a simpler alternative.
- **Redaction is security-critical.** Pixelating and blurring text can be reversed (Hill et al. 2016: 94–96 % recovery; Depix; Unredacter). So:
  1. The default **Redact** style is an opaque solid fill.
  2. The "Pixelate" and "Blur" styles **build their look from pixels outside the region** (Flameshot 13's approach) or from seeded random tiles. Styles that use the content itself are labelled "not secure".
  3. Redaction is flattened into the source pixels at export. Exports carry no layers, history or private chunks.
  4. Files are written as temp, then fsync, then rename: no aCropalypse-style tail leaks.
  5. After a redaction, the clipboard selection is replaced, because Omarchy's history already stored the unredacted copy. The "mark captures sensitive" setting avoids that.
  6. Project files keep the original, and the UI says so when one is shared.
- Text: QML `TextEdit` (QTextDocument) with fcitx5 IME; preedit and compose tested on Hyprland 0.56 (spike S4).

### 6.7 Pins
- An xdg toplevel per pin, tagged `snip-pin`. Rule: float, pin, border 0, no shadow, no anim, no dim, opacity override, keep aspect, suppress maximize/fullscreen.
- Position is set after mapping through `hl.dsp.window.move` by address; the first frame is transparent to hide the jump.
- Lock mode: empty input region (spike S6). Opacity from scroll; Ctrl+scroll resizes.

### 6.8 History
- SQLite index plus per-capture folders. Retention is enforced by a daily sweep and at start-up.
- The history strip is a layer surface across the top of the focused monitor (CleanShot layout), with keyboard `on_demand`.

### 6.9 Settings, config and IPC
- TOML with a schema-validated config (unknown keys warn instead of failing).
- How the settings UI writes the file depends on the language:
  - **B2:** Rust `toml_edit` edits the user's file **in place and keeps comments and ordering**, so one file can serve both hand edits and the UI.
  - **B1:** toml++ cannot keep comments, so the UI writes `config.toml` and hand-written overrides go in `config.d/*.toml`, merged on load (Q-S6).
- `ctl.sock`: newline-delimited JSON requests and responses, `SO_PEERCRED` checked (same uid), the directory is 0700.
- The CLI (`snip`) is a separate tiny Rust binary with no Qt (§3). Its commands mirror CleanShot's URL API plus `--json` output. Exit codes: 0 ok, 1 cancelled, 2 error, 3 daemon unavailable.

### 6.10 Processing pipelines
Full detail and measurements: [research/processing-pipelines.md](research/processing-pipelines.md).

**Scroll stitching** (1-D alignment, not a panorama stitcher)
- Per-row signatures, then a cost curve, then a ±2 px SAD refinement with a parabolic sub-pixel fit.
- An exact run-hash fast path.
- Sticky-band detection, plus fixed/scrolling/dynamic classification per tile.
- A float accumulator, because a 1.25 scale makes the per-notch offset vary (71/72 px).
- A mutable tail so lazy-loaded images can replace rows that are still on screen.
- Measured about 2–3 ms per frame at 1500×1200. Capture readback dominates.
- Study `snow-stitch-images` (Rust, Apache-2.0) before writing our own.

**OCR**
- A daemon-held `TessBaseAPI` per worker. Init costs about 60 ms, so load lazily and unload after 60 s idle.
- Preprocessing: luminance gray, per-region inversion, upscale chosen from glyph height, 10 px edge padding, no pre-binarisation.
- Run zxing-cpp first and skip OCR if a code covers the region (normcap's approach).

**Beautify and background tool**
- Preview on the GPU: SDF rounded rect, erf analytic shadow (Evan Wallace / Raph Levien) and gradient in one shader.
- Export runs through the same scene (offscreen render), or on the CPU with a cached **9-slice shadow** (about 1 ms to build the corner).
- Blend in sRGB-encoded space to match CSS and design tools.
- **No gradient dithering by default** (it inflated PNGs by 30–160 %).

**HiDPI metadata:** physical pixels by default, with `pHYs` = 72 × scale DPI (90 DPI at 1.25×, 144 at 2×) so web apps such as Gitea and Forgejo display the image at logical size.

**Colour:**
- Convert 10-bit captures with rounding (`(v*255+511)/1023`).
- Tag output sRGB.
- The clipboard always gets tone-mapped SDR PNG.

### 6.11 Memory model
- Capture buffers are **memfd-backed `wl_shm` pools**, preallocated per output.
  - The memfd *is* the raw frame, so there is no copy.
  - Buffers are marked `MADV_DONTDUMP`, so screenshots never land in systemd-coredump storage after a crash.
- Large pixel buffers are allocated with `mmap`/memfd, **never glibc malloc**. Frames under 32 MiB otherwise stay in malloc arenas after the first free, and RSS creeps.
- Tiers under a byte budget (default 384 MiB, configurable):

  | Tier | Contents | Cost to reuse |
  |---|---|---|
  | Hot | Raw frames for captures open in the overlay or editor, plus the last 1–2 | — |
  | Warm | The fast-PNG bytes (they already exist for the clipboard) | Decode about 7 ms |
  | Cold | A file path only | Read from disk |

  Eviction is LRU by bytes.
- **Never park frames in swap.** Omarchy runs zram plus systemd-oomd watching `app.slice`, and oomd kills the cgroup using the most swap first.
  - Run as a systemd user service with `MemoryHigh=`.
  - Compress explicitly instead: lz4/zstd-1 for frozen scroll-canvas tiles, at under 1 ms per tile.
- Tall scroll captures:
  - The canvas is held in 256–1024-row tiles.
  - Export streams rows into the PNG encoder one tile at a time, so peak memory is about one tile.
  - The editor shows a downscaled overview plus full-resolution visible tiles. GPU texture limits are 16384 on Intel/AMD and 32768 on NVIDIA.
- Set explicit decode limits for PNG import: dimension caps and an allocation cap.

---

## 7. Rendering details

### 7.1 Pixel exactness
- Overlays and editor at 100 % zoom: the texture origin is snapped to device pixels, nearest sampling, no mipmaps.
- Spike S2 is an automated **round-trip test**: capture, display, recapture, diff = 0 at scales 1, 1.25, 1.5, 1.6̅ and 2.

### 7.2 Theme
- Read `~/.local/state/omarchy/current/theme/colors.toml`, plus `shell.toml` and the Hyprland `decoration:rounding` and `general:gaps_out` values.
- Port the token logic from omarchy-shell `Commons/Color.qml` and `Style.qml` (MIT): palette roles, alpha companions, control states, monospace font, base size, corner radius.
- Re-read on inotify **and** through a hook in `~/.config/omarchy/hooks/theme-set.d/` for an immediate update.

### 7.3 Window shadow and corner mask
- Corner mask: a superellipse following Hyprland `rounding` and `rounding_power`, scaled to physical pixels.
- Shadow: the blurred alpha (3 box-blur passes ≈ Gaussian, or a GPU blur at export), with defaults close to the macOS look. CPU cost is about 10–20 ms for 2 MP.

---

## 8. Packaging, install and integration

- **AUR:** `snip-screenshot` (release) and `snip-screenshot-git`, with `conflicts=('snip')`. The AUR name `snip` belongs to an abandoned snippet manager. Built with CMake plus Corrosion (cargo).
- **Dependencies:**
  - qt6-base, qt6-declarative, qt6-wayland, qt6-multimedia-ffmpeg
  - layer-shell-qt (not installed on Omarchy by default; small, in extra)
  - kcoreaddons (portal DnD)
  - libdeflate, libjpeg-turbo, libwebp
  - tesseract, tesseract-data-eng, zxing-cpp
  - gpu-screen-recorder, ffmpeg
  - sqlite, tomlplusplus, wayland, xkbcommon
- **Installed files:**
  - `/usr/bin/snip`
  - `/usr/lib/systemd/user/snip.service`
  - `/usr/share/snip/hypr/snip.lua`, a module the user `require`s. It contains the binds (`hl.dsp.global`), layer rules (`no_anim` for `^snip-`) and window rules (by xdg tag).
  - `.desktop` files and MIME handlers
  - sounds
- **Onboarding** (first run) offers to:
  1. add `require("snip")` to `~/.config/hypr/hyprland.lua`
  2. unbind Omarchy's PRINT family
  3. enable the service
  4. install the bar widget
  
  Every step is reversible with `snip uninstall-integration`.
- **Rules are also injected at runtime** with `hyprctl eval` when the daemon starts, and re-injected on `configreloaded`, so the app works even before the user adds the require.
- **Optional omarchy-shell plugin** (separate git repo, QML only): a recording-timer bar widget and capture buttons that talk to `ctl.sock` through Quickshell's `Socket`.

---

## 9. Architecture decisions

### Decided (2026-10-06)

| ID | Decision |
|---|---|
| Q-S0 | **UI toolkit:** Qt 6.11 Qt Quick/QML with LayerShellQt |
| Q-S1 | **Language:** Rust core plus C++/QML UI (cxx-qt and Corrosion). B1 (C++ only) is kept as the fallback if spike S13 fails. |
| Q-S3 | **Omasnap:** its engine and code are the foundation (§10) |
| Q-S4 | **Recording default:** gsr KMS, with private-window warnings; privacy-safe backend later |
| Q-S9 | **Licence:** GPL-3.0-or-later |

### Still open

| ID | Question | Options | Leaning |
|---|---|---|---|
| Q-S2 | Process split | **Single process** / Rust engine process plus Qt UI process | Single process; revisit after spike S5 |
| Q-S5 | Keystroke transport | socket2 custom events / 0600 file | 0600 file in the runtime dir |
| Q-S6 | Config | toml_edit in-place (keeps comments) / plus `config.d` | toml_edit in-place; `config.d` for hand-written overrides |
| Q-S7 | Hyprland versions supported | ≥ 0.54 / **≥ 0.56** / latest only | ≥ 0.56, matching Omarchy 4 |
| Q-S8 | Annotation renderer | **Qt Quick nodes plus ShaderEffect** / Qt Canvas Painter (preview API) | Quick nodes; spike S8 looks at Canvas Painter |

## 10. Building on Omasnap

Omasnap is MIT and C++/Qt6 Widgets: about 60 source files, about 1 MB. Snip is GPL-3.0-or-later, so MIT code can be included **as long as Omasnap's copyright and licence notice is kept** (in `third_party/omasnap/LICENSE` and THIRD_PARTY.md, plus a header comment on ported files). Each module is used in one of three ways:

| Omasnap module | What it does | How Snip uses it | Where it goes |
|---|---|---|---|
| `surface-capture.cpp` | in-process ext-image-copy-capture client | **Port to Rust**: a small protocol client; we need it on connection A | `snip-wayland` |
| `scroll-inject.cpp`, `protocols/wlr-virtual-pointer*` | auto-scroll wheel injection, natural-scroll compensation | **Port to Rust** | `snip-wayland` |
| `stitch.cpp`, `stitch-replay.cpp` | tuned scroll stitcher **plus a replay harness** | **Vendor as C++ first** (built by CMake, called from Rust through `cxx`). **Port later**, using the replay corpus as the oracle that the port is equivalent | `third_party/omasnap` then `snip-core::stitch` |
| `scroll-capture.cpp` | scroll session flow, keyboard-grab dance around Hyprland's exclusive-focus quirk | **Reference.** Reimplement the flow in Rust and QML, keeping its Hyprland workarounds | `snip-core`, `app/qml/Scroll` |
| `capture.cpp`, `output-config.cpp` | monitor and window discovery, capture orchestration, output transforms | **Reference plus port of the logic** (rounding, transforms, smart-select rules) | `snip-hypr`, `snip-core::geometry` |
| `text-band.cpp` | text-line detection (smart highlighter) | **Port to Rust** | `snip-core` |
| `stroke-smoothing.cpp`, `cut.cpp`, `eyedropper.cpp` | pencil smoothing, cut-band tool, colour sampling | **Port to Rust** (pure algorithms) | `snip-core` |
| `png.cpp` | fast PNG | Replaced by the Rust `png`+fdeflate path | — |
| `chrome-theme.cpp`, `palette-config.cpp`, `background-config.cpp` | Omarchy theme and palette reading, backdrop settings | **Port the logic** into the Rust theme-token service (§7.2) | `snip-core::theme` |
| `pin.cpp`, `pin-layout.cpp`, `pin-expiry.cpp`, `pin-file.cpp` | pinned windows, Lua-dispatcher placement, deck layout | **Reference.** Rebuild the UI in QML with CleanShot's pin behaviour; reuse the Hyprland placement and edge-clamp logic | `app/qml/Pins`, `snip-hypr` |
| `editor.cpp` (330 KB), `overlay-chrome.cpp`, `recent-snaps.cpp`, `card-stack.hpp` | QPainter/Widgets editor and chrome | **Reference only.** Snip's editor and QAO are Qt Quick with CleanShot's UX; we borrow behaviours and edge-case handling, not code | `app/qml/*` |
| `docs/editing-model.md` | op-log working document format | **Evaluate adopting it** as the base of our document format, so Snip can open Omasnap working documents | `snip-core::document` |
| `docs/threading.md`, `docs/platform-scope.md` | threading rules, Hyprland quirks | **Required reading** for the team | — |

**Upstreaming:** fixes we make to vendored or ported logic that help Omasnap (for example multi-monitor) are offered upstream as PRs.

## 11. Code layout

```
snip/
├─ Cargo.toml                 (workspace)
├─ CMakeLists.txt             (top level: Qt app + Corrosion for the Rust crates)
├─ crates/
│  ├─ snip-core/              geometry, document/op-log, encode, resize, stitch, OCR, QR,
│  │                          history (rusqlite), config (toml_edit), naming, theme tokens
│  ├─ snip-wayland/           connection A: capture, clipboard, shortcuts, virtual pointer,
│  │                          toplevels, cursor session (wayland-client + calloop)
│  ├─ snip-hypr/              Hyprland socket2 / eval / window cache / Lua rule and hook injection
│  ├─ snip-recorder/          gpu-screen-recorder + ffmpeg job control
│  ├─ snip-ipc/               ctl.sock protocol (shared by daemon and CLI)
│  ├─ snip-bridge/            cxx-qt QObjects and models exposed to QML
│  └─ snip-cli/               `snip` binary (no Qt)
├─ app/                       `snipd`: main.cpp, C++ QQuickItems, LayerShellQt windows,
│  │                          DnD (KF6 KUrlMimeData), raw xdg-tag / hyprland-surface helpers
│  └─ qml/                    Overlay/ QuickAccess/ Editor/ Background/ Pins/ History/
│                             Settings/ Recorder/ Theme/ Components/
├─ third_party/omasnap/       vendored C++ (stitch) + LICENSE
├─ hypr/snip.lua              binds (hl.dsp.global), layer and window rules
├─ systemd/snipd.service
├─ packaging/aur/             PKGBUILDs (snip-screenshot, -git)
├─ bench/                     benchmark harness + corpora manifests
└─ tests/                     Rust unit/property tests, QML tests, pixel-exactness, replay
```
