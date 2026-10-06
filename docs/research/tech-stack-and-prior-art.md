Research date: 2026-10-06. Target: Omarchy 4.0.4, Hyprland 0.56.2, RTX 4080.

# Tech-stack research: a CleanShot X clone for Omarchy (Hyprland 0.56.2, RTX 4080, 3440×1440 at scale 1.25)

Date: 2026-10-06. Nothing was installed or modified. The shell was not touched. Two throwaway benchmark scripts are in the scratchpad: `eglinit_bench.py` and `vkinit_bench.py`.

## Executive summary

1. **Omarchy has already picked a screenshot tool, and it is not Tensaku.**
   - PR omacom/omarchy#9446, merged 2026-09-21 on the `quattro` branch, replaces Tensaku with **Omasnap** (`omacom/omasnap`, started by Tobi Lütke, 562 stars, v1.21.0).
   - Omasnap is C++ with Qt6 Widgets and LayerShellQt. It captures through ext-image-copy-capture in its own process.
   - It already has: a quick-access preview with drag-out, pins, a recents shelf of 5, OCR, scrolling capture, backdrops, redaction, and counters.
   - By design it has no recording, no settings UI, and no history browser. Multi-monitor is weak (#166, #164).
   - **What is left for us:** recording with on-screen controls, webcam and keystroke overlays, a trimmer, a history browser, settings, multi-monitor, and 144 Hz GPU polish.
   - **Name clash:** `FifthRooter/OpenShotX` already exists, a Rust "CleanShot-inspired" tool with 57 stars.
2. **The toolkit matters less than the daemon architecture.** These were measured on this machine:

   | Step | Time | Memory |
   |---|---|---|
   | Process exec plus dynamic linking (`tensaku --version` / `qs --version`) | 42–50 ms / 34–36 ms | |
   | EGL init on NVIDIA Wayland | ~19–25 ms | |
   | GL context | ~6–7 ms | |
   | Vulkan instance plus device | ~66–80 ms | |
   | Any NVIDIA GPU context | | ~65–70 MB RSS |

   - A cold start therefore cannot reliably put up a frozen-screen overlay in under 50 ms.
   - A resident daemon with the Wayland connection, GPU context, fonts and surfaces already warm can do it in about 15–35 ms (estimate).
3. **Recommendation**
   - **Primary: Option B.** C++20, Qt 6.11 Qt Quick/QML running on the OpenGL backend, LayerShellQt, and raw wayland-client protocol code, as one resident daemon. Recording uses gpu-screen-recorder as a child process over its `-ipc` socket.
   - **Fallback: Option A.** Rust with gtk4-rs and gtk4-layer-shell as the same kind of daemon, with GSK forced to the GL renderer.
   - **Use omarchy-shell only for a thin, QML-only bar-widget plugin.** Do not put the capture overlay or editor inside omarchy-shell (reasons in §4).

---

## 1. Facts checked on this machine

**Display and protocols**
- Display is DP-2, 3440×1440 at 143.975 Hz, scale 1.25. The logical size is exactly 2752×1152, so a pixel-exact 1:1 freeze is achievable.
- Wayland protocols found in the Hyprland 0.56.2 binary:
  - capture: ext-image-copy-capture-v1 (including cursor sessions), ext-foreign-toplevel-image-capture-source, hyprland-toplevel-export, wlr-screencopy
  - input: hyprland-global-shortcuts, wlr-virtual-pointer (auto-scroll), text-input-v1/v3, input-method-v2
  - surfaces: wp-fractional-scale, viewporter, zwlr_layer_shell_v1, hyprland-surface, xdg-toplevel-tag, ext-background-effect (blur)
  - other: wp-color-management, linux-drm-syncobj, ext-data-control / wlr-data-control
  - not present: xdg-toplevel-drag
- Hyprland's renderer is still GL (`IHyprRenderer` / `CHyprGLRenderer`).

**omarchy-shell**
- Runs as `quickshell -n -p /usr/share/omarchy/shell`, RSS ≈ 507 MB (high-water mark 581 MB), 13 threads.
- Session environment: `QT_QPA_PLATFORMTHEME=gtk3`, `QT_IM_MODULE=fcitx` (fcitx5 5.1.22 running), `NVD_BACKEND=direct`, `LIBVA_DRIVER_NAME=nvidia`.

**Plugin API** (README, `plugins/README.md`, `PluginRegistry.qml`, image-picker, clipboard)
- `manifest.json` with `schemaVersion: 1` and kinds bar-widget, panel, overlay, menu, service, bar.
- `keepLoaded: true` keeps an overlay window mounted between summons.
- Plugins are loaded in the same process. Third-party plugins get "capability-scoped facades", but the README says "visual plugins still share and can traverse the ordinary host scene" and "plugins run as unsandboxed code".
- Overlays are `PanelWindow` with `WlrLayershell.layer: Overlay` and `keyboardFocus: Exclusive`.
- IPC: `omarchy-shell shell summon|toggle|call`. That command is a bash wrapper around `quickshell ipc`, so each call spawns a process.
- The README says "Shared services and Pipewire/UPower/Hyprland consolidation … deferred". The API is new and still being reshaped.

**Theming sources**
- `~/.local/state/omarchy/current/theme/{colors.toml, shell.toml, gtk-4.0/gtk.css, qt6ct.conf}`.
- A theme-set hook directory: `~/.config/omarchy/hooks/theme-set.d/`.
- The shell's `Commons/Color.qml` and `Style.qml` are MIT-licensed (Omarchy is MIT). They define:
  - palette and surface roles, alpha companions, control-state tokens
  - font tokens based on a `monospace` fontconfig alias and `base-size`
  - `cornerRadius` from Hyprland rounding, `gapsOut` as half of `gaps_out`
- Theme switches reach the shell by IPC (`shell applyTheme`), not by a file watcher.

**Current capture pipeline**
- `hyprpicker -r -z` freezes the screen, `slurp` selects, `grim -g` captures, then `wl-copy` and a notification that opens `tensaku-edit`.
- Hardware cursors are forced on during capture so the cursor is not baked in.
- Recording: `gpu-screen-recorder -w <monitor|region> -k auto -f 60 -fm cfr -fallback-cpu-encoding yes`.
- The webcam is a separate mpv window titled "WebcamOverlay".
- OCR: `grim | tesseract`.

**Installed versions:** gpu-screen-recorder 6.1.0 (`-ipc` socket plus `gsr-cli`; `-w region -region WxH+X+Y`; webcam as a `|/dev/video0;x=..;width=25%` source; `-p plugin.so`; SIGUSR2 pause), quickshell 0.3.1 / Qt 6.11.2, gtk4 4.22.4, gtk4-layer-shell 1.3.0, libadwaita 1.9.3, egl-wayland 1.1.22 and egl-wayland2 1.0.2, tesseract 5.5.3. `layer-shell-qt` is **not** installed; it is 6.7.4 in Arch extra and depends only on qt6-base and wayland.

**Tensaku 0.29, from the binary and its man page**
- Rust: relm4 0.10.1, gtk4-rs 0.10.3, femtovg 0.25 over glow on a GLArea (epoxy), libadwaita 0.8.1, gtk4-layer-shell-rs 0.7.1, SCTK 0.19.2, image 0.25.
- It is a Satty fork (same CLI).
- It has a `--capture` overlay and `--scroll-capture` with auto-scroll.
- One process per invocation; no daemon.

**Drag-and-drop**
- libgtk-4 contains `application/vnd.portal.filetransfer`, so GTK drag-out works into sandboxed (Flatpak) apps automatically.
- Qt6Gui and QtWaylandClient do not. KF6CoreAddons 6.29, which is installed, does (`KUrlMimeData`, `OrgFreedesktopPortalFileTransferInterface`).

---

## 2. Requirements that hold whatever the toolkit (lessons from prior art)

**Capture before you show anything.**
- Grab every output through ext-image-copy-capture-v1 into shared memory (SHM) first, then map one layer-shell overlay per output that shows that buffer, and crop from memory.
- Tensaku, Omasnap, the COSMIC portal and Spectacle all do this.
- grim of a hyprpicker freeze recaptures a re-rendered surface and races it (grimblast #148/#155, hyprpicker #101/#119).

**SHM by default, dmabuf as an option.**
- DMA-BUF capture on NVIDIA is fragile:
  - wayshot #428 fails on an RTX 4070
  - Quickshell #839 hits a `qFatal` on `eglCreateImage` returning EGL_BAD_MATCH
  - HyprQuickFrame documents `QS_DISABLE_DMABUF=1` for gray screens on NVIDIA
  - wl-screenrec #147 got only NONE_INVALID modifiers
- Handle 10-bit formats (wayshot #181), output transforms (tensaku #64, wayshot #153), and red/blue swap (wayshot #364).

**Fractional scale.**
- Keep buffers in physical pixels.
- Render with wp-fractional-scale plus viewporter.
- Align image origins to whole device pixels and sample 1:1 with no smoothing or mipmaps.
- Decide early between physical and logical export (Omasnap #119/#170, Satty #240/#454, Flameshot #4818/#4920, grim's integer-scale blur).

**One layer surface per output.**
- Multi-monitor is the weakest area of every new tool (Omasnap #166, Kooha #305, NormCap #822, Flameshot #4894).

**Avoid the portal on Hyprland.**
- Screenshot portal timeouts (Flameshot #4639/#4571).
- Gradia #356.
- The gsr README says the Hyprland ScreenCast portal is "currently broken".
- Flameshot v14 got 4× slower to start (0.166 → 0.678 s, #4837).

**Recording on NVIDIA.**
- `nvidia-vaapi-driver` cannot encode, so Kooha, wl-screenrec, GNOME and Spectacle's KPipeWire fall back to the CPU or fail.
- Use gpu-screen-recorder (NVENC or Vulkan Video, KMS capture through `gsr-kms-server`) as a child process with `-ipc`:
  - stop with the IPC `stop` command and wait for the path; do not use SIGINT followed by a 5 s SIGKILL (Omarchy #12469)
  - pause with `set-paused`
  - the webcam can be composited inside gsr, or drawn by us
- Pitfalls:
  - CUDA after suspend
  - KMS bound to one card on hybrid laptops (Omarchy #7530)
  - Pascal NVENC mismatch (Omarchy #7217)

**Hotkey path.**
- Register `hyprland_global_shortcuts_manager_v1` in the daemon. There is no process spawn: Quickshell's `GlobalShortcut` does exactly this.
- A Hyprland `exec` of even a tiny client costs a fork/exec. The `omarchy-shell` wrapper costs about 35 ms just to start `quickshell ipc`.

**Quick-access card and pins.**
- The quick-access card should be a layer surface with keyboard mode None or OnDemand.
- Hyprland 0.56.2 breaks DnD out of layer surfaces whose keyboard interactivity is Exclusive. This is fixed by hyprwm/Hyprland#15780, merged 2026-08-08, after 0.56.2, so expect the fix in 0.57.
- Hand-rolled dragging of layer surfaces loses the implicit grab on empty workspaces (Hyprland #16156, reproduced with Quickshell on Qt 6.11.2 at scale 1.25, closed "not planned").
- So pins should be **floating xdg_toplevels** with Hyprland `pin`. Omasnap switched to this in PR #82.

**Drag-out data.**
- Offer `text/uri-list`, `image/png`, `text/plain` (the path, for terminals), and `application/vnd.portal.filetransfer` for Flatpak targets.
- The `hanlin-luo/omashot` prior art enumerates per-app behaviour.

**Clipboard.**
- On Wayland the owning process serves the clipboard.
- A daemon can own it. Otherwise fork `wl-copy`, which Satty #406/#129 and Swappy both had to do.

**Background work.**
- PNG encoding: use libdeflate. On a 6K frame Qt's encoder took about 3 s and libdeflate about 155 ms (Omasnap #161).
- Tesseract: set `OMP_THREAD_LIMIT=1`; it dropped from 1.64 s to 0.98 s with identical output (Omasnap #184).
- Copy NormCap's OCR post-processing heuristics.
- Data model: source image plus an operation log, which gives undo, crash recovery and editable history (Omasnap).

**Qt-specific: override `QT_QPA_PLATFORMTHEME=gtk3` in `main()`.**

| Measurement (Omasnap #112) | With gtk3 theme | Overridden |
|---|---|---|
| QApplication construction | 88–101 ms | 9–12 ms |
| RSS | 61 MiB | 38 MiB |

**GTK-specific: avoid running two GPU APIs at once.** Satty #685: GTK's Vulkan GSK plus femtovg on GL took 2–3 s on hybrid NVIDIA, about 0.3 s with `GDK_DISABLE=vulkan`. Ghostty also disables GTK Vulkan for startup.

**Latency budget for the warm path (estimate).** The total is roughly 15–35 ms from hotkey to frozen overlay.

| Step | Cost |
|---|---|
| Shortcut event | <1 ms |
| ext-image-copy-capture SHM frame (copy plus wait for the next 144 Hz frame) | ~5–15 ms |
| Texture upload (19.8 MB) | ~2–4 ms |
| Layer-surface configure roundtrip | ~1–2 ms |
| Render | ~1–2 ms |
| Presented at next vblank | ≤7 ms |

---

## 3. Options compared

### A. Rust + GTK4 (gtk4-rs 0.11.5) + gtk4-layer-shell 1.3 (+ libadwaita)

**Startup and memory**
- Cold: process ~45 ms, plus GTK init and CSS (libadwaita), plus GSK renderer init, plus capture and first frame. That is about 200–400 ms on desktop NVIDIA (estimate).
- Vulkan GSK is the Wayland default since 4.16. It costs about 70 ms for the device alone, and 2–3 s on hybrid machines (Satty #685).
- Forcing `GSK_RENDERER=ngl` (GL) costs about 26–32 ms of EGL.
- Warm daemon: about 20–40 ms.
- RSS about 90–150 MB, of which roughly 65 MB is the NVIDIA driver.

**Rendering**
- GSK: the unified ngl/vulkan renderers handle fractional scaling natively (a 1.25× framebuffer, no compositor downscale).
- GskPath fill/stroke has existed since 4.14. GTK listed "path rendering on the GPU" as future work (blog, Jan 2024), and I found no evidence it landed. Assume paths are rasterised as masks.
- Blur and scaling nodes are on the GPU.
- Custom shaders: GskGLShader belongs to the removed old GL renderer, so there is no supported custom-shader path.
- Tensaku and Satty bypass GSK entirely and use femtovg on a GLArea. That is what caused fractional blur (Satty #240, GTK MR 8503) and the dual-API startup stall.

**Fractional scale**
- Good since 4.14.
- An opt-in pixel-grid snapping API (`gtk_snapshot_set_snap()`) arrives in 4.23.1/4.24; Arch has 4.22.4.
- `scale_factor()` is an integer (tensaku PR #16 had to read the real scale from hyprctl).

**Layer shell, drag-and-drop, text**
- Layer shell: gtk4-layer-shell 1.x is mature (libwayland shim, works with any GTK4). Tensaku uses it in production.
- DnD out: GtkDragSource plus GdkFileList, with portal filetransfer built in. This is the best of all the options.
- IME: text-input-v3 natively, good preedit. On Hyprland 0.56 dead keys and compose break without fcitx (Satty #531), and preedit placement had bugs (#593).
- Text editing: Pango/GtkTextView is mature.

**Polish, theming, tooling**
- Animation: AdwAnimation/AdwSpringAnimation and CSS transitions. Fine for chrome, clunkier than QML for choreographed overlays.
- Theming: Omarchy ships `gtk.css` and libadwaita colour overrides per theme. The omarchy-shell look (monospace chrome, control tokens, gradient borders) would have to be redone in CSS.
- Packaging: cargo PKGBUILD, no ABI coupling to GTK minor versions. Watch icon loading on GTK 4.22: glycin blanked icons in tensaku #66/#67 and Satty #651, so bundle icons.
- Velocity: medium. gtk-rs subclassing is verbose; relm4 helps.
- Maturity: high.
- Testability: Rust unit tests for the core; GTK UI tests are weak.

### B. C++ (or Rust via cxx-qt 0.10) + Qt 6.11 Qt Quick/QML + LayerShellQt

**Startup and memory**
- Cold Qt Quick: process ~35–50 ms, plus `QGuiApplication` (~10 ms without the gtk3 theme), plus EGL ~26–32 ms, plus loading AOT-compiled QML, plus first frame. That is about 150–300 ms (estimate).
- Omasnap's Widgets/QPainter path measured 70–112 ms from launch to first paint at 5120×2160 (#96). That is the floor for a GPU-free Qt path.
- Warm daemon: about 15–35 ms.
- RSS about 110–160 MB with an open overlay. Omasnap's Widgets overlay measured 110–131 MB RSS and 73–83 MB PSS.

**Rendering**
- The Qt Quick RHI scene graph runs on OpenGL/EGL by default on Linux; `QSG_RHI_BACKEND=vulkan` is available.
- GPU vector paths: QtQuick.Shapes CurveRenderer. There is also Qt Canvas Painter, new in 6.11 as a tech preview: an accelerated 2D painter for Qt Quick with shadows, box gradients and custom shader brushes.
- MultiEffect and ShaderEffect cover blur, pixelate, shadows and mesh gradients on the GPU.
- Zero-copy dmabuf import is possible through EGLImage, as Quickshell does, but keep an SHM fallback.

**Fractional scale**
- QtWayland implements wp-fractional-scale-v1 plus viewporter, so device pixel ratio 1.25 maps exactly.
- There is one historical fractional-layout bug at startup (Quickshell #172, closed).
- Pixel-exact display still requires 1:1 sampling (`smooth: false`, integer device-pixel origin).

**Layer shell**
- LayerShellQt is KDE's production library (Plasma panels, Spectacle's per-output CaptureWindow, Omasnap) and has a public API.
- Use raw wayland-client for the other protocols, getting the `wl_display` through `QNativeInterface::QWaylandApplication` as Omasnap does. That avoids QtWaylandClient private headers.

**Drag-and-drop**
- QDrag from any QWindow, including layer surfaces (Plasma desktop/panel drags are layer surfaces).
- Portal filetransfer is **not** in QtGui: link KF6CoreAddons `KUrlMimeData` (installed) or call `org.freedesktop.portal.FileTransfer` yourself.

**Text and IME**
- QTextDocument-based rich text, best-in-class TextEdit preedit.
- fcitx5-qt plugin, matching the session's `QT_IM_MODULE=fcitx`.

**Animation:** the best of all options (States/Transitions, Behaviors, SpringAnimation, Animators on the render thread). omarchy-shell's own polish is QML.

**Theming:** best match. Port the MIT `Commons/Color.qml` and `Style.qml` logic (colors.toml plus shell.toml plus a Hyprland rounding/gaps query) and watch the theme directory. The result is pixel-identical controls to the bar and menus.

**Video and recording**
- QtMultimedia with the FFmpeg backend (installed) for trimmer playback with NVDEC.
- `ffmpeg -c copy` or a re-encode for cuts and GIFs.

**Packaging**
- CMake PKGBUILD; Qt public APIs are forward-compatible across minor versions.
- Only private-API use forces a rebuild per Qt minor. Quickshell has this problem; avoid it.

**Velocity, maturity, testing**
- Velocity: high for UI (QML hot reload, qmllint/qmlls); C++ core is medium.
- Maturity: highest (Spectacle, KQuickImageEditor 0.7, Plasma).
- Testability: QtTest plus Qt Quick Test, a headless offscreen platform, and a C++ core that is easy to unit-test.

**Weaknesses**
- C++ memory safety. cxx-qt 0.10 (2026-08) can host a Rust core, but QQuickItem/QSG code stays in C++ and the build becomes CMake plus cargo.
- Long-lived Qt Quick processes on NVIDIA have open issues:
  - Quickshell #1224: `eglMakeCurrent` 0x3003 then SIGABRT around lock, on Omarchy 4.0.4.
  - Quickshell #723: sync_file FD leak on NVIDIA. The maintainer says it is below Quickshell, so assume it applies to us too.
  - Quickshell #1204: SIGSEGV after resume on Omarchy 4.0.4.

### C. omarchy-shell plugin (Quickshell 0.3.1 QML) plus a native helper

**What Quickshell 0.3.1 provides** (from its installed qmltypes and docs)
- `PanelWindow` with `WlrLayershell` (layer, keyboardFocus, namespace, exclusionMode), `FloatingWindow`, `PopupWindow`.
- `ScreencopyView`:
  - `captureSource` is a ShellScreen (wlr-screencopy, or ext-image-copy-capture plus ext-capture-source) or a Toplevel (hyprland-toplevel-export)
  - properties `live`, `paintCursor`, `hasContent`, `sourceSize`, `constraintSize`; method `captureFrame()`; signal `stopped`
  - Vulkan support since 0.3.0
  - `live: false` gives a frozen frame, and many projects use it as the freeze: caelestia's area picker, hyprquickshot, HyprQuickFrame, ryoshot, hyprlens
- Hyprland module: `GlobalShortcut` (hyprland-global-shortcuts), `HyprlandFocusGrab`, Hyprland IPC objects and events, `HyprlandWindow` (opacity, mask).
- Io: `IpcHandler`, `Process`, `Socket`/`SocketServer`, `FileView`, `JsonAdapter`.
- Pipewire: audio nodes, links and peaks only, not video.
- Also: `BackgroundEffect` (ext-background-effect blur), `IdleInhibitor`, `WlSessionLock`, `ToplevelManagement`, `WindowManager` (ext-workspace), `ColorQuantizer`.
- Drag and drop: plain Qt Quick `Drag` and `DropArea`. There is no Quickshell-specific DnD API.
- Custom C++ QML modules can be imported if they are installed in the QML import path. Caelestia ships one, `CUtils.saveItem`, because `ScreencopyView` cannot save native-resolution pixels; you must `grabToImage()` (a GPU re-render plus readback) or write C++.

**Latency and memory**
- No cold start at all. In-shell summon measured about 30 ms ("keybind → IPC → visible ~30ms cold", plugins README).
- With a `GlobalShortcut` instead of the bash wrapper, add only the capture time.
- Memory: it adds to a 507 MB process. Each frozen 3440×1440 frame is about 20 MB plus a GPU texture.

**Coupling risks** (the reason not to build the product here)
- **Shared crash domain.** The bar, notifications, polkit agent and the **lock screen** (`omarchy.lock`) live in the same process. `omarchy-restart-shell` refuses to restart while locked.
- **Open screencopy crash bugs on exactly this stack:**
  - #1123: Wayland object-id reuse in the toplevel-export path "kills whole shell". Reproduced on Omarchy with Hyprland 0.56.2 by a third-party plugin's live previews.
  - #1094 and #1202: a second wl_output bind leaves a dangling QWaylandScreen, a use-after-free.
  - #897: segfault creating a ScreencopyView on session lock.
  - #839: `qFatal` on NVIDIA EGL dmabuf import failure.
  - #1228: use-after-free on hotplug in 0.3.1 with Hyprland 0.56.2.
  - The NVIDIA gray-screen fix (`QS_DISABLE_DMABUF=1`) is process-wide, so we cannot set it for just our plugin.
- **Heavy work must leave the process anyway.** PNG encoding, OCR, stitching and video run through `Process` and a native helper. Omarchy's plugin installer clones QML only and never installs binaries (Omasnap's README says the same). So we would ship an AUR package plus a git plugin and keep both versions in sync.
- **The API is young:** schemaVersion 1, built in phases, with consolidation deferred. Quickshell is 0.x with breaking changes in 0.2 and 0.3.
- **Security model:** unsandboxed. Users are warned at install time, and visual plugins can traverse the host scene. A screenshot tool that sees every pixel and lives next to the lock and polkit UI is a hard sell.
- **A native QML module inside the shell must match its exact Qt build.** A mismatch after a Qt update would take the desktop shell down.

**Variant C′: standalone Quickshell config (`qs -p /usr/share/ourapp`)**
- Gains:
  - an isolated crash domain
  - the same QML ergonomics, ScreencopyView, GlobalShortcut, IpcHandler and FloatingWindow
  - zero extra dependencies (quickshell is a dependency of omarchy)
  - Omarchy's Commons can be copied (MIT)
- Keeps:
  - all of the Quickshell screencopy bugs above
  - 0.x API churn
  - QML loaded without ahead-of-time compilation, a larger cold start, about 250–500 ms (estimate)
  - no native-resolution save without a C++ module, which then ties us to Quickshell's private Qt ABI
- **Verdict:** the best *prototype* vehicle (a day to a freeze overlay and quick-access card). It is weaker than B for the product, because B gives the same QML with control over capture, crash handling and C++ items.

### D. Pure Rust: SCTK + wgpu/vello/tiny-skia, plus iced, libcosmic, Slint, egui or GPUI for windows

**The hot path**
- SCTK 0.21.1 has a full layer-shell module and `DragSource::start_drag` (drag-out works).
- You bind fractional-scale and viewporter yourself.
- libwayshot 0.9 covers ext-image-copy, toplevel capture, dmabuf and `screenshot_freeze`.
- Fastest possible freeze: attach the captured SHM `wl_buffer` straight to the layer surface with a viewport set to 2752×1152, with no GPU at all. Selection chrome goes on a small tiny-skia subsurface.
  - Estimate: under 50 ms even cold, 10–20 ms warm, 10–30 MB RSS.
  - Prior art: latchshot.

**GPU costs**
- wgpu 30 imports dmabuf only through wgpu-hal (single plane).
- wgpu #9912: about 1.5 s startup on NVIDIA 610 plus Wayland from GLES probing; force `Backends::VULKAN`.
- The Vulkan device floor is about 70 ms.
- vello shader compilation takes about 200 ms uncached.
- None of these renderers has built-in blur or pixelate.

**Toolkits for the windows**

| Toolkit | Version | Strengths | Problems |
|---|---|---|---|
| iced | 0.14 (Dec 2025) | IME, animation API, headless and e2e tests | No upstream layer shell. Layer shell comes from iced_exwlshell 0.20.1 (fractional scale and text-input-v3, small project) with **no DnD source**. The image atlas maximum is 2048, so a 3440-wide frame gets tiled: xdg-desktop-portal-cosmic #309 shows exactly a frozen image cut off at about 60% of a 3440 monitor. |
| libcosmic | git-only | Layer shell plus DnD | DnD **freezes on Hyprland 0.56.2** (pop-os/libcosmic #1386). CJK IME only landed in 2026. |
| Slint | 1.18 | Excellent declarative animation | No layer shell. Third-party layer-shika is "not production ready", and a custom platform loses winit IME, clipboard and DnD. Cross-app drag works only on the Qt backend. GPL / royalty-free-with-attribution / commercial licences. |
| egui | | Fast to develop | Weakest IME (fcitx preedit bugs #7975/#2529, no bidi). Low polish. |
| GPUI | | Layer shell, Wayland drag-out (Jul 2026), editor-grade text | Snapshot-crate churn (`gpui-pre`). Open bugs hit this exact setup: #61368 (0×0 layer surface plus eager fractional scale makes Hyprland disconnect the app) and #65038 (blurry text from integer scale overriding fractional). |

**Text:** cosmic-text and Parley are far from QTextDocument or Pango for rich text with IME. Expect to build the editor on Parley's `PlainEditor`.

**Verdict:**
- It has the best theoretical latency and memory on the hot path.
- It is worst for velocity and polish of the other windows: settings, history, trimmer, editor text.
- The key technique (attach the raw buffer, no GPU) is worth keeping in reserve for B if measurements disappoint.

### E. Others

**Zig + custom**
- You would hand-write the Wayland client, Vulkan or GL renderer, HarfBuzz/FreeType text, IME, DnD, video playback and accessibility.
- No toolkit, pre-1.0 language churn.
- Reject. Only the raw-buffer freeze trick has merit, and it is available from C++ or Rust.

**Tauri v2, Electron, Flutter: reject.**
- Tauri v2 on Linux is WebKitGTK on GTK3, which has integer scaling only. At 1.25 that means a 2× buffer downscaled by the compositor, which is blurry and fatal for a pixel-exact freeze.
- No layer shell.
- WebKitGTK on NVIDIA has known blank-window and DMA-BUF renderer issues (`WEBKIT_DISABLE_DMABUF_RENDERER`).
- Cold start is about 0.4–1 s, RSS about 150–300 MB.
- Electron has fractional scaling and Ozone/Wayland but no layer shell and the same cost.
- Flutter's Linux embedder is GTK3, with the same integer-scale problem.

---

## 4. Prior art: stack, what worked, what broke

| Project | Stack | Got right | Pitfalls (issue numbers) |
|---|---|---|---|
| **Omasnap** (Omarchy's next default) | C++, Qt6 Widgets/QPainter, LayerShellQt, raw wayland ext-image-copy-capture, libdeflate; tesseract, wl-copy, hyprctl as subprocesses | Capture before map; quick preview; pins as floating toplevels (#82); op-log undo and crash recovery; UI thread never blocks; theme from colors.toml/shell.toml with a watcher; measured startup work (#96, #112) | CPU raster at 6K needed damage tracking and capped repaints at 60 Hz (spotlight took 37 ms/frame); RTX 6000 lockup (#79); single monitor (#166, #164); blurry editor preview (#170); no recording, history UI or settings by design |
| **Tensaku** 0.29 | Rust: relm4, GTK4, femtovg/GLArea, gtk4-layer-shell, SCTK wlr-screencopy over SHM | Capture-first overlay; mode keys after the hotkey; strong scroll-capture stitcher; IME preedit; secure blur | Cold process per use; real scale read from hyprctl; rotation ignored (#64); icons blank on GTK 4.22 (#66/#67); panic causes SIGABRT (#51); no quick-access card, recording, OCR, history or beautifier |
| **Satty** 0.22 | Rust: relm4, GTK4, femtovg/GLArea | Fast drawing, pipe-friendly CLI | 0.6 s to `main` cold (#132); 2–3 s on hybrid NVIDIA from Vulkan plus GL (#685); fractional blur (#240, #454); compose broken on Hyprland 0.56 (#531); clipboard lost on exit (#406) |
| **Swappy** | C, GTK3, Cairo | Tiny, fast | HiDPI bugs (#6, #54, #56), clipboard on exit, IME late (#154), no crop (#55) |
| **Flameshot** v14/v15 | C++, Qt6 Widgets, resident daemon | Daemon (0.166 s) | Portal-only on Wayland: 0.678 s (#4837), 30 s timeouts on Hyprland (#4639); fullscreen xdg_toplevel instead of layer shell (#4948); fractional pins and blur (#4920, #4818); one monitor at a time |
| **Ksnip** | Qt, kImageAnnotator | | Stalled since 2023; portal fullscreen only on wlroots |
| **KDE Spectacle** 6.x | C++ models, QtQuick/QML, LayerShellQt per output, KQuickImageEditor 0.7, libtesseract on a worker thread, KPipeWire recording | One app for stills, recording, annotation and OCR; per-output layer windows; C++ document with QML chrome | KWin-only capture: 70–100 ms, 250–400 ms under load; fractional-scale bugs (#490353, #510982); VAAPI-only hardware encode, so CPU on NVIDIA; recording audio only arrived in Plasma 6.8 |
| **GNOME screenshot UI** | GJS/Clutter inside the compositor | Instant: a GPU texture with no encode; one panel for screenshot and screencast | Compositor-only; no audio (#5524); no NVENC |
| **Gradia** | Python, GTK4/libadwaita | Beautify presets (background, padding, shadow, ratio) | Portal screenshot fails on Hyprland (#356) |
| **Kooha** | Rust, GTK4, GStreamer, portal | | Software encoders only (VAAPI experimental); multi-monitor (#305); pipeline failures (#345/#348) |
| **wl-screenrec** | Rust, ext-image-copy, dmabuf to VAAPI | Zero-copy encode (about 2.5% CPU at 4K60) | VAAPI cannot encode on NVIDIA (#79); fractional scale (#64); ffmpeg ABI break (Omarchy #2301) |
| **gpu-screen-recorder / gsr-ui** | C, NVENC/VAAPI/Vulkan, KMS via a capability helper | The right engine for NVIDIA; `-ipc`, region, webcam composite, pause | gsr-ui is "primarily X11" and grabs keyboards; portal broken on Hyprland; CUDA after suspend |
| **NormCap** | PySide6, tesseract CLI | OCR post-processing heuristics | Fullscreen Qt windows break on Hyprland multi-monitor (#822, #702) |
| **watershot / wayshot / libwayshot** | Rust, SCTK, wgpu / ext-image-copy, dmabuf | Per-output layer overlays; freeze API | hyprctl JSON drift panics (#30, #47); wgpu without Vulkan crashes (#17); dmabuf fails on an RTX 4070 (#428) |
| **hyprshot / grimblast** | bash: grim, slurp, hyprpicker | | Freeze race; cursor baked in; slurp border leaks via layer animation; grim integer-scale blur |
| **Quickshell pickers** (caelestia, hyprquickshot, HyprQuickFrame) | QML ScreencopyView freeze, grim or `grabToImage` save | Very fast to build, good animation | NVIDIA gray screen (`QS_DISABLE_DMABUF=1`); grim takes seconds at high resolution; native-resolution save needs C++ |
| **Others 2025–26** | Mark-Shot (Qt6 Widgets and QML, LayerShellQt, RapidOCR then Tesseract); ApexShot (Rust core plus Qt5 overlay plus GTK4 editor: 3 toolkits); omashot (Tensaku plus GTK4 Python card with multi-MIME DnD plus Quickshell pin); SSS (egui/wgpu beautifier); latchshot (SCTK plus libwayshot) | | |

---

## 5. Recommendation

### Primary: Option B, a C++20 + Qt 6.11 Qt Quick daemon

**Processes**
- `openshotxd`: a systemd user service with `Restart=on-failure`, started from Hyprland autostart.
  - It owns one Wayland connection, a warm OpenGL RHI context, the font database, and pre-built QML components.
  - It registers hyprland-global-shortcuts.
  - A tiny `openshotx` CLI talks to it over a Unix socket for scripts and menus.
- `gpu-screen-recorder` as a child process with `-ipc`.
- `ffmpeg` for trim and GIF.

**Capture**
- Own ext-image-copy-capture-v1 client for all outputs (SHM default, dmabuf with EGLImage behind a flag) and ext-foreign-toplevel capture for clean windows.
- wlr-virtual-pointer for auto-scroll.
- Keep raw buffers and crop exactly.

**Surfaces**
- One LayerShellQt overlay per output for the freeze, with keyboard Exclusive.
- Quick-access card on a layer surface with keyboard None or OnDemand, using QDrag with `text/uri-list`, `image/png`, `text/plain` and portal filetransfer via KF6CoreAddons.
- Pins, editor (or the editor as an overlay), history, settings and trimmer as xdg toplevels, with Hyprland float and pin rules.

**Rendering**
- A custom `QQuickItem`/QSGNode for the frozen frame (1:1 texture).
- Annotation canvas built on Shapes CurveRenderer or QSG nodes. Evaluate Qt Canvas Painter (6.11 tech preview).
- ShaderEffect for pixelate, blur, mesh gradients and shadows.
- Rich text with QML TextEdit (QTextDocument).

**Model:** source plus op log (Omasnap's model), SQLite history index (Qt SQL), libdeflate PNG, libtesseract on a worker thread with `OMP_THREAD_LIMIT=1`, QtConcurrent workers. The UI thread never blocks.

**Theming:** port Omarchy's MIT `Color.qml`/`Style.qml` token logic. Read colors.toml and shell.toml, watch the directory, and add a `theme-set.d` hook. Bypass `QT_QPA_PLATFORMTHEME=gtk3`.

**Omarchy integration:** an optional QML-only omarchy-shell `bar-widget` for recording state, start and stop, calling the daemon over IPC. If it crashes it cannot take capture down, and capture cannot take the shell down.

**Language:** C++ for anything that touches Qt or Wayland. Treat Rust through cxx-qt 0.10 as optional for pure compute (stitching, indexing) if the team strongly prefers Rust; it adds a CMake-plus-cargo build.

**Why it wins**
- The only stack that combines all of:
  - a GPU scene graph with real shader effects at 144 Hz
  - the best animation system available
  - a mature layer-shell library (LayerShellQt)
  - rich text with fcitx5 IME
  - QtMultimedia for the trimmer
  - pixel-identical Omarchy styling (the same QML idiom and tokens as omarchy-shell)
  - a strong testing story
- Omasnap and Spectacle prove Qt with LayerShellQt on this exact platform.
- Unlike Omasnap's CPU QPainter, it will not need 60 Hz repaint caps and hand-rolled damage tracking at 3440×1440 at 144 Hz.

### Fallback: Option A, a Rust + gtk4-rs + gtk4-layer-shell daemon

- Same architecture: resident daemon, own capture over SCTK or libwayshot SHM, per-output layer surfaces.
- Force `GSK_RENDERER=ngl`, or render the canvas through GSK only, to avoid running two GPU APIs.
- Draw the canvas with GtkSnapshot/GskPath, or femtovg on a GLArea with explicit device-pixel snapping.
- Use libadwaita for settings and history.
- Choose this if memory safety or a single Rust codebase outweighs animation and shader polish, or if the C++/Qt spikes fail. Drag-out (built-in portal filetransfer) and IME are excellent.
- Expect to rebuild the omarchy-shell look in CSS and to wait for GTK 4.24 snapping.

### Prototype vehicle, not the product: C′, a standalone Quickshell config

Use it for a one- or two-day spike on interaction design: freeze, quick-access card, recording HUD. Port the QML into B afterwards.

### Tradeoff table

Ratings run from 1 (worst) to 5 (best). Times and RSS are estimates unless a source is given.

| Criterion | B Qt Quick daemon | A GTK4-rs daemon | C omarchy-shell plugin | C′ standalone qs | D SCTK + iced | E Tauri/Electron |
|---|---|---|---|---|---|---|
| Cold start to overlay | ~150–300 ms | ~200–400 ms (Vulkan) / ~150–300 ms (ngl) | n/a, always resident | ~250–500 ms | <50 ms (raw buffer) / 200–400 ms (iced) | 400–1000 ms |
| Warm hotkey to overlay | 15–35 ms | 20–40 ms | ~15–30 ms | 20–35 ms | 10–20 ms | n/a |
| RSS while idle | ~110–160 MB | ~90–150 MB | +30–80 MB to a 507 MB shell | ~120–200 MB | 10–30 MB (raw) / 80–150 MB (iced) | 150–300 MB |
| GPU path on NVIDIA | GL/EGL RHI (~27 ms init), Vulkan optional | Vulkan GSK by default (~70 ms) or ngl | Shared shell GL; dmabuf gray-screen workaround is global | GL RHI | Vulkan via wgpu (GLES probe trap #9912) or none | WebKit/Chromium GPU quirks |
| Fractional scale | 5 | 4 (snapping API in 4.24) | 4 | 4 | 4 (do it yourself) / GPUI bugs | 1–3 |
| Layer-shell quality | 5 LayerShellQt | 5 gtk4-layer-shell | 5 | 5 | 4 SCTK / 2–3 toolkit wrappers | 1 |
| Drag-out from layer surface | 4 (add portal filetransfer) | 5 | 3 (works, crash domain shared) | 4 | 4 SCTK / 1–2 iced, libcosmic | 2 |
| Text editing and IME | 5 | 4 (compose quirks) | 4 | 4 | 2 | 4 |
| Animation polish | 5 | 3 | 5 | 5 | 3 (iced) / 4 (Slint, GPUI) | 4 |
| Omarchy theming match | 5 (same QML idiom) | 3 | 5 | 5 | 2 | 3 |
| AUR packaging | 4 | 5 | 2 (QML repo plus native binary) | 3 | 4 (git deps for cosmic, GPUI) | 3 |
| Developer velocity | 4 | 3 | 5 at first, 2 later | 4 | 2 | 4 |
| Ecosystem maturity | 5 | 5 | 3 | 3 | 2–3 | 4 |
| Testability | 5 | 3 | 2 | 2 | 3–4 | 3 |
| Crash isolation | 5 | 5 | **1** (bar, notifications, lock) | 5 | 5 | 5 |
| Memory safety | 2–3 | 5 | 4 | 4 | 5 | 4 |

### Main risks of the primary, with mitigations

1. **Long-lived Qt Quick on NVIDIA.**
   - Risks: EGL context loss (`eglMakeCurrent` 0x3003 aborts around lock and DPMS, Quickshell #1224), a sync_file FD leak below the Qt layer (#723), crashes after resume (#1204).
   - Mitigations:
     - Destroy overlay and editor `QQuickWindow`s when idle; keep only the context-free core warm.
     - Run under systemd with `Restart=on-failure`.
     - Make the op log the source of truth so restarts lose nothing.
     - Export an FD and RSS watchdog.
     - Do a 72-hour soak test across lock, suspend and DPMS before committing.
2. **Hyprland behaviour drift.**
   - Known now: DnD out of Exclusive layer surfaces is broken in 0.56.2 (fixed in #15780 for 0.57); the implicit grab is lost on empty workspaces (#16156, won't fix); Lua config and dispatcher changes; hyprctl JSON drift (watershot #30/#47).
   - Mitigations: prefer protocols over hyprctl; keep the quick-access card non-Exclusive; run CI against a nested or headless Hyprland at pinned versions.
3. **Capture-format edge cases on NVIDIA.** Dmabuf modifier import failures, 10-bit, transforms, red/blue swap. Make SHM the default and treat dmabuf as opt-in after per-driver probing. Add a "capture, show, recapture, diff == 0" pixel-exactness test at scale 1.25.
4. **C++ memory safety and image parsing.** Run ASan/UBSan and clang-tidy in CI, fuzz the PNG and op-log readers, and keep parsing in a small module. Use cxx-qt only if the team wants Rust there.
5. **Qt ABI and private APIs.** Use only public Qt APIs plus LayerShellQt plus raw wayland-client, as Omasnap does. Never use QtWaylandClient private headers or Quickshell internals, so Arch Qt minor updates do not break AUR users.
6. **Daemon cost.** About 110–160 MB resident, mostly NVIDIA driver mappings; PSS is lower. Offer an "exit after idle" mode with a socket-activated cold path, which still lands at about 150–300 ms.
7. **Product overlap with Omasnap.** It is now Omarchy's default. Either differentiate clearly (recording suite, history, multi-monitor, settings, 144 Hz GPU polish) or contribute the shared screenshot pieces upstream. Pick a name other than "OpenShotX".
8. **Recording engine dependence on gpu-screen-recorder.** It needs a capability-granted KMS server, has CUDA-after-suspend problems, and has hybrid-GPU card binding issues. Drive it through `-ipc`, surface its errors in the HUD, and keep the encoder choice (NVENC or Vulkan) configurable.

### Spikes to run before writing product code (about one week)

1. **Warm latency:** a Qt Quick daemon showing a frozen overlay. Measure hotkey to first presented frame using wp_presentation feedback. Target: under 35 ms.
2. **Pixel exactness at 1.25** on DP-2: capture, display, recapture, diff. Also check editor zoom views.
3. **Drag-out** from a layer-shell card into Firefox, Chromium, Nautilus, a terminal and a Flatpak app on Hyprland 0.56.2, with keyboard None vs OnDemand.
4. **IME:** QML TextEdit on an Exclusive layer surface with fcitx5 (preedit position, compose and dead keys).
5. **NVIDIA soak:** 72 h idle-plus-use across lock, suspend and DPMS, tracking FD and RSS.
6. **gpu-screen-recorder IPC:** start a region recording, pause, stop through IPC, and measure the latency to the final file path.

## Key sources

- Local: `/usr/share/omarchy/shell/{README.md, plugins/README.md, services/PluginRegistry.qml, Commons/Color.qml, Commons/Style.qml, plugins/image-picker, plugins/clipboard}`, `/usr/lib/qt6/qml/Quickshell/**.qmltypes`, `omarchy-capture-*` scripts, `man gpu-screen-recorder`.
- Omarchy and Omasnap:
  - https://github.com/omacom/omarchy/pull/9446
  - https://github.com/omacom/omasnap (docs/dependencies.md, docs/threading.md, PRs #82, #96, #112, #161, #184; issues #79, #119, #164, #166, #170)
- Quickshell:
  - https://quickshell.org/docs/v0.3.0/types/Quickshell.Wayland/ScreencopyView/ and https://quickshell.org/changelog/
  - issues on github.com/quickshell-mirror/quickshell: #1123, #1094, #1202, #897, #839, #1228, #1224, #723, #1204, #172
- Hyprland: https://github.com/hyprwm/Hyprland/pull/15780, https://github.com/hyprwm/Hyprland/issues/16156
- Quickshell-based pickers: https://github.com/Ronin-CK/HyprQuickFrame (`QS_DISABLE_DMABUF`), https://github.com/JamDon2/hyprquickshot, caelestia-dots/shell `plugin/src/Caelestia/cutils.cpp`
- GTK:
  - https://blog.gtk.org/2024/01/28/new-renderers-for-gtk/
  - https://blog.gtk.org/2026/05/28/snapping/
  - wmww/gtk4-layer-shell releases
- Qt: https://doc.qt.io/qt-6/whatsnew611.html (Qt Canvas Painter); KDAB/cxx-qt v0.10.0
- Satty, Tensaku, Flameshot, Spectacle, wayshot, wl-screenrec, Kooha, gpu-screen-recorder, libcosmic #1386, wgpu #9912, iced_exwlshell, Slint, GPUI #61368/#65038: issue numbers are cited inline above, from their GitHub trackers and the subagent surveys.
- Benchmarks:
  - EGL and Vulkan init on this machine: `scratchpad/eglinit_bench.py` and `scratchpad/vkinit_bench.py`
  - Omasnap PR measurements (#96, #112, #161, #184)
  - Satty #132, #685
  - Flameshot #4837
