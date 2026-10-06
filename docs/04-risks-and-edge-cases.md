# Risks and edge cases

Status: **DRAFT**, 2026-10-06.

- Part A is the risk register: things that can sink the project or a milestone.
- Part B is the edge-case catalogue: specific situations with the behaviour we expect. These become test cases.

## Part A: Risk register

Likelihood and impact are rated H (high), M (medium) or L (low). "Spike" refers to [05-roadmap.md §M0](05-roadmap.md).

| ID | Risk | L | I | Mitigation | Spike |
|---|---|---|---|---|---|
| R1 | **Omasnap is Omarchy's default** (merged 2026-09-21). Users already get a good screenshot tool for free. | H | H | **Decided:** build on Omasnap's engine and win on the CleanShot experience: QAO, history, Background tool with Omarchy themes, pins, settings, multi-monitor, then recording. Keep Omasnap's MIT notice and upstream useful fixes. | — |
| R2 | **Hyprland API drift.** The Lua config, `hl.*` dispatchers, the `input.keyboard.key` event (Jul 2026), rule syntax and `hyprctl` JSON all change often, and Omarchy follows Hyprland closely. | H | M | Prefer Wayland protocols over hyprctl. Keep every Hyprland call in one `HyprlandBridge` module with a version probe. CI runs nested Hyprland at pinned versions (current, and Omarchy's next). Degrade features instead of failing. | S9 |
| R3 | **Long-lived Qt Quick on NVIDIA:** EGL context loss around lock and DPMS, a sync_file fd leak, a crash after resume. These are known Quickshell issues on this exact stack. | M | H | No GL context while idle. Destroy windows after use. systemd `Restart=on-failure`. An fd and RSS watchdog. The op log makes restarts lossless. A 72 h soak before committing to the stack. | S5 |
| R4 | **Capture-format edge cases:** dmabuf modifier failures and gray screens on NVIDIA, 10-bit `XBGR2101010` when `screencopy_force_8b=false`, transforms, red/blue swap. | M | M | shm by default. A format-conversion module with tests for every advertised format. Pixel-exact round-trip tests. | S2 |
| R5 | **gpu-screen-recorder dependency:** KMS needs a capability-granted helper; CUDA breaks after suspend; on hybrid GPUs it is bound to one card; Pascal NVENC issues; Omarchy's own scripts `pkill` **every** gsr process. | M | H | Drive it through `-ipc`. Detect and show its errors. Handle an external stop gracefully. Stop and finalise on logind `PrepareForSleep` (delay inhibitor). Keep a Phase-2 pipeline as the escape hatch. | S7 |
| R6 | **KMS recording ignores `no_screen_share`.** Private windows (1Password, Bitwarden) end up in recordings. | H | H (privacy) | Watch focus and open windows during a recording. Warn before start and pause or show a banner when a private window appears in the region. Offer a privacy-safe backend in 1.x. Document it plainly. | S7 |
| R7 | **Keystroke and click hook safety:** a stuck `mouse:272` bind breaks clicking; keys could leak to other socket2 listeners; the hook runs on the compositor thread. | M | H | Install only while recording. A Lua watchdog removes the hooks when our heartbeat stops. Remove them on daemon exit and on start-up (crash cleanup). Private 0600 file transport. Mute while locked or a private window is focused. | S9 |
| R8 | **Drag-and-drop from layer surfaces** is broken while exclusive (0.56.2) and the implicit grab is lost on empty workspaces (won't fix). | H | M | Never drag from exclusive surfaces. Pins are toplevels. Card drags start from a non-exclusive layer. Retest on 0.57. | S3 |
| R9 | **Paste compatibility:** Electron and terminals pick text types over images; Flatpak apps can't read our paths. | M | M | `image/png`-only copy by default. A separate "copy as file". Portal FileTransfer for drags. A maintained compatibility matrix. | S10 |
| R10 | **Fractional-scale off-by-one** between our overlay, Hyprland's `CBox::round`, and gsr's truncation. | H | M | Physical-pixel model. Snap recording selections to rectangles gsr can express. Round-trip tests at 1, 1.25, 1.5, 1.6̅, 2, and 1.25 with transform 1. | S2 |
| R11 | **Scrolling stitch failures:** sticky headers, animations, lazy loading, smooth-scroll settle. | H | M | A robust algorithm (processing doc) plus a confidence score per seam. Show a live preview so the user sees failures at once. "Slow down" hints. A test corpus of real pages. | S11 |
| R12 | **OCR quality** below Apple Vision, especially for small UI text, dark themes and non-Latin scripts (Georgian, CJK). | H | M | Screenshot preprocessing (upscale, invert, binarise). Per-language packs. An optional high-accuracy ONNX engine. A test corpus. | S12 |
| R13 | **Very large captures:** scroll captures over 30k px exceed GPU texture limits (8k–32k) and use a lot of RAM. | M | M | Tiled textures in the editor and tiled export. Keep tall images as strips; stream PNG rows. | S11 |
| R14 | **C++ memory safety** in parsers (PNG import, project zip, TOML, IPC JSON). | M | H | ASan/UBSan in CI. Fuzz every parser. Clamp sizes. Keep parsing in small modules. Optionally move compute to Rust (Q-S2). | — |
| R15 | **Scope creep:** CleanShot 5.0's Studio Mode alone is a video editor. | H | H | Studio Mode is P2, after 1.0. Every milestone has exit criteria. Parity means features *users notice*, not every toggle. | — |
| R16 | **Trade dress and IP:** copying CleanShot's icons, sounds, gradients, wording or name. | M | H | Clone behaviour, not assets. All visuals, sounds and gradients are our own. No "CleanShot" in the product name. | — |
| R17 | **Omarchy integration drift:** shell IPC method names, theme file formats, the bind helper `o.bind`, and `quattro` changes. | H | L–M | Integration lives in thin, optional adapters with feature probes. Fall back to `notify-send` and default colours. | — |
| R18 | **Exclusive overlay takes pointer input across monitors.** On multi-monitor, events reach the wrong surface. | H | M | Make every per-output overlay exclusive; route input centrally. Test with 2 and 3 monitors of mixed scale. | S1 → **verified (S01 R18): a drag on the second monitor and Esc both route correctly** |
| R19 | **Session lock while recording:** KMS records the lock screen, and the key hook sees password keystrokes. | M | H | Pause recording and mute key events on lock (Hyprland lock event and logind `Lock`). Resume on unlock only if the user chooses to. | S7, S9 |
| R20 | **Licensing.** gsr is GPL-3.0 (used as a separate process: OK). Qt is LGPL-3.0 (dynamic linking: OK). Omasnap is MIT (attribution required). Tesseract is Apache-2.0. KF6 is LGPL. | L | M | Choose our licence (product Q8). Keep a `THIRD_PARTY.md`. | — |
| R21 | **Daemon memory on NVIDIA:** about 65 MB per GPU context. | M | L | Idle with no context; "low memory" mode exits after idle. | S1 |
| R22 | **Name and packaging:** the AUR name `snip` is taken (an abandoned snippet manager that probably ships `/usr/bin/snip`); "Mathpix Snip" is an unrelated OCR product | M | L | Package `snip-screenshot` with `conflicts=('snip')`; no Mathpix-like branding | — |
| R23 | **Design tension:** CleanShot polish vs arbitrary Omarchy themes (26 or more, light and dark, gradient borders, square corners). Some themes will make the UI ugly or unreadable. | H | M | A token mapping with fallbacks, a runtime contrast guard, a gallery that renders every installed theme, and visual-regression goldens (design doc §6) | S17 |
| R24 | **Vendored Omasnap C++ drift:** upstream improves the stitcher while our copy stays behind; C++ inside a Rust core | M | L | Pin the upstream commit. Use the replay harness to compare. Port to Rust once it stabilises. | S16 |
| R25 | **QML garbage-collects C++-owned objects** returned from `Q_INVOKABLE`s without a parent. Spike S03: QML deleted Qt's real `QScreen`, every window lost its screen, and the next map segfaulted inside QtWayland. | M | H | Set `QQmlEngine::CppOwnership` on every object handed to QML. Unit-test the ownership of every exposed object. Run ASan in CI. | done (S03) |
| R26 | **Render loop on NVIDIA:** Qt falls back to the *basic* loop (125 fps, stutter). Forcing *threaded* gives 144 fps, but threaded GL on NVIDIA is where the known Quickshell EGL issues live. | M | M | Force `QSG_RENDER_LOOP=threaded`. Run a second 72 h soak with the threaded loop. Keep an automatic fallback to basic if EGL errors occur. | S05, S5b |

## Part B: Edge-case catalogue

Format: *situation* → **expected behaviour**. IDs link to the product spec.

### B1. Capture and overlay (CAP, SEL)
- The hotkey is pressed while the overlay is already up → **cancel** (default) or cycle mode (Q6). Never open a second overlay.
- The hotkey is pressed while recording → the screenshot works normally. The recording's dim layer, border and controls would otherwise be in the frozen frame, so they are hidden for that one frame (`hyprland_surface_v1.set_opacity(0)`) before the capture and restored at once. The recording outside the frozen frame is not affected.
- The hotkey is pressed while the session is locked → do nothing. Optionally log it.
- A monitor is DPMS-off or never commits → per-output capture timeout (150 ms). That monitor shows a live, unfrozen overlay with a dim notice. Capture on it re-captures after the overlay unmaps.
- A monitor is hot-plugged or unplugged while the overlay is open → close the overlay on the removed output; keep the selection if it was on another monitor; reallocate capture buffers.
- Mixed scales (1.0 plus 1.25 plus 2.0) → each overlay renders at its own scale. A selection never crosses monitors. A drag that crosses monitors is clamped to the monitor where it started.
- Rotated monitor (transform 1/3, or a flipped variant 4–7) → the frozen frame is shown with the buffer transform; the selection is mapped through the inverse transform; the output is upright.
- A monitor at negative global coordinates → only matters for hyprctl and gsr conversions (floor division); tested.
- Scale 5/3 → physical rectangles that integer logical coordinates can't express are fine for stills (we crop from memory). Recording snaps to the nearest rectangle gsr can express.
- **Software cursor active** (tearing, `no_hardware_cursors`, or a headless or virtual output such as a streaming dummy plug) → the cursor is baked into monitor captures, so the frozen frame and the screenshot would contain it (seen in spike S01). → For the capture frame only, set `cursor:invisible` (or force hardware cursors, as Omarchy does) and restore it immediately. "Show cursor" draws the cursor ourselves from the cursor session.
- **Overlay placement on multi-monitor** → each overlay's layer surface must get the real `QScreen*` through LayerShellQt's `screen`. QML screen wrappers silently place every overlay on the first monitor (spike S01 bug).
- A fullscreen game with tearing or direct scanout → capture forces compositing (one hitch). Software cursor → excluded from screenshots by Omarchy's workaround; we use the cursor session for "show cursor".
- 10-bit monitor with `screencopy_force_8b=false` → convert `XBGR2101010` to 8-bit sRGB correctly (not the format-flip hack).
- HDR monitor → the frozen frame is tone-mapped sRGB, so HDR content looks different during the freeze. Document it; don't fight it.
- Night light (hyprsunset) or `cursor:zoom_factor` active → verify whether they end up in captures (test T9 in the platform doc). If they do, document it, or capture with the CTM off where possible.
- A one-pixel or zero-area drag → treated as a click (window/monitor snap). A drag under 4×4 px after release counts as a click.
- Very fast click-drag-release before the overlay is visible → buffer pointer events from the moment of the hotkey; never lose a drag.
- `no_screen_share` window under the selection → it appears **black** in the captured pixels, because the compositor blanks it. Show a hint: "Private window hidden".
- Window capture of a `no_screen_share` window → "This window is private"; nothing is captured.
- Capture Previous Area after the monitor layout changed → if the monitor is gone, use the focused monitor and show a toast; if the rectangle no longer fits, clamp it and show a toast.

### B2. Quick Access Overlay (QAO)
- More than N captures → collapse into "+k"; Save All and Close All still cover hidden ones.
- Monitor of the stack unplugged → move the stack to the focused monitor.
- The user is in a fullscreen app → the QAO shows on the overlay layer, does not steal focus, and doesn't hide the app's own UI for longer than its timeout. Optional setting: "don't show over fullscreen".
- Auto-close fires while a drag is in progress → postpone until the drag ends.
- Auto-close with "Save and Close" and a full disk → don't close; show an error card. With plain "Close" (default) nothing is lost; the capture stays in history.
- History disk usage grows (for example 50 captures a day at about 1.5 MB each) → retention sweep (default 1 week), usage shown in Settings, a warning when history passes a size cap (default 2 GB).
- History set to *Never* → captures live only in memory and in QAO cards; closing asks for confirmation (CleanShot behaviour).
- **Save** when the export folder doesn't exist → create it (as Omarchy does) and show a toast; if it can't be created (permissions or a missing mount) → error card, capture kept in history.
- Save with a name collision → apply the template's `%i` counter or append `-1`, `-2`; never overwrite.
- A drag target never requests data (cancelled) → the card stays.

### B3. Clipboard and drag (QAO-04, SET-07)
- Paste requested before the PNG encode finishes → block that `send()` until the encode is done (cap 2 s), then write. Never offer a partial PNG.
- Several concurrent `send()` calls (Omarchy's clipboard manager plus the target app) → serve each one independently with non-blocking writes.
- The daemon restarts while it owns the clipboard → the selection is lost. On restart, re-offer the most recent capture if it is less than 60 s old (setting).
- The receiver is a Flatpak app → portal FileTransfer key for drags. Clipboard `image/png` needs no file access.
- A very large image (a 50 MP scroll capture) on the clipboard → still `image/png`, with a warning toast about size. Browsers may reject it.

### B4. Editor (ANN)
- Text input with an IME (fcitx5 Pinyin, dead keys, compose) → preedit shown inline; commit is correct.
- Redaction then export → the redacted pixels are irreversibly replaced in the flattened output. Project files keep the original, and "Share project" warns about that.
- Crop outward → the canvas grows and is filled with the detected background colour; undo restores it exactly.
- An image above `GL_MAX_TEXTURE_SIZE` → tiled display and tiled export; the editor stays responsive.
- Opening a project from a newer version → open read-only, with a placeholder for unknown objects.
- Crash mid-edit → on restart, offer to restore from the working op log.
- An editor window on a monitor with a different scale from the capture → render at the window's scale; 100 % zoom means one image pixel per device pixel.

### B5. Pins (PIN)
- A pin is dragged to another monitor → it moves (Hyprland handles toplevel moves). "Pinned" then applies to the new monitor's workspaces.
- A fullscreen window opens → the pin stays on top (Hyprland allows pinned floating windows over fullscreen).
- Locked (click-through) pin, and the user needs to unlock it → global hotkey "unlock all pins", plus a context menu from the bar widget.
- A pin is excluded from a new screenshot → set the pin's opacity to 0 for one frame while capturing (setting: "include pins in screenshots", default off).

### B6. Recording (REC)
- The region overlaps the controls → the controls move outside the region automatically. With no room on that monitor, they go to another monitor or are hidden; hotkeys and the bar widget remain.
- Odd physical dimensions → snap to even before start (no gsr padding).
- External stop (Omarchy's indicator or `pkill`) → detect the gsr exit, finalise if the file is valid, show the QAO card, and clean up hooks and DND.
- Daemon crash while recording → gsr keeps running in its own group. On restart, re-attach through the IPC socket, or show "Recording recovered".
- Suspend while recording → stop and finalise on `PrepareForSleep`; never resume a broken CUDA session.
- Disk space under the threshold (default 1 GB or 2 minutes at the current bitrate) → warn, then auto-stop with a valid file.
- Mic muted or disconnected → warn before start; continue without audio if the user agrees.
- Bluetooth headset mic → warn that system audio will drop to HFP quality.
- Lock while recording → auto-pause (R19).
- Audio pop at the start → open audio, discard the warm-up, then trim the first 300–400 ms of audio at post-process.
- 144 Hz monitor → default 60 fps. Native-rate recording is optional; on iGPUs at more than 1440p warn about dropped frames.
- Webcam window: it is in the region but the user moves it out → it is no longer recorded (what you see is what you get). Show a hint.
- Recording a region on a monitor that gets unplugged → stop and finalise.
- GIF longer than 30 s or wider than 1280 px → warn about size; offer MP4.

### B7. Scrolling capture (SCR)
- The content doesn't scroll (the selection isn't over a scrollable area) → after 2 auto-scroll steps with no movement, stop with "Nothing scrolled".
- Natural scroll is enabled → pre-compensate the injected direction.
- Focus-follows-mouse moves focus when we warp the cursor → expected; restore the cursor position at the end.
- A sticky header or footer → detect static bands and include them once.
- The page reaches its end → 3 consecutive frames with no movement → finish.
- The user moves the mouse out during auto-scroll → pause; resume when it moves back.
- Stitch confidence low at a seam → mark the seam in the preview; the user can stop and keep what they have.

### B8. OCR (OCR)
- Nothing recognised → "No text detected" toast; clipboard unchanged.
- A QR code and text both present → prefer QR if it covers more than 30 % of the region; otherwise text, and offer the QR value as an action.
- A malicious QR URL → never open automatically. Show the URL in full with the scheme highlighted; allow only http/https by default.
- Mixed scripts (Georgian plus English) → language list from settings; "automatic" uses OSD script detection and the installed packs.

### B9. Integration and lifecycle
- Hyprland config reload → re-inject runtime rules; re-register is not needed (global shortcuts survive).
- Theme switch → re-read tokens; open windows restyle live.
- A newer Hyprland (API change) → the version probe disables only the affected feature (for example keystrokes) and shows the reason in Settings → About → Diagnostics.
- Omarchy's own PRINT binds still active → our onboarding detects the duplicate binding and offers to unbind. Never edit Omarchy's files.
- The daemon is not running and the CLI is called → socket activation or a direct cold start; exit code 3 if that is impossible.
