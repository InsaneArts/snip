# Snip M4 research brief: recording without Studio Mode (CleanShot X 4.x parity)

2026-10-09. This brief merges six research reports into one: CleanShot 4.x parity, gpu-screen-recorder, Qt 6.11 Multimedia, ffmpeg recipes, Omarchy integration, and Hyprland input with window recording. It also includes facts checked while merging them. It replaces the six reports. The reports are also committed in full under `docs/research/recording-2026-10/`.

**Confidence markers**
- **[V]** verified locally: run or measured on this machine. For CleanShot rows, [V] means read statically from the 4.7.6 and 4.8.11 app files, NIBs or official images. The app itself was never run.
- **[S]** read in source; the path and line are given.
- **[D]** documented: vendor docs, changelog, reviews, issue trackers, man pages.
- **[I]** inferred.
- **[syn]** checked during this synthesis (§0.3).

**Paths.** `R` = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research` holds all scratch evidence:
- **gsr:** runs in `R/runs-summary.txt`, `R/r*.analysis.json`, `R/spec_r*.report.json`; harness `R/harness.py` and `R/analyze.py`; source `R/gsr-6.1.0` (6.1.0) and `R/gsr-src` (master 7f9405e).
- **Hyprland and XDPH sources:** `R/hyprland-src` (v0.56.2), `R/xdph-src` (v1.4.1).
- **Qt and FFmpeg sources:** `R/src` (Qt 6.11.2), `R/ffsrc` (FFmpeg n9.0.1).
- **ffmpeg scripts and logs:** `R/gen.sh`, `R/recipes.py`, `R/logs/`.
- **Qt harnesses:** `R/player/`, `R/cam/`, `R/qml/`, `R/snippets/`; test media in `R/media/`.
- **CleanShot:** the DMGs, `R/cs476.apfs`, `R/nibs476/`, `R/s476.txt`, `R/img/`.
- **Lua module:** `R/snip-rec.lua` and `R/mock-test.lua`; latency benchmark `R/hop.c`.
- **Omarchy copies:** `R/upstream-*`, `R/webcam-presets.sh`.

**CleanShot source keys**
- **[BIN]** Official DMGs `https://updates.getcleanshot.com/v3/CleanShot-X-4.7.6.dmg` (SHA-256 `677178b8…3307b55`) and `…/CleanShot-X-4.8.11.dmg` (SHA-256 `298afb3d…a1a81`), decoded to APFS.
  - Hex numbers are file offsets in `R/cs476.apfs`.
  - 4.7.6 and 4.8.11 have the same recording settings, Video Editor NIB and recorder strings, so the rows below describe roughly 4.6 to 4.8.11.
- **[IMG]** Official pre-5.0 images in `R/img/`: `screen_recording_2x.797f6110.png`, `video_editor_2x.28d97fdb.png`, `recording.baccd44b.jpg` and `recording.7d27e588.png`.
- **[CL x.y]** https://cleanshot.com/changelog
- **[API]** https://cleanshot.com/docs-api
- **[F24]** http://web.archive.org/web/20240620164438/https://cleanshot.com/features
- **Reviews:**
  - [DS] https://daveswift.com/cleanshot-x/ (2024-03-18)
  - [TSS] https://thesweetsetup.com/how-to-record-professional-screen-shares-with-cleanshot-x-or-loom/ (2022-06-07)
  - [BT] https://birchtree.me/blog/cleanshot-x-4-3-is-out-now/
  - [9to5] https://9to5mac.com/2022/01/31/cleanshot-x-utility-for-mac-adds-new-quick-controls-blur-options-more/
  - [SK] https://www.screenkite.com/blog/reduce-cleanshot-x-screen-recording-file-size
- **[HK]** The history of soffes/HotKey `Key.swift`.
- YouTube captions could not be fetched (HTTP 429).

---

## 0. Read first

### 0.1 What Snip already has

Snapshot: branch `fix/video-playback`, HEAD `f0136b8`, 2026-10-09 05:16 +04, plus uncommitted work. The implementing session kept committing while this brief was being written. [V git log, code read]

| Commits | Built |
|---|---|
| a061794, 6cd10a3, f39f9e8, 77cf557 | **Record mode in the overlay**, on the live screen. Hint: "Drag to record an area · Enter: the whole screen · Space: a window".<br>**`RecordHud.qml`**: a full-output overlay layer `snip-record`. It takes input only on the toolbar and controls (`Platform.setInputRects`) and draws nothing inside the area (2 px margin).<br>**Toolbar**: Video/GIF, the microphone list (gsr `--list-audio-devices` names), system audio, Cancel (Esc), Record (Enter).<br>**Countdown**, and **controls**: stop with the time, pause, restart, discard. A whole-output take has no controls.<br>**Also**: DND hold, `snip record …`, and the gsr builder `crates/snip-core/src/record.rs`. |
| 6e964e7, b812a4e, cbde8ce | Videos and GIFs as captures: cards, preview, Copy, Save, history. |
| c0e3a2d | Settings › Recording. Record Screen / Stop Recording in the tray menu. |
| 779aeee, 6ffd58b, eca4ef3, d0befa2, 94f3f16 | **Video editor**: trim, Split (B), Cut (Delete), per-track volume 0–200 %, mono, MP4 or GIF.<br>**Quality**: Low/Medium/High/Very high = x264 CRF 32/27/23/19; NVENC uses CQ = CRF + 9, preset p4.<br>**Size and output**: Original/1080p/720p/480p, an estimate, Replace / Save as New.<br>**Export**: exact cuts (`fps=` + trim/atrim + concat, cut points snapped to frames) and a two-pass GIF with `sierra2_4a`.<br>**Player**: a seek gate and mid-frame steps. `QT_FFMPEG_{EN,DE}CODING_HW_DEVICE_TYPES=,` is set. GIF preview uses `cache: false`. |
| f3b6014, 94f3f16 | **REC-14**: takes cut short are recovered at start (`-ignore_editlist 1 -fflags +discardcorrupt -map 0`).<br>It stops and keeps a take below 1 GiB of free disk, or when its display goes away.<br>An outside stop after the first frame counts as a normal stop. |
| 5894153 | Hyprland requests go out as `/eval` plus `shutdown(Write)`. A session-key timer now ends only its own session. |
| f0136b8 | Docs: `docs/design/recording.md` (plan, slices, 7 owner questions), the six reports, D43 (decided) and D44–D47 (proposed), plus spec, architecture, performance and roadmap updates. |

**Uncommitted at 05:16:** the camera bubble.
- `app/qml/CameraBubble.qml`.
- `[recording] camera` (by name), `camera_shape = "circle"`, `camera_size = "medium"`, `camera_mirror = true`.
- `SNIP_CAMERA=fake` shows a test card in place of a camera.

**Not built yet:**
- Clicks (REC-09) and keystrokes (REC-10).
- Area adjust, W×H, ratio and remember-last-area (SEL-08).
- Controls: the level meter, Restart/Delete confirmations, Alt+Enter for GIF, the tray icon and timer, start/pause/stop sounds.
- Safety: mic warnings, lock, suspend and idle handling, and the private-window warning (D6).
- GIF: quality, optimise, cancel and trim. The MP4 is deleted once the GIF is made.
- Encoding options: "Scale to 1×" and record-time mono.
- gsr fixes: snapping to gsr's constraints (today the area is rounded) and the `-s` upscale guard.
- Editor speed: a proxy, and keyframe thumbnails (N accurate seeks still run before the editor opens).
- The After Capture "Recording" column.
- A gsr stub for e2e: `SNIP_RECORDER=fake` bypasses gsr entirely.

### 0.2 Findings that shape the design

1. **CleanShot 4.x already had several things the inventory credits to 5.0.** [V BIN]
   - Window recording with **Space**, the **Vertical** camera shape, FPS 10–60, a 7-stop editor quality, an audio radio group, and **Trim Only vs Trim & Convert**.
   - It had **no** mid cuts, no GIF export from the editor, and no low-disk auto-stop.
2. **gsr pads small videos.** Hardware encoders pad every video to **even and ≥128 px per side**, centred on black.
   - NVENC H.264 also rejects widths under about **146 px**.
   - The CPU encoder pads the width to a **multiple of 4**.
   - Snip's selection must snap to these rules, or show black edges. [S][V][syn]
3. **gsr's `-s` also upscales.** Pass it only to shrink. Snip passes it whenever a maximum size is set. [V r3]
4. **Exit code 0 does not mean success.** It is also returned when capture fails to start or when the take stops before the first frame. Use the IPC reply, the `.ts` file and ffprobe. [V r8; Omarchy runs]
5. **gsr does not survive a snipd crash.** Under systemd, `KillMode=control-group` SIGTERMs gsr, which then finalises cleanly. Recovery at the next start is the right model (built). [V]
6. **gsr cannot record HEADLESS-1 or the nested e2e output.** Inside the nested session, `-w DP-2` would record the user's real screen. e2e therefore needs a gsr stub. [V][S][I]
7. **Qt 6.11 cameras use V4L2 only**, with no PipeWire path.
   - MJPEG is decoded on the CPU: 8.5–23 % of a core at 1080p30.
   - The bubble should use YUYV at 640×480 or smaller.
   - After an unplug, the Camera object must be created again. [S][V]
8. **The trimmer is slow on originals.** A seek on a 3440×1440 recording costs 55–310 ms. An all-intra 1280 px proxy seeks in about 3 ms and costs about 7.5 % of real time to build. [V]
9. **ffmpeg cuts are exact only one way:** `fps=` first, then trim/atrim + concat, with cut points snapped to frames.
   - `select` was 2 frames and 43 ms off.
   - A single-pass GIF held 1.55 GB.
   - `bayer:5` is worse for UI content than `sierra2_4a` or no dithering.
   - All three are fixed in 94f3f16. [V]
10. **Hyprland input:** keystrokes come from `hl.on("input.keyboard.key")`, and clicks from a non-consuming `MOD3+MOD5+mouse:272` bind with `ignore_mods`.
    - The key hook sees passwords and the lock screen, and fcitx5 duplicates keys.
    - `HL.Keybind:remove()` removes **every** bind with the same modifiers and key, in every submap.
    - It crashes on an expired handle. [S][V][syn]
11. **Omarchy sees a gsr only if its `argv[0]` starts with `gpu-screen-recorder`.** Snip's absolute-path spawn is invisible to Omarchy (D44, built). [V]
12. **Window recording v1 is the window's rectangle.** CleanShot 4.x recorded a display rectangle too: none of its binaries call ScreenCaptureKit's single-window capture. True window capture is P2. [V][D]

### 0.3 Checked during this synthesis [syn]

- **gsr padding** [S]
  - Hardware encoders: `R/gsr-6.1.0/src/encoder/video/nvenc.c:125-132`, `vaapi.c:270-274` and `vulkan.c:517-521` align to 2 and raise each side to at least 128.
  - CPU encoder: `software.c:11,55-56` aligns the width to 4 and the height to 2, with no minimum.
- **gsr 6.1.0 accepts these flags** (`args_parser.c:49-77, 206-207, 256, 274`) [S]:
  - `-bm auto|qp|cbr|vbr`
  - `-tune performance|quality`
  - `-encoder gpu|cpu`
  - `-fallback-cpu-encoding yes|no`
- **`HL.Keybind:remove()`** → `removeKeybind(modmask, {key, keycode, catchAll})` → `erase_if` across all submaps. [S `R/hyprland-src/src/config/lua/objects/LuaKeybind.cpp` keybindRemove; `src/managers/KeybindManager.cpp:197-202`]
- **Hyprland socket2** cuts payloads at 1024 characters and turns newlines into spaces. It removes any client whose queue reaches **64 events** after a full socket buffer. [S `src/managers/EventManager.cpp:126-190`]
- **Lua stubs** list `input.keyboard.key`, `hl.dsp.event`, `get_cursor_pos`, `get_layers`, `get_active_window`, `timer`, `HL.EventSubscription` and the bind options `non_consuming`, `ignore_mods`, `dont_inhibit` and `submap_universal`. [V `/usr/share/hypr/stubs/hl.meta.lua`]
- **systemd kill mode.** `snip-dev.service` has `KillMode=control-group` and `Restart=on-failure`. `systemd/snipd.service` sets no KillMode, so it uses the same control-group default. [V `systemctl --user show`, repo file]
- **Snip's socket2 reader** (`snip_hypr::subscribe`) exits on EOF and **never reconnects**. [V `crates/snip-hypr/src/lib.rs:206-218`]
- **Omarchy's slurp-time binds** are `hl.bind("RETURN" …)` and `"TAB"` handles, released with `unbind()`. Key strings are case-sensitive, so they do not collide with Snip's `"Return"`/`"Escape"`. [V `/usr/share/omarchy/default/hypr/bindings/utilities.lua:44-82`]
- **`omarchy-hyprland-monitor-watch`** reads socket2 and ignores unknown events cheaply. [V]

### 0.4 Side effects of the research runs (tell the user)

- **Qt agent:**
  - Its test program `playerprobe` crashed twice at exit (SIGSEGV, 04:10:32 and 04:10:36 +04). A video frame outlived the CUDA player.
  - Two core dumps (12.5 MB and 35.7 MB) are in systemd-coredump. One "Process crashed: playerprobe" toast probably appeared.
  - A 440 Hz test tone played twice for about 1 s through the default sink, the INZONE H5 headset. It is muted now.
- **gsr agent:**
  - It made 12 short runs (≤2.6 s each) on a strip of the DP-2 bar, with `argv[0]=snip-gsr-research`, so Omarchy never saw them.
  - No microphone was recorded, and all files are deleted.
  - Every gsr call, including Snip's own `--list-audio-devices`, rewrites `~/.nv/nvidia-application-profiles-rc.d/10-gsr-cuda-no-stable-perf-limit`. This is harmless. [S main.c `install_cuda_no_stable_perf_limit`]
- **Omarchy agent:** it ran gsr on a 160×96 region and deleted the files. No processes or stray layers were left.
- The research agents say they edited nothing in the repo or in `~/.config`.
- `R/` takes **750 MB on the RAM-backed /tmp** (DMGs, APFS images, media, source clones). Delete it when it is no longer needed. [V du]

---

## 1. Feature parity list (CleanShot X 4.x, before Studio Mode)

**Priorities (suggested):**
- **P0:** the first slices, meaning the owner's D43 scope plus correctness fixes.
- **P1:** later in M4.
- **P2:** after M4.

**"Snip now"** values: built, part (partly built), no, WIP (uncommitted).

### 1.1 Setup and selection

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Entry points:** menu "Record Screen"; global "Record Screen / Stop Recording" (unassigned, and the same key stops); All-In-One Record; URL `cleanshot://record-screen[?x=&y=&width=&height=&display=]`. The parameters need 4.7; with no `display`, the display under the cursor is used | REC-01, API-01, OMA-08, CAP-07 | built (`snip record`, `snip record area\|fullscreen`, tray); All-In-One no | P0 done; All-In-One P1 | V [BIN menu NIB], D [API] |
| **Hint:** "Drag to record a part of the screen." + " Press Space to select a window." | REC-01 | built (own wording) | done | V [BIN 0x36ce50] |
| **Area:** drag, 8 handles, edge snapping, ⌥ for a uniform selection, ⌘/⇧+arrows to move or resize. Typed **W×H in points**: on Retina, 1920×1080 records 3840×2160 unless "Scale Retina videos to 1x" is on | REC-01, SEL-08 | part: drag only. The area is fixed once chosen | P1 | V fields [IMG]; D [CL 1.0.2, 3.1.2, 3.8, 3.9.4], [DS] |
| **Window:** Space (recorder-only, rebindable) switches to window picking. The area becomes the window's frame and stays fixed (no tracking). A window-name label shows | REC-01 | built (Space, then click = the window's rectangle) | P0 done; client-area and re-read after the countdown P1 | V Space [BIN 0x526945]; I frame; D [DS]. 4.7.6, 4.8.11 and 5.0.1 use only display capture plus `setSourceRect:` [V grep] |
| **Full screen:** a ⤢ button "Toggle fullscreen" (toggles back). Return with no area records that display | REC-01, SEL-09 | Enter built; ⤢ no | ⤢ P1 | V [BIN `recorderFullscreen`], D [CL 3.5.1, 4.2.2] |
| **Aspect-ratio lock** (4.1): a ratio button with a dropdown; the last ratio is remembered. List: Freeform, "1 : 1 (Square)", "5 : 4 (10 : 8)", "7 : 5", "3 : 2 (6 : 4)", "16 : 10", "16 : 9", "2.35 : 1", portrait variants, "Custom Ratio" | SEL-08 | no | P1 | V strings; I that the recorder shows this exact list; D [9to5] |
| **Remember last selection** (default off; restores `lastRecordingArea`; fixed for external monitors) | REC-01 | no | P1 | V [BIN defaults], D [CL 3.8] |
| One overlay per display; a recording covers one display; other displays are not dimmed | REC-01 | built (gsr records one monitor) | done | I/D [API], [CL 3.3.1] |
| Esc cancels the overlay but never stops a running recording | REC-01 | built | done | D [CL 3.8] |

### 1.2 Toolbar and inputs

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Panel 1, row 1:** Settings (sliders icon, "Show preferences" → Preferences › Recording), W [ ] × H [ ], ⤢ "Toggle fullscreen" | REC-02 | no | P1 | V [IMG], [BIN 0x3c6000–0x3c8fca] |
| **Panel 1, row 2** (toggles with a check mark when active): Microphone ("Record Microphone": a menu with "Do Not Record Microphone" and the devices, plus a **live level meter**); Computer audio; Camera (menu, §1.4); Highlight Clicks; Show Keystrokes | REC-02 | part: mic list and system audio built; camera WIP; clicks, keys and meter no | camera P0; others P1 | V [IMG], [BIN `vuMeter`]; D [CL 3.9.4] |
| **Panel 2:** "Record GIF ⌥↩" and "Record Video ↩" | REC-02 | part: a Video/GIF switch plus one Record button | P1 | V [IMG] |
| **Recorder-only keys:** Return = video, ⌥Return = GIF, Space = window (all rebindable) | SEL-09 | Enter and Space built; Alt+Enter no | P1 | V [BIN shortcut table 0x526945–0x526b05] decoded with [HK] (index 47 = Space, 49 = Return; 113 = none) |
| Toolbar toggles write the same settings as Preferences (`highlightClicks`, `showKeystrokes`, `recordComputerAudio`) | REC-02 | built (mic, system, kind persisted) | done | V |
| Mic device names | REC-02 | built: gsr names. Qt `MediaDevices.audioInputs[i].id` strings equal gsr `-a` names, e.g. `alsa_input.usb-Sony_INZONE_H5-00.mono-fallback` | done | V (Qt run) |

### 1.3 During recording

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Controls pill** (`RecorderControlsView`): a red **Stop** showing the elapsed time and an **audio-level indicator** (`levelLayer`, fed by the mic and computer-audio meters); **Pause/Resume** (the icon swaps); **Restart**; **Delete** (trash); a drag grip (`recorderAnchor`). It is movable (4.6), shown during full-screen recordings, and never captured | REC-05 | part: stop + time, pause, restart and discard built. No meter, no drag. Placed **outside** the area by design (spec §6). No pill for a whole-output take | meter P1; drag P2 | V [BIN], [IMG recording.baccd44b.jpg]; D [CL 3.8, 3.9.1, 4.6] |
| **Restart confirmation:** "Are you sure you want to cancel this recording and start a new one?" / "You can't undo this action." [Restart Recording] [Cancel] | REC-05 | no (acts at once) | **P0** (small; prevents loss) | V [BIN 0x51375c] |
| **Delete confirmation:** "Are you sure you want to delete this recording?" / "You can't undo this action." [Delete] [Cancel] | REC-05 | no | **P0** | V [BIN 0x5140ff] |
| "Show controls while recording" (default on). When off, a one-time menu-bar tip says "Press to stop recording" | NEW | no | P2 | V flag `stopRecordingPopupDisplayed`; I trigger |
| **Menu bar:** the icon becomes stop (resume while paused) and a click stops. Optional elapsed time ("Display recording time", default off). GIF progress | REC-06, OMA-04 | part: the tray menu has "Stop Recording"; no icon state or timer | P1 | V outlets `stopIcon`, `resumeIcon`, `gifProgressIndicator`; I look |
| **Global keys:** Record Screen / Stop Recording, Pause/Resume Recording, Restart Recording, Toggle Camera Fullscreen (all unassigned) | spec §7 | part: actions `record`, `record-gif` and `record-pause` exist (no default keys); restart and camera-fullscreen no | P1 | V [BIN table] |
| **Sounds:** `startRecording.aif` (also on resume, I), `pauseRecording.aif`, `stopRecording.aif`, `countdown.aif` | SET-09 | part: only "shutter" plays when the take lands | P1 | V files; I resume |
| **Countdown** 3-2-1, "Show countdown" default **off** | REC-03 | built, default **3 s** (D47, proposed) | done; default = Q6 | D [CL 4.6]; V default |
| **Dim outside** the area (default on); a border when dimming is off; paused recordings dim only when dimming is on | REC-04 | built (dim on; the frame is always drawn, red while recording and amber while paused) | done | D [CL 4.1, 4.7.5] |
| **Do Not Disturb** (default **off**); **display sleep prevented**; desktop clutter hidden when the general option is on | REC-11, CAP-12, NEW | DND built (default on, D47); idle inhibit no; clutter no | idle P1; clutter P2 | D [CL 3.8.1], [F24] |

### 1.4 Camera

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Camera menu:** devices (virtual cameras included; Continuity Camera) plus "Don't Record Camera" (the default) | REC-08 | WIP | **P0** | V [BIN 0x5fd67c–0x600804]; D [CL 3.5.2], [DS] |
| **Sizes** Small, **Medium** (default, I), Large, Huge. The size **scales with the recording area** (`cameraSizeAuto`, 4.3) | REC-08 | WIP | **P0** | V; D [CL 4.3] |
| **Shapes:** **Circle** (default), Square, Rectangle, **Vertical** | REC-08 | WIP | **P0** | V; D [DS] "circular, square, or vertical" |
| **"Flip Camera"** (mirror), **on** by default | REC-08 | WIP (`camera_mirror = true`) | **P0** | V getter |
| **Drag anywhere**, before and during recording. Position kept (`cameraPositionX/Y`); no snapping found. The overlay exists while the recorder is open, so it can be placed before starting | REC-08 | WIP | **P0** | D [TSS] "move the video around as you record"; V keys; I semantics |
| **Full screen:** a click on the camera, or the global "Toggle Camera Fullscreen" key, fills the recording area; again restores it. The cursor is hidden while it fills the area | REC-08 (key NEW) | WIP (click) | click P0; key P1 | D [CL 4.3, 4.6], [BT] "great for presentations" |
| The camera is a window on screen, recorded as seen | REC-08 | n/a | design | I |

### 1.5 Audio

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| Microphone optional; computer audio toggle | REC-07 | built | done | V |
| **"Computer Audio Settings…":** "Record on separate tracks" ("This allows you to edit the microphone audio and the computer audio separately in a video editing software.") or "Record on a single track" (the default, I: `mergeAudioTracks` defaults to true) | REC-07 | part: a setting that gives **stems only** (see Q11) | P0 fix | V [BIN NIB]; I default |
| **"Record audio in mono"**, default off | REC-07 | no at record time (gsr always writes 48 kHz stereo); the editor's Mono is built | P1 (post-stop remux) | V; [S audio_codec.c:138] |
| **Post-recording merge dialog:** "Merge audio to adjust the volume of each track separately:" with "Microphone volume:" and "Computer audio volume:" sliders, [Merge Audio] [Don't Merge], "Don't ask again". Plus "Merging audio...", a stop-processing confirmation and error texts | NEW | no. The editor's per-track volumes do this (D45, proposed) | P2 | V [BIN 0x4eb060–0x4ecbd1]; I trigger |
| **Live level** on the mic button and on Stop | REC-02/05 | no | P1 | V; D [CL 3.9.4, 4.6] |

### 1.6 Cursor, clicks, keystrokes

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Show cursor** (default on) | CAP-11 | built (`recording.show_cursor` → `-cursor`) | done. The NVIDIA cursor plane with a visible pointer is unverified (S7) | V |
| **Highlight clicks › Options…:** size slider 25–75 (default 50); colour "System Accent color" (default), Blue, Purple, Pink, Red, Orange, Yellow, Green, Gray; style **Outline** (default) or Filled; "Animate clicks" on; a preview box ("Click here to preview") and OK | REC-09 | no | P1 | V [BIN NIB] |
| **Show keystrokes › Options…:** size 18–72 (default 45); Top-Left/Center/Right or Bottom-Left/**Center** (default, I)/Right; **Dark** (default) or Light; "Blur background" on; "Show all keys" or "**Show only command keys**" (default); "CleanShot can't display any keystrokes you make within password fields."; [Preview] [OK]. Held modifiers stay visible (4.7); repeated presses are animated (4.2.2) | REC-10 | no | P1 | V/I [BIN NIB]; D [CL 4.7, 4.2.2] |

### 1.7 GIF

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| Recorded as video and converted after Stop (bundled gifsicle) | REC-13 | built (two-pass, `sierra2_4a`, 94f3f16) | done | V [BIN `recordingToGif`, `GIFUtility`] |
| **GIF settings:**<br>FPS: number field + slider 5–60, default **15**.<br>Quality: slider 0–100, 11 ticks Low–High, default **80** (caption: "Setting the quality to maximum can speed up the processing time, but it will increase file size.").<br>"**Optimize GIFs**": gifsicle lossy, default **on**, optional since 3.5.<br>GIF size: Original, 1200, 960, "**800 x auto (default)**", 640, 500, 480, 320, 256, 200 × auto (caption "…CleanShot will only downscale the GIF if needed.") | REC-13 | part: fps 5–30 and width 200–2000 (800) built; quality and optimise no | P1 | V [BIN NIB, getters]; D [CL 3.5] |
| **Conversion card** in the stack with a progress bar and "Cancel Processing...", plus menu-bar progress. Cancelling asks "Are you sure you want to stop the conversion?" / "You can delete the recording or stop the GIF conversion and save it as a video instead." [Delete] [Save as a Video] [Cancel] | REC-13 | no: a toast "Creating GIF…", and the MP4 is deleted after conversion | P1 | V [BIN 0x5c2055] |
| **"Trim the GIF..."** re-renders from the kept MP4 (gifsicle trim + optimise) | NEW | no (the editor opens videos only) | P1 | V `GIFTrimmer`; I flow; D [CL 3.3] |

### 1.8 After recording and the Quick Access card

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| The After Capture "Recording" column runs at Stop. "Open Video Editor after recording" (Video tab) is the same switch | SET-03 | part: always a card; no Recording column | P1 | V |
| **Video card** (`ScreenRecordingPopup`): thumbnail with a type badge, **duration**, **file size**, a **sound indicator**; "Open Quick Look (Space)"; "**Trim & encode video (⌘E)**"; double-click opens the editor; context items "Open Video Editor..." (videos) and "Trim the GIF..." (GIFs). Tip texts: "Large videos can be compressed to reduce file size. Double-click on the overlay to open Video Editor." | QAO-02/03/05 | part: card, preview, Copy and Edit Video built; exact labels unverified | P1 polish | V [BIN tooltips]; D [CL 1.0.2] |
| "Are you sure you want to close this recording?" / "You may lose the file if it hasn't been saved." | n/a | not needed: history keeps every capture (spec §5.0) | n/a | V [BIN 0x592936] |
| MP4 (H.264) or GIF; screenshot name template (I); any video can be opened into a card (3.6); MP4 from the clipboard (4.6) | SET-04, NEW | part | P2 | D [CL 3.6, 4.6]; I template |
| No "convert this video to GIF" action in 4.x | n/a | Snip's editor exports GIF (extra) | n/a | V by absence |

### 1.9 Video editor and trimmer (pre-5.0 "VideoConverter", 3.6 to 4.8.11)

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| Opens from the card (double-click, edit button, ⌘E, context menu) or After Capture. Several editors can be open at once (`videoEditors`) | VED-01 | built (card, double-click, `snip edit FILE`) | done | V |
| Window 750×728 (min 750×650): player on top. The trim bar sits under it: Play, thumbnail strip, **yellow** in and out handles, **red** playhead | VED-01 | built (own layout) | done | V [IMG]; D [CL 3.7.1, 4.2] |
| **Start and end trim only.** No mid cuts, no timeline zoom, no GIF export | VED-01/03 | Snip adds cuts and GIF export ("cut things up"; D45 proposed) | extras kept | V by absence; D [TSS] "you can't remove bits from the middle" |
| **"Dimensions:"** popup: "W x H  (Original)", then "W x H  (~NN%)" built from standard frames (7680×4320, 4096², 3840×2160, 2560×1440, 2048², 1920×1080, 1280×720, 858×480, 480×360, 352×240), probably only sizes below the original (I). Width/Height fields switch the popup to Custom (I) | VED-02 | part: Original/1080p/720p/480p | P1 | V [BIN 0x5b7b21] |
| **"Video quality:"** slider with 7 stops: Poor, Very Low, Low, Medium, **High** (default), Very High, Ultra. The name shows beside it | VED-02 | part: 4 stops (see Q27) | P1 | V [BIN jump table 0x5b9808], [IMG]; D [SK] |
| **"Audio:"** Don't change (default), Convert to mono, Mute, Change volume 0–200 % (ticks 0/100/200; 3.9.1) | VED-02 | built (per-track 0–200 %, Mono, 0 % mutes) | done | V; D [CL 3.9.1] |
| **Bottom row:** [Cancel] (Esc), "Estimated file size: ~9,2 MB", **[Trim Only]** ("Trimming Video...", no re-encode, I) and **[Trim & Convert]** (Return, default; "Converting Video...") | VED-02 | part: estimate and Replace / Save as New built; Trim Only no | Trim Only P1 | V [IMG]; I no re-encode |
| "**Do you want to replace the existing video?**" [Replace Video] [Save as New Video] [Cancel]. The card updates (`screenRecordingDidEdit`). Summary: "Video encoded successfully!" / "The final file size is X (Y% of the original)." [OK] + "Don't show again" (shown by default). Stop-processing confirmation. Last encoder settings remembered | VED-02 | part: two buttons; no summary, no remembered settings | P2 | V [BIN 0x44dcc3]; I what is restored |
| Space plays and pauses (4.5 fix); Return and Esc as above. No editor shortcut group in 4.x | VED-01 | built: Space, B, Delete, ←/→ (Shift ±10), Home/End | done | V; D [CL 4.5] |

### 1.10 Settings (Preferences › Recording: General | Video | GIF)

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **General defaults:** Show controls on; menu-bar time off; Scale Retina videos to 1x off; DND off; Show cursor on; Highlight clicks off; Show keystrokes off; Remember last selection off; Dim screen on; Show countdown off | REC-03/04/05/06/11, CAP-11 | part: cursor, dim, DND (on), countdown (3 s) | P1 for the rest | V [BIN NIB + fallback defaults] |
| **Max resolution:** Original (default), 4k, 1440p, 1080p, 720p, 480p = boxes 3840×2160, 2560×1440, 1920×1080, 1280×720, 854×480. Caption: "Set maximum resolution to reduce file size and upload time." | REC-12 | part: Original, 2160p, 1440p, 1080p, 720p (no 480p) | P1 | V [BIN 0x5ba6c8] |
| **Video FPS:** 10, 15, 24, 25, 30, 48, 50, **60** (default). Nothing above 60 | REC-12 | part: 24/30/60 | P1 (Q8) | V [BIN getter 0x53b200]; I which version added which |
| **Audio:** [Computer Audio Settings…], "Record audio in mono" | REC-07 | part: separate-tracks checkbox | P1 | V |
| **"Open Video Editor after recording"**, caption "Use Video Editor to change the recording quality, resolution and adjust audio settings." | SET-03 | no | P1 | V |
| **Encoding:** MP4, H.264 **Main** only (auto level); AAC (I); Retina tagged `com.apple.quicktime.pixeldensity` | REC-12 | part: `-k auto` (H.264 **High** here), plus a quality setting CleanShot doesn't have | P0 (`-k h264`) | V imports; I AAC and behaviour |

### 1.11 Safety

| CleanShot 4.x behaviour | Spec | Snip now | Pri | Conf. / source |
|---|---|---|---|---|
| **Crash recovery:** a warning-sign title, "CleanShot quit unexpectedly", then "Your screen recording has been successfully recovered." | REC-14 | built ("Recovered a recording that was cut short") | done | V; D [CL 3.3, 3.9.1] |
| **Low disk:** "Your free disk space is low." / "The recording might stop unexpectedly and you may lose the file." [OK] + "Don't warn me again". Threshold near 3000 (`0xbb7`; unit unverified). **No auto-stop in 4.x** | REC-14 | part: **auto-stop below 1 GiB** built (D46, a Snip extra); no warning | warning P1 | V [BIN 0x5c402d]; I unit |
| **Mic muted** (4.6): "Microphone is muted" / 'It appears that "<device>" might be muted. Would you like to continue recording?' [Ignore] [Stop Recording] + "Don't warn me again" | REC-14 | no | P1 | V [BIN 0x5c2c0e] |
| **Mic disconnected:** "Microphone Disconnected" / '"<device>" has been disconnected during the recording. Do you want to continue recording without audio or stop the recording?' [**Continue Without Audio**] [Stop Recording].<br>**Computer audio failed:** "Audio Recording Failed" / "Unable to capture computer audio. Do you want to continue…", same buttons | REC-14 | no. gsr writes silence and carries on | P1 | V [BIN 0x5c32ad, 0x5c3a77]; [S audio_capture.c:240-262] |
| **Unexpected stop:** "Screen Recording stopped unexpectedly." / "An error occurred with screen recording."; "Recording Stopped" + "It may have been caused by your microphone disconnecting during the recording." | REC-14 | part: toast "Can't record: <gsr's last stderr line>" | P1 wording | V |
| No mic chosen with the camera on: "To have sound in your video, you need to select a microphone first." [Don't Record Audio] [Cancel]. Lid closed with the built-in mic (3.7.1) | NEW | no | P2 | V/I |
| Display sleep prevented (3.8.1) | NEW | no | P1 (idle inhibitor) | D |
| (Snip only) Display unplugged → stop and keep; outside stop → keep; stop before the first frame → "nothing was recorded" | REC-14 | built | done | V code |
| (Snip only) Lock → pause the video and mute keystrokes; suspend → stop and finalise; Bluetooth HFP warning | REC-14, R5, R19 | no | P1 | D risks doc |
| (Snip only) A private window (`no_screen_share`: 1Password, Bitwarden) in the area → warn before start, banner during | D6, R6 | no | P1 | D decision log |
| (Snip only) Privacy-safe mode | REC-15 | no (KMS can't hide windows) | P2 | D platform doc |

### 1.12 Suggested slice order (one small PR each)

1. **gsr correctness (core only, unit-tested):**
   - Snap to gsr's constraints (§2.2) and draw the snapped frame.
   - Pass `-s` only to shrink; add 480p and "Scale to 1×".
   - `-k h264`, with HEVC only above the H.264 maximum.
   - Separate tracks = mix + stems.
   - Start the timer from the `.ts` monotonic value.
   - Run recovery on a killed take within the session too (§7.3).
2. **Controls:** Restart and Delete confirmations; Alt+Enter = GIF; start, pause and stop sounds.
3. **Camera bubble** (owner P0; WIP): toolbar camera menu, shapes, sizes, drag before and during, mirror, remembered position, the fake camera in e2e, and the stop ordering in §3.1.
4. **Editor speed:** open the window first and show keyframe thumbnails; build the proxy; add Trim Only (stream copy); attach the AudioOutput only while playing; extrapolate the playhead between position updates.
5. **GIF parity:** keep the MP4; a progress card with Cancel → Delete / Save as a Video; GIF quality and Optimize; Trim the GIF.
6. **P1 parity:**
   - Clicks and keystrokes (S9).
   - SEL-08 adjust, ratio, W×H and remember.
   - Lock, suspend and idle handling.
   - Mic warnings and the level meter.
   - Tray stop icon and timer.
   - After Capture "Recording" column; FPS list and mono setting.
   - ALT+PRINT opt-in; OMA-03 menu entries; the private-window warning.

---

## 2. Backend facts: gpu-screen-recorder 6.1.0

### 2.1 Versions, preflight and environment

- **Versions** [V][S git]
  - gsr **6.1.0** is installed from Arch `extra` (sync database of Sep 8). Upstream is at 6.1.3.
  - `-ipc` exists since **6.0.0** (2026-08-08).
  - The `omarchy` repo also ships **5.12.3, which has no `-ipc`**. `extra` comes first in `/etc/pacman.conf`.
- **Fixed after 6.1.0** [S git]
  - 6.1.1: every stream is aligned to the first video keyframe (ad6981c), and CUDA no longer picks the wrong GPU.
  - 6.1.2: `gsr-kms-server` argument checks are hardened (4de50ca).
  - After 6.1.3: the audio thread's 1 ms poll (about 1000 wakeups/s) is fixed (ab068e1).
  - 6.0.1 fixed an NVENC API mismatch on Pascal/Maxwell with 580xx drivers.
  - 6.1.0 picks each monitor's own GPU, which fixed Omarchy #7530.
- **The scratch man page is older** than the binary: it describes 6.0.1. In 6.1.0, **`-region` is deprecated**; use `-w WxH+X+Y`. [S main.c `validate_args_with_capture_sources`; V warning]
- **`gsr-kms-server` needs `cap_sys_admin`.** Arch's `post_install` and `post_upgrade` run setcap. Without the capability, gsr falls back to `pkexec`, which shows a **polkit password dialog on every recording**. [S kms_client.c:274-342; V `/var/lib/pacman/local/gpu-screen-recorder-6.1.0-1/install`]
- **Info commands:** `--info` takes 376 ms, `--list-monitors` 115 ms and `--list-audio-devices` 27 ms. [V `R/info.txt`]
  - `--list-monitors` shows only `DP-2|3440x1440`.
  - `--list-capture-options` shows `DP-2`, `region` and `portal`.
- **Preflight** at daemon start, after hotplug and after a package change, off the hot path:
  - version ≥ 6.0.0;
  - `getcap`;
  - the DRM monitor list;
  - codecs from `--info`;
  - the audio devices;
  - the H.264 maximum size.
  - [I gsr report]
- **Security:** `cap_sys_admin` on the helper is a known concern (omarchy#8245). [D]

### 2.2 Geometry: what gsr actually records

- **Regions are logical and global, like slurp: `-w WxH+X+Y`.** [S capture_setup.c:65-82, 186; V r9 error lists monitors as `"DP-2" (2752x1152+0+0)`, `"HEADLESS-1" (1536x864+2752+0)`]
  - The monitor is the one containing the region's centre `(x + w/2, y + h/2)` (integer division).
  - Edges are inclusive, and the first match wins.
- **Logical to physical** [S capture_setup.c:219-226; V r1, r2, r3, r6]
  - gsr subtracts the monitor's logical origin.
  - Then x, y, w and h are each multiplied by `phys_w / logical_w`, as C `int *= double`, which **truncates**.
  - Measured: 2752×34 → 3440×42; 401×33 at (1,1) → 501×41 at (1,1); 400×34 → 500×42.
- **Hardware encoders** (NVENC, VAAPI, Vulkan) [S nvenc.c:125-132, vaapi.c:270-274, vulkan.c:517-521, re-read in synthesis; V]
  - Encoded size = `max(align2(w),128) × max(align2(h),128)`.
  - The content sits at `(W/2 − w/2, H/2 − h/2)` on black.
  - Measured: 3440×42 → **3440×128** (rows 43–84); 501×41 → **502×128** (one padding column); 1000×84 → 1000×128; 250×21 → 250×128.
  - The offsets can be odd, and H.264 crops only in steps of 2, so **removing the padding needs a re-encode**.
- **NVENC H.264 width minimum** [V Omarchy runs]
  - 140 px (112 logical) failed: "Frame Dimension less than the minimum supported value".
  - **146 px worked.**
  - HEVC and AV1 accepted 130×128.
- **CPU encoder** (`-encoder cpu`): width aligned to **4**, height to 2, **no 128 minimum**. [S software.c:11, 55-56; syn] Where the content sits inside the padded frame on this path was not checked [I: the same centring code].
- **`-w DP-2`** records the DRM mode size (3440×1440), rotated for odd transforms. `-s 0x0` means the original size. [S kms.c:198-218]
- **`-s WxH`** [S utils.c:715, kms.c:119-126; V r3; I aliasing]
  - It fits the video inside the box, keeping the aspect ratio ("fit" since 5.15.0, c09fb1b).
  - **It also upscales:** 500×42 with `-s 1000x1000` gave 1000×84.
  - Scaling is bilinear without mipmaps, so shrinking by more than 2× aliases.
  - Scaled regions were broken before 5.12.1 (054282b).
- **Example:** `-s 1920x1080` on 3440×1440 gives 1920×803, which NVENC pads to 1920×804: one black row. [I from the formula]
- **What this means for Snip:**
  - The selection must snap to: whole logical coordinates whose truncated physical size is even, with width ≥146 (H.264) and height ≥128 on the GPU path; or width a multiple of 4 and even height on the CPU path.
  - Draw the snapped edges live.
  - The algorithm is in §2.11.

### 2.3 Timing (spawn = t0)

All [V]: 7 runs, plus the Omarchy runs.

| Step | Time |
|---|---|
| Socket appears | 23–28 ms |
| Recording starts (IPC reply; output file created) | **349–498 ms** (median about 405) |
| — stderr detail | about 100–180 ms: monitor resolved and `gsr-kms-server` spawned (+30 ms); about 290–350 ms: `using h264 encoder`; about 340–480 ms: NVENC open |
| First encoded frame (`.ts` file) | **+51 ms at 60 fps** (5 runs), +101 ms at 30 fps; 473 ms after spawn in the Omarchy run |
| `stop` → reply (sent after the file is finalised) | 12–33 ms |
| Reply → process exit | 158–242 ms |
| SIGINT/SIGTERM → path on stdout | about 16 ms; exit 164–242 ms after the signal |

- **What this means for the UI:**
  - With no countdown, users wait about 0.45 s after Record.
  - The old 150–250 ms budget was wrong; `03-performance.md` was corrected in f0136b8.
- **Untested idea (pre-roll):** spawn gsr during the countdown, queue `set-paused true` as soon as the socket exists, and send `set-paused false` at zero.
  - Gain: an instant start and warm audio devices. A mic pop would fall inside the paused span and be discarded.
  - Risk: one frame might slip through before the pause applies. [I recorder.c:842-849 loop order]

**Run table** (`R/runs-summary.txt`):

| Run | Tested | Exit | Socket | Start | First frame | Stop → reply | Reply → exit |
|---|---|---|---|---|---|---|---|
| r1 | `-w 2752x34+0+0 -f 60 -a default_output`, opus, VFR, IPC errors, pause | 0 | 28 ms | 403 ms | 454 ms | 18.8 ms | 221 ms |
| r2 | `-w 401x33+1+1 -f 30 -fm cfr`, 2 AAC tracks | 0 | 25 ms | 436 ms | 537 ms | 33.2 ms | 189 ms |
| r3 | `-k hevc -s 1000x1000`, SIGINT | 0 | – | 498 ms | 549 ms | path 16 ms after SIGINT | 242 ms |
| r4 | `-k av1 -s 250x250 -bm vbr -q high -fm cfr` | 0 | 23 ms | 349 ms | – | 15.3 ms | 158 ms |
| r5 | AAC, CFR, SIGKILL at 2.6 s | −9 | 23 ms | 413 ms | 464 ms | – | – |
| r6 | stale socket from r5, `-cursor no`, SIGTERM | 0 | – | 398 ms | – | path 16 ms after SIGTERM | 164 ms |
| r7 | `-w HEADLESS-1` | **51** | | | | | |
| r8 | region on HEADLESS-1 (`400x34+2800+0`) | **0** (bug) | | | | | |
| r9 | region off-screen | **51** | | | | | |
| r10 | `-ipc` path already listened on | **1** | | | | | |
| r11 | `-a device:does_not_exist.monitor` | **50** | | | | | |
| r12 | r5 + `-ffmpeg-opts flush_packets=1;frag_duration=1000000`, SIGKILL | −9 | 23 ms | 382 ms | 433 ms | – | – |

### 2.4 The `-ipc` socket

[S ipc.c; V r1; V Omarchy runs]

**Wire format**
- One JSON object per line, ending in `\n`. Several requests can share a connection, in sequence.
- The socket is mode 0600, with at most 8 clients and 4096 bytes per request.
- **The socket path is limited to 107 characters.** A longer path makes gsr exit 1 with "the ipc socket path is too long".
- **A request sent during startup is answered only when recording actually starts.** That makes a clean "recording started" handshake.

**Verbatim exchange (r1):**
```
→ {"id":1,"name":"set-paused","data":false}   sent at 28 ms; replied at 403 ms, when recording started
← {"id":1,"result":"ok"}
→ {"id":2,"name":"bogus"}           ← {"id":2,"result":"error","data":"unknown request name 'bogus'"}
→ {not json                         ← {"id":0,"result":"error","data":"failed to parse the request: unknown token"}
→ {"id":3,"name":"set-paused","data":"yes"} ← {"id":3,"result":"error","data":"expected 'data' to be true to pause or false to unpause"}
→ {"id":4,"name":"save-replay"}     ← {"id":4,"result":"error","data":"option -r is required to save a replay"}
→ {"name":"stop"}                   ← {"id":0,"result":"error","data":"the request is missing the 'id' field"}   (it does NOT stop)
→ {"id":5,"name":"set-paused","data":true}  ← ok (immediate); stderr "gsr info: Paused" ~1 ms later
→ {"id":6,"name":"set-paused","data":true}  ← ok (idempotent)
→ {"id":7,"name":"toggle-pause"}    ← ok; "Unpaused" applied 18 ms later (the paused loop sleeps 20 ms)
→ {"id":8,"name":"stop"}            ← {"id":8,"result":"ok","data":"<the -o path exactly as given>"}  after 18.8 ms
→ (2nd connection) {"id":9,"name":"stop"} ← {"id":9,"result":"error","data":"GPU Screen Recorder is already stopping"}
then EOF on both connections, process exits 0 ~220 ms after the reply. stdout also prints the path.
```

**Behaviour around the replies**
- **`stop` returns `ok` even if finalising failed:** the trailer's return value is ignored. Always ffprobe the file. [S main.c:501, recorder.c:889]
- If gsr exits while a `stop` is pending, the reply is `"GPU Screen Recorder exited before the request finished"`. [S ipc.c:657]
- **If the client disconnects or half-closes, the reply is lost.** `socat` half-closes after stdin EOF: the file was saved but no reply came. Keep the socket open for both read and write until the reply arrives. Snip's `gsr_request` already does this. [S `ipc_remove_client`; V Omarchy]
- Error `data` is a string, and unparseable requests get id 0.
- A stale socket file left by a killed gsr is removed silently at the next start. [V r6]
- A path that another process is listening on gives exit 1: `gsr error: gsr_ipc_init: another program is already listening on "..."`. [V r10]

### 2.5 Pause and A/V sync

- **Pause compresses time.** [S recording_clock.c, audio_capture.c:213; V r1]
  - The video clock leaves out paused time, and audio read during a pause is discarded. No drift is added.
  - r1 had 0.82 s of pause: 93 frames, largest gap 16.73 ms, video 1.550 s. That is the active time (1.584 s) minus about 2 frames.
- **Pause commands.** `set-paused` is idempotent. `toggle-pause` applies within about 18 ms. **SIGUSR2** toggles pause. [V]
- **Start and end alignment in 6.1.0** [S encoder.c:139-151, recorder.c:868-896; V r1, r2]
  - Each stream starts at its own first packet.
  - AAC priming (1024 samples, 21.3 ms) is not marked: no discard flags, start_time 0.
  - So audio is about **23–55 ms late**.
  - The video encoder is never flushed, so the **last video frame is lost**.
  - Audio runs past the video by 16–117 ms.
  - **Treat the video's duration as the length** and cut the audio to it when remuxing.
  - Fixed upstream in 6.1.1 (ad6981c).
- **Omarchy's indicator** still shows "recording" while paused, because it only runs pgrep. [S]

### 2.6 Audio

- **One mixed track or several** [V r2; S audio_capture.c:86]
  - `-a "a|b"` gives **one mixed track** through amix with `normalize=false`: full volume, and loud sources can clip.
  - Repeated `-a` gives one track each.
  - Up to 32 sources per track, and the same device may appear in two tracks.
- **Track titles leak device names.** The title is stored in MP4 `udta/name`, which ffprobe shows as tag **`name`**, not `title`. `-exclude-metadata yes` removes it. [V r2; S muxer.c:174-175]
- **Always 48 kHz stereo**; a mono microphone is upmixed. Mono belongs in the export step. `-ffmpeg-audio-opts ch_layout=mono` is untested. [S audio_codec.c:132-138; V]
- **Codecs.** Opus 128 kbps is the default for mp4/mkv; `-ac aac` gives AAC-LC at 160 kbps (tag `mp4a`). [V r1, r2]
  - **Use AAC** (built).
  - Opus in MP4 fails in many editors, older Safari/iOS and several social sites [I], and on Apple per a forum report ([Apple forum](https://developer.apple.com/forums/thread/722399)) [D, unverified].
- **No start pop on monitor sources:** −inf dBFS from the first 10 ms window on every track, because gsr flushes stale device audio at start. [V r1, r2; S audio_capture.c:195]
  - A **microphone** pop was not tested, since recording the mic was not allowed.
  - Omarchy mutes the first 400 ms for a "PipeWire capture-open pop" of 130–200 ms. [D local `omarchy-capture-screenrecording`]
- `default_output` and `default_input` follow default-device changes. A device with no data gets silence inserted. [S sound.c:110-135, audio_capture.c:240-262]
- **Harmless stop noise (AAC only):** `Application provided invalid, non monotonically increasing dts … gsr error: … failed to write frame index 1 … (-22)` and `[aac] Qavg: 65536.000`. The last audio packet is dropped. Never report this as a failure. [V r2]
- **A missing device at start** gives exit 50: `Audio device 'x' is not a valid audio device, expected one of:` followed by the list. [V r11]

### 2.7 Codecs, quality and frame-rate mode

- **Codec choice.** `-k auto` picks h264 when the size fits NVENC H.264's limit, otherwise HEVC, then AV1. Here it said "using h264 encoder because a codec was not specified". [S codec_select.c:476-480; V]
- **Codec details** [V r1–r4]
  - h264: High profile, tag `avc1`.
  - hevc: Main, tag **`hev1`**; remux with `-tag:v hvc1` for Apple players.
  - av1: Main, `av01`.
  - All are tagged BT.709 limited range. The Vulkan variants are listed but experimental.
- **Quality.** The default `-bm auto` is constant QP: medium/high/very_high/ultra = **QP 35/30/25/22**, ×4 for AV1. [S video_codec.c:10-34, 340-360; V r4]
  - `-bm vbr` sets qmin = qmax = QP and warns "Passing qmin/qmax…".
  - `-bm cbr -q <kbps>` gives a predictable size.
  - Defaults: `tune=ll`, `forced-idr`, **no B-frames**. `-tune quality` adds 2 B-frames and multipass, which hurts smart cutting.
- **GOP** = fps × keyint frames (keyint default 2 s). It is counted in frames, so VFR keyframes drift in time. [S video_codec.c:68-71; V `invfr`]
- **Use `-fm cfr`** (built). [S recorder.c:539-541; V r1–r4]
  - With KMS on Wayland every tick counts as damaged, so VFR saves nothing.
  - CFR gives exact 1/fps timestamps (MP4 timebase 1/15360) and duplicates frames when the loop runs late.
  - `-fm content` does not work with KMS.
  - VFR timestamps use a µs timebase with jitter ≤ 0.07 ms.
- **Frame rate.** `-f` accepts 1–1000, and gsr can capture faster than the monitor refreshes.
- **Cost.** At 3440×128@60, `nvidia-smi pmon` showed sm ≤ 1 % and enc 0 %. This is not representative; full 3440×1440@60 and @144 are for spike S7. [V `R/r1.pmon.txt`]

### 2.8 Signals and exit codes

[S error.h, main.c:205-215; V r3–r11, Omarchy runs]

| Event | Exit | File |
|---|---|---|
| IPC `stop` | 0 | valid; the reply carries the `-o` path |
| SIGINT | 0; prints the path | valid |
| SIGTERM (what `systemctl --user stop` sends) | 0 | valid |
| SIGUSR2 | n/a | toggles pause |
| SIGHUP (no handler) | 129 | **corrupt** ("moov atom not found") |
| SIGKILL, default flags | −9 / 137 | MP4: 88 bytes, unreadable. MKV: 0 bytes |
| SIGKILL with `flush_packets=1;frag_duration=1000000` | −9 | recoverable: 1.93 s of 2.55 s (116 frames, 94 AAC packets) |
| SIGINT 0.1–0.3 s after spawn (before the first frame) | **0**; prints the path | corrupt; no `.ts`; stderr `not finalizing the video file because it has no video/audio data` |
| Capture fails to start (region on HEADLESS-1) | **0** | no file; stderr `gsr_capture_kms_start: failed to find monitor by name "HEADLESS-1"`, then `gsr_capture_start failed` |
| Argument error, socket in use, path too long | 1 | n/a |
| Unsupported (`-w focused` on Wayland; app audio without PipeWire) | 2 | n/a |
| Capture failed during recording (portal or window ended; never KMS) | 3 | n/a |
| Codec query failed / OpenGL load failed | 11 / 22 | n/a |
| Audio device not found | 50 | n/a |
| Monitor not found: `display "HEADLESS-1" not found, expected one of: "screen" "DP-2" (3440x1440+0+0)`, or `the region … doesn't match any monitor` | 51 | n/a |
| No codec / resolution not supported / codec not supported | 52 / 53 / 54 | n/a |

### 2.9 Crash recovery and supervision

- **How gsr writes the file.** It sets `movflags=+hybrid_fragmented` and `use_editlist=1` when lavf ≥ 62.6.101. [S muxer.c:86-101]
  - A clean stop gives `ftyp, free, mdat, moov`: the moov is at the end, **not** faststart.
  - A crash leaves `ftyp, free(8), free(44), moov, mdat, moof, mdat…`. **Any top-level `moof` means the file was not finalised.** [V]
- **Default flags lose the start of a take.** [S ffmpeg file.c:316, aviobuf.c:464-467; V r5, r12]
  - Fragments are cut at each keyframe (2 s), but FFmpeg's file protocol buffers 256 KiB.
  - So a low-bitrate take has **nothing on disk for seconds**: r5 left 88 bytes after 2.55 s.
  - `-ffmpeg-opts "flush_packets=1;frag_duration=1000000"` wrote at about 1 s and 2 s (r12). A normal stop with these options still gives a clean, non-fragmented MP4 (built).
- **Recovery needs `-ignore_editlist 1`.** The provisional edit list (segment_duration 0) hides the first fragment: a plain copy recovered 61 frames, against 116 with the flag. [V; S mov.c:4545]
- **ffmpeg-writer simulation of gsr's mux settings** (`R/s7.py`): [V]
  - killed at about 5 s → 3.03 s of video and 2.02 s of audio;
  - with `frag_duration=500000`, killed at 1.2 s → 1.4 s and 1.0 s;
  - killed at about 9 s → 7.03 s and 6.02 s.
- `hybrid_fragmented` together with `faststart` is rejected ("Setting both hybrid_fragmented and faststart is not supported."). [V; D [FFmpeg commit](https://ffmpeg.org/pipermail/ffmpeg-cvslog/2025-May/148737.html)]
- **A finalised file that lost its tail later:** rewrite the box at offset 44 (`mdat` → `free`, size 44), then remux with `-ignore_editlist 1`. All 10 s came back with exact sync. This depends on the muxer's internal layout, so check the bytes first. [V; S movenc.c:9103-9109]
- **Finalising can be slow.** VAAPI took more than 5 s on loaded machines, and Omarchy's fixed 5 s SIGKILL truncated recordings ([omarchy#12469](https://github.com/omacom/omarchy/issues/12469)). **Never kill on a timer**; show "Finishing…" after 1 s. [D]
- **gsr has no SIGPIPE handler.** If its stdout or stderr is a pipe and the reader dies, gsr dies at its next log line (once a second with `-v yes`). Use files, not pipes (built). [S main.c install_signal_handlers; I]
- **`process_group(0)` does not escape systemd.** [V systemctl, unit file; D man systemd.kill; V r6]
  - `KillMode=control-group` means a snipd crash or restart SIGTERMs gsr, and gsr finalises cleanly.
  - To outlive snipd, gsr would need its own scope (`systemd-run --user --scope` or `StartTransientUnit`).
- **Orphan helper.** `gsr-kms-server` exits with its client (fixed in 6.0.0, e522789). [V r5]
- **Monitor unplugged or DPMS off.** KMS `should_stop` always returns false. gsr re-encodes the last frame, logs `failed to find monitor by name` every 2 s, and resumes if the output returns. So stop on `monitorremoved` (built). DPMS-off fires no such event; an idle inhibitor avoids it. [S kms.c:19, 800-818, 1037; I untested]
- **Disk full.** gsr logs write errors (`failed to write frame index … No space left`) and keeps "recording". Poll free space (built). [S encoder.c:163-168]
- **Never stage in `$XDG_RUNTIME_DIR`:** it is a **3.2 GB tmpfs** here. Takes are staged on disk (built). [V df]
- **Suspend.** CUDA breaks after suspend; per the gsr README, Vulkan encoding avoids it. Omarchy ships `gsr-nvidia.conf` with `PreserveVideoMemoryAllocations=1` ([omarchy#8126](https://github.com/omacom/omarchy/issues/8126)). [D]

### 2.10 What gsr cannot do here

- **Non-DRM outputs.** HEADLESS-1 (wayvnc) cannot be recorded: `-w HEADLESS-1` exits 51, and a region on it exits **0** with no file. [V r7, r8]
- **The nested e2e Hyprland.** Its output fails the same way. Worse, `-w DP-2` inside the nested session would record **the user's real DP-2**: monitor lookup uses DRM connectors, and the Wayland data is used only for rotation and position. [S capture_setup.c:134, utils.c:377-405; I, not run]
- **Window capture** (`-w focused` and window IDs are X11-only, exit 2). The portal alternative, `-w portal`, is fragile here. [S capture_setup.c:355-359; D issues; V `git tag --contains`]
  - It shows the XDPH picker every time; a restore token restores by window class.
  - It is fragile on XDPH 1.4.1, the latest tag (cc8e5ef):
    - the multi-plane dmabuf fix ([#420](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/420), dec05eba's #427) is not in any tag;
    - freezes: #424, #425 and [#403](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/403);
    - a wedge at 36 % CPU on an RTX 4060 with 610.57.04 ([#434](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/434));
    - a segfault ([#442](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/442)), EGL_BAD_MATCH ([#311](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/311)) and a region offset ([#386](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/386)).
  - Omarchy made the portal opt-in with `OMARCHY_SCREENRECORD_USE_PORTAL` ([omarchy#5632](https://github.com/omacom/omarchy/issues/5632)).
- **Hiding anything.** KMS records every composited surface, Snip's own layers and Omarchy's toasts included, and it ignores `no_screen_share`. [D platform doc]
- **More than one monitor per take.** [S]
- **Mono, loudness and per-track volume:** all are post-processing.
- **A movable or shaped camera.** gsr's camera composite `-w "screen|/dev/video0;hflip=…"` is a fixed rectangle chosen at start, with no shape, and it takes the device exclusively. [D man page]

### 2.11 Argument builder (the gsr report's pseudocode, verbatim) and its corrections

```rust
// Done once at daemon start, again on hotplug or package change. Off the hot path.
struct Caps { version: Ver, kms_cap: bool, drm_monitors: Map<Name,(w,h)>, codecs: Set<Codec>, audio: Set<String>, h264_max: (i32,i32) }
fn preflight() -> Caps {
  version  = run("gpu-screen-recorder --version");                // require >= 6.0.0 (-ipc); warn < 6.1.1 (A/V align), < 6.1.2 (helper hardening)
  kms_cap  = getcap("/usr/bin/gsr-kms-server").has(CAP_SYS_ADMIN); // else every recording would pop a pkexec password dialog
  drm_monitors = parse("--list-monitors");                          // "DP-2|3440x1440". HEADLESS-*, WAYLAND-* never appear
  codecs   = parse("--info" section video_codecs);  audio = parse("--list-audio-devices");
}

// Replica of gsr's int*=double truncation (capture_setup.c:222-226)
fn to_phys(l: i32, ratio: f64) -> i32 { (l as f64 * ratio) as i32 }   // ratio = mon.phys_w as f64 / mon.logical_w as f64

// Area or window rectangle -> logical ints that gsr maps back to the physical pixels we want
fn snap_for_gsr(r: PhysRect /*monitor-local*/, m: &Monitor) -> Snap {
  let ratio = m.phys_w as f64 / m.logical_w as f64;
  let lx = max_l(|l| to_phys(l, ratio) <= r.x);  let ly = max_l(|l| to_phys(l, ratio) <= r.y);  // snap outward
  let (px, py) = (to_phys(lx, ratio), to_phys(ly, ratio));
  let need_w = max(r.x + r.w - px, 128); let need_h = max(r.y + r.h - py, 128);  // hardware encoders pad below 128 px
  let lw = min_l(|l| { let p = to_phys(l, ratio); p >= need_w && p % 2 == 0 });   // even: no FFALIGN padding column
  let lh = min_l(|l| { let p = to_phys(l, ratio); p >= need_h && p % 2 == 0 });
  // Clamp inside the monitor; shift the origin if needed. gsr does not clip, and picks the monitor by the
  // centre (lx+lw/2, ly+lh/2) with inclusive edges, so the centre must stay on m.
  Snap { gx: m.logical_x + lx, gy: m.logical_y + ly, lw, lh, pw: to_phys(lw, ratio), ph: to_phys(lh, ratio) }
} // The overlay draws the snapped edges live (WYSIWYG)

fn gsr_fit(from: (i32,i32), to: (i32,i32)) -> (i32,i32) {        // exact replica of utils.c:715
  if from.0 == 0 || from.1 == 0 { return (0,0) }
  let r = from.1 as f64 / from.0 as f64; let mut w = to.0; let mut h = (w as f64 * r) as i32;
  if h > to.1 { let r2 = w as f64 / h as f64; h = to.1; w = (h as f64 * r2) as i32; }
  (w.max(1), h.max(1))
}
fn size_arg(cap: (i32,i32), o: &Opts, m: &Monitor) -> Option<(i32,i32)> {
  let mut b = cap;
  if o.scale_to_1x { b = (round(cap.0 as f64 / m.scale), round(cap.1 as f64 / m.scale)); }
  if let Some(mx) = o.max_box { b = (min(b.0, mx.0), min(b.1, mx.1)); }
  if b == cap { return None; }                                       // never pass -s >= capture: gsr upscales
  for d in 0..4 { for e in 0..4 { let (w,h) = gsr_fit(cap, (b.0-d, b.1-e)); if w%2==0 && h%2==0 { return Some((b.0-d, b.1-e)); } } }
  Some(b)                                                            // fallback: accept a 1 px black edge
}

fn build(t: &Target, o: &Opts, caps: &Caps, id: &str) -> Result<Vec<String>> {
  ensure!(caps.version >= v("6.0.0") && caps.kms_cap && caps.drm_monitors.contains(&t.monitor.name));
  let mut a = vec![];
  let cap = match t.kind {
    Display => { a.push2("-w", &t.monitor.name); caps.drm_monitors[&t.monitor.name] /* swap w/h for odd transforms */ }
    Area | WindowRect => { let s = snap_for_gsr(t.phys_rect, &t.monitor);
                           a.push2("-w", format!("{}x{}+{}+{}", s.lw, s.lh, s.gx, s.gy)); (s.pw, s.ph) } };
  let out = size_arg(cap, o, &t.monitor).map(|b| { a.push2("-s", format!("{}x{}", b.0, b.1)); gsr_fit(cap, b) }).unwrap_or(cap);
  let codec = if o.codec == H264 && (out.0 > caps.h264_max.0 || out.1 > caps.h264_max.1) { Hevc } else { o.codec };
  a.extend(["-k", codec.name(), "-c", "mp4", "-f", &o.fps.to_string(), "-fm", "cfr",
            "-bm", "qp", "-q", o.quality.name() /* default very_high */, "-tune", "performance",
            "-cursor", if o.cursor { "yes" } else { "no" }, "-exclude-metadata", "yes"]);
  let sys = "default_output".to_string();                            // follows default-sink changes
  let mic = o.mic.as_ref().map(|m| if m == "default" { "default_input".into() } else { format!("device:{m}") }); // validated vs caps.audio
  match (o.system_audio, mic, o.separate_tracks) {
    (false, None, _) => {},
    (true, None, _) => a.push2("-a", sys),
    (false, Some(m), _) => a.push2("-a", m),
    (true, Some(m), false) => a.push2("-a", format!("{sys}|{m}")),                 // one mixed track
    (true, Some(m), true) => { a.push2("-a", format!("{sys}|{m}")); a.push2("-a", sys); a.push2("-a", m); } // mix first (players), then stems (editor)
  }
  if o.any_audio() { a.extend(["-ac", "aac", "-ab", "160"]); }
  a.extend(["-ipc", &format!("{}/snip/g-{id}.sock", xdg_runtime_dir()) /* must be <= 107 bytes */,
            "-write-first-frame-ts", "yes", "-v", "yes" /* 1 Hz "update fps" health line */,
            "-ffmpeg-opts", "flush_packets=1;frag_duration=1000000",          // crash recovery (verified r12)
            "-o", &format!("{}/rec-{id}.mp4", disk_staging_dir())]);          // on disk, same fs as history, NOT tmpfs
  Ok(a)
}

fn spawn(args) -> Child {
  Command::new("/usr/bin/gpu-screen-recorder")
    .arg0("snip-gsr")                       // product decision: hides Snip from Omarchy's `pgrep/pkill -f ^gpu-screen-recorder`
    .args(args).stdin(null())
    .stdout(log_file).stderr(log_file)      // FILES, not pipes: no SIGPIPE handler in gsr
    .process_group(0)
    // To outlive snipd: put it in its own systemd scope (systemd-run --user --scope / StartTransientUnit).
    // Otherwise KillMode=control-group SIGTERMs it (gsr then finalizes cleanly).
}
// Start: wait for the socket (~25 ms), connect, send {"id":1,"name":"set-paused","data":false}.
//   Reply = recording started (~0.35–0.5 s after spawn). EOF, refused connection or exit = startup failed,
//   whatever the exit code. Watch "<out>.ts" (inotify): its monotonic µs starts the UI timer.
// Pause/resume: set-paused true|false.  Stop: {"name":"stop"} -> await the path (no kill timer), wait for exit (<=250 ms),
//   ffprobe, then remux: ffmpeg -i rec.mp4 -map 0 -c copy -t <video_dur> -movflags +faststart [-tag:v hvc1] final.mp4
// Discard: stop and delete. Restart: discard, then start again (or use the pre-roll idea in Q6 after spike S7 verifies it).
// Recovery after a signal death: ffmpeg -ignore_editlist 1 -i rec.mp4 -c copy -movflags +faststart recovered.mp4
```

**Corrections from the other reports and from synthesis:**
- **Δ1, H.264 width.** On the GPU path, H.264 also needs a physical **width ≥ 146** [V Omarchy]. Use `max(…, 146)` for `need_w`, or keep Snip's CPU fallback below 160×128 (built).
- **Δ2, CPU path.** With `-encoder cpu -k h264` there is no 128 minimum. Snap the physical **width to a multiple of 4** and the height to even. [S software.c]
- **Δ3, `stop` needs an `id`.** Send `{"id":N,"name":"stop"}`: a request without `id` is rejected and does **not** stop. [V r1]
- **Δ4, recovery.** The recovery line above keeps only one audio stream, because ffmpeg picks one stream per type by default [I]. Use §4.5's template with `-map 0` (built in 94f3f16).
- **Δ5, process name.** Snip spawns gsr **by its absolute path** (D44) instead of using `arg0`. Both hide it from Omarchy's `pgrep -f ^gpu-screen-recorder`. `/proc/PID/comm` stays `gpu-screen-reco` either way, so `pgrep -x`, `killall` and unanchored patterns still match Snip's gsr. [V]
- **Δ6, logging.** `-v no` (built) is fine with stderr in a file. `-v yes` adds a 1 Hz health line.
- **Δ7, CPU fallback flag.** `-fallback-cpu-encoding yes` exists [S args_parser.c:274]. Whether it rescues an NVENC open failure (for example CUDA after suspend) is untested.

**Where the built `crates/snip-core/src/record.rs` stands:**
- It already has `-w WxH+X+Y`, `-fm cfr`, `-ac aac`, the flush options, `-exclude-metadata`, `-write-first-frame-ts`, `-ipc`, log files and the absolute path.
- Not yet:
  - **snapping**: `area_from_physical` rounds; the area may be 1 px off and can have odd sizes;
  - **the `-s` guard**: `max_box()` is always passed;
  - **`-k h264`**: it uses `-k auto`;
  - **mix + stems**: it writes stems only.

### 2.12 Failure modes

| # | Failure | Detect | Do or show | Snip now |
|---|---|---|---|---|
| 1 | gsr missing, or older than 6.0 (no `-ipc`; the Omarchy repo ships 5.12.3) | preflight `--version` | "Install or update gpu-screen-recorder (6.1 or newer)." Disable Record | no preflight (a spawn error toast) |
| 2 | `gsr-kms-server` without `cap_sys_admin` | preflight `getcap` | Block before pkexec can prompt: "The capture helper lost its permission. Reinstall gpu-screen-recorder." | no |
| 3 | Output not recordable (HEADLESS, nested, virtual) | not in `--list-monitors`; at runtime exit 51 `display "X" not found`, or **exit 0** after `gsr_capture_start failed` | Grey out Record on that output. Treat "exited before the reply / first frame" as failure whatever the exit code | part ("Can't record: …") |
| 4 | Region off the monitor or spanning monitors | exit 51 `doesn't match any monitor` | An internal bug: snapping and clipping must prevent it. Log it | part |
| 5 | Socket path in use or too long | exit 1, `another program is already listening` / `the ipc socket path is too long` | A unique short path per take; retry once | unique path |
| 6 | Audio device missing at start | exit 50 with the list | Re-enumerate; offer "Continue without microphone" | toast |
| 7 | Device gone mid-recording | gsr writes silence and carries on | Watch PipeWire (`pactl subscribe`); show "Microphone disconnected" | no |
| 8 | NVENC/CUDA failure (after suspend, session limit, driver mismatch) | early exit; stderr `Could not open video codec`, `failed to load cuda`, `failed to start video encoder`; codes 1/52/54 | Retry once with `-k h264_vulkan`; otherwise show the stderr tail | stderr tail |
| 9 | Capture too large for H.264 | exit 53 | HEVC automatically above the H.264 maximum | `-k auto` does it |
| 10 | Monitor unplugged or DPMS off | socket2 `monitorremoved`; stderr `failed to find monitor by name` every 2 s | Stop and finalise: "Display disconnected; recording saved." | unplug built; DPMS no |
| 11 | Disk full, write errors | statvfs poll; stderr `failed to write frame index … No space left` (gsr keeps running) | Warn early; auto-stop at a threshold; never stage on tmpfs | built (1 GiB) |
| 12 | Outside stop (Omarchy ALT+PRINT, its indicator SIGINT) | IPC EOF, exit 0, path on stdout | Treat as a normal stop; avoided by the hidden argv[0] | built |
| 13 | gsr killed (OOM, GPU reset, SIGKILL) | exit by signal | Recover with `-ignore_editlist 1`: "Recording recovered; last ~1 s lost." | part (only at the next start, §7.3) |
| 14 | snipd crash | systemd SIGTERMs gsr, and the file is finalised | Pick up the finished file at restart | built |
| 15 | Slow finalise | `stop` reply delayed | "Finishing…" after 1 s; no SIGKILL timer (omarchy#12469) | part: 30 s read timeout, then SIGINT, 10 s, SIGKILL |
| 16 | Stop before the first frame | stderr `not finalizing the video file because it has no video/audio data`; reply still `ok`; ffprobe fails | "Nothing recorded." Delete | built |
| 17 | Padding and upscale artefacts | at plan time (snap, `size_arg`) | Snap; never pass a larger `-s` | no |
| 18 | Harmless stderr noise | `ignoring invalid SAR`, `Passing qmin/qmax`, the AAC `non monotonically increasing dts … -22`, `Qavg` | Filter; never report | n/a (only the last line is shown on failure) |
| 19 | Private windows recorded by KMS | D6 | Warn | no |
| 20 | Portal path (if ever offered) | `canceled by the user`, `failed to provide dmabuf`, `timed out waiting for pipewire` | Opt-in only; fall back to KMS with a notice | n/a |
| 21 | `stop` over a half-closed socket | no reply, although the file is saved | Keep the socket open for read and write until the reply | built |

### 2.13 What this means for testing

- **Real gsr cannot run in the nested e2e session**, and there it would record the real screen. Put a **gsr stub** behind an env var (e.g. `SNIP_GSR=<path>`) so the real argument builder, socket and exit handling run. The stub implements:
  - the CLI subset Snip passes;
  - the newline-JSON IPC, including the **queued reply at startup**, `set-paused`, `stop` (replying after the file is written) and the error replies;
  - the `<out>.ts` file;
  - exit codes 0, 1, 50 and 51, including the "exit 0 without a file" case;
  - SIGINT/SIGTERM finalisation, and SIGKILL leaving a fragmented file, to test recovery.
- **The stub's MP4** can come from ffmpeg lavfi with gsr's mux flags (`-use_editlist 1 -movflags +hybrid_fragmented`; see `R/gen.sh`). Or it can come from ext-image-copy-capture frames of the nested output, which would record Snip's own HUD and camera for real.
- **Today's `SNIP_RECORDER=fake`** skips gsr entirely: no IPC, no `.ts` and no exit codes; the clip is made at stop. It covers the HUD, history and editor flows only. The e2e cases are `recordings_in_history`, `record_by_command`, `record_with_the_hud`, `recording_safety` and `video_editor`. [V `tests/e2e/e2e.py`]
- **Keep a manual "live" tier**: a tiny region of the DP-2 bar, as in the research runs, with gsr hidden from Omarchy.
- **Spike S7** (still open) must cover:
  - cost at 3440×1440@60 and @144;
  - the cursor plane with a visible pointer (`-cursor yes` vs `no`);
  - A/V sync with a flash and a beep;
  - the paused pre-roll start;
  - stop latency on a long take;
  - the systemd-scope option;
  - a **mic start pop** (needs the owner's OK to record the mic);
  - `frag_duration=500000` through `-ffmpeg-opts`.

---

## 3. UI building blocks

### 3.1 The camera bubble (REC-08)

**How Qt 6.11.2 (FFmpeg backend) gets camera frames** (no webcam is attached, so these are [S] unless marked):

| Fact | Detail | Conf. |
|---|---|---|
| Backend | FFmpeg plugin → `QV4L2Camera`: raw V4L2 ioctls; FFmpeg is not used for cameras | [S] `qv4l2camera.cpp` |
| PipeWire / portal | **None for cameras.** PipeWire appears in 6.11 only for audio and screen capture. The GStreamer backend even skips PipeWire video sources ("infinite futex wait") | [S] `src/multimedia/pipewire/*`, `qgstreamervideodevices.cpp:152-155` |
| Enumeration | Scans `/dev/video*`, opening each `O_RDONLY`. Keeps devices with VIDEO_CAPTURE and STREAMING that are not META_CAPTURE (UVC metadata nodes are skipped). Formats are collapsed to min/max fps per resolution | [S] `qv4l2cameradevices.cpp:55-170` |
| Default camera | The first `/dev/videoN` in **name order** (`video10` before `video2`). A v4l2loopback node such as OBS's virtual camera can become the default | [S] `:167` |
| Hot-plug | `QFileSystemWatcher` on `/dev` → rescan → `MediaDevices.videoInputsChanged` | [S] `:36-49` |
| Pixel formats | YUV420, YUV422P, YUYV, UYVY, RGB32 variants, GREY, Y16, NV12, NV21, **MJPEG/JPEG → `Format_Jpeg`**. H.264 cameras are not supported | [S] `qv4l2camera.cpp:21-45` |
| Default format | The highest resolution with ≥29 fps; ties go to the higher fps. MJPEG is not penalised, so the result is typically **MJPEG 1920×1080@30** | [S] `qplatformcamera.cpp:19-52` |
| Frame rate | Qt always requests the format's **maximum** fps (`VIDIOC_S_PARM`). A lower rate means picking another format | [S] `qv4l2camera.cpp:621-628` |
| Device held open | Assigning a device opens it `O_RDWR` for its controls, even with `active: false`, and keeps it open. Other apps can still stream, but the UVC device stays awake. **Create the Camera on demand; destroy it after the take** | [S] `:106-121, 400-416`; [I] autosuspend |
| Exclusive streaming | Only *streaming* is exclusive: `S_FMT`/`REQBUFS` EBUSY → `errorOccurred(CameraError, "Camera is in use")`. Opening is shared | [S] `:391-398, 586-588, 643-645` |
| Thread | Frames are dequeued by a `QSocketNotifier` on the **GUI thread**, with 2 buffers. GUI-thread stalls drop frames, and the recorded bubble stutters | [S] `:686-693`, `qv4l2memorytransfer.cpp:32` |
| No camera | `error = CameraError`, "No camera detected" in the constructor, and `active` stays false. `errorOccurred` fires before QML handlers connect, so check `videoInputs.length` | [V] `R/qml/devices.qml` |
| Unplug while streaming | ENODEV; Qt stops and **closes the fd**. `active` stays **true**, with no error and no null frame (QTBUG-135869, -137118 open) | [S] `:359-373` |
| Replug | The same `QCamera` **never streams again**: toggling `active` is a no-op, and setting an equal device is a no-op. Create a new Camera (QTBUG-136979 open) | [S] `qcameradevice.cpp:285-288` |
| Created inside a `Loader` | A second instance showed blank output (QTBUG-139209). Use `createObject`; destroy, then create a tick later | [D][I] |

**CPU cost of MJPEG** (`QImage::fromData` + ARGB convert on the render thread; `R/cam/jpegbench.cpp`) [V]:

| MJPEG frame | Busy frame | Light frame | % of one core at 30 fps |
|---|---|---|---|
| 640×480 | 1.44 ms (78 KB) | 0.43 ms (4 KB) | 1.3–4.3 % |
| 1280×720 | 4.41 ms (237 KB) | 1.27 ms (11 KB) | 3.8–13 % |
| 1920×1080 | 7.79 ms (546 KB) | 2.85 ms (25 KB) | 8.5–23 % |

- QTBUG-126695, a whole app on a Xeon: MJPEG 1080p30 = 60 %, MJPEG 640×360@30 = 18 %, YUYV 640×360@30 = 2 % of a core.
- YUYV, NV12 and YUV420P are uploaded as they are and converted by a shader. [S `qvideotexturehelper.cpp:133-137, 397-398, 752-770`]
- **Use YUYV 640×480 for bubbles up to about 480 physical px.** Use MJPEG 720p or 1080p only for the camera's full-screen toggle.
- Changing `cameraFormat` while active stops, closes, reopens and restarts the stream (short freeze). [S `:126-141`]
- **Latency:** the camera's own pipeline (1–3 frames), plus GUI dequeue, the render pickup, JPEG decode and the compositor. In recordings the face lags the mic by about 50–150 ms, as in CleanShot's standard mode. [I]

**Drawing**
- `VideoOutput.mirrored: true` (6.9+) is applied in texture coordinates, so it costs nothing. [D; S `qquickvideooutput.cpp:305-330, 597`]
- `fillMode: VideoOutput.PreserveAspectCrop` crops through texture coordinates, with no extra pass. [S `:514-540`]
- **Mask options**, each adding one bubble-sized offscreen pass per frame (negligible):
  - **`MultiEffect` with `maskEnabled`** is house style (`Pin.qml`, `History.qml`, `Magnifier.qml`). Use `maskThresholdMin: 0.5; maskSpreadAtMin: 1.0` for a soft edge.
  - Alternatives: `ShapePath.fillItem` with CurveRenderer, Qt5Compat `OpacityMask`, or an SDF `ShaderEffect` (needs a `qsb` build step).
- `endOfStreamPolicy: VideoOutput.KeepLastFrame` (6.9+) and `clearOutput()` exist. Clear **before** hiding: Hyprland fades unmapped layers.

**Where the bubble lives**

| | Pinned floating toplevel (old architecture §6.3) | Layer surface / inside the HUD layer |
|---|---|---|
| Drag | `startSystemMove()`, then snap through hyprctl | Move the item or set margins; no compositor round trip when inside the HUD [I] |
| Size presets, full screen | Need Hyprland: a floating window can't resize itself (CLAUDE.md) | The client sets its own size |
| Always on top | Other floating or fullscreen windows can cover it [I] | The overlay layer is above every window |
| Focus | Needs a `no_focus`-style rule | No keyboard focus by construction |
| Multi-monitor | The compositor moves it | One output per layer (gsr records one monitor anyway) |

- **Recommended, and now the plan** (`docs/design/recording.md`, WIP): draw the bubble as an item **inside `RecordHud.qml`'s full-output layer**.
  - Add its rectangle to `Platform.setInputRects` so it can be dragged.
  - It is the one deliberate exception to "the HUD draws nothing inside the area".
  - It needs no window rules (CLAUDE.md's rule gotchas). [I: verify drag smoothness and stacking above fullscreen apps in nested Hyprland]
- **If it ever becomes a toplevel:**
  - use one match field (`xdg_tag = "^snip-camera$"` or an ASCII title);
  - set `float = true, pin = true, no_initial_focus = true, no_dim = true, no_anim = true, border_size = 0, no_shadow = true, no_blur = true, rounding = 0, opacity = "1 override 1 override", suppress_event = "maximize fullscreen", tag = "+snip-camera"`;
  - **never** use the name `WebcamOverlay`: Omarchy runs an unanchored `pkill -f WebcamOverlay`. [S Omarchy]

**Lifecycle rules**
- **Start:** open the Camera when the toolbar selects a device, before Record. The countdown hides the first-frame delay.
  - Omarchy waits 0.6 s for its mpv window to slide in. Snip's `^snip-` layer rule sets `no_anim`, so no settle wait is needed. [S `omarchy-capture-screenrecording:94-107`; I]
- **Stop:** send gsr `stop` **first**, and remove the bubble after the reply (12–33 ms). Otherwise its removal or fade can land in the last frames. Clear content before unmapping. [I; CLAUDE.md]
- **Camera lost or busy mid-take:** hide the bubble (it is recorded) and toast afterwards. A 1.5 s frame watchdog catches it; create a new Camera on replug.
- **Format:** the smallest YUYV/NV12 format covering the bubble's physical size at ≥29 fps. MJPEG only for the full-screen toggle.
- **Persistence:** the WIP stores the camera "by its name". `device.id` is an ArrayBuffer (`id === id` is false in JS), so store a decoded id string with the description as a fallback. Two identical cameras collide on description alone. [V `R/qml/idcmp`; I]

**Omarchy's bubble geometry** (the only measured numbers available) [S `omarchy-capture-webcam-resize:78-94, 137-141`; V script arithmetic `R/webcam-presets.sh`]
- It is an 8:9 portrait **centre crop** (`crop=ih*8/9:ih`) with no mirroring.
- Heights are 9/50, 1/4 and 27/80 of the anchor's height; widths are 8/9 of the height.
- For narrow regions: if 0.3·H > W−80, the sizes scale from (W−80)·10/3.
- It sits 40 px from the anchor's right and bottom, and at least 40 px from its top-left.

| Anchor (logical px) | small | medium | large | medium top-left |
|---|---|---|---|---|
| DP-2 full, 2752×1152 (3440×1440 @1.25) | 184×207 | 256×288 | 346×389 | 2456,824 |
| HEADLESS-1, 1536×864 at x=2752 | 139×156 | 192×216 | 260×292 | 4056,608 |
| 1920×1080 @1 | 172×194 | 240×270 | 324×365 | 1640,770 |
| 2560×1440 (4K @1.5) | 230×259 | 320×360 | 432×486 | 2200,1040 |
| Region 800×600+100+100 on DP-2 | 96×108 | 133×150 | 180×203 | 727,510 |
| Tall region 300×1000 | 117×132 | 163×183 | 220×247 | 97,777 |

- **Omarchy's feed:** the first of 640×360, 1280×720 and 1920×1080 at 30 fps (the smallest wins). Medium on DP-2 is 320×360 physical, exactly the cropped 640×360 feed; large is 432×486, an upscale of about 1.35×. [S `:75-84`; I]
- **Omarchy's keys:** SUPER+ALT+`[` and `]` step the size and reset the position to bottom-right. A dragged position is lost. [S]
- **Proposed for Snip** [I]:
  - CleanShot's four sizes = 18 / 25 / 33.75 / **45 %** of the recorded area's height (Huge is inferred), with Omarchy's cap for narrow areas.
  - Medium is the default, anchored 40 px from the area's bottom-right at first, then remembered relative to the area.
  - CleanShot's aspect ratios for Rectangle and Vertical are unverified; propose 16:9 and 9:16.

**`CameraBubble.qml` from the Qt report** (verbatim). It passed `qmllint` and loaded offscreen with no camera. Its format picker was unit-tested against a C920-like list: 200 → 320×240 YUYV, 300/480 → 640×480 YUYV, 600 → 1280×720 MJPEG, 900/1440 → 1920×1080 MJPEG. Camera behaviour itself is unverified.

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

**Camera pitfalls and QML quirks** [V unless marked]:
- `MediaDevices.defaultVideoInput.isNull()` throws `TypeError` in QML, although the docs suggest it; use `videoInputs.length`.
- Devices compare by value with `===`, but `device.id` is an ArrayBuffer.
- `cameraFormat.pixelFormat` is a bare int: YUYV = 17, NV12 = 18, YUV420P = 13, Jpeg = 29.
- `setCameraDevice` emits `cameraDeviceChanged` and **then** resets `cameraFormat` [S `qcamera.cpp:490-494`], so a binding `cameraFormat: pick(cameraDevice)` gets overwritten. Set the format imperatively, after the device.
- Related bugs: QTBUG-139682 (in use, yet "active" with artefacts) and QTBUG-126695 (JPEG CPU).
- **Testing:** there is no `/dev/video*` and no `videodev` module loaded. Offscreen runs use the software scene graph (`GraphicsInfo.api` = Software), so `VideoOutput` and `MultiEffect` rendering can only be checked in the nested Hyprland. A fake feed for e2e is WIP (`SNIP_CAMERA=fake`).

### 3.2 Player and trimmer (MediaPlayer + VideoOutput)

**Load and metadata** [V]
- Loading takes 6–22 ms.
- `duration`, `seekable`, `hasAudio`/`hasVideo` and `videoTracks`/`audioTracks` are correct.
- `metaData` gives Resolution (a QSize), VideoFrameRate, Duration, VideoCodec, AudioCodec, the bitrates and HasHdrContent.
- **VideoFrameRate is the average** (50.9 for a nominally 60 fps VFR file).
- `pitchCompensation` defaults to true (6.10+). `playbackRate` 0.5, 2 and 4 work.
- **Qt plays one audio track at a time** (`activeAudioTrack`) [D]. With separate mic and system tracks, the preview plays track 0 only.

**Seeking** [S `qffmpegdemuxer.cpp:272`, `qffmpegstreamdecoder.cpp:130`, `qffmpegrenderer.cpp:91`; V]
- Seeks are **exact**: `AVSEEK_FLAG_BACKWARD`, then frames ending before the target are dropped.
- The playback engine objects are rebuilt on every seek (QTBUG-122057).
- **Boundary rule:** Qt shows the first frame whose end is at or after the position.
  - At 60 fps, `setPosition(1000)` shows the frame [983.33, 1000.00), and `setPosition(50)` shows [33.3, 50.0).
  - At a keyframe (4000) it shows [4000, 4016.7).
  - `position` reports the requested value, not the frame's time.

| Paused seek latency, ms (2 s keyframe interval) | Software | CUDA | VAAPI |
|---|---|---|---|
| 1080p60 x264 | 4–28 (audio attached) | 10–118 | – |
| 3440×1440 screen-like | **55–111** | 31–274 | 34–276 |
| 3440×1440 heavy pattern | 20–300 | 27–310 | 35–317 |
| **All-intra 1280 px proxy** | **2.6 mean / 4.0 max** | 4.0 / 4.4 | – |

Latency grows with the distance from the previous keyframe: CUDA costs about 2 ms per frame at 3440×1440. Under CPU load a paused seek can deliver extra frames (QTBUG-140440).

**Frame stepping** [V]
- QML rounds fractional `position` values (16.5 → 17).
- `position += 1000/60` drifts +⅓ ms per step and skips a frame about every 50 steps.
- **Fix:** step to `Math.round((n + 0.5) * frameMs)` (built in d0befa2).
- **VFR needs real timestamps.** QML gets `frame === undefined` from `videoFrameChanged`. Use a C++ helper with `QVideoFrame::startTime()/endTime()`: next = `floor(end/1000) + 1`, previous = `ceil(start/1000) − 1`, both tested. Or use an ffprobe packet index.

**Scrubbing** (60 seeks at 8 ms intervals) [V]:

| | Frames shown during a 0.5 s drag | Last frame after release |
|---|---|---|
| Original, one seek per move | **1** (software and CUDA) | +121 ms / +281 ms |
| Original, seek gate (one in flight, newest kept; built in d0befa2) | 7 (14 fps) / 5 (10 fps) | +207 / +313 ms |
| All-intra proxy, no gate | 60 (117 fps) | +0 ms |

**Proxy**
- The command is in §4.5. It builds 20 s of 3440×1440 in 1.49 s (about 7.5 % of real time) and is 7.8 MB.
- Its timestamps are **identical** to the original's. Seeks take 3.0 ms; software playback costs 9 % of a core. [V]
- Building it with NVENC is not faster (1.8–2.8 s), because decoding dominates.
- **Always export from the original.**

**Position and mute** [S `qffmpegmediaplayer.cpp:41`; measured 19.7–20 Hz]
- `positionChanged` fires at **20 Hz** (a 50 ms timer). Extrapolate the playhead with a `FrameAnimation`; the built editor still binds the playhead to `position`.
- Skipping cuts from `onPositionChanged` (as the editor does) overshoots into the cut by up to 50 ms plus the seek latency. Preview only; export is exact.
- `AudioOutput.muted` and `volume` (linear, 0..1). `audioOutput: null` stops audio rendering.

**Hardware decode on NVIDIA** [S `qffmpeghwaccel.cpp:41-50`; V]
- Qt prefers CUDA, then VAAPI. CUDA is used only when `/proc/driver/nvidia/version` exists **and** `libnvcuvid.so` loads (QTBUG-122199).
- **Probe stall:** unless **both** `QT_FFMPEG_ENCODING_HW_DEVICE_TYPES` and `…DECODING…` are set, the first QtMultimedia use creates vdpau, cuda, vaapi, qsv and vulkan devices on the GUI thread.
  - That takes 655–746 ms in C++ and 814–1382 ms in `qml6`.
  - A whole `qml6` run with `MediaDevices` took 950–1747 ms, against 96–103 ms with both set.
  - **Fixed in d0befa2**: both are set to `,`, meaning software.
- With CUDA, each source costs about 60 ms of GUI-thread time, and frames are copied GPU → CPU → GPU.
- The only zero-copy path is VAAPI EGL dma-buf, disabled on `EGL_BAD_MATCH` (QTBUG-112312) and "mostly could not be opened" on NVIDIA (QTBUG-147731). `QT_DISABLE_HW_TEXTURES_CONVERSION=1` is the escape hatch.
- **A `QVideoFrame` that outlives a CUDA player crashes at exit** (SIGSEGV in CUDA cleanup). Never keep frames past the player. [V]

| Playback CPU, 60 fps (sink only) | Software | CUDA / +download | VAAPI / +download |
|---|---|---|---|
| 1080p | 42 % | 4 % / 8 % | 4 % / 11 % |
| 3440×1440 screen-like | 51 % | – / 10 % | – / 11 % |
| 3440×1440 heavy | 80 % | 5 % / 13 % | 6 % / 18 % |
| 1280 all-intra proxy | 9 % | 2 % | – |

First frame after `pause()` at 3440×1440: software 60 ms, CUDA 105–127 ms, VAAPI 265–283 ms.

**Sparse-frame files with an AudioOutput attached** [V, the same family as QTBUG-150596]
- On a file with 1 fps for 10 s and then 60 fps (with AAC), `pause()` showed no frame within 5 s.
- A paused seek **never** showed a frame (20 s timeout), and `play()` showed its first frame after 5.9 s.
- With no AudioOutput, frames came in 4–15 ms.
- Snip's own CFR recordings are not affected: gsr emits a frame every tick [S gsr 6.1.3 recorder.c:524-525, 833-857].
- **Cheap insurance:** attach the AudioOutput only while playing (not built).

**Thumbnail strip** (3440×1440, N = 12, 120 px tall, best of 3) [V]:

| Method | 20 s screen-like | 60 s |
|---|---|---|
| **`-discard nokey`** (keyframes only) | **0.25 s** (0.13 s on the pattern clip) | **0.32 s** |
| `-skip_frame nokey` | 0.64 s | 1.66 s |
| N accurate `-ss` inputs (Snip's `media::thumbnails`, run **before** the editor opens) | 1.05–1.6 s | 1.1–1.3 s |
| Full decode with `fps=N/dur` | 1.3–2.5 s | 3.1–7.2 s |
| In-process QVideoSink seeks | 1.4 s (software) / 4.1 s (CUDA) | 1.4 / 2.3 s |
| From the proxy | 0.12 s | – |

Open the editor at once, run the keyframe pass, and place each tile at its timestamp, or use the nearest keyframe. Swap in exact frames from the proxy later.

**GIF playback** [V; S `qquickanimatedimage.cpp:20-49, 397`]
- `AnimatedImage` with the default `cache: true` keeps every frame: 800×450 went from 169 to 294 MB RSS, and 1280×536 from 244 to 482 MB.
- With **`cache: false`**, RSS stayed flat at 90–91 MB (built in d0befa2). Decoding costs 1.15–1.5 ms per frame.
- Assigning `undefined` to `sourceSize.width` broke playback.
- `MediaPlayer` also plays GIFs, but every seek decodes from frame 0. Trim the MP4 source instead.

**Hosting a VideoOutput** [D scene graph docs; S `qsgvideonode_p.cpp:217-231`, `qquickvideooutput.cpp:480-485`; V]
- Each window has its own render thread, so a camera's MJPEG decode stays on its own window.
- With several Snip windows shown, Qt animates on a system timer.
- On `sceneGraphInvalidated` the textures are dropped: reseek after showing the window again.
- **Calling `pause()` and setting `position` inside `onMediaStatusChanged` recursed endlessly** (52 calls before a guard; a RangeError without it). Defer with `Qt.callLater` and a one-shot flag.
- An open alpha-surface ghosting bug on Wayland (QTBUG-140518): keep the video backdrop opaque.

**`TrimmerPlayer.qml` from the Qt report** (verbatim). It was exercised offscreen on `t1080.mp4`: 5 fast steps landed on frame 5, and a 40-seek drag ended idle at 6000.

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

**Qt bugs that matter** (https://qt-project.atlassian.net/browse/KEY):

| Bug | What it means for Snip |
|---|---|
| QTBUG-136979, -135869, -137118 | A reconnected camera never streams again; `active` stays true after an unplug. Recreate the Camera and run a watchdog |
| QTBUG-139682 | A camera already in use still reports active and shows artefacts. Hide the bubble on error |
| QTBUG-139209 | A Camera recreated inside a Loader shows blank. Use `createObject`, destroying first |
| QTBUG-126695 | JPEG camera formats are CPU-heavy. Prefer YUYV |
| QTBUG-140440 | A paused seek can produce several frames. Affects frame stepping |
| QTBUG-150596 | The first frame is gated by a frame count, not by time. See the sparse-file finding |
| QTBUG-122057 | Every seek rebuilds the playback objects (a fixed cost per seek) |
| QTBUG-129764 | Black screen after EOS; fixed with `endOfStreamPolicy` (6.9). Use KeepLastFrame |
| QTBUG-145575 | Qt's Slider example makes seeking jumpy. Don't feed `position` back into a pressed slider |
| QTBUG-139848 | Crash on graphics device loss (suspend, GPU reset) |
| QTBUG-140518 | Wayland ghosting with alpha surfaces. Keep the video backdrop opaque |
| QTBUG-145825 | Audio but no video on Wayland (AMD) in 6.11.0. Watch item |
| QTBUG-147731, -112312 | VAAPI zero-copy is unreliable on NVIDIA |
| QTBUG-107895, -103710 | AnimatedImage ignores `asynchronous`; AnimatedImage performance |

---

## 4. Verified ffmpeg templates (ffmpeg n9.0.1; copy exactly)

**Setup and conventions** [V]
- **Machine:** i5-13600KF (20 threads), RTX 4080 (610.57.04), ffmpeg n9.0.1 (lavc/lavf 63.1.101, lavfi 12.1.101). `/tmp` is tmpfs, so copy and remux timings will be a little slower on SSD.
- **Encoders present:** `h264/hevc/av1_nvenc`, `h264/hevc/av1_vulkan`, `libx264`, `libx265`, `libsvtav1`, `aac`, `libopus`, `gif`, `libwebp_anim`.
- **NVENC probe:** `ffmpeg … -f lavfi -i color=…:d=0.1 -c:v h264_nvenc -f null -` takes 175–200 ms. With CUDA hidden it returns rc 171 `CUDA_ERROR_NO_DEVICE`; fall back to libx264 (that probe takes 29 ms).
- **Prefix every run with:** `ffmpeg -hide_banner -nostdin -y -loglevel error -progress pipe:1 -nostats -stats_period 0.5`
- Pass each argument as its own argv element. Quotes inside filter strings are filtergraph syntax.
- `{A|B}` marks a choice: in the source tables the pipe was escaped as `\|`.
- **Status:** the export graph, the two-pass GIF, NVENC p4 with CQ = CRF + 9 and the recovery template are in Snip (94f3f16). The thumbnails, poster, proxy, Trim Only and post-stop remux are not.

### 4.1 Probe, index, poster, strips

```
# Probe (29–31 ms). Has audio = any stream with codec_type=="audio". For VFR use r_frame_rate (60/1), not avg_frame_rate (213/5).
ffprobe -v error -of json -show_entries format=duration,size,bit_rate:stream=index,codec_type,codec_name,width,height,pix_fmt,r_frame_rate,avg_frame_rate,time_base,start_time,duration,nb_frames,channels,sample_rate:stream_tags=name,title SRC

# Frame/keyframe index (55 ms for 3,600 frames; a 126 ms keyframe index in the Qt runs)
ffprobe -v error -select_streams v:0 -show_entries packet=pts_time,flags -of csv=p=0 {SRC}

# Poster: the first non-black frame at or after 0.5 s (84–91 ms). Threshold 20, not 32 (#121212 counts as 99 % black at 32)
-ss 0.5 -t 5 -i {SRC} -vf "blackframe=amount=0:threshold=20,metadata=select:key=lavfi.blackframe.pblack:value=99:function=less" -frames:v 1 -update 1 {OUT}.png
# all-black input: exit 0 and NO file → check the file exists, then fall back (84 ms):
-ss 0.5 -i {SRC} -frames:v 1 -update 1 {OUT}.png

# Strip A, D ≤ 30 s (0.38 s for 10 s 720p; 3.3 s for 60 s 1080p): exact slot centres. H = 64 × scale (80 at 1.25); sprite 1368×64, 14–42 KB
-ss {D/(2N)} -i {SRC} -an -vf "fps={N}/{D},scale=-2:{H}:flags=area,tile={N}x1" -frames:v 1 -update 1 {OUT}.png

# Strip C2, long clips (0.50 s for 60 s 1080p): the keyframe at or before t_k; cost grows with N, not duration
(-noaccurate_seek -skip_frame nokey -ss {t_k} -i {SRC})×N -filter_complex "[k:v]scale=-2:{H}:flags=area[tk];…;[t0]…hstack=inputs={N}" -frames:v 1 -update 1 {OUT}.png

# Keyframe tiles named by timestamp in ms (Qt report; 0.13–0.32 s at 3440×1440) → kf_0.jpg kf_2021.jpg kf_4021.jpg …
ffmpeg -v error -y -discard nokey -i REC.mp4 -an -vf scale=-2:144 \
  -fps_mode passthrough -enc_time_base 1/1000 -frame_pts 1 -q:v 4 DIR/kf_%d.jpg
```

Strip B (`-skip_frame nokey` with fps/tile) took 70 ms for 10 s, but short clips only have D/2 keyframes, so the last 2 tiles came out blank (Y=16). [V]

### 4.2 Trim, cuts and audio

```
# Fast trim, stream copy (37–40 ms)
-ss {A} -to {B} -i {SRC} -map 0 -c copy -movflags +faststart {TMP}
# Mute (38 ms)
-i {SRC} -map 0:v -c:v copy -an -movflags +faststart {TMP}
# Keep one track (41 ms)
-i {SRC} -map 0:v -map 0:a:{i} -c copy -movflags +faststart {TMP}
# Replace audio (0.18 s): exactly D long for 7 s or 15 s replacements; -shortest cut a 10 s video to 7 s
-i {SRC} -i {AUD} -map 0:v -map 1:a -c:v copy -af "apad=whole_dur={D},atrim=end={D}" -c:a aac -b:a 160k -movflags +faststart {TMP}
```

**Fast trim** [V]
- It is exact when A is a keyframe.
- When A is not (A = 2.4, keyframe at 2.0), the file stores frames from the keyframe plus an edit list (`media_time` 0.4 s):
  - ffmpeg-based players show exactly 294 frames, 0.0 ms off;
  - players that ignore edit lists show a 0.4 s lead-in.
- QtMultimedia uses libavformat, so it should honour the edit list [I].
- **Never use `-avoid_negative_ts make_zero`:** it starts at the previous keyframe (+0.53 s).
- The out point is frame-exact for video (no B-frames). Audio ends on a packet boundary (+13.5 ms).

**Exact trim with a re-encode:** `-ss A -to B -i SRC` (input seek, accurate) took 699 ms with NVENC and 485 ms with x264 for 4.9 s of 720p, both 0.0 ms off. [V]

**Cut methods compared** (10 s 720p in, 6.65 s out) [V]:

| Method | Time | Frame-aligned cuts | Unaligned cuts | VFR source |
|---|---|---|---|---|
| **F: one input, trim/atrim + concat** | about 1.0 s | exact | ≤1 frame (video 3.0, audio 6.7, A/V 3.7 ms) | +67 ms / 480 ms A/V without `fps`; **exact with `fps=60` first** |
| S: `-ss a -to b -i SRC` per segment + concat | 0.84 s | exact | +1 frame, 23 ms | needs `fps` per input |
| `select`/`aselect` + `setpts=N/FRAME_RATE/TB` | 0.9 s | **+2 frames, A/V −27…−43 ms** | wrong | breaks VFR timing |
| concat demuxer `inpoint`/`outpoint`, copy | 48 ms | +6 lead-in frames | +7 | n/a |
| concat demuxer, re-encode | 0.83 s | +16.7 ms, A/V −10 ms | 19.7 ms | n/a |

- **Why `fps=` first:** the `concat` filter assumes CFR. It sets a segment's length to `last_pts × n/(n−1)`. [S `libavfilter/avf_concat.c` push_frame]
- **Larger tests** [V]
  - 60 s 1080p with 3 cuts (17.15 s out): F + fps with NVENC p5 took 5.87 s (557 MB RSS); S with NVENC p5 3.76 s (637 MB); F with x264 4.06 s (1,146 MB); decoding alone 3.73 s.
  - A full export of a 2-track 3440×1440 file (mix + cuts + 1080p cap): NVENC p5 cq28 2.44 s (704 MB); x264 veryfast crf20 1.79 s (1.47 GB). All video events, all 13 merged-audio events and the 6.650 s duration were exact.

**Audio operations** (10 s, 2 tracks) [V]:

| Operation | Command | Time | Check |
|---|---|---|---|
| Mono | `-af "pan=mono\|c0=0.5*c0+0.5*c1" -c:a aac` | 188 ms | level kept (max −5.3 dB). `-ac 1` makes correlated stereo **3.1 dB louder** |
| Gain | `-af volume=6dB` | 0.2–0.4 s | clips at 0 dBFS on loud peaks |
| Gain + limiter | `volume=12dB,alimiter=limit=0.891:level=disabled:latency=1` | 195 ms | peak −0.8 dBFS. **Without `latency=1` the audio is 5.1 ms late** |
| Merge with volumes | `[0:a:0]volume=1.0[s];[0:a:1]volume=0.8[m];[s][m]amix=inputs=2:duration=first:normalize=0` | 359 ms | all 20 beeps. Same result as `amix weights='1 0.8':normalize=0` and `amerge,pan=stereo\|c0=c0+0.8*c2\|c1=c1+0.8*c3`. **The default `normalize=1` drops each input by 6 dB** |

- **Speed for 60 s of stereo:** native AAC 1.9 s, `-aac_coder fast` 0.7 s, libopus 0.32 s.
- **Tail:** a re-encode without atrim gives Opus a 13.5 ms tail. Always bound the audio with atrim or `-t`.

### 4.3 Export

```
-i {SRC} -filter_complex "{GRAPH}" -map "[v]" -map "[a]" -c:v h264_nvenc -preset p4 -tune hq -rc vbr -cq {CQ} -profile:v high -pix_fmt yuv420p -c:a aac -b:a 160k -movflags +faststart {TMP}
# fallback
-c:v libx264 -preset veryfast -crf {CQ-9}
```
Measured: 1.0 s for 10 s of 720p; 2.4 s for 3440×1440 → 1080p.

**`{GRAPH}` builder** (exact; `R/recipes.py:export_graph`). Snap every cut point to k/FPS first.
```
[0:a:0]volume=SV[as];[0:a:1]volume=MV[am];[as][am]amix=inputs=2:duration=first:normalize=0[,alimiter=limit=0.891:level=disabled:latency=1][,pan=mono|c0=0.5*c0+0.5*c1],asplit=K[ain0]…;
[0:v]fps=FPS,split=K[vin0]…;
[vin{i}]trim=start=A_i:end=B_i,setpts=PTS-STARTPTS[v{i}];[ain{i}]atrim=start=A_i:end=B_i,asetpts=PTS-STARTPTS[a{i}];…
[v0][a0]…concat=n=K:v=1:a=1[vc][a];[vc]scale=-2:'min(MAXH,ih)':flags=lanczos[v]
```
- With one track, start the audio chain at `[0:a:0]volume=SV`.
- With no audio, or muted: drop the audio parts, use `concat=n=K:v=1:a=0` and pass `-an`.
- Build the graph even for one segment: it keeps the audio's length exact.
- Scale **after** concat, so cut-away frames are never scaled.

**Quality** [V]
- `-b:v 0` made identical output in ffmpeg 9, so it is not needed.
- Equal SSIM: **NVENC CQ ≈ x264 CRF + 9 (±1.5)**:

| NVENC | UI 1080p | testsrc2 720p | 3440×1440 | x264 CRF at equal SSIM |
|---|---|---|---|---|
| cq28 | 358 KiB, .99897 | 6.5 MiB, .9946 | 36.7 MiB, .9956 | 18.6–19.8 |
| cq32 | 335 KiB, .99812 | 3.9 MiB, .9889 | 21.3 MiB, .9909 | 22.5–24.0 |
| cq36 | 268 KiB, .99673 | 2.4 MiB, .9831 | 12.9 MiB, .9871 | 25.4–27.8 |

**Proposed presets** (output size relative to the gsr source, with p4). Use stream copy for "Original" when nothing forces a re-encode. [V]

| Preset | NVENC | x264 | Size, UI / busy |
|---|---|---|---|
| High | cq24 | crf ~16 (extrapolated) | 0.83 / 0.93–0.98× |
| Medium (default) | cq28 | crf 19 | 0.70 / 0.61–0.65× |
| Low | cq32 | crf 23 | 0.62 / 0.36–0.39× |
| Very low | cq36 | crf 27 | 0.51 / 0.22–0.24× |

**Encoder speed for 10 s** [V]
- NVENC: p1 1.05 s / 2.29 s, **p4 1.46 s / 3.02 s**, p5 2.35 s / 5.0 s, p7 2.39 s / 5.4 s (UI 1080p / 3440×1440).
- x264 veryfast: 0.9–1.1 s / 2.4–3.1 s.
- x264 uses 1.4 GB RSS at 2580×1080; `-threads 8` brings it to 773 MB at +60 % time.
- At equal SSIM on UI content, `hevc_nvenc` cq28 is 8 % smaller and `av1_nvenc` cq36 is 28 % smaller.

**Resolution** (10 s of 3440×1440, NVENC p4) [V]
- 1080p cap: `scale=-2:'min(1080,ih)':flags=lanczos` → 2580×1080, 1.96 s.
- 720p cap: the same with 720 → 1720×720, 1.86 s.
- 1× of a 1.25 capture: `scale='trunc(iw/1.25/2)*2':'trunc(ih/1.25/2)*2':flags=lanczos` → 2752×1152, 2.18 s.
- GPU path: `-hwaccel cuda -hwaccel_output_format cuda` + `scale_cuda=-2:'min(1080,ih)':interp_algo=lanczos`, 1.87–2.03 s, SSIM 0.995 against the CPU path. The gain is negligible.

**Size estimates** [V]
- **Stream copy:** exact. Sum the packet sizes in the range (`ffprobe packet=pts_time,size`).
- **Instant re-encode estimate:** kept source bytes × the preset ratio × (output pixels / input pixels)^e, plus audio kbps × D/8. Measured e: 0.67–0.79 on UI, 1.2 on busy content. Error ±40 %.
- **Refined estimate:** encode 5 one-second samples in one process (0.6–2 s):
  ```
  (-ss {s_k} -t 1 -i {SRC})×5 -filter_complex "[0:v:0]…concat=n=5:v=1:a=0[v]" -map "[v]" {VENC} {TMP}
  ```
  - Estimate = the source's non-keyframe bytes × (sample P-bytes / source P-bytes in those windows) + output keyframes × the mean sample I-frame size (NVENC's default GOP is 250).
  - Error ≤10 % on busy content; +10–21 % over on UI.
  - Plain extrapolation of the samples overestimated UI content by 30–60 %.

### 4.4 GIF

```
# pass 1 (use -skip_frame nokey and drop fps= when the segment is ≥10 s)
ffmpeg … -ss A -t D -i SRC -an -vf "fps=F,scale='min(W,iw)':-2:flags=lanczos,palettegen=stats_mode=diff" -update 1 pal.png
# pass 2
ffmpeg … -ss A -t D -i SRC -i pal.png -an -lavfi "fps=F,scale='min(W,iw)':-2:flags=lanczos[x];[x][1:v]paletteuse=dither=sierra2_4a:diff_mode=rectangle" -loop 0 DST.gif
```

The template-table form of the same two passes:
```
[-skip_frame nokey] -ss {A} -t {D} -i {SRC} -an -vf "fps={F},scale='min({W},iw)':-2:flags=lanczos,palettegen=stats_mode=diff" -update 1 {PAL}.png
-ss {A} -t {D} -i {SRC} -i {PAL}.png -an -lavfi "fps={F},scale='min({W},iw)':-2:flags=lanczos[x];[x][1:v]paletteuse=dither={sierra2_4a|none}:diff_mode=rectangle" -loop {0|-1} {TMP}.gif
```
- Pass 1 takes 0.09–0.46 s on keyframes only, or 0.3–4.9 s on all frames.
- Pass 2 takes 0.2–0.6 s for a 10 s UI clip.

**10 s clips, 800 px wide, both passes** (time and KiB) [V]:

| fps | UI 720p sierra / none | UI 1080p→800 sierra / none | testsrc2 720p sierra / none |
|---|---|---|---|
| 10 | 0.56 s 1,055 / 0.45 s 987 | 0.64 s 988 / 0.51 s 922 | 1.66 s 3,765 / 1.24 s 3,251 |
| 15 | 0.71 s 1,495 / 0.48 s 1,409 | 0.83 s 1,421 / 0.67 s 1,324 | 2.16 s 5,237 / 1.59 s 4,564 |
| 20 | 0.81 s 1,893 / 0.57 s 1,761 | 1.05 s 1,791 / 0.76 s 1,655 | 2.73 s 6,886 / 1.58 s 5,946 |
| 30 | 1.14 s 2,705 / 0.84 s 2,542 | 1.40 s 2,501 / 1.03 s 2,331 | 3.61 s 9,873 / 2.01 s 8,584 |

**Dither comparison** (UI 1080p, 15 fps, size KiB / SSIM) [V]:
- none 1,324 / .9855
- floyd_steinberg 1,367 / .9856
- sierra2_4a 1,421 / .9853
- **bayer:5 1,476 / .9756**
- bayer:3 1,696 / .909
- bayer:2 1,726 / .788
- none with `max_colors=128`: 1,098 / .9847
- none with `max_colors=64`: 860 / .9835

**Other GIF facts** [V]
- `diff_mode=rectangle` halves sierra's time (1.21 → 0.64 s) at the same size.
- `mpdecimate` gave no gain. `stats_mode=full` was mixed.
- A suggested quality mapping:
  - High = sierra2_4a with 256 colours;
  - Medium = sierra2_4a or none;
  - Low = none with 128 or 64 colours (testsrc2 SSIM drops .81 → .68).
- **Memory:** a single pass held 393 MB for 10 s of UI, 613 MB at 30 fps, 714 MB at 1280 px, and **1,549 MB for 60 s of 1080p (6.2 s)**. Two passes held 231 + 203 MB (4.3 + 3.6 s).
- **Keyframe palette:** pass 1 dropped to 0.46 s, with SSIM .9850 against .9853 on UI and size +0–10 %.
- **Progress:** a single-pass GIF printed **no progress** for 4.5 s, then 12→100 % in 1.2 s.
- `-ss 2 -t 3` gave 45 frames and 3.000 s; delays at 15 fps are 7/6/7 cs.
- Without `fps` first, a 60 fps VFR source made 2/1/2 cs delays, which browsers clamp. Always put `fps` first.
- Looping: `-loop 0` loops forever (NETSCAPE count 0), `-loop -1` plays once (no NETSCAPE block), `-loop 3` writes count 3.

**Optimisers** [V]
- Both are in Arch `extra` but not installed. They were checked with sha256 against `extra.db` and run from `R/pkgs/root`.
  - `gifsicle` 1.96-1: GPL-2.0-only, depends on libx11.
  - `gifski` 1.34.0-3: AGPL-3.0-or-later, links ffmpeg.
- Commands:
  ```
  gifsicle -O3 --lossy=30 {IN}.gif -o {TMP}.gif                      # 0.4–1.2 s on UI; 6 s on busy content (cap it with a timeout)
  ffmpeg … -ss {A} -t {D} -i {SRC} -an -vf "fps={F},scale='min({W},iw)':-2:flags=lanczos" -f yuv4mpegpipe -pix_fmt yuv444p - | gifski --fps {F} -Q 90 --quiet -o {TMP}.gif -   # 0.43 s for a 3 s clip
  ```
- Results:

| Tool | UI 1080p (from 1,421 KiB sierra) | testsrc2 720p |
|---|---|---|
| `gifsicle -O3` | 1,369 KiB, 0.39 s | 4,989 KiB, 1.6 s |
| `gifsicle -O3 --lossy=30` | **854 KiB, 1.2 s, SSIM .9802** | 4,881 KiB, **6.2 s** |
| `gifsicle --lossy=80` | 794 KiB, .9733 | 4,749 KiB |
| `gifski --fps 15 --width 800 -Q 90` | **1,093 KiB, 1.09 s, .9857** | **3,984 KiB, 1.31 s, .883** (ffmpeg: .812) |
| `gifski -Q 70` | 669 KiB, .950 | 2,929 KiB, .860 |

- gifski's colour-matrix handling in the pipe is ambiguous; no colour shift was measured. [V/I]

### 4.5 Recording post-processing

```
# Recover a crashed gsr take (30–42 ms); built in 94f3f16
-ignore_editlist 1 -fflags +discardcorrupt -i {SRC} -map 0 -c copy -movflags +faststart {TMP}

# After Stop (gsr report): faststart, trim audio to the video's length, hvc1 tag for HEVC. Not built.
ffmpeg -i rec.mp4 -map 0 -c copy -t <video_dur> -movflags +faststart [-tag:v hvc1] final.mp4
# Faststart only (0.1 s for 137 MB)
-i {SRC} -map 0 -c copy -movflags +faststart {TMP}

# All-intra scrubbing proxy (Qt report; 1.49 s for 20 s of 3440×1440; identical timestamps)
nice -n 10 ffmpeg -v error -y -i REC.mp4 -map 0:v:0 -map 0:a? \
  -vf scale=1280:-2:flags=fast_bilinear -fps_mode passthrough -enc_time_base demux \
  -c:v libx264 -preset ultrafast -tune fastdecode -g 1 -crf 28 -pix_fmt yuv420p \
  -video_track_timescale 1000000 -c:a copy PROXY.mp4
```
- **Proxy flags:** all three are needed (`-fps_mode passthrough`, `-enc_time_base demux`, `-video_track_timescale 1000000`). Without `-enc_time_base demux`, VFR timestamps get quantised to 1/60 s.
- **Proxy audio:** `-map 0:a?` copies every audio track. To pre-mix the tracks for preview (Qt plays track 0 only), replace it with an `amix` chain [I].

### 4.6 Progress, cancellation and Rust rules

**Progress** [V]
- Each block arrives every 0.5 s and ends with `progress=continue` or `progress=end`:
  `frame=31 fps=0.00 stream_0_0_q=28.0 bitrate=8674.4kbits/s total_size=786480 out_time_us=725333 out_time_ms=725333 out_time=00:00:00.725333 dup_frames=0 drop_frames=0 speed=1.45x progress=continue`
- `speed` can carry leading spaces. `total_size` can be `N/A`. **`out_time_ms` is in µs**, the same value as `out_time_us`.
- Percent = clamp(`out_time_us` / expected µs), where expected = Σ(b−a) for exports and D for GIF pass 2.
- During cut-away regions the value plateaus (42 % for about 1 s).
- **Success = rc 0 and `progress=end` seen.**
- The first report waits until every output is initialised [S `fftools/ffmpeg.c:595-597`]. So **show GIF pass 1 as indeterminate**, and make it cheap with the keyframe palette. The gifski pipe reports fine with `-progress pipe:2`.

**Cancellation** (x264 slow export of the 60 s source) [V]

| Signal | Stops in | rc | Output |
|---|---|---|---|
| SIGINT / SIGTERM | 0.3–1.2 s | 255 | a valid truncated MP4 (moov written, faststart included) |
| SIGKILL | 10 ms | −9 | `moov atom not found` |
| `q` on stdin | 1.2 s | **0** | valid but truncated, and looks like success |

**Rust rules**
- Spawn with `-nostdin`, stdin null and `process_group(0)`.
- Write to a temp file **in the destination directory**, then rename on success.
- Cancel with `killpg(SIGKILL)` and unlink the temp file. Use SIGTERM only for "stop and keep the partial file".
- `PR_SET_PDEATHSIG` so ffmpeg dies with snipd [I].

**Checklist**
- Always use `-update 1` for single images, and check the file exists: exit 0 does not mean a file was written.
- Use `r_frame_rate` for the export fps; put `fps=` before `trim` and in GIF graphs.
- `normalize=0` on amix, `latency=1` on alimiter, pan-mono instead of `-ac 1`.
- Never use `-shortest` for replacement audio, and never the `q` key to cancel.

---

## 5. Omarchy integration

### 5.1 How Omarchy's recording surfaces work

Omarchy 4.0.4-1 is installed; upstream `quattro` was compared at commit b44fb74780 (#8374) and 54e3b53b88 (#14253).

- **The indicator** (`/usr/share/omarchy/shell/plugins/bar/indicators/ScreenRecording.qml:16-29`) [S]
  - It runs `pgrep --quiet -f ^gpu-screen-recorder`, only at creation, on `onBarChanged`, and when something calls `omarchy-shell -q omarchy.indicators refresh` (`Indicators.qml:168-174`).
  - There is no timer and no polling, and hovering does not re-check.
  - Callers of the refresh: `omarchy-capture-screenrecording` (lines 199-200, 214, 245-247), `omarchy-toggle-notification-silencing` and `omarchy-reminder`.
- **No IPC sets its state or click action.** `qs ipc show` lists only `refresh()` for `omarchy.indicators` [V]. `BarIndicator.activeOverride` exists but nothing exposes it. A plugin can't add an indicator except by cloning the widget. [S]
- **A click while active** runs `omarchy-capture-screenrecording --stop-recording` (`ScreenRecording.qml:38-42`). The script then: [S][I]
  1. sends `pkill -SIGINT -f "^gpu-screen-recorder"` (line 205);
  2. waits up to 5 s;
  3. refreshes the indicators, runs `pkill -f "WebcamOverlay"` (unanchored) and deletes `$XDG_RUNTIME_DIR/omarchy-screenrecord-region`;
  4. if gsr is still alive, runs `pkill -9` and shows the critical toast "Recording process had to be force-killed. Video may be corrupted.";
  5. for a gsr it didn't start, still fires "Screen recording saved" with an **empty path**, as app `omarchy-action`, which **gets past DND**.
- **A click while inactive** opens Omarchy's recording menu (`omarchy-menu.jsonc:53-63`): no audio / desktop audio / desktop + microphone / + webcam (only when a camera exists) / "Stop Screenrecording" (only while a bare-named gsr runs). Every "start" item **stops** a bare-named gsr instead of starting (lines 282-288). [S]
- **ALT+PRINT** = `omarchy-capture-screenrecording --stop-recording || omarchy-menu toggle trigger.capture.screenrecord` (`default/hypr/bindings/utilities.lua:38`). `omarchy screenrecord` and Omarchy's AI skill use the same script. [S]
- **Visibility depends only on argv[0].** A process run as `exec -a /usr/bin/x` is not matched by `pgrep -f '^x'`, and `/proc/PID/comm` is unchanged. gsr finds `gsr-kms-server` through `/proc/self/exe` and PATH, so argv[0] doesn't affect capture. [V]
- **Omarchy's gsr call** (line 191): `-k auto -f 60 -fm cfr -fallback-cpu-encoding yes -a "default_output|default_input" -ac aac` (one mixed track). Monitors use `-s 0x0`, or `-s 3840x2160` above 4K. Regions are logical `-w WxH+X+Y`. [S]
- **Omarchy post-processes in place** (lines 253-280): [S]
  - trims the first 0.1 s;
  - re-encodes (libx264 veryfast crf 20) only if `ffprobe -read_intervals %+0.2 -show_entries packet=flags` shows a `D` flag, otherwise stream-copies;
  - audio: `volume=enable='lt(t,0.4)':volume=0,afade=t=in:st=0.4:d=0.05,loudnorm=I=-14:TP=-1.5:LRA=11`.
- **Folder and name.** `$OMARCHY_SCREENRECORD_DIR`, else `XDG_VIDEOS_DIR` from a sourced `~/.config/user-dirs.dirs`, else `~/Videos`. It must already exist; the script does not create it. Files are named `screenrecording-YYYY-MM-DD_HH-MM-SS.mp4` (local time). The suggested example in `default/uwsm/default:16-17` is `$HOME/Videos/Screencasts`. Here, `XDG_VIDEOS_DIR="$HOME/Videos"` and neither variable is set. [S][V]
- **Finding videos.** `omarchy-transcode` and `omarchy-menu-file` search `~/Pictures:~/Videos` recursively and skip hidden folders, so Snip's history under `~/.local/share` doesn't appear there. [S]
- **No pause, restart or discard** anywhere in Omarchy, locally or upstream. [S]
- **Recording keys** (all live in `hyprctl binds` [V]):
  - PRINT screenshot; ALT+PRINT record/stop.
  - SUPER+ALT+code:34/35 = webcam smaller/larger.
  - SUPER+PRINT colour picker; SUPER+CTRL+PRINT OCR.
  - SUPER+CTRL+C capture menu.
  - SUPER+ALT+comma runs the last toast's action; SUPER+CTRL+comma toggles DND.
  - SUPER+CTRL+PERIOD transcode.
  - SUPER+left-drag / right-drag moves / resizes windows.
  - RETURN, CTRL+RETURN, TAB, CTRL+TAB and the arrows are bound only while a layer named **`selection`** (slurp) is open.
  - Free Print chords: SHIFT, CTRL, CTRL+SHIFT, ALT+SHIFT, CTRL+ALT, SUPER+SHIFT and SUPER+ALT + PRINT.
  - `kernel.sysrq=16` allows only sync, so ALT+PRINT is safe.
- **DND IPC** [S Service.qml:857-878; V]
  - Target `notifications`: `setDnd true|1|on|yes` (anything else = off) returns on/off; `isDnd`, `dndState` and `toggleDnd` also exist.
  - DND is saved to `~/.local/state/omarchy/notifications.json` and survives shell restarts. An `omarchy-shell` call takes about 35 ms.
  - DND is **on right now** (since 2026-10-07 01:09, before Snip's setDnd code existed, so Snip did not cause it).
  - **Toasts that get past DND:** app `omarchy-action` (the default of `omarchy-notification-send`, which Snip's OMA-05 uses) and critical `notify-send`. Toasts sit on the overlay layer, so KMS records them. [S `NotificationLogic.js:118-121`; I]
- **`omarchy-shell` needs `OMARCHY_PATH`**; it fails silently with `-q`. snipd gets it from the user manager. [S; V `/proc/<snipd>/environ`]
- **Idle.** Omarchy uses Quickshell `IdleMonitor { respectInhibitors: true }`. Hyprland counts an idle inhibitor on any visible surface, layers included [S `IdleInhibitor.cpp:20-25, 35-63`]. `omarchy-shell idle disable` persists to disk and lights an indicator.
- **Tray** [S `Tray.qml`]
  - Stock Omarchy drawers unpinned items and hides Passive ones (154-180). This user's clone `tgomareli.tray` pins every item.
  - A left click calls `Activate` unless `ItemIsMenu` (832-841). `-symbolic` icons are recoloured (770-795).
  - Snip's item: `ItemIsMenu=false`, a symbolic icon, CONSTANT properties (`app/src/tray.h:14-35`).
- **Menu extension.** `~/.config/omarchy/extensions/omarchy-menu.jsonc` is merged by id (`Menu.qml:51`, `MenuModel.js:65-95`). A `when` is a bash test run each time the menu opens. The user already uses this file. [S]
- **Plugins.** `omarchy plugin add` takes git URLs only. From a package: symlink into `~/.config/omarchy/plugins/<id>/`, then `omarchy-shell shell rescanPlugins` and `omarchy plugin enable <id> --before omarchy.indicators`. The user's talkify plugin is the pattern: a `service` runs `<daemon> watch` (JSON lines) and the widget binds to it. [S; V symlinked plugins load]
- **Omarchy's webcam** is an mpv window: `--title=WebcamOverlay --wayland-app-id=WebcamOverlay-<small|medium|large>`. Its rules are float, pin, `no_initial_focus` and `no_dim`, and mpv's `--stop-screensaver` keeps the screen awake as a side effect. Geometry is in §3.1. [S `apps/webcam-overlay.lua`; V mpv options]

### 5.2 Recommendations

1. **Keep gsr hidden from Omarchy** (D44, built: absolute path). Omarchy's indicator, ALT+PRINT, its menu and its toggling script then never stop Snip's take, and never show the misleading toast.
2. **Handle outside stops anyway:**
   - `pkill gpu-screen-recorder` without `-f`, and `killall`, still match through `comm`.
   - Count "gsr exited" as a normal stop only once the `.ts` file exists (built).
   - Keep the IPC connection open until the reply (built).
   - SIGTERM from `systemctl` saves the file [V].
   - A non-fragmented MP4 is lost after SIGKILL or SIGHUP; Snip's fragment options cover SIGKILL.
3. **Recording indicator (spec Q4; design Q4):** Snip's own surfaces.
   - **Zero install:** while recording, the tray item becomes the stop button: `Activate` stops, a red icon without `-symbolic` (via `NewIcon`), and the elapsed time in the tooltip. This needs NOTIFY properties and SNI signals; whether Quickshell listens to them is unchecked. On stock Omarchy, onboarding should offer to pin the item.
   - **Optional bar plugin (OMA-04):** the talkify pattern, with `snip watch` JSON lines, a red dot and mm:ss. Left click stops; a menu offers pause, restart and discard. Place it with `--before omarchy.indicators`.
   - **Rejected:** reusing Omarchy's indicator. It brings the misleading toast past DND, "start" items that stop Snip, no timer, no paused state, and a stale icon if a refresh is missed.
4. **ALT+PRINT:** offer it in the `snip_print_keys` opt-in family as record/stop.
   - Never take it over only during a recording: both binds would fire, and Omarchy's would open its menu.
   - With a Lua config, Omarchy's bind can't be restored later (`hyprctl binds` shows only `__lua`).
5. **Camera size keys:** do **not** add temporary SUPER+ALT+code:34/35 binds.
   - The Omarchy report suggested adding them and removing them at stop. But `HL.Keybind:remove()` erases **every** bind with the same modmask, key and keycode, so it would delete **Omarchy's** webcam binds until `hyprctl reload`. [S; syn]
   - Offer sizes in the camera menu and on the bubble, plus optional, unbound-by-default Snip shortcuts.
   - If a temporary bind on a shared key is ever needed, bind it once per Lua state and toggle `set_enabled` rather than removing it [I].
6. **Camera defaults:** CleanShot's semantics with Omarchy's measured size ladder (§3.1).
   - Map the bubble before the countdown, with `no_anim`, so frame 0 needs no wait.
   - Keep the position the user chose; Omarchy's reset to bottom-right is a weakness.
7. **Folder:**
   - Keep `$XDG_VIDEOS_DIR/Screencasts`, Omarchy's own suggested example, and create it when needed.
   - Optionally honour `OMARCHY_SCREENRECORD_DIR` and `OMARCHY_SCREENSHOT_DIR` when set; `paths::user_dir` ignores the environment today.
   - Snip's `Snip … at ….mp4` names never collide with `screenrecording-…`.
8. **DND, idle and toasts:**
   - Reuse `notifications isDnd/setDnd`, but **persist "DND was off before"** in `$XDG_RUNTIME_DIR/snip/`. Today it is held in memory (`engine.rs` `dnd_holds`), so it is lost on a crash or a `scripts/dev.sh` restart, while Omarchy persists DND.
   - **Send no toasts during a recording:** `omarchy-action` toasts get past DND and would be recorded.
   - **Keep the screen awake** with a `zwp_idle_inhibitor_v1` on a mapped Snip layer; the HUD layer is mapped for the whole take. Never use `omarchy-shell idle disable`.
9. **Capture menu (OMA-03, offered in onboarding):** add `trigger.capture.snip-record` ("Record with Snip", `snip record`), and `trigger.capture.snip-stop` with `"when": "test -e \"$XDG_RUNTIME_DIR/snip/recording\""`. That needs a marker file, which also helps scripts.
10. **Names to avoid:** `WebcamOverlay` anywhere in a command line or title; a layer named `selection`; a bare `argv[0]` of `gpu-screen-recorder`.
11. **Selection limits:** offer recording only on outputs gsr lists for KMS; HEADLESS-1 is disabled with its reason. Snap per §2.2. Use `-w WxH+X+Y`, never `-region`.
12. **Audio post-processing:** don't copy Omarchy's loudnorm or its 0.4 s mute by default. Monitor sources have no pop [V]; revisit the mic after S7.

---

## 6. Hyprland inputs (clicks, keystrokes) and window recording

### 6.1 Keystrokes: `hl.on("input.keyboard.key", fn)`

[S Hyprland v0.56.2 efb5099; V stubs]

| Argument | Meaning | Notes |
|---|---|---|
| 1 | XKB keycode (evdev + 8) | e.g. 38 = A, 37 = Control_L, 133 = Super_L; Snip's `keys.rs` uses these codes |
| 2 | `SKeyEvent.timeMs`, uint32 ms | The input event's own time: libinput's, CLOCK_MONOTONIC in practice [I]. The wiki's "Unix timestamp" is wrong; check against the low 32 bits of Snip's monotonic ms |
| 3 | 0 released, 1 pressed | The wiki's 2 = repeated never comes from libinput (repeat is client-side). Ignore anything else |

`LuaEventHandler.cpp` L179–185:
```cpp
dispatch("input.keyboard.key", 3, [&](lua_State* L) {
    lua_pushinteger(L, keyEvent.keycode + 8); // Because to xkbcommon it's +8 from libinput
    lua_pushinteger(L, keyEvent.timeMs);
    lua_pushinteger(L, keyEvent.state);
});
```

- **The payload** has no keysym, no modifier mask and no device; modifiers arrive as their own key events.
  - `hl.is_key_down("Control_L" | keycode)` checks physical keys. [S LuaBindingsToplevel.cpp L418–458]
  - The callback's return value is ignored: **a hook cannot swallow keys**.
- **When it fires.** It is the first thing `onKeyboardKey` does (`InputManager.cpp` L1619–1632), so it fires:
  - whatever has focus, including fullscreen games;
  - for keys that binds consume;
  - under shortcut inhibitors and input-capture;
  - for **virtual keyboards** (wtype, IME keyboards; L1156–1159);
  - **while the session is locked, including the password.**
  - It does not fire for a keyboard disabled in config or denied by permissions.
- **Duplicates here.** `hyprctl devices -j` shows `hl-virtual-keyboard-fcitx5` as `"main": true`. Under an fcitx5 grab, each key fires the hook twice: once for the physical key, once for fcitx5 forwarding it. Dedupe with a down-state machine. [V; S SeatManager.cpp L161–172]
- **Privacy.** The hook sees password fields, pinentry and the lock prompt. macOS has Secure Event Input (Apple TN2150) and Hyprland has nothing like it.
  - Filter at the source: "command keys only" by default.
  - Stay silent for private window classes **and private layer namespaces**: Omarchy's polkit prompt is the layer `omarchy-polkit` with exclusive focus. [V PolkitAgent.qml L221–228]
  - Mute while locked; Lua can't see the lock state (§6.7).
- **Lifetimes:**

| Object | Remove | Twice | After `hyprctl reload` |
|---|---|---|---|
| `HL.EventSubscription` (`hl.on`) | `sub:remove()`, `sub:is_active()` | **safe** (`LuaEventSubscription.cpp` L34–43) | gone: `clearEvents()`, then a new `lua_State` (`ConfigManager.cpp` L700–725, L506–520) |
| `HL.Timer` (`hl.timer`) | no remove; `t:set_enabled(false)` | no-op on an expired timer (`LuaTimer.cpp` L37–51) | cancelled |
| `HL.Keybind` (`hl.bind`) | `b:remove()` = `b:unbind()` | **crash** (§6.4) | gone (`clearKeybinds`) |

- **Limits** [S ConfigManager.hpp L107–114, ConfigManager.cpp L857–870, L520–529; V]
  - Callbacks run on the compositor thread under an instruction-count watchdog: **50 ms** for events and timers, 100 ms for keybinds, 250 ms for eval.
  - **A callback error shows an on-screen notification** ("Runtime error in lua", 5 s), which would land in the recording. Wrap callback bodies in `pcall`.
  - Hyprland runs **Lua 5.5**: `io`, `os` and `package` are available, `debug.sethook` is removed, and **a for-loop variable is constant**.

### 6.2 Clicks

- **There is no Lua mouse event in 0.56.2.** `input.mouse.button` exists on the bus (`InputManager.cpp` L750), but `knownEvents()` doesn't expose it (`LuaEventHandler.cpp` L266–303). A plugin would break on every ABI change. [S]
- **Use a bind** (`LuaBindingsToplevel.cpp` L58–256; wiki binds/flags.md):
  ```lua
  local h = hl.bind("MOD3 + MOD5 + mouse:272", function() pcall(on_click) end, {
    non_consuming = true,    -- the press and release still reach the app
    ignore_mods = true,      -- also fires on Ctrl/Shift+click and SUPER+drag; modmask is then ignored for matching
    transparent = true,      -- can't be shadowed by other binds
    submap_universal = true, -- also fires inside submaps (you have a "service" submap)
    dont_inhibit = true,     -- also fires when the focused app inhibits shortcuts (VMs, remote desktops)
    description = "Snip: recording clicks",
  })
  ```
  - 272 is left, 273 right, 274 middle.
  - The callback gets **no arguments**, so use one bind per button.
  - Don't use the `click` or `drag` options: they imply release and drag-threshold logic.
  - For releases, add a second bind with `release = true, non_consuming = true` on another unused modmask (e.g. `MOD2 + MOD3 + MOD5`).
- **Behaviour** [S KeybindManager.cpp]
  - **Passes through:** a non-consuming press leaves `found = false`, so the event is sent (L499–505, L835). Clicks, drags, DnD and text selection are unaffected; e2e should confirm.
  - **All matching binds fire** (L631–763), so Omarchy's SUPER+drag still moves windows.
  - **Not while locked** without `locked = true` (L649).
- **Cursor position:** `hl.get_cursor_pos()` returns global logical floats, the same space as `hyprctl cursorpos` (which truncates): 1339.391, 572.619 vs 1339, 572. Physical = (p − monitor.pos) × scale, which is (1674.24, 715.77) on DP-2. Call it inside the callback. [S LuaBindingsQuery.cpp L282–296; V]

### 6.3 Transport from Lua to Snip

| Option | Latency | Compositor cost and risk | Privacy | Verdict |
|---|---|---|---|---|
| **socket2 `custom>>`** via `hl.dispatch(hl.dsp.event(s))` | **3.1 µs median, 46 µs p99** [V synthetic `R/hop.c`, 3000 samples] | Non-blocking writes. Payload cut at 1024 characters; newlines become spaces; **a client with 64 queued events is removed** [S EventManager.cpp:126-190; syn] | Broadcast to every socket2 client: here snipd, quickshell and Omarchy's `socat` monitor-watch [V `ss -xp`] | **Recommended.** The official API; Snip already reads socket2 |
| 0600 file in the runtime dir + inotify | 2.9 µs / 44 µs | A tmpfs `write()` never blocks; `io.open` may fail | Private | Switch to it if "all keys" becomes the default |
| FIFO or `io.popen(cmd,"w")` | n/a | **Blocks the compositor** when the reader stalls (64 KiB pipe) | n/a | Reject |
| `os.execute`, `io.popen(…,"r")`, `hl.exec_cmd` per event | ≥ 1 ms | Forks a 250 MB Hyprland per event; `os.execute` blocks, and the watchdog can't interrupt C calls | n/a | Reject |
| `hl.dsp.global` per key | n/a | One shortcut per keycode, no payload | n/a | Reject |

- **Why socket2 is acceptable:**
  - The latency is negligible against frame pacing (7–14 ms at 144 Hz, 17–33 ms at 60 Hz).
  - Filtering at the source means plain typing never leaves the compositor in the default mode.
  - Any same-user process can already install its own hook with `hyprctl eval`. The real risk is a debug script logging socket2.
- **Snip-side requirement (new):** `snip_hypr::subscribe` runs the handler inline and **never reconnects after EOF**. If a stalled handler ever overflows the 64-event queue, Hyprland drops Snip's socket2: `monitorremoved` and `configreloaded` are lost until a restart.
  - Hand events off to a channel, and reconnect on EOF. [V code; S]

### 6.4 Hazards with `HL.Keybind`

[S LuaKeybind.cpp L11–14, L46–87, L104–171; KeybindManager.cpp:197-202; syn]

1. **Collateral removal.** `remove()` calls `removeKeybind(modmask, {key, keycode, catchAll})`, which erases **every** matching bind in **every** submap. Key strings are case-sensitive.
   - The user has SUPER+mouse:272 "Move window" (modmask 64). Modmask 160 (MOD3|MOD5) is unused here [V `hyprctl binds`], hence the module's choice.
   - **Snip's session keys** (`keys.rs::session_lua`) bind bare `"Return"` and `"Escape"` with modmask 0. Removing them also removes any user bind spelled exactly `Return` or `Escape` with no modifiers, in any submap.
     - It is harmless here: the user's service submap uses `"escape"`, and Omarchy's slurp binds use `"RETURN"`.
     - A safer form: a unique modmask with `ignore_mods`, as for clicks. That would also give **Alt+Enter = GIF** by checking `hl.is_key_down("Alt_L")` in the same callback. [I]
2. **Crash on an expired handle.** `getKeybindFromUserdata` returns an always-engaged `std::optional` from `WP::lock()`. So `remove()`, `set_enabled()`, `is_enabled()` **and reading any field** on a dead handle dereference null.
   - A handle dies when you removed it, when a same-key bind was removed (point 1), or after `hl.unbind`.
   - `tostring(h)` is safe: it returns `"HL.Keybind(expired)"`. `==` is also safe.

### 6.5 The Lua module (exact)

Checked against a mock of the 0.56.2 semantics: 33 checks pass under lua5.5 and lua5.4 (`R/mock-test.lua`).

```lua
-- Snip recording hooks for Hyprland 0.56 (Lua config). Sent whole with
-- an eval or repl request at record start and after every configreloaded event;
-- idempotent. SNIP_REC.off() removes everything.
SNIP_REC = SNIP_REC or {}
local S = SNIP_REC

-- Remove everything this module installed. Safe to call any number of times.
function S.off()
  local key, btns = S.key, S.btns
  S.live, S.key, S.btns = false, nil, nil -- clear first: no handle is removed twice
  if key then key:remove() end -- HL.EventSubscription:remove() is idempotent
  for _, b in ipairs(btns or {}) do
    -- HL.Keybind:remove() on an expired handle segfaults 0.56.2; tostring() is safe
    if tostring(b) ~= "HL.Keybind(expired)" then b:remove() end
  end
  if S.wd then S.wd:set_enabled(false) end
  return "off"
end

-- cfg.sid: session id. cfg.keys: nil (no keystrokes) or { all = bool,
-- mods, cmd, special = sets of xkb keycodes }. cfg.private: set of window
-- classes, cfg.private_layers: set of layer namespaces (a password prompt
-- such as "omarchy-polkit") to stay silent in. cfg.buttons: { 272, 273 } or {}.
function S.on(cfg)
  S.off()
  S.sid, S.muted, S.miss, S.down = cfg.sid, false, 0, {}
  S.private, S.private_layers = cfg.private or {}, cfg.private_layers or {}
  local event, dispatch, fmt = hl.dsp.event, hl.dispatch, string.format
  local function emit(s) dispatch(event("snip:" .. S.sid .. ":" .. s)) end

  local function private()
    local w = hl.get_active_window()
    if w ~= nil and S.private[w.class] then return true end
    for ns in pairs(S.private_layers) do
      for _, l in ipairs(hl.get_layers({ namespace = ns })) do
        if l.mapped then return true end
      end
    end
    return false
  end

  local k = cfg.keys
  if k then
    local function cmd_held()
      for c in pairs(k.cmd) do
        if S.down[c] then return true end
      end
      return false
    end
    local function on_key(code, ms, st)
      if st ~= 0 and st ~= 1 then return end
      local down = st == 1
      if (S.down[code] == true) == down then return end -- an IME's virtual keyboard echo
      S.down[code] = down or nil
      if not (k.all or k.mods[code] or k.special[code] or cmd_held()) then return end
      if S.muted or private() then return end
      emit(fmt("k:%d:%d:%d", code, st, ms))
    end
    S.key = hl.on("input.keyboard.key", function(code, ms, st) pcall(on_key, code, ms, st) end)
  end

  S.btns = {}
  for _, btn in ipairs(cfg.buttons or {}) do
    local function on_click()
      if S.muted then return end
      local p = hl.get_cursor_pos()
      if p then emit(fmt("c:%d:%.2f:%.2f", btn, p.x, p.y)) end
    end
    -- MOD3+MOD5 only gives the bind a modmask nobody else uses: remove() takes out
    -- every bind with the same modmask and key. ignore_mods makes it match anyway.
    S.btns[#S.btns + 1] = hl.bind("MOD3 + MOD5 + mouse:" .. btn, function() pcall(on_click) end, {
      non_consuming = true,
      ignore_mods = true,
      transparent = true,
      submap_universal = true,
      dont_inhibit = true,
      description = "Snip: recording clicks",
    })
  end

  -- Watchdog: one timer per Lua state, reused; off() once Snip stops beating.
  S.wd = S.wd or hl.timer(function()
    if not S.live then return end
    S.miss = S.miss + 1
    if S.miss > 3 then S.off() end
  end, { timeout = 1000, type = "repeat" })
  S.live = true
  S.wd:set_enabled(true)
  return "on"
end

function S.beat(sid)
  if S.live and sid == S.sid then
    S.miss = 0
    return "on"
  end
  return "off"
end

function S.mute(on)
  S.muted = on == true
  return S.muted and "muted" or "unmuted"
end
```

### 6.6 Requests over `.socket.sock`

Always send a **leading `/`** and **`shutdown(Write)` after writing**. Snip's `request()` already does both (5894153).

| When | Request | Reply |
|---|---|---|
| Install (at start and after `configreloaded>>`) | `/repl <module>\nreturn SNIP_REC.on({ sid = "7f3a9c21", keys = { all = false, mods = {[37]=true,[105]=true,[50]=true,[62]=true,[64]=true,[108]=true,[133]=true,[134]=true}, cmd = {[37]=true,[105]=true,[64]=true,[108]=true,[133]=true,[134]=true}, special = {[9]=true,[22]=true,[23]=true,[36]=true,[119]=true,[111]=true,[113]=true,[114]=true,[116]=true} }, private = {["1password"]=true,["1Password"]=true,["Bitwarden"]=true,["chrome-nngceckbapebfimnlniiiahkandclblb-Default"]=true}, private_layers = {["omarchy-polkit"]=true}, buttons = {272, 273} })` | `on` |
| Heartbeat (every 1 s while recording) | `/repl SNIP_REC and SNIP_REC.beat("7f3a9c21") or "off"` | `on`, or `off`, which means reinstall |
| Mute / unmute (lock, pause) | `/eval SNIP_REC and SNIP_REC.mute(true)` | `ok` |
| Stop, discard, restart | `/eval SNIP_REC and SNIP_REC.off()` | `ok` |

- **`repl` returns values; `eval` doesn't.** An empty-string result comes back as `unknown request`. [S HyprCtl.cpp L1110–1124; V `hyprctl repl 'hl.version()'` → `0.56.2`]
- **Without the leading `/`**, a request containing `/` gets its first word parsed as flags. The `r` in `repl` sets `reloadAll`, which reloads monitor rules, resets keyboard layout and pointer configs, and re-applies window rules. [S L2069–2096, L2126–2150]
- **Requests of exactly n×1023 bytes** blocked Hyprland's main thread before the `shutdown(Write)` fix. [S L2242, L2271–2281]
- **Key sets:** compute `mods`, `cmd` and `special` from Snip's own xkb keymap, so remaps like Caps→Ctrl are respected; the codes above are US defaults.
- **Watchdog:** without heartbeats, the hooks are gone within 4–5 s (covers `kill -9` and a hung Snip). A config reload removes them at once.
- **Alternative heartbeat:** check `/proc/<pid>/stat` from the timer. It needs no IPC but misses a hung Snip.

### 6.7 Snip's side

- **Events** arrive on the existing socket2 reader as `custom>>snip:<sid>:k:<code>:<state>:<ms>` and `custom>>snip:<sid>:c:<button>:<x>:<y>`. Ignore other sids.
  - Feed keys to `xkb_state_update_key`, taking the layout from `activelayout>>`, to get labels, held modifiers and repeat counts.
  - Map clicks to their monitor and physical pixels. Drop clicks on Snip's own controls.
- **Lock** (Lua can't see it in 0.56.2; Omarchy notes "Hyprland reports no lock state directly"):
  1. At lock, Hyprland posts `activewindowv2>>` with **empty** data [S SessionLockManager.cpp L96, FocusState.cpp L241–247]. Mute at once.
  2. Then read `j/monitors`. If any `solitaryBlockedBy` contains `"LOCK"`, stay muted; otherwise unmute. [S Monitor.cpp L1834–1837, HyprCtl.cpp L150]
  3. Repeat the check with every heartbeat, as a backstop for locking while focus was already empty.
- **Private windows beyond the static set** (optional): on `activewindowv2>>ADDR`, run `getprop address:0xADDR no_screen_share` (works here) and mute while it is true. [V]
- **Pause:** mute while paused.
- **Drawing:** in the HUD layer, inside the area, so it is recorded. Add no input region for it.
- **`hyprctl binds` parser:** skip the "Snip: recording clicks" binds (modmask 160).

### 6.8 Verification plan (S9, in the nested e2e)

The key hook fires for wtype keys, because the emit runs before any bind logic, and harness clicks go through `onMouseButton`. So both can be tested in the nested session:
- **Keys:** install, send Ctrl+C with wtype, and check that the `k:` lines arrive in order within 5 ms.
- **Click passthrough:** a test client receives press, release and a full drag, while `c:272` lines arrive.
- **Watchdog:** stop the heartbeats. Within 5 s `hyprctl binds` has no "Snip: recording clicks" and `beat` answers `off`.
- **Reload:** run `hyprctl reload`. The hooks are gone, and Snip reinstalls them.
- **Idempotent:** install twice and `off()` twice; `hyprctl version` still answers.

### 6.9 Window recording

**What CleanShot did** [V/D]
- 4.7.6 and 4.8.11 carry "Drag to record a part of the screen." followed by the `selectWindowVideo` Space asset and " to select a window."
- The 1.0 changelog lists "Single window recording", and [DS] describes "snap to a specific window by pressing the spacebar".
- 4.7.6, 4.8.11 and 5.0.1 reference only `initWithDisplay:excludingApplications:exceptingWindows:` and `setSourceRect:`; never `initWithDesktopIndependentWindow:` or `initWithDisplay:includingWindows:`. So **Space snaps a fixed display rectangle**, with no tracking evidence. [V grep; I]
- 5.0 added the automatic background ([Digital Trends](https://www.digitaltrends.com/computing/the-best-screenshot-app-for-mac-just-became-a-screen-recording-powerhouse/)), which is outside this scope.

| | (i) gsr region (KMS) | (ii) gsr `-w portal` | (iii) Snip's own toplevel capture + encoder |
|---|---|---|---|
| Follows the window | No (fixed rectangle) | Yes (toplevel export) [S XDPH Screencopy.cpp L375–381] | Yes |
| Covered by other windows | Records what is on screen | Never | Never |
| Window resized | Fixed | gsr keeps the **initial** size and scales into it on black [S gsr portal.c L288–301, L340–373] | Snip handles it |
| Snip's keystroke, click and camera overlays | **Recorded** | Not recorded (needs burn-in) | Composited in-process |
| Translucent areas | As seen | Alpha cleared to **black** [S portal.c L360] | Alpha available |
| `no_screen_share` windows | Recorded | Black | A "denied" texture |
| Outputs | DRM only (no HEADLESS-1) | Any | Any |
| UI | Snip's overlay | The XDPH picker **every time**, unless Snip becomes the picker | Snip's overlay |
| Robustness here | Omarchy's default path | Omarchy keeps it opt-in; windows capped at 60 fps (XDPH default) | New NVIDIA code (dmabuf → EGL/CUDA or Vulkan Video) |
| e2e in nested Hyprland | Yes (with the stub) | No [I] | Yes |
| Cost | Days | About a week, plus taking over the user's `xdph.conf` | Weeks |

- **Making Snip the portal picker** [S XDPH ScreencopyShared.cpp L28–128, PortalManager.cpp L70–82]:
  - XDPH runs `screencopy:custom_picker_binary` synchronously (with `--allow-token`). It passes `XDPH_WINDOW_SHARING_LIST` entries `handleLo[HC>]class[HT>]title[HE>]<address>[HA>]` and expects `[SELECTION]<flags>/window:<handleLo>` on stdout.
  - It needs a `~/.config/hypr/xdph.conf` edit (the user's consent), an XDPH restart, and a racy handoff file.
  - **Restore tokens restore by handle, else by the first window of the same class** (Screencopy.cpp L197–226), which can be the wrong window.
- **Path (iii) costs** [I]:
  - A 2000×1250 shm frame means a synchronous `glReadPixels` on Hyprland's main thread of about 2–6 ms per frame, which stutters the desktop at 60 fps.
  - Dmabuf needs GBM buffers plus zero-copy into NVENC or Vulkan encoders (`h264_vulkan` is present; gsr calls Vulkan "experimental").
  - Plus PipeWire audio, A/V sync, pause and a crash-safe container.
  - `QVideoFrameInput` + `QMediaRecorder` headers are installed; hardware encode through Qt is unverified.
- **Recommendation:** **v1 = (i)**, as built and matching CleanShot 4.x:
  - the **client area** (`at`/`size` without the border, as `omarchy-capture-region` L48–49 does);
  - clamped to the window's monitor and **snapped inward** (§2.2);
  - **re-read by address after the countdown**;
  - the camera, keystrokes and click rings inside the rectangle, the controls outside;
  - disabled on outputs that gsr KMS doesn't list.
- **(iii) is the P2 path** for CleanShot-5-style window recording with a background. (ii) would be at most an opt-in, as Omarchy does.

---

## 7. Contradictions and open questions

### 7.1 Contradictions between the reports

| # | Topic | Report A | Report B | Resolution |
|---|---|---|---|---|
| 1 | Hiding gsr from Omarchy | gsr: `arg0("snip-gsr")` | Omarchy: absolute path | Both verified. Keep the absolute path (built, D44). `comm` still matches `killall` either way |
| 2 | Small areas | gsr: snap up to ≥128 px | Omarchy: NVENC H.264 needs width ≥146 | The ≥128 rule alone fails H.264 at widths 128–145. GPU path: even, w≥146 (H.264), h≥128 (else padded). CPU path: w%4==0, even h. Snip uses the CPU encoder below 160×128 (built) |
| 3 | Separate tracks | gsr: mix + stems (3 tracks) | Qt: one track plays | Mix on track 0 plus stems (built: stems only) |
| 4 | Recovery command | gsr: no `-map 0` (drops the 2nd audio track [I]) | ffmpeg: `-map 0 -fflags +discardcorrupt` | ffmpeg's (built in 94f3f16) |
| 5 | Fragment length | gsr: `frag_duration=1000000` (verified with gsr r12) | ffmpeg: consider `500000` (verified with an ffmpeg writer only) | Keep 1 s; test 0.5 s in S7 |
| 6 | Thumbnails | Qt: `-discard nokey` (0.13–0.32 s) + proxy frames | ffmpeg: method A for ≤30 s, C2 for longer; `-skip_frame nokey` left blank tiles | Not in conflict: `-discard` (demuxer) beats `-skip_frame` (decoder). Use the keyframe pass at open, exact frames later |
| 7 | VFR | ffmpeg: "gsr records VFR by default; `fps=` first" | Snip passes `-fm cfr` | concat is exact on CFR anyway; keep `fps=` first for imported or recovered files |
| 8 | Camera size keys | Omarchy: temporary SUPER+ALT+[/] binds removed at stop | Hyprland + [syn]: `remove()` erases every same-key bind | Don't do it (§5.2 item 5) |
| 9 | Camera surface | Omarchy: toplevel with rules | Qt: layer favoured | The bubble inside the HUD layer (the plan; WIP) |
| 10 | Camera look | CleanShot: circle, Flip **on**, four sizes, Vertical | Omarchy: unmirrored 8:9 rectangle, three sizes | CleanShot's semantics with Omarchy's numbers (Q2–Q3) |
| 11 | Audio start pop | Omarchy mutes 0.4 s (mic pop 130–200 ms) | gsr: no pop on monitor sources [V]; mic untested | No trim for system audio; mic → S7, with the paused pre-roll as the remedy |
| 12 | `hyprctl` answers | CLAUDE.md: "eval answers only ok or an error" | Hyprland: `repl` returns values | Complementary; always send `/repl` with the leading slash |
| 13 | Class+title window rules | CLAUDE.md: never match | Omarchy report: the source shows AND semantics; the old miss is explained by `SNIP_APP_ID` and an em-dash title | Unverified at runtime; keep one-field rules (no change needed) |

### 7.2 Repo docs against the findings (f0136b8 fixed several; these remain)

- **`docs/02-architecture.md` §6.3 and §9 Q-S5** still read "socket2 … or a 0600 file", with the leaning "**0600 file**". `docs/04-risks-and-edge-cases.md` R7 says "Private 0600 file transport". **`docs/design/recording.md` says socket2.** One needs to change: record the choice (Q20).
- **`docs/04-risks-and-edge-cases.md` B6** still says:
  - "Daemon crash → gsr keeps running in its own group… re-attach". Under `KillMode=control-group` it is SIGTERMed and finalises; recovery at start (as architecture §3 now says).
  - "Audio pop at the start → … trim the first 300–400 ms". No pop on monitor sources; the mic is untested.
  - "Odd physical dimensions → snap to even". Add the ≥128 / ≥146 / ×4 rules.
- **`docs/02-architecture.md` §4.3**
  - It still shows `-k auto` (→ `-k h264` + HEVC fallback, Q9).
  - It claims the overlay "snaps the rectangle to what gsr can express". The code rounds instead (§7.3 item 1).
- **`docs/03-performance.md`**
  - "Trimmer open ≤ 300 ms … keyframe thumbnail strip 140 ms": the editor still waits for N accurate seeks (1–1.6 s at 3440×1440).
  - "Trimmer seek ≤ 50 ms … proxy 26–38 ms (CLI)": the in-process numbers are 55–310 ms on originals and 2.6–4 ms on the proxy.
  - "GIF export … (M, ffmpeg diff/bayer)" and "`fps=15` first (gsr records VFR)": now two-pass `sierra2_4a`, and Snip records CFR.
- **`docs/01-product-spec.md`**
  - REC-12 "fps 24/30/60/native (≤ 144)" and "HEVC or AV1 optional": CleanShot had 10–60 and H.264 only. These are Snip extensions to decide (Q8, Q9).
  - REC-13 lacks GIF quality, Optimize, the conversion card, Save as a Video and Trim the GIF.
  - REC-02 lacks Alt+Enter, ⤢, W×H and the settings button.
  - §7 lacks the Restart and Toggle Camera Fullscreen globals.
  - VED-02 lacks Trim Only, the 7 quality stops and the summary.
  - REC-14 lacks "Continue Without Audio".
- **`docs/design/recording.md`**
  - "**Safety (REC-14, D47)**" should cite **D46** (D47 is the defaults).
  - Its gsr line `-k auto` matches the code; see Q9.
- **Historical research docs** (not edited by policy) are superseded by `docs/research/recording-2026-10/`:
  - inventory §6.1: window recording "since 5.0" is wrong; 4.x had Space [V];
  - §6.3: FPS and max-resolution lists unverified, and Vertical "Studio only"; all verified, and Vertical is 4.x [V];
  - §6.2: low-disk auto-stop; 4.x only warned [V by absence];
  - `processing-pipelines.md`: GIF with bayer:5 in one pass;
  - `tech-stack-and-prior-art.md`: `-w region -region`.
- **CLAUDE.md gotchas worth adding** (suggestions only; editing it is the user's call):
  - `HL.Keybind:remove()` removes **every** same-modmask, same-key bind, and **any** access to an expired handle crashes, not only a second remove;
  - Lua callback errors show an on-screen notification, so use `pcall`;
  - `/repl` returns values;
  - socket2 drops a client at 64 queued events.

### 7.3 Current code against the findings

As of 05:16 +04.

| # | Where | Finding | Fix | Status |
|---|---|---|---|---|
| 1 | `record.rs` `area_from_physical` | Rounds and doesn't snap: up to 1 px off at 1.25, odd sizes padded with a black column or row | §2.11 snap, with Δ1/Δ2 per encoder; draw the snapped frame | open |
| 2 | `engine/record.rs` `max_size: cfg.max_box()` | `-s` is passed even when the box is larger than the capture, **so gsr upscales** (e.g. an 800×600 area with "1080p" becomes 1440×1080) [V r3] | `size_arg()`; add 480p and Scale to 1× | open |
| 3 | `engine/record.rs` `codec: "auto"` | Fine here; `-k auto` can pick HEVC (`hev1` tag) | `-k h264` + HEVC above the max + `-tag:v hvc1` on remux | open, minor |
| 4 | `record.rs` `Audio::sources` | Separate = stems only; Qt previews and most players hear track 0 only | Mix + stems | open |
| 5 | `engine/record.rs` `record_watch` → `record_finish` | A gsr killed mid-session (SIGKILL/OOM) is ingested as is: no `fragmented()` check, so the edit list hides the first fragment | Route through `media::fragmented` → `recover` | open [I code] |
| 6 | `engine/record.rs` `record_end_run` | After a failed `stop`: SIGINT, 10 s reap, then SIGKILL | Show "Finishing…" and don't kill a finalising gsr; recovery covers a kill anyway | minor |
| 7 | `engine/record.rs` `record_finish` (GIF) | The MP4 is deleted once the GIF exists | Keep it, for Save as a Video and Trim the GIF (Q30) | open |
| 8 | `engine/video.rs` | Exports are written to `$XDG_RUNTIME_DIR/snip/export` (tmpfs, 3.2 GB); a long 4K export can fill RAM | Write beside history on disk, then rename | open [I] |
| 9 | `media::thumbnails` + `video_open` | 12 accurate seeks before the window opens (1–1.6 s at 3440×1440) | Open first; keyframe pass; proxy frames later | open |
| 10 | `media::poster` | First frame, possibly black | First non-black frame ≥0.5 s (§4.1) | open, minor |
| 11 | `media::probe` | `avg_frame_rate` drives `fps=` in the cut graph; wrong for VFR inputs (`snip edit FILE` accepts any file) | Use `r_frame_rate` | open, minor |
| 12 | `keys.rs::session_lua` | Bare `Return`/`Escape` (modmask 0): removal is collateral | Unique modmask + `ignore_mods`; Alt check for GIF | open, low risk |
| 13 | `snip_hypr::subscribe` | No reconnect after EOF; the handler runs inline | Channel + reconnect | open |
| 14 | `engine.rs` `dnd()` | The previous DND state lives in memory only | Persist it in `$XDG_RUNTIME_DIR/snip/` | open |
| 15 | `VideoEditor.qml` | Playhead bound to `position` (20 Hz); AudioOutput always attached; no `KeepLastFrame` | `FrameAnimation` extrapolation; attach on play; `endOfStreamPolicy` | open |
| 16 | `RecordHud.qml` | Restart and discard act at once | CleanShot's confirmations | open |
| 17 | Timer | Starts when the `.ts` file is seen (150 ms polling) | Use its monotonic µs value | minor |
| 18 | Settings | GIF fps capped at 30 (CleanShot 5–60); 24/30/60 fps; no 480p | Q8, Q10 | open |
| 19 | Fixed already | select/aselect cuts, one-pass GIF, bayer, NVENC p5 with CQ = CRF, recovery without `-map 0` (94f3f16); QT_FFMPEG env, seek gate, mid-frame steps, GIF `cache: false` (d0befa2); `/eval` + `shutdown(Write)` (5894153) | n/a | done |

### 7.4 Open product questions, with defaults so work can proceed

D43 is decided. D44–D47 are proposed and await review. Record each answer below as **D48 onward**. Questions 1–7 of `docs/design/recording.md` are marked [design Qn].

| # | Question | Recommended default (proceed with this) | Alternative | Basis |
|---|---|---|---|---|
| Q1 | Where the camera bubble lives | Inside the recording HUD layer (WIP) | A separate `snip-camera` overlay layer; a pinned toplevel | Self-sizing, above everything, no focus or rules; drag without round trips [I] |
| Q2 | Camera look [design Q7] | **Circle, Medium, mirrored** on screen and in the recording (WYSIWYG); 40 px from the area's bottom-right at first, then remembered relative to the area | Unmirrored, like Omarchy | CleanShot Flip default on [V]; WIP config already defaults to this |
| Q3 | Camera size ladder | Small/Medium/Large/Huge = 18/25/33.75/**45 %** of the recorded height, capped for narrow areas; Rectangle 16:9, Vertical 9:16 | CleanShot's real numbers are unknown | Omarchy [S]; Huge and the ratios [I] |
| Q4 | Camera fails or is taken mid-take | Hide the bubble, keep recording, toast afterwards | Freeze the last frame | It is recorded [I] |
| Q5 | Camera full-screen format | Switch to MJPEG 720p/1080p when toggled (short freeze) | Always 720p MJPEG (4–13 % of a core) | §3.1 CPU numbers [V] |
| Q6 | Countdown default [design Q1] | **Keep 3 s** (D47): it hides gsr's ~0.45 s start and the camera warm-up | CleanShot: off | [V] timings |
| Q7 | DND default | Keep **on** (D47): `omarchy-action` toasts bypass DND and would be recorded | CleanShot: off | [S] |
| Q8 | Frame rates | CleanShot's list **10, 15, 24, 25, 30, 48, 50, 60** (default 60); no "native ≤144" in M4 | Spec's native ≤144 | [V BIN]; 144 Hz cost unmeasured (S7) |
| Q9 | Codec | H.264 only in the UI (`-k h264`, HEVC automatically above the H.264 max) | HEVC/AV1 options (spec REC-12) | CleanShot H.264 only [V] |
| Q10 | Max resolution and 1× | Add **480p** and "**Scale to 1×**" (default off) | n/a | [V BIN] |
| Q11 | Separate-tracks layout | **Mix on track 0 + mic and system stems** | Stems only (built) | Players and Qt use one track [D][V] |
| Q12 | Merging audio | In the editor (D45); no post-recording dialog | CleanShot's dialog | Owner's words; [V BIN] |
| Q13 | Record-time mono | Post-stop remux with `pan=mono` when on (default off) | n/a | gsr is stereo only [S] |
| Q14 | Small areas | Keep the CPU encoder below 160×128 physical, snapping width ×4 and height even; GPU path: even, H.264 w≥146 | Enlarge small selections to 128/146 | Exact WYSIWYG [S][V] |
| Q15 | Low disk | Keep auto-stop at 1 GiB (D46) **plus** a CleanShot-style warning before start under about 3 GB ("Don't warn me again") | Warn only (CleanShot) | [V BIN] |
| Q16 | Daemon crash | A crash ends the take cleanly and it is recovered at restart (built) | systemd scope + re-attach | Simpler [V] |
| Q17 | Visibility to Omarchy | Hidden (D44) | Use Omarchy's indicator | Both reports |
| Q18 | Bar [design Q4] | The tray icon becomes the stop button with the time while recording; bar plugin later (OMA-04) | A bar plugin now | Zero install |
| Q19 | Record key [design Q2] | Offer **ALT+PRINT** (record/stop) in the opt-in PRINT family; no default otherwise | No key | Spec §7; Omarchy habit |
| Q20 | Keystroke transport (Q-S5) | **socket2 `custom>>`** with source filtering; switch to the 0600 file if "all keys" becomes the default | 0600 file (architecture leaning, R7) | §6.3; fixes the doc conflict |
| Q21 | Keystroke default [design Q6] | **Command keys only**: modifiers, keys held with Ctrl/Alt/Super, and Esc/Tab/Enter/Backspace/arrows/F-keys | All keys | CleanShot default [V]; privacy |
| Q22 | Lock during a take [design Q5] | **Pause the video and mute keys**; the user resumes | Mute keys only | R19 |
| Q23 | Outputs gsr can't record (HEADLESS-1) | Disable recording there, with the reason | Portal fallback | [V] |
| Q24 | Window recording v1 | Fixed client-area rectangle, re-read after the countdown; true window capture P2 | Portal opt-in | CleanShot 4.x [V] |
| Q25 | Scrubbing proxy | Build the 1280 px all-intra proxy **when the editor opens** (background, swapped in when ready); cache it with the capture; pre-mix the tracks | Build after every recording | 7.5 % of real time [V] |
| Q26 | Editor quality ladder | CleanShot's 7 stops on NVENC CQ: Ultra 20, Very High 24, **High 28 (default)**, Medium 32, Low 36, Very Low 40, Poor 44 (x264 = CQ − 9); "Original" = stream copy when nothing forces a re-encode | Built 4 stops (High = CRF 23 → CQ 32, the report's "Low") | [V] for CQ 24–36; ends [I] |
| Q27 | Trim Only | Add it: stream copy when only the ends change and nothing else does | Always re-encode | 37–40 ms [V]; edit-list caveat (§4.2) |
| Q28 | GIF optimiser | gifsicle as an optional dependency; "Optimize GIFs" on when installed (`-O3 --lossy=30` with a timeout); gifski "HQ" later as a CLI | None | CleanShot default on [V]; licences [V] |
| Q29 | GIF quality control | Map 0–100 to dither and colours: High (≥80) = sierra2_4a/256, Medium = none/256, Low = none/128 or 64 | n/a | §4.4 [V]; mapping [I] |
| Q30 | Keep the source of a GIF | Keep the MP4 beside the GIF in history, for Save as a Video and Trim the GIF | Delete it after conversion (built) | CleanShot [V] |
| Q31 | Audio post-processing | None by default (no loudnorm, no start trim); revisit the mic after S7 | Omarchy's −14 LUFS + 0.4 s mute | [V] no pop on monitor sources |
| Q32 | Folder [design Q3] | Keep `~/Videos/Screencasts`; use `OMARCHY_SCREENRECORD_DIR` when set and Snip's own folder isn't | Ignore it | Omarchy report |
| Q33 | Minimum gsr | Require ≥ 6.0.0; no warning for 6.1.0; trim audio to the video's length after stop | Warn below 6.1.1 | Arch `extra` = 6.1.0 |
| Q34 | Restart and Delete confirmations | Yes, with CleanShot's texts (§1.3) | None (built) | [V BIN] |
| Q35 | Private windows (D6) | Warn before start if a `no_screen_share` window intersects the area; a banner if one appears during the take | n/a | D6, R6 |
| Q36 | Mic test in S7 | Ask the owner to allow one short mic recording, to measure the start pop and the pre-roll | Skip | Research never recorded the mic |