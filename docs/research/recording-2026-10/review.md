# Completeness critique: the M4 recording brief, checked against the code as it is now

I checked this between 05:40 and 06:05 +04 on 2026-10-09, against HEAD `7d7d148` plus the working tree. The implementing session kept committing while I worked, so line numbers may drift.

**The brief's "not built yet" list (§0.1) is out of date.** These are now built:
- the camera bubble (`9d73f47`)
- clicks and keystrokes (`39d7e96`)
- keep-awake and pause-on-lock (`87513fc`)
- Restart/Delete confirmations and Alt+Enter for GIF (`a56b70c`)
- the muted-mic flag in the toolbar (`2225a3d`)
- uncommitted: the CAP-12 cover that hides the desktop while recording

Everything below is checked against that code, not the brief's snapshot.

**Markers:** [V] verified here now · [S] read in source (path:line) · [D] documented or web · [I] inferred. `R` = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research`. My files are in `R/critic/`.

**CleanShot 4.x parity:** I compared brief §1 with the inventory's §6. No 4.x recording feature is missing from the brief's tables. Two 4.x behaviours can't be copied on KMS, and both are already decided: controls left out of the capture, and controls shown during full-screen takes.

---

## Gaps, most important first

### 1. The countdown does not hide gsr's startup, and the HUD says "recording" about 0.5 s before anything is recorded
**Status: closed, see C1.**
- **As built, gsr starts only after the countdown.**
  - QML calls `Engine.recordGo()` only after the countdown has left the screen (`app/qml/RecordHud.qml:236-246`).
  - `record_go` then starts gsr (`crates/snip-bridge/src/engine/record.rs:300`), sets `State::Recording` (`:310`) and sends `elapsed: 0` (`:330`) straight away.
  - Restart calls `record_go` straight away too, with no countdown.
- **The daemon's own trace shows the delay.** It took 491 ms and 488 ms from the `gsr` spawn line to the first-frame status [V `journalctl --user -u snip-dev`, takes at 04:27 and 04:54].
- **The docs say the opposite.** `docs/design/recording.md:62` and brief Q6/§3.1 say "the countdown hides it".
- **Effect:**
  - On every take, roughly the first 0.5 s after "1" is lost, which is where people start talking.
  - The red frame and the 0:00 timer are wrong for that time.
  - The argument that the countdown also hides the camera's warm-up fails for the same reason.
- **To close:** C1 has the measurements. Pick one:
  - (a) A "Starting…" HUD state until the `.ts` file appears.
  - (b) A paused pre-roll, with the rule from C1 for the frame it leaks.

### 2. Keep-awake (`87513fc`) probably does nothing during a take where nobody touches anything
**Status: closed at source level; the fix was verified at runtime. See C2.**

### 3. The camera bubble has never opened a real V4L2 device, and its format picker asks for too much
**Status: open. It can't be closed here: there is no `/dev/video*`, v4l2loopback is not installed, and `sudo` needs a password. The vivid module is present [V].**
- **The picker asks for twice the logical size, not the screen scale.**
  - `pickFormat(device, Math.ceil(Math.min(width, height) * 2))` (`app/qml/CameraBubble.qml:46`) uses a factor of 2. The screen's scale (dpr) here is 1.25.
  - The default Medium bubble on the whole of DP-2 is 288 logical px, so it asks for 576 px.
  - On a C920-class camera, YUYV at 29 fps or more stops at about 800×448 [I, typical UVC tables]. The picker will therefore choose MJPEG at about 1024×576.
  - That contradicts the design doc ("uncompressed, at most about 640×480"). It also costs the CPU JPEG decode the brief warned about.
- **A stalled GUI thread shows in the video.**
  - Camera frames are dequeued on the GUI thread [S brief §3.1].
  - `record_pause` runs on the GUI thread and blocks on a gsr socket read with a 30 s timeout (`engine/record.rs:134`, `:394`).
  - Any stall therefore shows in the recording as a frozen bubble.
- **Not measured at all:** first-frame latency, "camera in use", unplug and replug with the watchdog, CPU cost per format, and the bubble's frame pacing during a take.
- **To close:**
  1. Load a virtual webcam: `sudo modprobe vivid n_devs=1 node_types=0x1 num_inputs=1 input_types=0x0`. It offers 320×180, 640×360 and 1280×720 at 10–60 fps, YUYV only, no MJPEG ([D] https://docs.kernel.org/admin-guide/media/vivid.html).
  2. Check what Qt will see: `v4l2-ctl -d /dev/video0 --list-formats-ext`.
  3. Run snip-dev without `SNIP_CAMERA` on a tiny area.
  4. Test "in use": run `v4l2-ctl -d /dev/video0 --stream-mmap --stream-count=600` while the bubble runs.
  5. Test unplug: `v4l2-ctl -d /dev/video0 -c disconnect=1` (vivid's "Disconnect" control).
  6. Unload: `sudo modprobe -r vivid`.
  - MJPEG and real latency still need a real webcam.
  - Code fix: divide by the screen scale, not by 2.

### 4. Stop edge: the HUD and the desktop cover go away before gsr stops, and gsr 6.1.3 is already in Arch
**Status: partly closed, see C3.**

### 5. No fallback when the GPU encoder can't start (CUDA broken after suspend, driver mismatch)
**Status: closed, see C4.**

### 6. gsr's errors reach the user as gsr's last stderr line, and the saved microphone's name can change
**Status: partly closed, see C5.**

### 7. Separate tracks: the brief's Q11 layout (mix + stems) would double the editor's default export
**Status: closed by reading the source.**
- The editor sets every track to 100 % (`app/qml/VideoEditor.qml:46`) and mixes every live track with `amix` (`crates/snip-core/src/media.rs:106`).
- With three tracks (mix, system, mic), the sound is exported twice: +6 dB and clipping [S].
- Tracks show up as anonymous "Track 1/2" (`VideoEditor.qml:275`), because `-exclude-metadata yes` strips their titles.
- Today, with stems only (`separate_tracks`, default false, `config.rs:202`), the editor's preview plays track 0 only, which is system audio [D, Qt's `activeAudioTrack`].
- **To close:**
  - Record the layout, either in history metadata or with Snip's own `-metadata:s:a:N title=` at a post-stop remux.
  - When stems exist, the editor treats track 0 as preview only.
  - Add a unit test of the export graph with 3 tracks.

### 8. The planned start, pause and stop sounds would be recorded
**Status: open (a design rule).**
- `play()` runs `pw-play` (`crates/snip-bridge/src/engine.rs:1902`), a separate PipeWire client.
- System audio (`default_output`) records it, and so does a mic near speakers.
- gsr throws audio away only while paused [S, `audio_capture.c:213` per the brief].
- **To close:**
  - Start sound: play it, wait for it to end, then unpause.
  - Pause sound: play it after `set-paused true` is answered.
  - Resume sound: play it before unpausing.
  - Stop sound: play it after the `stop` reply.
  - Check the order in e2e from the trace events.
- **Alternative (untested):** play with `pw-play -P '{ application.name = "Snip" }'` and record `-a "app-inverse:Snip"`. Testing it means playing a sound, which needs the owner present.

### 9. e2e: only the harness's environment keeps real gsr out of the nested session, and nothing checks what ends up in the video
**Status: closed by reading.**
- `tests/e2e/e2e.py:117-119` sets `SNIP_RECORDER=fake` for every snipd it starts [S]. That works today.
- Snip has no check of its own: `record_go` never compares the output with `gpu-screen-recorder --list-monitors`.
  - Per gsr's source, a snipd in the nested session without that variable would record the real DP-2 [S brief §2.10].
  - On HEADLESS-1 today, the frame turns red for about 0.4 s, then a toast appears (run r8: exit 0, no file).
- The fake recorder makes its clip at Stop. So e2e can never show that the bubble, rings and keystrokes are in the video and the controls are not.
- **To close:**
  - Add a preflight that refuses outputs missing from `--list-monitors`. It also greys out HEADLESS-1.
  - Add a stub that captures the nested output with `grim -o OUT` in a loop at a low frame rate (grim is installed and the harness already uses it), muxes it with ffmpeg, then checks pixels.

### 10. Codec and size: the brief's `-k h264` advice is wrong; NVENC's real limits
**Status: closed [V].**
- `--info` lists codec names but no maximum sizes [V `R/info.txt`]. `-k auto` already chooses H.264 whenever the size fits, otherwise HEVC [S `codec_select.c:476-480`].
- Measured with ffmpeg on this RTX 4080, driver 610.57.04 (`R/critic/nvenc-*.txt`):

| Encoder | Min width | Min height | Max size |
|---|---|---|---|
| H.264 | 145 (146 after gsr's even rounding) | between 34 and 64 | 4096 per side; 4098×1440 and 5120×1440 fail |
| HEVC | 129 (128×128 fails) | between 33 and 64 | 8192 |
| AV1 | 129 | above 64 | not probed |

- **What this means:**
  - Keep `-k auto`. Forcing `-k h264` would fail on monitors 5120 px wide.
  - gsr's 128 px floor is too small for HEVC and AV1.
  - Snip's CPU-encoder threshold (160×128) is safe.
- **Still open in the code:**
  - `area_from_physical` rounds where gsr truncates (`snip-core/src/record.rs:121-127`), so odd widths get a black column.
  - `-s` (maximum size) is always passed (`engine/record.rs:282`), so gsr upscales small areas.

### 11. "Show cursor: off" only works while Hyprland uses a hardware cursor
**Status: partly closed.**
- `cursor:no_hardware_cursors` is 2 (auto). On NVIDIA that switches to software cursors when VRR is on, with more than one GPU, or while tearing [S `hyprland-src/src/output/Monitor.cpp:2290-2307`].
- Here VRR is off, there is one GPU and the format is XRGB8888 [V hyprctl]. So the cursor plane is used, and gsr can leave the pointer out.
- With VRR on, the pointer is baked into the frame and `-cursor no` can't remove it. Settings should say so.
- **Not verified:** that gsr draws the hardware cursor at the right place and size at 1.25 scale. The pointer sat at 1339,572, outside my test strip, and I may not move it.
- **To close:** with the user's OK, record a 160×128 area under the pointer with `-cursor yes`, then with `-cursor no`.

### 12. The Hyprland event thread can stall on gsr
**Status: open [S].**
- socket2 events call `record_input` directly on the event thread (`crates/snip-bridge/src/bridge.rs:572-584`).
- `record_input` takes the recording mutex. `record_pause` holds that mutex while it waits for gsr: about 0.45 s during startup, or up to 30 s if gsr hangs.
- Hyprland drops a socket2 client once 64 events queue up [S brief §0.3]. `subscribe` never reconnects (`snip-hypr/src/lib.rs:216`).
- Keystroke events now go over socket2. Losing the connection would lose keystrokes, `monitorremoved` and `configreloaded` until Snip restarts.
- **To close:**
  - Hand events to a channel and reconnect on EOF (brief §7.3 #13).
  - Send gsr requests off the GUI thread, outside the mutex, with a 2 s timeout.
  - Test with a stub gsr that never answers.

---

## Closed now (the top 5 that local checks could close)

### C1: start edge and paused pre-roll (gap 1) [V]

**Method.** `R/critic/gsrrun.py` ran 5 live takes on a 400×34 logical strip at the centre of the bar (`-w 400x34+1176+0`), which includes the clock.
- Each take lasted at most 2.2 s.
- gsr ran by its absolute path with argv[0] set to `snip-critic-gsr`, so Omarchy never saw it.
- p1, p3 and p4 recorded system audio only, never the mic.
- Each take: send `set-paused true` the moment the socket appears, unpause after the clock's minute changes, then stop.
- The clock's minute digit tells the frames apart (`R/critic/digit.py`, columns 290–340). A real minute change measures as a max difference of 130.

| Run | Audio | Socket | Started (pause reply) | Unpause → first packet (`.ts`) | Frames (of 30 expected) | Frame 0 |
|---|---|---|---|---|---|---|
| p3 | system | 23 ms | 416 ms | 68 ms | 27 | new minute (diff 18): clean |
| p4 | system | 42 ms | 412 ms | 41 ms | 28 | **old minute (diff 132): leaked** |
| p5 | none | 30 ms | 372 ms | 41 ms | 28 | **old minute (diff 132): leaked** |
| p1 | system | 38 ms | 399 ms | 37 ms | 34 of 36 | inconclusive (see below) |
| p2 | none | 36 ms | 436 ms | 34 ms | 35 of 36 | inconclusive (see below) |

p1 and p2 are inconclusive because the bar clock itself changed about 0.45 s late on screen, after the unpause.

**Findings**
- **A pre-roll works.** gsr answers the queued pause when recording starts, 372–436 ms after it is launched. It stays paused, and the first packet comes 34–68 ms after the unpause.
- **But one frame leaks.** In 2 of the 3 runs that could show it (with and without audio), the frame captured at the moment recording starts ends up in the file. The frame counts agree: 28 against 27.
  - Cause [S]: `gsr_recorder_run` sets `paused=false` (`R/gsr-6.1.0/src/recorder/recorder.c:804`). Its loop captures (`:862`) before it applies a pause request (`:863`). The IPC thread starts just before the loop (`src/cli/main.c:497-499`). That is a race.
  - Neither 6.1.0 nor upstream master (`7f9405e`) has an option to start paused.
- **Consequence:** if Snip pre-rolls with the countdown drawn inside the area, about 2 takes in 3 start with a frame showing the "3". The card's poster is the first frame, so it would show it too.
- **Rule to adopt:**
  - Launch gsr when Record is pressed.
  - Show the countdown only after the pause reply arrives, or draw it outside the area.
  - Unpause at zero, after `notifyNextFrame`.
  - Take the poster from 0.5 s or later.
  - With no countdown (CleanShot's default), use the "Starting…" state from gap 1 instead.
- **The `.ts` file is written at the first encoded packet, not the first capture.** That is about 2–3 frames later; the brief's own runs show +51 ms at 60 fps and +101 ms at 30 fps. Snip also polls for it every 150 ms. Both delay the timer.
- **gsr 6.1.0 loses the last 2–3 frames**, not one: 27–28 frames of 30. The brief says "the last frame".
- With a pre-roll, audio starts at the unpause: 0.512 s of audio against 0.45–0.467 s of video for 0.5 s of recording.

### C2: keep-awake inhibitor (gap 2) [S for the bug, V for the fix]

**What Snip does [S].** It builds the `snip-awake` surface in this order (`crates/snip-wayland/src/state.rs:324-355`):
1. create a surface
2. make it a 1×1 background layer
3. commit it with no buffer
4. create the idle inhibitor

The buffer is attached only later, when the layer's configure event arrives (`:655-670`).

**What Hyprland 0.56.2 does with that [S].**
- It evaluates an inhibitor once, when the inhibitor is created (`src/managers/input/IdleInhibitor.cpp:32`).
- A layer's surface object exists from `get_layer_surface` on (`src/desktop/view/LayerSurface.cpp:28`, `:76`). So the inhibitor is not treated as "not part of the desktop, assume visible".
- `aliveAndVisible()` requires the surface to be mapped (`View.cpp:41-50`, `LayerSurface.cpp:103-105`). At creation it isn't, so the first evaluation says "not inhibiting".
- When the layer maps (`CLayerSurface::onMap`, `:162`), nothing re-evaluates.
- Re-evaluation happens only when:
  - a window maps or unmaps (`Window.cpp:2494`, `:2704`)
  - a window gets focus (`FocusState.cpp:146`, `:214`)
  - fullscreen changes (`FullscreenController.cpp:504`)
  - another inhibitor is created or destroyed

**Result.** In a take where nobody touches a window, the inhibitor does nothing.
- Stock Omarchy starts its screensaver at 150 s and locks at 300 s (`/usr/share/omarchy/shell/plugins/services/idle/Service.qml:17-18`; its IdleMonitor has `respectInhibitors: true`, `:251-256`).
- The screensaver gets recorded. The new pause-on-lock covers only the lock.
- It is the screensaver window's own map that finally triggers a re-evaluation, too late.

**Why nobody noticed.**
- This user's idle timeouts are 36000 s (`~/.config/omarchy/shell.json`, "idle").
- The e2e test only checks that the layer is mapped (`tests/e2e/e2e.py:1644`, `:1658`).

**Probe [V]** (`R/critic/idleprobe/idleprobe.c`; it creates no window or layer; it held off the session's idle for 6 s):
- An idle notification that respects inhibitors, with a 1000 ms timeout, fired at 1000.5 ms when there was no inhibitor.
- With an inhibitor on a surface that has no role and no buffer, it did not fire in 6 s.

**Fix.**
- Put the inhibitor on a role-less surface and drop the layer, or create the inhibitor after the first buffer commit.
- In e2e, run the probe inside the nested Hyprland during a fake take with no input, and assert there is no `idled` event.

**Not run:** the layer-ordering bug itself. Reproducing it needs a mapped layer, which my rules forbid. The e2e probe would close it.

### C3: stop edge and the gsr 6.1.3 update (gap 4) [V/D/S]

**Arch already ships gsr 6.1.3.**
- Arch `extra` has gpu-screen-recorder **6.1.3-1**, built 2026-09-23 [D https://archlinux.org/packages/extra/x86_64/gpu-screen-recorder/].
- This machine has 6.1.0-1, and its package database is from Sep 8 [V `pacman -Si`/`-Qi`]. The next `pacman -Syu` will install 6.1.3.

**What changes in 6.1.1 and later [S].**
- gsr flushes the video encoder at stop (commit `0261850`, `gsr_recorder_flush_video`, `R/gsr-src/src/recorder/recorder.c:873-893`; in tags 6.1.1–6.1.3, absent in 6.1.0).
- A/V alignment changed (`ad6981c`, `19f94e1`).
- `gsr-kms-server` was hardened (`4de50ca`).
- So every timing and A/V number in brief §2.3 and §2.5 holds for 6.1.0 only. That includes the planned "cut the audio to the video's length" remux.

**Snip's stop order today** (`engine/record.rs:597-612`, `:552-571`):
1. `record_close` closes the HUD (bubble, rings, keystrokes). `close()` sets `visible=false` in the same handler (`RecordHud.qml:115-117`), so Hyprland fades a snapshot of it for about 150 ms.
2. Keep-awake off.
3. `desktop("show")` (the uncommitted CAP-12 cover): the bars come back.
4. The click and keystroke hooks are removed (a Hyprland eval).
5. Only then does gsr get `stop`.

On 6.1.0, those last ~50 ms are lost along with the 2–3 unflushed frames (C1). On 6.1.3 they will be in the file.

**To close.**
- Stop gsr first and wait for its reply. Then clear the HUD's content, unmap it, and restore the desktop.
- After the update, rerun `python3 R/critic/gsrrun.py preroll …` and the brief's r1/r2 takes. Measure tail frames, A/V start and audio overhang again.
- Do one live take with the fake camera on a tiny area and check the last frame.

### C4: GPU encoder failure (gap 5) [V]

I simulated a broken CUDA with `CUDA_VISIBLE_DEVICES=`. CUDA can't load, but KMS capture still works. Same bar strip, each take 1.8 s at most.

| Run | Flags | Result |
|---|---|---|
| c1 | Snip's current flags | socket at 21 ms; IPC connection reset at ~180 ms; **exit 52** at 344 ms. stderr says "failed to load cuda"; its last line is "Ensure that you have installed the proper vaapi driver…" |
| c2 | plus `-fallback-cpu-encoding yes` | records with libx264. Starts in 151 ms. 500×42, no padding to 128 px on the CPU path. 71 frames in 1.18 s; decodes |
| c3 | `-k h264_vulkan` | records on the GPU without CUDA. Starts in 417 ms. 500×128, 70 frames; decodes |
| c4 | `-k h264_vulkan -fallback-cpu-encoding yes` | same as c3 |

- Snip passes neither flag (`snip-core/src/record.rs:78-81`). Omarchy passes `-fallback-cpu-encoding yes`.
- The fallback applies only when gsr's codec check fails [S `codec_select.c:380-400`, `510-520`]. It costs nothing when NVENC works.
- x264 "veryfast" encodes a synthetic 3440×1440 video at 60 fps input at 281–346 fps on 20 threads [V]. gsr's full-screen copy from GPU to CPU was not measured.
- This machine has `NVreg_PreserveVideoMemoryAllocations=1` and `NVreg_UseKernelSuspendNotifiers=1`, the nvidia-suspend units are disabled, and it hasn't suspended since boot [V]. So a real failure after suspend is untested.
- **Recommendation:**
  - Always pass `-fallback-cpu-encoding yes`.
  - On an early exit with code 52 or 54, or "Could not open video codec", retry once with `-k h264_vulkan`.
  - Show a proper message (C5).

### C5: error messages and microphone identity (gap 6) [V/S]

**What the user sees today.** Snip toasts gsr's last non-empty stderr line (`engine/record.rs:376-377`).
- Missing mic, exit 50: the last line is an entry from gsr's device list. The toast reads "Can't record: alsa_output.pci-0000_01_00.1.hdmi-stereo.monitor (Monitor of AD103 … [DELL S3422DWG])" [V `R/spec_r11.report.json`].
- Exit 52: the vaapi hint from C4.
- An area on HEADLESS-1: exit 0 with no file, and the log ends with "gsr_capture_start failed" [V, brief run r8].

**Microphone names.**
- Mic ids are PipeWire node names that include the card profile, e.g. `alsa_input.usb-Sony_INZONE_H5-00.mono-fallback` [V `pactl list sources short`].
- Changing the card's profile renames the node. The saved mic then fails with exit 50 every time [I].
- The list itself is fresh: `audio_inputs()` reruns `--list-audio-devices` each time the toolbar opens (`engine/record.rs:180`).

**To close.**
- Map exit codes to messages:

| gsr result | Message |
|---|---|
| exit 50 | "'<label>' isn't connected", with an offer to record without it |
| exit 51, or exit 0 with no file | "This display can't be recorded" |
| exit 52, 53, 54 | the encoder message, plus the retry from C4 |

- Store the mic by `device.name` plus its description, and resolve the current node at start.
- Losing the mic mid-take (gsr writes silence [S]) is still untested. Testing it means adding and removing a throwaway PipeWire source during a 3 s take, which changes the audio graph, so it needs the owner's OK.

---

## Also checked
- **Gaps 7 and 9** were closed by reading the source cited above. **Gap 10** was closed by the NVENC probes.
- **The daemon won't trigger a password prompt.** snip-dev runs with `NoNewPrivileges=no`, `NoNewPrivs: 0` and a full capability bounding set, and `gsr-kms-server` has `cap_sys_admin=ep` [V systemctl, /proc, getcap]. The journal shows two real recordings made by the daemon.
- **Brief §0.4 is out of date on audio.** The default output (INZONE H5) is not muted now [V `pactl get-sink-mute`].

## Side effects of this pass
- **gsr recordings:** 9 takes on the bar-centre strip, each 2.2 s at most, with system audio only and never the mic.
  - All videos, `.ts` files and sockets are deleted; only logs and JSON remain.
  - No gsr process is left running, and there are no `snip-*` layers [V].
- **gsr's own file:** gsr rewrote `~/.nv/nvidia-application-profiles-rc.d/10-gsr-cuda-no-stable-perf-limit`, as every gsr call does.
- **Encoder probes:** ffmpeg NVENC and x264 runs on synthetic input only; nothing was captured.
- **Idle probe:** one Wayland client held off the session's idle for 6 s. It created no window and no layer.
- **Web:** two lookups, archlinux.org and the kernel docs.
- **No edits** to the repo or to `~/.config`.

## Files (R = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research`)
- `R/critic/gsrrun.py`, `R/critic/analyze.py`, `R/critic/digit.py`
- `R/critic/p1.json` to `p5.json`, `R/critic/c1.json` to `c4.json`, `R/critic/runs/*.log`
- `R/critic/nvenc-minw.txt`, `R/critic/nvenc-minh.txt`, `R/critic/nvenc-max.txt`
- `R/critic/idleprobe/idleprobe.c`, with its generated protocol code and binary