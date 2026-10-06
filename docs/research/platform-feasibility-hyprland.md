Research date: 2026-10-06. Target: Omarchy 4.0.4, Hyprland 0.56.2.

# Platform feasibility study: CleanShot X clone on Omarchy 4.0.4 / Hyprland 0.56.2

Evidence key: **[local]** means I checked it on this machine (read-only). **[src]** means I read the Hyprland v0.56.2 source or another upstream source. **[infer]** means it is reasoning or an estimate that needs a hardware test. No captures or recordings were run, as instructed, so every latency number below is an estimate. A test checklist is at the end.

## 0. The ten findings that matter most

1. **Hyprland 0.56.2 supports every capture protocol you listed, and all of them share one backend.** It exports `zwlr_screencopy_manager_v1`, `ext_image_copy_capture_manager_v1` (including cursor sessions), `ext_output_image_capture_source_manager_v1`, `ext_foreign_toplevel_image_capture_source_manager_v1`, `hyprland_toplevel_export_manager_v1`, `ext_foreign_toplevel_list_v1` and `hyprland_toplevel_mapping_manager_v1` **[local: strings of /usr/bin/Hyprland]**. All of them go through one `Screenshare::CScreenshareManager` (`src/managers/screenshare/`), so their behaviour is the same **[src]**.
2. **`no_screen_share` paints BLACK boxes. It does not make anything invisible.** It applies to every Wayland capture path: grim, ext-image-copy-capture, toplevel export, the XDPH portal and Quickshell. It does **not** apply to gpu-screen-recorder's KMS backend, because KMS reads the scanout framebuffer. A layer rule `no_screen_share` also exists **[src + wiki]**. A PR to make hidden content transparent is still open (#11572).
3. **Omarchy's default recorder (gsr with KMS) records windows that Omarchy marks private.** Omarchy sets `no_screen_share` on 1Password and Bitwarden, and KMS capture ignores it **[local: apps/1password.lua, bitwarden.lua]**.
4. **Window capture works for occluded and other-workspace windows.** Hyprland re-renders the window offscreen in "standalone" mode. You get no border, no shadow, square corners, none of Hyprland's opacity or dim, and the client's own alpha is kept **[src: Renderer.cpp renderWindow]**.
5. **Hyprland always reports full-frame damage**, which is a TODO in its code. Frames are only copied on a monitor commit. The shm path calls `glReadPixels` synchronously on the compositor thread **[src]**.
6. **Any layer surface with exclusive keyboard focus also takes all pointer input in Hyprland, even with an empty input region** **[src: InputManager.cpp, "forced above all"; discussion #14136]**. This matters for multi-monitor overlays and for scrolling capture.
7. **Keystroke capture has a clean option on Hyprland 0.56.** Lua `hl.on("input.keyboard.key", …)` (added Jul 2026, #14779) can forward events to socket2 via `hl.dsp.event(...)`. It can be installed and removed at runtime with `hyprctl eval`, needs no root or `input` group, and only runs while recording **[local stubs + src]**. Mouse clicks need a `non_consuming` bind on `mouse:272`, which still needs testing.
8. **Global hotkeys can reach a running daemon with no process spawn.** The daemon registers through the `hyprland_global_shortcuts_v1` protocol, and the user binds `hl.dsp.global("openshotx:area")` **[src]**.
9. **Clipboard needs ext-data-control in the daemon.** `wl_data_device` requires focus, and GTK and Qt clipboards use it. For drag and drop, Chromium/Electron do **not** accept raw `image/png` drags on Wayland. You must offer `text/uri-list` file URIs **[src: Chromium wayland_exchange_data_provider.cc]**. Offer only the COPY action, because Hyprland picks MOVE whenever the source allows it **[src: DataDevice.cpp]**.
10. **NVIDIA has no VA-API encode.** libva-nvidia-driver only decodes, so wl-screenrec and any VAAPI pipeline cannot encode on your RTX 4080. Use NVENC through CUDA or Vulkan Video. Separately, XDPH 1.4.1 (the Arch package) does not include the 2026-08-29 fix for multi-plane dmabufs. That is the most likely cause of Omarchy's warning about the portal path failing DMA-BUF modifier import **[src]**.

---

## 1. Screen capture protocols on Hyprland 0.56

**What is supported** **[local + src]**
- **ext-image-copy-capture-v1 + ext-image-capture-source-v1.** Merged in PR #11709 on 2026-02-25, so it ships from 0.54.0 (released 2026-02-27). Both output sources and foreign-toplevel sources work, as do cursor sessions.
- **wlr-screencopy-unstable-v1.** Supports `capture_output` and `capture_output_region`, with `copy` and `copy_with_damage`.
- **hyprland-toplevel-export-v1.** You can target a window by `hyprctl` address (lower 32 bits) or by a wlr toplevel handle.
- **ext-foreign-toplevel-list-v1 + hyprland-toplevel-mapping-v1.** The mapping protocol converts an ext handle to the Hyprland window address, so you can join it with `hyprctl clients` geometry.
- Installed client tools: grim 1.5.0 already uses ext-image-copy-capture and `-T <toplevel id>`. hyprpicker 0.4.7 uses wlr-screencopy plus fractional-scale and viewporter. slurp 1.5.0 has **no** fractional-scale or viewporter support, so it renders at an integer buffer scale and the compositor downsamples it **[local strings]**.

**How Hyprland does a copy** **[src: ScreenshareFrame.cpp / ScreenshareSession.cpp / ScreenshareManager.cpp]**
- **Monitor and region capture.** Hyprland copies the monitor's "mirror texture", which is the composited frame. It then paints black boxes over `no_screen_share` windows, layers and their popups. If the cursor is requested, it renders it as a software cursor on top. The copy happens in `onOutputCommit`, so only when that monitor commits a frame.
- **Region sessions** (screencopy only) take a logical box, scale it and round it with `CBox::round()`. That function rounds x, and then rounds the right edge (`round(x+w-round(x))`). So region capture is edge-rounded, but it can only start from integer logical coordinates.
- **Window capture** calls `renderWindow(window, monitor, now, decorate=false, RENDER_PASS_ALL, ignorePosition=true, standalone=true)` into a buffer cleared to (0,0,0,0).
  - Standalone mode skips the `isHidden()` early return and forces rounding to 0, blur off, alpha and fadeAlpha to 1, and decorations off. Popups are included but clipped to the buffer.
  - Buffer size is `round(window logical size × monitor scale)`.
  - The frame is copied at the window's monitor commit, and only if the window box intersects that monitor.
  - If the window itself has `no_screen_share`, you get a "screencopy denied" texture.
- **Occluded or other-workspace windows** **[src + infer]**:
  - **Screenshot:** works. The first frame of a new session calls `scheduleFrame` and `damageMonitor`, so a commit is forced.
  - **Continuous capture:** later frames only arrive when that monitor commits for some other reason. Hidden-window commits probably don't damage the monitor, so expect stalls on an idle monitor.
  - **Stale content:** a hidden client may show an old buffer. Hyprland does unblock frame callbacks for invisible captured windows (`m_bBlockSurfaceFeedback = shouldRenderWindow(w)`), so the client does get frame callbacks and keeps drawing.
- **Cursor control.**
  - The flag is `overlay_cursor` for screencopy and toplevel export, and the `paint_cursors` option for ext.
  - An ext cursor session gives you the cursor image, hotspot and position separately. That lets you draw your own cursor, enlarge it or follow it. It is gated by the `cursorpos` permission, and positions update on each monitor commit.
  - **Quirk:** Hyprland sends `position = Pointer::position() - sourceBox.pos()` in **logical** coordinates. The spec asks for buffer pixels, so multiply by the scale and verify.
  - **Caveat:** when Hyprland uses a **software** cursor, the cursor is already baked into the mirror texture, so monitor captures can't exclude it. Window captures are unaffected. Omarchy works around this by forcing `cursor:no_hardware_cursors=0` during capture. On this machine `no_hardware_cursors=2` (auto: off only when tearing) and `use_cpu_buffer=2` (auto: on for NVIDIA), so hardware cursors normally work **[local hyprctl getoption]**.
- **Formats.**
  - Hyprland offers two formats: the alpha variant and the opaque variant of the monitor format, for example ARGB8888 and XRGB8888.
  - Screencopy and toplevel export advertise only `formats[0]` (the alpha one) for both shm and dmabuf.
  - ext advertises both, with the renderer's modifier list and a `dmabuf_device`.
  - **10-bit:** `misc:screencopy_force_8b` defaults to **true** (it is true here), so 10-bit monitors are captured as 8-bit. If a user sets it to false, Hyprland offers `XBGR2101010` (a format-flip hack), and your code must handle that.
  - Output is always converted to sRGB for clients that are not colour-management-aware (`cmBackToSRGB`), so HDR desktops come out tone-mapped SDR.
- **Damage.** `// TODO: add a damage ring` means the reported damage is always the full frame. wlr `copy` (without damage) forces a full monitor repaint. `copy_with_damage` and ext wait for the next natural commit, so on a static screen no frame arrives. That is good for variable frame rate, but **a screenshot that reuses a session within 500 ms can hang on a static screen**, because the "first frame forces a render" logic only applies when `m_sharing` is false, and that flag resets after 500 ms. **Create a fresh session for each screenshot.**
- **Side effects while a capture session is active:** direct scanout is blocked, each frame gets an extra blit into the mirror framebuffer, and `screencast` / `screencastv2` events are sent on socket2. The permission system (`ecosystem:enforce_permissions`, false here) would show a prompt for `screencopy` and `cursorpos`.
- **Presentation timestamp** is `steadyNow()` at copy time, not the page flip. It is CLOCK_MONOTONIC and good enough for A/V sync.

**shm vs dmabuf latency for a 4K frame** (estimates **[infer]**, 33.2 MB for 3840×2160 BGRA; your 3440×1440 is 19.8 MB)

| Path | Wait for capture point | Copy cost | Compositor impact |
|---|---|---|---|
| dmabuf | up to 1 refresh (6.9 ms at 144 Hz, 16.7 ms at 60 Hz) | GPU blit, about 0.1–0.3 ms on a dGPU, about 0.5–1 ms on an iGPU; completes asynchronously | negligible |
| shm | same | synchronous `glReadPixels` on the compositor thread, about 4–12 ms on a dGPU and 5–15 ms on an iGPU | causes a dropped frame or hitch; continuous shm capture stutters (wl-screenrec measured wf-recorder at about 500% CPU for 4K60) |

Encoding dominates the total for screenshots: zlib level 6 PNG of 5–8 MP takes roughly 100–400 ms. Use a fast deflate (zlib-ng level 1–2, fdeflate or mtpng), or encode lazily in the background.

**Recommendation**
- **Screenshots:** use ext-image-copy-capture with **shm**, one fresh session per output, all outputs captured in parallel, cursor off. It is simple, robust on multi-GPU setups, and gives CPU pixels for crop, OCR and PNG. Fall back to wlr-screencopy. Crop in **physical pixels in your own process** instead of using `capture_output_region`.
- **Window screenshots:** use an ext toplevel source, mapped with hyprland-toplevel-mapping.
- **Continuous capture:** use dmabuf only.
- **Libraries:**
  - libwayshot 0.9.0 (Aug 2026): Rust, toplevel and screencast modules, dmabuf with EGL. Its crates.io description still says screencopy-only, so check its ext coverage.
  - grim's C code is a good minimal reference (ext first, shm, pixman composition at the greatest output scale).
  - Quickshell `ScreencopyView` already does ext, screencopy and toplevel capture with live dmabuf.
  - **hyprpicker's freeze approach** is screencopy into shm, shown on per-output overlay layer surfaces with fractional scale and viewporter. Omarchy runs it as a separate process and then waits `sleep .1`, which is a race. Your in-process freeze removes that race.

## 2. Freeze screen and selection overlay

**Recommended flow**
1. On the hotkey, the already-running daemon captures every output (shm, no cursor).
2. Create one `zwlr_layer_surface_v1` per output: layer `overlay`, anchored to all four edges, `exclusive_zone = -1` so it also covers the bar, `keyboard_interactivity = exclusive`.
3. Wait for each `configure`, attach the frozen buffer **before the first commit**, then commit all outputs in the same event-loop turn. Never commit an empty buffer first.
4. Use a static subsurface for the frozen image. Draw the dim areas and the selection rectangle with **`wp_single_pixel_buffer_v1` + viewporter subsurfaces** (supported here), so the compositor does that work. Only the small label and loupe surface needs redrawing on pointer motion. At 3440×1440@144 Hz this avoids re-uploading about 20 MB per motion event.
5. Use `wp_cursor_shape` crosshair. Crop from the frozen buffer, never re-capture, so your own overlay can never end up in the screenshot.

**Instant appear**
- Omarchy animates layers: `layersIn` is a 400 ms fade and `layersOut` is a 150 ms fade **[local looknfeel.lua]**.
- Add `hl.layer_rule({ match = { namespace = "^openshotx-" }, no_anim = true, animation = "none" })`, the same as Omarchy does for `selection`.
- You can ship that rule as a Lua snippet the user `require`s, or inject it with `hyprctl eval` at daemon start. Rules added with eval disappear on config reload, so re-add them on the socket2 `configreloaded` event or the Lua `config.reloaded` event.
- Without the rule, the fading-out overlay ends up in the first frames of a recording.

**Multi-monitor**
- Because of finding 6, an exclusive layer surface takes pointer focus everywhere.
- If only one monitor's overlay is exclusive, pointer events on the other monitor fall back to the **first exclusive layer surface, with coordinates relative to that surface** (out of range).
- So **make every per-output overlay exclusive.** Hit-testing then picks the right one. Route keyboard input centrally, because focus lands on only one of them.
- Hyprland binds still fire before your surface gets the key, so pressing PRINT again will re-trigger your hotkey. Treat that as cancel or cycle-mode, as Omarchy does with `pkill slurp && exit 0`.

**Exact mapping between logical and physical coordinates**
- Use `wp_fractional_scale_v1.preferred_scale`, which is N/120 (1.25 = 150, 1.5 = 180, 5/3 = 200). The buffer is the physical size and the viewport destination is the logical size.
- Hyprland only accepts scales that give integer logical monitor sizes **[wiki positioning.md]**, so full-output overlays map 1:1. For example, 3440×1440 at 1.25 is exactly 2752×1152.
- Pointer coordinates are `wl_fixed` (1/256 of a logical pixel). Compute physical positions as `round(x·N/120)` and keep the **selection in physical pixels**.
- Different tools disagree by one pixel:

| Scale | Logical x, w | Edge rounding (Hyprland region) | Rounding x and w separately | Truncation (gsr) |
|---|---|---|---|---|
| 1.25 | y=37, h=77 → 46.25…142.5 | 46, h=97 | 46, h=96 | 46, h=96 |
| 1.5 | 3, 5 → 4.5…12 | 5, w=7 | 5, w=8 | 4, w=7 |
| 5/3 | 2, 1 → 3.33…5 | 3, w=2 | 3, w=2 | 3, w=1 |

- At a scale of 5/3, integer logical coordinates **cannot reach every physical pixel** (1, 4, 6 and so on). So any protocol or tool that takes integer logical regions (screencopy region, gsr, slurp, grim `-g`) cannot express an arbitrary physical rectangle.
- **Snap-to-window** has to round window rectangles the way Hyprland draws them: `(at − monitorPos)·scale`, then `CBox::round()`.
- **Transforms:** the captured buffer is in the panel's native orientation, and a `transform` event follows. Display it with `wl_surface.set_buffer_transform(output transform)` and map selections through the inverse transform. Odd transforms swap width and height. All 8 transform values (0–7), including the flipped ones, are possible.
- **Negative coordinates:** layer surfaces use output-local coordinates. Only conversions to and from `hyprctl` or gsr global coordinates need signed arithmetic. Use floor-division for negative values.
- **Mixed scales:** limit a selection to one monitor, which gsr and wl-screenrec also require. For screenshots that span monitors, composite at the largest scale as grim does, and accept resampling on the lower-scale part.
- **HDR monitors:** the frozen image is sRGB. On an HDR monitor, HDR content (video, games) looks different during the freeze.
- **Software cursor:** if it is active, it will be baked into the freeze image.

## 3. Hiding our own UI from captures and recordings

- **Window rule `no_screen_share`** and **layer rule `no_screen_share`** (PR #11664, Sep 2025; popup fix #12260) cause black rectangles in all Wayland captures. A box is the window's current box (minimum 5 px), with rounding except in region captures, plus its popups **[src renderMonitor; wiki window-rules/layer-rules]**. It covers grim, your ext pipeline, XDPH portal casts (OBS, browsers, gsr `-w portal`) and toplevel export.
- **gsr with KMS: no effect.** KMS reads the scanout plane, which Hyprland composited without black boxes. This is reasoning from the code (black boxes exist only in `CScreenshareFrame`), but it is solid.
- **Transparent exclusion is not available.** PR #11572 (multiple render targets) is still open, last active 2026-09-03.

Consequences for each element:

| Element | Wanted in recording? | Recommendation |
|---|---|---|
| Recording controls toolbar | No | Place it **outside the region**, or on another monitor. For full-monitor recordings, use no toolbar and rely on a hotkey plus the Omarchy bar indicator. Never put a `no_screen_share` toolbar inside the region (it shows as a black box, and KMS records it anyway). |
| Dim area outside the region | No | It sits outside the crop. Keep a margin of at least 1–2 physical pixels and draw the border *outside* the region, because 1 px rounding differences (table in §2) otherwise show as a dark edge. Do not mark the full-screen dim layer `no_screen_share`, or the whole monitor turns black in Wayland captures. |
| Keystroke and click overlay | Yes | Overlay layer, `keyboard_interactivity=none`, empty input region, `no_anim`. Or burn it in without showing it on screen (§9 and §10). |
| Webcam overlay | Yes | Floating, pinned window; no `no_screen_share`. |
| Pinned screenshots | Optional | To exclude them from a screenshot, set them to opacity 0 for one frame with `hyprland_surface_v1.set_opacity(0)` (supported here), capture, then restore. Using `no_screen_share` would give black boxes. |

## 4. Global hotkeys

- **`exec` bind:** Hyprland runs `sh -c`, which starts your CLI, which then sends an IPC message to the daemon. That costs about 3–10 ms for a native binary and 50–150 ms for Python or Node **[infer]**. `hl.dsp.exec_raw` skips `sh`.
- **`hyprland_global_shortcuts_v1` (recommended on Hyprland)** **[src GlobalShortcuts.cpp]**:
  - The daemon calls `register_shortcut(id, app_id, description, trigger_description)` and receives `pressed` and `released` events with timestamps over Wayland. That is under 1 ms with no spawn.
  - The user binds `hl.bind("PRINT", hl.dsp.global("openshotx:area"))`, and `hyprctl globalshortcuts` lists your entries.
  - Press and release events allow hold-to-record and push-to-talk behaviour.
  - Hyprland's duplicate check (`isTaken(id, app_id)`) is called with its arguments swapped, so it doesn't protect against duplicates. Register each shortcut once per connection.
- **Portal GlobalShortcuts:** XDPH implements it on top of the same protocol. Use it on other desktops (KDE, GNOME). It costs a D-Bus round trip and a binding dialog.
- **Lua bind with `hl.dsp.event("openshotx>>…")`:** delivers `custom>>…` on socket2. This is also fast, but every socket2 listener sees it.
- **Conflicts:** Omarchy binds PRINT, ALT+PRINT, SUPER+PRINT and SUPER+CTRL+PRINT **[local bindings/utilities.lua]**. Ship an opt-in Lua module that unbinds or rebinds them.
- **Scoped binds while the overlay is up:** Omarchy adds binds in `hl.on("layer.opened")` for the `selection` namespace and removes them on close using handles. Your exclusive overlay gets keys directly, so you rarely need that. You do need it during scrolling capture (§8).
- Avoid a catch-all submap. If the daemon dies, the user is stuck in it.
- **Never keep `.socket.sock` open.** Hyprland serves it synchronously and freezes for up to 5 s **[wiki IPC]**. Open it, send, close.

## 5. Clipboard

- Both `zwlr_data_control_v1` and `ext_data_control_v1` exist **[local]**. Use **ext-data-control in the daemon**: it can set the selection without focus and serves `send(mime, fd)` requests with non-blocking writes.
- GTK and Qt clipboards (`wl_data_device`) need a focus serial, so copying from an unfocused daemon silently fails.
- `wl-copy` (2.3.0, also ext-data-control) forks a server and stays alive, but it offers one type per call (plus text aliases).
- **MIME types:**
  - Default copy: **`image/png` only.** This works for Chromium web apps, Electron apps (Slack, Discord), Firefox paste, Telegram, GIMP and LibreOffice.
  - Separate "Copy as file" action: `text/uri-list` (`file:///…` with percent-encoding and CRLF) plus `x-special/gnome-copied-files` (`copy\nfile:///…`, for Nautilus, Nemo and Thunar). Optionally add `text/plain;charset=utf-8` with the path.
  - Mixing image and text types is risky. Some Electron apps and terminals prefer the text type, so the user gets the path pasted instead of the image **[infer — needs a test matrix]**.
- **Omarchy's clipboard manager** runs two `wl-paste --watch` processes (types `text` and `image/png`), so it reads your PNG as soon as you copy. It also stores a copy in `~/.local/state/omarchy/clipboard-images/` **[local plugins/clipboard]**. Its `capture.sh` skips entries that carry `x-kde-passwordManagerHint`, so you can offer that type for a "don't keep in history" mode. Your source must handle several concurrent `send` requests.
- The selection is lost when your process exits. The daemon lifetime normally covers this; otherwise hand off to a forked child.

## 6. Drag and drop

- **Starting a drag:** `wl_data_device.start_drag` from a layer surface or a toplevel works. Hyprland does not validate the serial or the type of the origin surface **[src DataDevice.cpp]**. Start it from a real button press.
- **Action:** set **COPY only.** Hyprland pre-selects MOVE whenever the source allows it, which could make Nautilus *move* your saved screenshot.
- **Chromium and Electron receivers** accept `text/uri-list` (as files), `text/plain`, `text/html`, `text/x-moz-url`, `application/octet-stream;name=…`, and the portal file-transfer types. **They do not accept `image/png`** **[src Chromium wayland_exchange_data_provider.cc]**.
- **What to offer:** `text/uri-list`. For Flatpak receivers also offer **`application/vnd.portal.filetransfer`**, with the key from `org.freedesktop.portal.FileTransfer.StartTransfer` / `AddFiles`. A sandboxed app can't read your path otherwise.
- **File lifetime:**
  - Keep the file alive long after the drop, because receivers such as Discord or Nautilus copy asynchronously.
  - Store drag files in a persistent cache, not `$XDG_RUNTIME_DIR`, which is tmpfs and 0700.
  - Keep the data source alive until `dnd_finished` or `cancelled`, and expect repeated `send` calls (some receivers ask on enter).
- XWayland receivers are handled by Hyprland's XWayland DnD bridge.
- Drops cannot reach other apps while any exclusive overlay is mapped, because of finding 6. Make the floating thumbnail or quick-access window non-exclusive.

## 7. Pinned screenshots

**Recommended: an `xdg_toplevel` tagged with `xdg_toplevel_tag_v1`.** Hyprland supports the protocol, and rules can `match = { xdg_tag = "^openshotx-pin$" }` **[wiki]**. Suggested rule:
`float=true, pin=true, border_size=0, no_shadow=true, no_blur=true, no_dim=true, no_anim=true, rounding=0, opacity="1 override 1 override", tag="-default-opacity", suppress_event="maximize fullscreen", keep_aspect_ratio=true`.

Omarchy adds `+default-opacity` (0.985/0.96) to every window, so the opacity override is required **[local windows.lua]**.

**Behaviour**
- **Over fullscreen windows:** pinned floating windows are drawn over fullscreen windows, because `isAllowedOverFullscreen()` is true when the window is pinned **[src Window.cpp, Renderer.cpp]**.
- **Workspaces:** "pin" means every workspace of *that monitor*. To show a pin on every monitor you need one window per monitor.
- **Moving and resizing:** the user can drag with `xdg_toplevel.move` / `.resize`, which Hyprland handles, including across monitors.
- **Programmatic position:** clients can't set their own position. Map the window, then call `hl.dsp.window.move` by address (from hyprland-toplevel-mapping), and draw the first frame transparent to hide the jump. An alternative is a Lua `window.open_early` hook that reads a pending-position table you set by eval.
- **Click-through:** set an empty input region and restore it with a global hotkey. Whether Hyprland honours empty input regions on toplevels is **[infer — test]**.
- **Layer-shell alternative:** works on all workspaces of an output and above fullscreen (on the overlay layer), and is not listed by `hyprctl clients`. But there's no compositor move or resize (you change margins on each commit), dragging to another output means recreating the surface, keyboard focus needs `on_demand`, and stacking control is poor. Keep it as a fallback.

## 8. Scrolling capture

- **Synthetic scrolling:** `zwlr_virtual_pointer_v1` is supported, including `axis_discrete` (multiplied by 120). Hyprland also generates discrete steps from smooth-only wheel input (15 units per 120-unit detent) **[src VirtualPointer.cpp, InputManager.cpp]**.
  - Flow: `motion_absolute` into the region (this moves the real cursor), then `axis_source(wheel)` + `axis_discrete` + `frame`.
  - `uinput` or `ydotool` would need `/dev/uinput` access and the ydotoold daemon. Avoid them.
  - There is no scroll dispatcher. `hl.dsp.send_shortcut({window=…, key="Next"})` (PageDown) can scroll an unfocused window via the keyboard, but the result depends on the app.
- **Avoiding stolen input:** while any exclusive overlay is mapped, injected wheel events go to *your* surface. During scrolling capture:
  - Use a HUD with `keyboard_interactivity=none` and an empty input region, outside the region.
  - Get Esc and Enter through temporary binds (`hl.bind(...)` handles added by eval, or `hl.dsp.global`), removed afterwards.
  - Prefer **capturing the toplevel source**. Its frames never contain your UI and work even if it's partly covered. Offer manual scrolling by the user (the CleanShot default) and auto-scroll.
- **Settle detection:** damage is always full, so you can't use it. Use two signals together:
  - The next ext frame only arrives on a commit, so no frame for about 80–120 ms suggests the content has stopped.
  - Diff a downscaled image against the previous frame (blinking carets and animations defeat the first signal).
  - Smooth scrolling in Firefox and Chromium takes about 150–300 ms per wheel step.
- **Stitching:** use row-hash or phase correlation for the offset, and detect static header and footer bands (sticky elements) and exclude them.
- **Side effects:** with Omarchy's focus-follows-mouse, warping the cursor moves keyboard focus. `cursor:hide_on_key_press` is on here.

## 9. Recording pipelines

| Pipeline | Capture path | NVIDIA encode | Intel/AMD encode | Respects `no_screen_share` | HDR | Prompts or permissions | Notes |
|---|---|---|---|---|---|---|---|
| **gsr 6.1.0, KMS** (Omarchy default) | DRM planes via `gsr-kms-server` (cap_sys_admin, set here), zero-copy | NVENC (CUDA) | VAAPI | **No** | yes (hevc_hdr, av1_hdr) | none | Region is logical and global; monitor picked by the region centre; scaled and **truncated** to physical; one monitor only; dims rounded up to even (`FFALIGN(…,2)`, 64×16 for some AMD codecs). Captures the cursor plane, records the lock screen, can't capture monitors on another GPU. |
| **gsr, portal** | XDPH → wlr-screencopy / toplevel export → PipeWire dmabuf | NVENC | VAAPI | yes (black) | no | picker dialog; restore token (Omarchy sets `allow_token_by_default=true`; gsr `-restore-portal-session yes`) | XDPH 1.4.1 lacks the 2026-08-29 multi-plane fix (fails on Intel CCS / AMD DCC modifiers); `max_fps` defaults to 120; `-fm content` only works here; window capture. |
| **wl-screenrec 0.3.1** | ext-image-copy-capture dmabuf, zero-copy | **No** (VAAPI only; Vulkan experimental) | VAAPI | yes | no | none | About 2.5% CPU vs about 500% for wf-recorder at 4K60 (their benchmark); Mesa VAAPI can't handle rotated outputs; one display only. |
| **wf-recorder 0.6.0** | wlr-screencopy, shm by default | CPU or VAAPI | VAAPI | yes | no | none | High CPU, stutter. |
| **Own pipeline** (ext dmabuf → GL/Vulkan compositing → NVENC via CUDA interop or Vulkan Video; VAAPI on Mesa; libavcodec/libavformat 9.0) | ext | yes | yes | yes | no (sRGB) | none | Exact physical region, burned-in overlays, custom cursor from the cursor session. Large effort (gsr is a big codebase). |
| **Own portal + PipeWire** (GStreamer 1.28 `pipewiresrc` → `nvh264enc` / `vulkanh264enc` / `vah264enc`) | XDPH | yes | yes | yes | no | dialog | Portable to KDE and GNOME; dmabuf→CUDA path on NVIDIA **[infer]**. |

**Recommendation**
- **Phase 1:** drive gsr as a subprocess.
  - Start with `-ipc $XDG_RUNTIME_DIR/openshotx-gsr.sock`. Use the JSON requests `stop` (it replies once the file is saved), `set-paused true/false` and `save-replay`. SIGUSR2 and SIGINT also work.
  - Use `-write-first-frame-ts yes` to get a monotonic first-frame timestamp for aligning keystroke and click logs. Use `-v no`.
  - KMS is the fast default (with HDR). Provide a **"privacy-safe" mode** (portal or own pipeline) that respects `no_screen_share`.
  - For webcam: show it on screen (what you see is what's recorded), or composite it with `-w "region|/dev/video0;halign=end;valign=end;width=25%…"`. A V4L2 device is usually exclusive to one process, so it can't be both on screen and composited by gsr unless you share it through a PipeWire camera node.
- **Phase 2:** build the own ext-based pipeline if burned-in overlays, cursor effects or exact pixels matter.

**Omarchy interop**
- The bar's ScreenRecording indicator runs `pgrep -f ^gpu-screen-recorder`. Clicking it, or ALT+PRINT, runs `omarchy-capture-screenrecording --stop-recording`, which SIGINTs **every** gsr process. Your controller must handle being stopped externally.

**Region, frame rate, HDR and audio**
- **Even sizes:** choose region dimensions that are even in physical pixels yourself, so gsr's `FFALIGN` doesn't pad an extra row or column.
- **Frame rate:** Hyprland frames are driven by damage, so variable frame rate is natural. Offer a constant-frame-rate export for editors (Omarchy uses `-fm cfr -f 60`). Cap at 60 by default; NVENC on a 4080 can do 3440×1440@144. iGPUs at 4K144 probably cannot **[infer]**.
- **HDR:** only gsr KMS with `hevc_hdr` or `av1_hdr`. Wayland capture is tone-mapped to sRGB.
- **Audio:**
  - gsr `-a "default_output|default_input"` mixes into one track. Separate `-a` flags give separate tracks, but most players only play one.
  - Opening the PipeWire capture causes a pop: a near-clipping transient around 130–200 ms. Omarchy hard-mutes the first 400 ms **[local omarchy-capture-screenrecording]**. Better: open the audio streams before video starts and drop the warm-up.
  - **Bluetooth headsets:** opening the mic switches to the HFP profile, which degrades system audio during the recording. Warn the user.
  - Clock drift is absorbed by PipeWire's graph resampling. Sync with monotonic timestamps.
- **GIF:** record MP4 first, then convert (ffmpeg palettegen/paletteuse; ffmpeg has `gif` and `libwebp_anim` encoders), capping fps (15–20) and width.

## 10. Keystroke display and click highlights

| Option | Works? | Privilege | Notes |
|---|---|---|---|
| **Lua hook (recommended)** | Keys: yes. Clicks: likely (via bind). | None beyond `hyprctl eval` | `input.keyboard.key` fires for every key **before** bind processing (also virtual keyboards and the lock screen), with an XKB keycode, a ms timestamp and state 0/1/2 **[src InputManager.cpp, LuaEventHandler.cpp]**. Hyprland-only; the API is only months old. |
| Hyprland plugin (hyprpm) | yes (full event bus, mouse) | plugin permission | Must be rebuilt for every Hyprland ABI change, which Omarchy updates often. Keep as a fallback for mouse buttons. |
| evdev via `input` group | yes | permanent raw access to all input for every user process | Bad security trade-off; needs a re-login. |
| Privileged helper (systemd + polkit, logind active-session check, filtered events) | yes, works on any compositor | root service | Most portable, most to ship and audit. |
| socket2 / `hyprctl cursorpos` polling | no keys or clicks; position only | — | Polling the synchronous socket at 60–144 Hz costs compositor time. |
| ext cursor session | position and image only | `cursorpos` | Cheap passive position stream (logical-coordinate quirk). |
| InputCapture portal / `hyprland_input_capture_v1` | **no** | — | Exclusive barrier capture (input-leap style), not passive monitoring **[src InputCapture.cpp]**. |

**Sketch of the Lua hook** (unverified; install at record start with `hyprctl eval`, remove at stop):
```lua
OSX = OSX or {}
if OSX.k then OSX.k:remove() end
OSX.k = hl.on("input.keyboard.key", function(code, t, st)
  if OSX.on then hl.dispatch(hl.dsp.event("openshotx-key:"..code..","..st..","..t)) end end)
OSX.c = hl.bind("mouse:272", function() local p = hl.get_cursor_pos()
  hl.dispatch(hl.dsp.event("openshotx-click:"..p.x..","..p.y)) end,
  { non_consuming = true, ignore_mods = true, transparent = true })
```
- The daemon reads `custom>>openshotx-…` from socket2. It turns keycodes into keysyms with xkbcommon, using the keymap its own `wl_keyboard` receives, and tracks `activelayout`.
- `hl.on` returns a subscription with `:remove()`, and `hl.bind` returns a handle with `:remove()` / `:unbind()` **[local stubs]**.
- **Safety:**
  - Add a Lua `hl.timer` watchdog that removes the hooks if the daemon's heartbeat (sent by eval) goes stale. A stuck mouse bind is the worst failure mode.
  - Re-install after a config reload.
  - Default to showing only modifier combos and special keys, and suppress output when a password-manager window is focused or the session is locked.
  - socket2 broadcasts to every listener the same user runs (that user could already read the Hyprland config anyway). The more private alternative is `io.open` appending to a 0600 file in `$XDG_RUNTIME_DIR`: Hyprland calls `luaL_openlibs`, so `io` is available. It is a tmpfs write on the compositor thread. Never write to a FIFO there: it blocks with no reader, and SIGPIPE is a risk.
- **Rendering:**
  - On screen: an overlay layer with no input, which KMS and screencopy both record.
  - Burned in only: a gsr plugin (`-p`, `/usr/include/gsr/plugin.h`: GLES `draw(width,height)`, `is_damaged`) fed events by shared memory.
  - Or re-encode after recording using the event log and the first-frame timestamp (NVENC re-encode takes seconds).

## 11. Window capture with transparency, corners and shadow

- **What the capture gives you** **[src]**: the client's own buffer with alpha preserved (for example a translucent Ghostty), cleared to transparent. Hyprland's border and shadow are not included, corners are **square** (rounding is forced to 0), and the opacity and dim rules (including Omarchy's default-opacity) are not applied.
- **Client-side decorations:** GTK4/libadwaita rounded corners come out transparent. Shadow pixels outside the window geometry are clipped.
- **Popups:** included but clipped to the window size. XWayland override-redirect menus are not included.
- **Building the macOS look yourself:**
  1. Read `decoration:rounding` and `rounding_power` (here 10 and 2.5, a superellipse; Omarchy's default rounding is 0). Mask the corners with radius×scale.
  2. Pad the image by about 2× the blur radius.
  3. Shadow: Gaussian blur of the alpha (three box-blur passes ≈ Gaussian; about 10–20 ms on the CPU for 2 MP **[infer]**). Roughly radius 40–60 px@1x, y-offset 12–20 px, opacity 0.35–0.5, plus a 1 px outline at about 10% black.
  4. Export PNG with alpha.

## 12. Do-not-disturb, bar and cursor during recording

- **DND:** Omarchy shell IPC is idempotent: `omarchy-shell notifications isDnd` and `setDnd on|off` **[local Service.qml]**. Save the previous state and restore it. Don't use the toggle script.
- **Bar:** `omarchy-toggle-bar off|on` changes the reserved space, so tiled windows re-layout and content moves mid-recording. Avoid it for region recordings.
- **Cursor:** gsr `-cursor no`, or no `paint_cursors` in your own pipeline. Don't use global `cursor:invisible`, because the user loses their cursor.

## 13. NVIDIA (RTX 4080, nvidia-open 610.57) vs Intel/AMD (Mesa 26.2)

**NVIDIA**
- No VAAPI encode, so use NVENC (CUDA) or Vulkan Video. `h264_vulkan`, `hevc_vulkan` and `av1_vulkan` are in ffmpeg 9 here; GStreamer has `nvh264enc`, `nvautogpu*` and `vulkanh264enc` on the 4080 **[local]**.
- NVENC downclocks memory ("CUDA P2 state"). gsr installs the app profile `~/.nv/…/10-gsr-cuda-no-stable-perf-limit` (needs driver 580+; present here).
- CUDA breaks after suspend. `/usr/lib/modprobe.d/gsr-nvidia.conf` sets `NVreg_PreserveVideoMemoryAllocations=1` **[local]**.
- Hardware cursors work through `use_cpu_buffer=auto`. Tearing or `no_hardware_cursors=1` makes the cursor software, so it gets baked into captures.
- Explicit sync: `linux-drm-syncobj-v1` is present. For your GL/Vulkan surfaces use EGL/WSI (egl-wayland 1.1.22 and egl-wayland2 are installed). Capture dmabufs signal `ready` after the GPU work completes **[src endRender callback]**.
- Allocate capture buffers with the modifiers the compositor advertises, through GBM (`15_nvidia_gbm.json`). Hybrid laptops need a LINEAR transfer between GPUs, or encoding on the GPU that drives the display (gsr requires capture and encode on the same GPU).

**Intel/AMD**
- VAAPI encode is available. wl-screenrec works there, except on rotated outputs.
- AMD: AV1 64×16 padding (hardware), and the HEVC black-bar issue fixed in ffmpeg ≥8. Cursor flicker with KMS on some generations (the portal avoids it). The `-low-power` option.
- Compressed modifiers (CCS/DCC) are multi-plane, which triggers the XDPH 1.4.1 portal bug.
- iGPU shm readback is cheaper, but 4K144 encoding is borderline.

## Recommended architecture

A long-running native daemon (Rust or C++) with:
- ext-image-copy-capture (fallback wlr-screencopy) + ext-foreign-toplevel-list + hyprland-toplevel-mapping
- layer-shell + fractional-scale + viewporter + single-pixel-buffer + cursor-shape
- ext-data-control
- wlr-virtual-pointer
- hyprland-global-shortcuts
- hyprland-surface (opacity) + xdg-toplevel-tag
- linux-dmabuf
- the Hyprland sockets (eval to add rules and hooks, socket2 for events)

It drives gsr for recording in Phase 1. The editor and pinned windows are separate GTK4 (4.22, fractional scaling) or Qt 6.11 / QML toplevels.

## Things to test on hardware before committing (I could not run captures)

1. shm vs dmabuf full-frame latency on DP-2 and on an Intel/AMD box, plus compositor frame-time hitch.
2. Whether a reused session within 500 ms hangs on a static screen.
3. Continuous capture of a hidden-workspace window.
4. Cursor-session coordinates at a scale of 1.25.
5. A `non_consuming` `mouse:272` Lua bind (does it pass through, and does it break drag binds), and that `hyprctl eval` hooks survive until a reload.
6. Empty input region click-through on toplevels.
7. Clipboard paste matrix (Chromium, Firefox, Slack, Discord, Nautilus, terminals) for each MIME combination; DnD into Flatpak apps.
8. gsr portal on Intel/AMD with XDPH 1.4.1 vs a build with commit ba31964.
9. Night light (hyprsunset/CTM) and zoom (`cursor:zoom_factor`) appearing in captures.
10. Keystroke Lua hook overhead and behaviour at the lock screen.

## Sources

**Hyprland v0.56.2 source**
- [ScreenshareFrame.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/managers/screenshare/ScreenshareFrame.cpp), [ScreenshareSession.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/managers/screenshare/ScreenshareSession.cpp), [ScreenshareManager.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/managers/screenshare/ScreenshareManager.cpp)
- [ImageCopyCapture.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/ImageCopyCapture.cpp), [ImageCaptureSource.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/ImageCaptureSource.cpp), [Screencopy.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/Screencopy.cpp), [ToplevelExport.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/ToplevelExport.cpp)
- [GlobalShortcuts.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/GlobalShortcuts.cpp), [VirtualPointer.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/VirtualPointer.cpp), [InputCapture.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/InputCapture.cpp), [core/DataDevice.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/protocols/core/DataDevice.cpp)
- [InputManager.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/managers/input/InputManager.cpp), [Renderer.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/render/Renderer.cpp), [Monitor.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/output/Monitor.cpp), [LuaEventHandler.cpp](https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/config/lua/LuaEventHandler.cpp)
- [hyprutils Box.cpp](https://github.com/hyprwm/hyprutils/blob/main/src/math/Box.cpp)

**Hyprland PRs and discussions**
- [#11709 ext-image-copy-capture](https://github.com/hyprwm/Hyprland/pull/11709), [#11664 layer noscreenshare](https://github.com/hyprwm/Hyprland/pull/11664), [#12260 noscreenshare popups](https://github.com/hyprwm/Hyprland/pull/12260), [#11572 transparent noscreenshare (open)](https://github.com/hyprwm/Hyprland/pull/11572), [#14779 Lua key event](https://github.com/hyprwm/Hyprland/pull/14779), [discussion #14136](https://github.com/hyprwm/Hyprland/discussions/14136)

**Hyprland wiki (GitHub source)**
- [layer-rules](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/rules/layer-rules.md), [window-rules](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/rules/window-rules.md), [events](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/advanced-configuration/events.md), [lua-utilities](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/advanced-configuration/lua-utilities.md)
- [dispatchers](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/dispatchers.md), [bind flags](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/binds/flags.md), [globals](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/binds/globals.md)
- [IPC](https://github.com/hyprwm/hyprland-wiki/blob/main/content/ipc/_index.md), [permissions](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/advanced-configuration/permissions.md), [config-options](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/config-options.md), [monitor positioning](https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/monitors/positioning.md), [XDPH](https://github.com/hyprwm/hyprland-wiki/blob/main/content/hypr-ecosystem/user/xdg-desktop-portal-hyprland.md)

**Protocol specs (wayland.app)**
- [ext-image-copy-capture-v1](https://wayland.app/protocols/ext-image-copy-capture-v1), [ext-image-capture-source-v1](https://wayland.app/protocols/ext-image-capture-source-v1), [wlr-screencopy](https://wayland.app/protocols/wlr-screencopy-unstable-v1), [hyprland-toplevel-export](https://wayland.app/protocols/hyprland-toplevel-export-v1)
- [wlr-layer-shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1), [fractional-scale](https://wayland.app/protocols/fractional-scale-v1), [ext-data-control](https://wayland.app/protocols/ext-data-control-v1), [wlr-virtual-pointer](https://wayland.app/protocols/wlr-virtual-pointer-unstable-v1)

**Portal**
- [XDPH multi-plane fix ba31964](https://github.com/hyprwm/xdg-desktop-portal-hyprland/commit/ba31964ee42b56bcb0d3b78a64ead5d8a1c3c6f6), [Arch PKGBUILD (1.4.1, no patch)](https://gitlab.archlinux.org/archlinux/packaging/packages/xdg-desktop-portal-hyprland/-/blob/main/PKGBUILD)

**Recorders and capture libraries**
- [gpu-screen-recorder README](https://git.dec05eba.com/gpu-screen-recorder/about/), plus the local man page and `/usr/include/gsr/plugin.h`
- [wl-screenrec](https://github.com/russelltg/wl-screenrec), [wf-recorder releases](https://github.com/ammen99/wf-recorder/releases), [nvidia-vaapi-driver (decode only)](https://github.com/elFarto/nvidia-vaapi-driver)
- [libwayshot docs](https://docs.rs/libwayshot), [Quickshell ScreencopyView](https://quickshell.org/docs/types/Quickshell.Wayland/ScreencopyView/)

**Receivers and other**
- [Chromium Wayland DnD MIME handling](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/ui/ozone/platform/wayland/host/wayland_exchange_data_provider.cc), [FileTransfer portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileTransfer.html)
- [Weston MR 356 (shm screenshot cost)](https://gitlab.freedesktop.org/wayland/weston/-/merge_requests/356)

**Local files**
- `/usr/share/omarchy/bin/omarchy-capture-{region,screenshot,screenrecording,text}`
- `/usr/share/omarchy/default/hypr/{bindings/utilities.lua,bindings/clipboard.lua,apps/*.lua,windows.lua,looknfeel.lua,nvidia.lua}`
- `/usr/share/omarchy/shell/plugins/{clipboard,notifications,bar/indicators/ScreenRecording.qml}`
- `/usr/share/hypr/stubs/hl.meta.lua`, `/usr/lib/modprobe.d/gsr-nvidia.conf`, `~/.config/hypr/xdph.conf`

Scratch copies of the source and wiki files I read are in `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/` (`hl/`, `wiki/`, `gsr/`).
