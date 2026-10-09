# gpu-screen-recorder 6.1.0 as Snip's recording backend (Hyprland 0.56.2, RTX 4080, driver 610.57.04)

This builds on `docs/research/platform-feasibility-hyprland.md` §9 and `processing-pipelines.md` §6.1, and does not repeat them. Each claim carries one of these tags:
- **[V]**: verified locally.
- **[S file:line]**: read in the gsr source at tag 6.1.0.
- **[D]**: documented upstream or in an issue (URL given).
- **[I]**: inferred.

R = `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research`

**How the runs were done**
- 12 of the 12 allowed gsr runs were used.
- Every run used a region inside the DP-2 bar, which is 2752×35 logical px. The longest recording was 2.6 s.
- No microphone was opened. Audio came only from `default_output` and the IEC958 monitor source.
- Every gsr process was started as `argv[0]=snip-gsr-research`. Omarchy's `pgrep -f ^gpu-screen-recorder` cannot match that name, so the bar indicator never saw a run.
- All video files are deleted. No stray processes, sockets or layers remain.

**Where the evidence is**
- Run table: `R/runs-summary.txt`
- Per-run JSON: `R/spec_r*.report.json` and `R/r*.analysis.json`
- Harness: `R/harness.py` and `R/analyze.py`
- gsr source at tag 6.1.0: `R/gsr-6.1.0`. Upstream master (7f9405e, 2026-10-05): `R/gsr-src`.
- FFmpeg n8.0 source: `R/movenc.c`, `R/aviobuf.c`, `R/mux.c`

> The scratchpad man page `gsr-man.txt` is older than the binary. It describes 6.0.1 and its `--version` line says 5.10.2. In 6.1.0, `-region` is deprecated in favour of `-w WxH+X+Y`. **[S main.c validate_args_with_capture_sources]**

---

## Most important findings

1. **Small regions are padded, and `-s` can upscale.**
   - Every hardware encoder rounds each dimension up to even and to **at least 128 px**. The content is centred, with black padding. **[S nvenc.c:125-132, vaapi.c:270, vulkan.c:517] [V]**
   - A 400×34 logical region at scale 1.25 becomes 500×42 physical, then a **500×128** video.
   - `-s` means "scale to fit inside". It also scales **up**: `-s 1000x1000` on a 500×42 capture gave 1000×84 content. **[S utils.c:715] [V r3]**
2. **Region rule.** Region coordinates are logical and global. gsr computes physical = `(int)(logical_local × phys_w/logical_w)`, truncating the origin and the size separately. The monitor is chosen by the region's centre. **[S capture_setup.c:184-227] [V r1, r2, r9]**
3. **The IPC works as the man page says.** Verified details:
   - The socket appears about 25 ms after spawn.
   - A request sent during startup is answered when recording actually starts. This makes a clean "recording started" handshake.
   - `stop` replies after the file is finalized: **15–33 ms**, then the process exits **160–240 ms** later. **[V]**
4. **Exit code 0 is not proof of success.** When the capture fails to start (for example a region on HEADLESS-1), gsr prints `gsr_capture_start failed`, writes no file and exits **0**. The bug is still on upstream master. **[V r8] [S capture_setup.c:561 + error.h]**
5. **Crash recovery needs extra flags.**
   - With default flags, a SIGKILL after 2.55 s left an **88-byte, unreadable file**. FFmpeg's file protocol buffers 256 KiB, and the first fragment is not written until the second keyframe. **[V r5] [S ffmpeg file.c:316]**
   - With `-ffmpeg-opts "flush_packets=1;frag_duration=1000000"` the killed file was recoverable: 1.93 s of 2.55 s.
   - The recovery command needs `-ignore_editlist 1`. **[V r12]**
6. **gsr dies from SIGPIPE if its output pipes lose their reader.** gsr installs no SIGPIPE handler. If snipd holds gsr's stdout/stderr pipes and crashes, gsr dies at its next log line (once a second with the default `-v yes`). **[S main.c install_signal_handlers; grep shows no SIGPIPE] [I]**
7. **A new process group does not escape systemd.** `snip-dev.service` uses `KillMode=control-group` with `Restart=on-failure`. A snipd crash or restart therefore SIGTERMs gsr. gsr then finalizes and exits cleanly. **[V `systemctl --user show snip-dev`] [D man systemd.kill] [V r6: SIGTERM gives a clean file]**
   - "gsr survives a daemon crash" (architecture §3) therefore needs gsr in its own systemd scope, plus file-backed stdout/stderr (finding 6).
8. **Pause compresses time; nothing is left as a gap.** Audio during a pause is discarded. No drift is added. **[V r1] [S recording_clock.c, audio_capture.c:213]**
   - In 6.1.0 each stream's timestamps start at its own first packet. Audio therefore lags video by about one frame plus AAC priming (~38–55 ms) [I].
   - The video loses about one frame at each end.
   - Upstream fixed the stream alignment in 6.1.1 (ad6981c). **[S encoder.c:139-151; git show ad6981c]**
9. **Audio.**
   - `-a "a|b"` gives one mixed track (amix with `normalize=false`). Repeated `-a` flags give one track each. **[V r2]**
   - Output is always 48 kHz **stereo**; mono must be done after recording. **[S audio_codec.c:138]**
   - **No start pop on monitor sources:** digital silence from the first 10 ms window [V r1, r2]. A microphone pop was not tested.
10. **HEADLESS-1 and the nested e2e output cannot be recorded.** `--list-monitors` shows only DRM connectors (`DP-2|3440x1440`). **[V]**
    - Inside the nested e2e Hyprland, `-w DP-2` would record the **user's real screen**, because monitor capture looks up DRM connectors, not Wayland outputs. **[S capture_setup.c:134] [I]**
    - In e2e, gsr must be replaced by a stub.
11. **Recording files do not belong in `$XDG_RUNTIME_DIR`.** It is a 3.2 GB tmpfs here. On write errors (disk full) gsr only logs and keeps "recording". **[V `df`] [S encoder.c:163-168]**
12. **The portal path is not usable here today.** XDPH 1.4.1 (the latest tag) lacks dec05eba's multi-plane fix and several freeze fixes. There is an open NVIDIA 610.57.04 report of the portal hanging. Omarchy turned the portal path off. Details in Q9.

---

## Run table (spawn = t0)

| Run | What it tested | Exit | Socket | Recording start (IPC reply / file) | First frame (.ts) | Stop → reply | Reply → exit |
|---|---|---|---|---|---|---|---|
| r1 | `-w 2752x34+0+0 -f 60 -a default_output`, opus, VFR, IPC errors, pause | 0 | 28 ms | 403 ms | 454 ms | 18.8 ms | 221 ms |
| r2 | `-w 401x33+1+1 -f 30 -fm cfr`, 2 AAC tracks | 0 | 25 ms | 436 ms | 537 ms | 33.2 ms | 189 ms |
| r3 | `-k hevc -s 1000x1000`, SIGINT | 0 | – | 498 ms | 549 ms | path on stdout 16 ms after SIGINT | 242 ms after signal |
| r4 | `-k av1 -s 250x250 -bm vbr -q high -fm cfr` | 0 | 23 ms | 349 ms | – | 15.3 ms | 158 ms |
| r5 | AAC, CFR, SIGKILL at 2.6 s | -9 | 23 ms | 413 ms | 464 ms | – | – |
| r6 | stale socket from r5, `-cursor no`, SIGTERM | 0 | – | 398 ms | – | path on stdout 16 ms after SIGTERM | 164 ms after signal |
| r7 | `-w HEADLESS-1` | **51** | | | | | |
| r8 | region on HEADLESS-1 (`400x34+2800+0`) | **0** (bug) | | | | | |
| r9 | region off-screen | **51** | | | | | |
| r10 | `-ipc` path already listened on | **1** | | | | | |
| r11 | `-a device:does_not_exist.monitor` | **50** | | | | | |
| r12 | r5 plus `-ffmpeg-opts flush_packets=1;frag_duration=1000000`, SIGKILL | -9 | 23 ms | 382 ms | 433 ms | – | – |

Info commands:
- `--info`: 376 ms. `--list-monitors`: 115 ms. `--list-audio-devices`: 27 ms. **[V R/info.txt]**
- On NVIDIA, every invocation, including `--version`, rewrites `~/.nv/nvidia-application-profiles-rc.d/10-gsr-cuda-no-stable-perf-limit`. **[S main.c install_cuda_no_stable_perf_limit] [V file exists]**

---

## Q1. `-region` / `-w WxH+X+Y` with fractional scale 1.25

- **Region coordinates are logical and global, like slurp.** **[V]**
  - The r9 error lists monitors in logical space: `"DP-2" (2752x1152+0+0)`, `"HEADLESS-1" (1536x864+2752+0)`.
  - Omarchy passes slurp output unchanged.
- **How the monitor is picked.** gsr takes the region centre `(x + w/2, y + h/2)` (integer division) and tests it against each Wayland output's logical rectangle, inclusive on both edges. The first match wins. **[S capture_setup.c:65-82, 186]**
- **How the conversion is done.** **[S capture_setup.c:219-226]**
  - gsr subtracts the monitor's logical origin.
  - Then `x, y, w, h *= (double)phys_w / logical_w`, applied separately to each, as C `int *= double`, which truncates.
  - Measured results:
    - 2752×34 → 3440×42 (42.5 truncated). **[V r1]**
    - 401×33 at (1,1) → 501×41 at (1,1). **[V r2]**
    - 400×34 → 500×42. **[V r3, r6]**
- **Encoded size and padding.** **[S nvenc.c:125-132] [V]**
  - Final size = `max(align2(w), 128) × max(align2(h), 128)`.
  - The content is placed at `X = W/2 − w/2` and `Y = H/2 − h/2` (C integer division), with black around it.
  - Measured placements:
    - 3440×42 content in **3440×128** at rows 43–84.
    - 501×41 in **502×128** at rows 44–84; the odd width gets one padding column.
    - 1000×84 in 1000×128 at rows 22–105.
    - 250×21 in 250×128 at rows 54–74.
  - These offsets can be odd. H.264 cropping in the bitstream works only in steps of 2 for 4:2:0, so removing this padding needs a re-encode.
- **`-w DP-2` (not recorded, by rule).** The output is the DRM mode size, 3440×1440, rotated for odd transforms. `-s 0x0` means original size. **[S kms.c:198-218] [V --list-monitors]**
  - Example: `-s 1920x1080` on 3440×1440 gives 1920×803. NVENC pads that to 1920×804, which leaves 1 black row. **[I, from the formula]**
- **`-s` semantics.** gsr keeps the aspect ratio and fits inside the given box. It also **upscales**. It became "fit" in 5.15.0 (c09fb1b). **[S utils.c:715] [V r3: 500×42 → 1000×84]**
  - Scaling uses bilinear texture sampling without mipmaps (`GL_LINEAR`), so downscaling by more than 2× aliases. **[S kms.c:119-126] [I]**
- **Scaled regions were broken in old versions.** The bug was fixed in 5.12.1 (054282b). **[S git]**

## Q2. The `-ipc` socket

Wire format **[S ipc.c] [V r1]**:
- One JSON object per line, ending in `\n`. Several requests can share one connection, in sequence.
- Socket mode is 0600. At most 8 clients and 4096 bytes per request. Socket paths are limited to 107 characters.
  - Our scratch path was too long for that limit, so the runs used relative socket paths.

Measured replies (r1):

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

Behaviour around the replies:
- **`stop` returns `ok` even if finalizing failed.** The reply is `true` unconditionally and the trailer's return value is ignored, so always run ffprobe on the file. **[S main.c:501, recorder.c:889]**
- **If gsr exits while a `stop` is pending,** the reply is `"GPU Screen Recorder exited before the request finished"`. **[S ipc.c:657]**
- **If the client disconnects while a request is pending,** that reply is dropped. **[S ipc.c ipc_remove_client]**
- **Error replies.** The `data` field is a string. Requests that cannot be parsed get `id` 0.
- **A stale socket file** left by a killed gsr is removed silently on the next start. **[V r6]**
- **A path that another process listens on** gives exit 1: `gsr error: gsr_ipc_init: another program is already listening on "..."`. **[V r10]**
- **Pause compresses time.**
  - Video timestamps run on a clock that excludes paused time. **[S recording_clock.c]**
  - Audio read during a pause is discarded. **[S audio_capture.c:213]**
  - r1 had 0.82 s of pause: 93 frames, largest frame gap 16.73 ms, video 1.550 s. That matches the active time (1.584 s) minus about 2 frames. **[V]**
- **A/V sync (6.1.0).**
  - Pausing adds no drift: audio and video share one clock.
  - At the start, each stream begins at its own first packet. Audio t=0 is roughly the recording start, while video t=0 is the first captured frame, about 1/fps later.
  - AAC priming (1024 samples, 21.3 ms) is not marked for skipping: no discard flags, start_time 0 **[V r2]**. Expect audio about **23–55 ms late**, which is inside broadcast tolerance [I].
  - The video encoder is never flushed at stop, so the last frame is lost. **[S recorder.c:868-896, only audio is flushed]**
  - Audio runs past the video at the end by +16 to +117 ms. **[V r1, r2]**
  - Upstream ad6981c (6.1.1) aligns every stream to the first video keyframe. **[S git]**
  - Snip should treat the **video duration as the length** and cut audio to it when remuxing.
- **SIGINT and SIGTERM both stop normally** and print the path. SIGUSR2 toggles pause. SIGHUP has no handler, so it kills gsr without finalizing. **[S main.c:205-215] [V r3, r6]**

## Q3. `-write-first-frame-ts`

- **Format** **[V r1 and the other runs]**. The file is named `<-o path>.ts` and contains:

  ```
  monotonic_microsec\trealtime_microsec\n
  119656355264\t1791504649795862\n
  ```
  The values are CLOCK_MONOTONIC and CLOCK_REALTIME in µs.
- **When it is written.** It is written once, when the first **encoded** video packet reaches the muxer, not when the frame is captured. **[S encoder.c:24-37, 144-147]**
- **Timing.** In every run it appeared about 3 frame intervals after the recording started: +51 ms at 60 fps (5 runs) and +101 ms at 30 fps. Frame 0 itself is captured about 1/fps after the start. So the .ts time is about 2 frames later than the first capture, which fits the NVENC output delay. **[V] [I]**
- **Use for the UI timer.** Timer start = the .ts monotonic value (Qt's monotonic clock and `clock_gettime(CLOCK_MONOTONIC)` use the same clock). Elapsed = now − start − total paused time.
- **Precise alignment (Studio Mode, later).** Use the IPC-ready reply as clock 0 and add each frame's PTS. **[I]**
- **When it is skipped.** Live streaming and piped output.

## Q4. Audio

- **One mixed track versus separate tracks** **[V r2]**. `-a IEC958.monitor -a "default_output|IEC958.monitor"` produced 1 video and **2 AAC** streams.
  - Stream titles: `Devices: Monitor of Built-in Audio Digital Stereo (IEC958)` and `Devices: Default output, Monitor of …`.
  - These titles leak device names. `-exclude-metadata yes` removes them.
  - The same device may appear in two tracks.
  - The mix uses amix with `normalize=false`: full volume, so loud sources can clip. **[S audio_capture.c:86]**
  - There are at most 32 sources per track.
- **Codecs.** **[V r1, r2]**
  - Both work in MP4: opus (the default for mp4/mkv, 128 kbps, tag `Opus`) and `-ac aac` (AAC-LC, 160 kbps, tag `mp4a`).
  - For compatibility, use AAC. Omarchy does. Opus-in-MP4 does not play in many editors, in older Safari and iOS, or on several social sites [I].
- **Channels.** Always 48 kHz stereo; a mono microphone is upmixed. **[S audio_codec.c:132-138] [V channels=2]**
  - Mono output belongs in the export step (`-ac 1`). Forcing it inside gsr through `-ffmpeg-audio-opts ch_layout=mono` is untested.
- **Start pop.** None on monitor sources: −inf dBFS from the first 10 ms window on all tracks. **[V r1, r2]**
  - gsr flushes stale device audio when it starts. **[S audio_capture.c:195]**
  - Omarchy's script mutes the first 400 ms for a "PipeWire capture-open pop" of 130–200 ms. That applies to the **microphone** case, which was not testable here. **[D local omarchy-capture-screenrecording]**
- **Following device changes.**
  - `default_output` / `default_input` follow default-device changes. **[S sound.c:110-135]**
  - When a device has no data, silence is inserted. **[S audio_capture.c:240-262]**
- **Noise at stop.** AAC-only: `Application provided invalid, non monotonically increasing dts … gsr error: … failed to write frame index 1 … (-22)` and `[aac] Qavg: 65536.000`. The last audio packet is dropped. This is harmless and must not be reported as a failure. **[V r2]**
- **CPU.** In 6.1.0 the audio thread polls every 1 ms, which upstream measured at about 1000 wakeups/s. This is fixed after 6.1.3 (ab068e1). **[S sound.c:426] [D commit message]**

## Q5. Cursor with KMS on NVIDIA

- **How it works.** gsr draws the DRM **cursor plane** into the frame when `-cursor yes`. NVIDIA uses an `EXTERNAL_OES` texture. **[S kms.c:1011-1024, 683-762]**
- **History.** The NVIDIA hardware-cursor plane has been supported since 2024 (846bda7, f3c32a8, 6c5acf2). A cursor flicker on NVIDIA was fixed in 5.11.3 (3117f30). **[S git log]**
- **Software cursor.** If Hyprland draws the cursor in software (while tearing, or with `no_hardware_cursors=1`), it is baked into the primary plane. Then `-cursor no` cannot remove it. **[I]**
- **Not verified locally.**
  - The pointer was at 1339,572, outside the bar region, and synthetic input was not allowed.
  - `modetest` showed the cursor plane (id 194) without a framebuffer, which is consistent with `cursor:hide_on_key_press=true` hiding it.
  - `no_hardware_cursors=2` and `use_cpu_buffer=2` were checked.
  - Spike S7 should compare `-cursor yes` and `-cursor no` while the pointer is visible.

## Q6. Startup, cost, exit codes, unplug, stop, crash recovery

**Startup** **[V, 7 runs]**:
- Spawn → socket: 23–28 ms.
- Spawn → recording start (IPC reply, output file created): **349–498 ms**, median about 405 ms.
- Then the first frame: +51 ms at 60 fps, +101 ms at 30 fps.
- The stderr timeline shows where the time goes:
  - About 100–180 ms: the monitor is resolved and gsr-kms-server is spawned (+30 ms).
  - About 290–350 ms: `using h264 encoder`.
  - About 340–480 ms: NVENC is open.
- `03-performance.md` budgeted 150–250 ms, which is too optimistic. With no countdown, users wait about 0.45 s.
- **Idea, not tested:** start gsr during the countdown, queue `set-paused true` as soon as the socket appears, and send `set-paused false` at zero. That gives an instant start and warm audio devices; the microphone pop would fall inside the paused span and be discarded.
  - Risk: a race could let one frame through before the pause applies. **[I, from recorder.c:842-849 loop order]**

**CPU/GPU cost**
- Not measured representatively: only a 3440×128 strip was allowed.
- At 3440×128@60, `nvidia-smi pmon` showed sm ≤ 1% and enc 0%. **[V R/r1.pmon.txt]**
- A sibling agent's ffmpeg NVENC job was running at the same time.
- Full 3440×1440@60/144 must be measured in spike S7.

**Exit codes** **[S error.h; V where marked]**:

| Code | Meaning | Example stderr |
|---|---|---|
| 0 | Normal stop. **Also returned when the capture fails to start** | r8: `gsr error: gsr_capture_kms_start: failed to find monitor by name "HEADLESS-1"` / `gsr_capture_start failed` |
| 1 | Generic or argument error | r10: socket in use; path too long |
| 2 | Unsupported | `-w focused` on Wayland; app audio without PipeWire |
| 3 | Capture failed during recording | portal or window ended; never raised by KMS |
| 11, 22 | Codec query failed / OpenGL load failed | |
| 50 | Audio device not found | r11: `Audio device 'x' is not a valid audio device, expected one of:` followed by the list |
| 51 | Monitor not found | r7: `display "HEADLESS-1" not found, expected one of: "screen" "DP-2" (3440x1440+0+0)`; r9: `the region … doesn't match any monitor` |
| 52, 53, 54 | No codec / resolution not supported / codec not supported | |
| −9 | SIGKILL | r5, r12 |

**Missing `cap_sys_admin`**
- gsr falls back to `pkexec gsr-kms-server`, which shows a polkit **password dialog on every recording**. Not tested, by rule. **[S kms_client.c:274-342]**
- Arch's `post_install` and `post_upgrade` run `setcap`. **[V /var/lib/pacman/local/gpu-screen-recorder-6.1.0-1/install]**

**Monitor unplugged or off**
- KMS `should_stop` always returns false. **[S kms.c:1037]**
- gsr keeps encoding the last frame. It logs `gsr_capture_kms_update_connector_ids: failed to find monitor by name` every 2 s and resumes if the output comes back. **[S kms.c:19, 800-818] [I: not tested]**
- Snip must stop or pause on Hyprland's `monitorremoved`.

**SIGINT and IPC stop finalize the file the same way.** The difference is that IPC also returns a reply with the path.

**Crash recovery**
- **What gsr writes.** gsr sets `movflags=+hybrid_fragmented` and `use_editlist=1` (FFmpeg ≥ 62.6.101). **[S muxer.c:86-94]**
- **When fragments reach the disk.**
  - A fragment is cut at each keyframe; the default `-keyint` is 2 s.
  - FFmpeg writes to the file in 256 KiB chunks. **[S ffmpeg file.c:316; aviobuf.c:464-467]**
  - So a low-bitrate recording may have nothing on disk for seconds. r5 left 88 bytes after 2.55 s, and ffprobe reported `moov atom not found`.
- **Synthetic check at high bitrate** (ffmpeg, not gsr): fragments were written within about 1 s and the killed file was recoverable.
- **The working recipe.** `-ffmpeg-opts "flush_packets=1;frag_duration=1000000"` wrote at about 1 s and 2 s. **[V r12, R/r12.sizes.txt]**
  - The killed file decodes, but its provisional edit list (segment_duration 0) hides the first fragment.
  - A plain `-c copy` remux recovered only 61 frames. `ffmpeg -ignore_editlist 1 -i f -c copy -movflags +faststart out.mp4` recovered **116 frames and 94 AAC packets (about 2.0 s of 2.55 s)** and decodes cleanly. **[V]**
  - A normal finish with these options still produces a clean, non-fragmented MP4. **[V synthetic]**
- **Stale socket.** After a SIGKILL the socket file stays; gsr reuses it later (finding in Q2).
- **Orphan helper.** `gsr-kms-server` exits by itself (`kms client shutdown`), fixed in 6.0.0 (e522789). **[V r5]**
- **Finalize can be slow.** On slow or loaded machines, VAAPI finalize can take more than 5 s. Omarchy's fixed 5 s SIGKILL truncated recordings ([omarchy#12469](https://github.com/omacom/omarchy/issues/12469)). **[D]** Never kill on a timer.

## Q7. HEADLESS outputs, nested Hyprland, testing

- **Only DRM connectors are recordable.** `--list-monitors` lists `DP-2|3440x1440` only, and `--list-capture-options` lists `DP-2`, `region` and `portal`. HEADLESS-1 is a Wayland output with no DRM connector. **[V]**
  - `-w HEADLESS-1` exits 51.
  - A region on HEADLESS-1 matches it through the Wayland output list, then fails at KMS with **exit 0**.
- **Nested e2e Hyprland.** Its `WAYLAND-1` output fails the same way. `-w DP-2` inside the nested session would record the **real** DP-2: monitor lookup on Wayland uses DRM connectors (`monitor_connection_type` → `GSR_CONNECTION_DRM`), and the Wayland data is used only for rotation and position. **[S capture_setup.c:134, utils.c:377-405] [I: not run, by rule]**
- **What other Linux recorders do.**
  - wl-screenrec's CI only builds; its test step is commented out, and `tests/cmdline.rs` covers argument parsing only. **[D: github russelltg/wl-screenrec .github/workflows/rust.yml, via `gh api`]**
  - Compositor projects test capture on virtual or headless outputs, such as mutter virtual monitors, the KWin virtual backend and the wlroots headless backend. **[I]**
  - KMS recorders cannot run in CI.
  - gsr itself has no public tracker; bugs go by email ("Send an email to dec05eba@protonmail.com…", [git.dec05eba.com/?p=about](https://git.dec05eba.com/?p=about)) **[D]**.
- **For Snip.**
  - Put a **gsr stub** behind `SNIP_GSR=…`. It should implement the CLI subset, the newline-JSON IPC (including the queued reply at startup), the `.ts` file, the exit codes, and SIGINT/SIGTERM. Its MP4 can come from ffmpeg lavfi or from ext-image-copy-capture frames of WAYLAND-1.
  - Keep real-gsr checks as a manual "live" tier with a tiny bar region, like these runs.

## Q8. Codecs, quality, frame rate mode, size

- **Codecs on this machine.**
  - `-k auto` picks **h264** when the capture fits NVENC H.264's limit; otherwise HEVC, then AV1. **[S codec_select.c:476-480] [V "using h264 encoder because a codec was not specified"]**
  - h264 gives High profile and tag `avc1`. hevc gives Main with tag **`hev1`**, which needs `-tag:v hvc1` when remuxing for Apple players. av1 gives Main, `av01`. **[V r1–r4]**
  - All outputs are tagged BT.709 limited range. Vulkan variants are listed but experimental.
- **Quality.**
  - The default `-bm auto` means constant QP: medium/high/very_high/ultra = **QP 35/30/25/22**; ×4 for AV1. **[S video_codec.c:10-34]**
  - `-bm vbr` sets qmin = qmax = QP (warns "Passing qmin/qmax…") [V r4]. `-bm cbr -q <kbps>` gives a predictable size.
  - Defaults: `tune=ll`, `forced-idr`, no B-frames. `-tune quality` adds 2 B-frames and multipass, which hurts smart-cut. **[S video_codec.c:340-360]**
- **CFR or VFR for editing: use `-fm cfr`.**
  - With KMS on Wayland, gsr captures at the requested fps whether or not the screen changed (`damaged=true` unless `content`). VFR saves nothing. **[S recorder.c:539-541]**
  - VFR timestamps are the actual capture times: µs time base, jitter ≤ 0.07 ms. **[V r1, r3]**
  - CFR gives exact 1/fps timestamps (mp4 time base 1/15360, r = avg = 30/1 or 60/1) and duplicates frames when the loop runs late. **[V r2, r4]**
  - CFR keeps trimmer frame stepping and GIF conversion simple, and Kdenlive/Resolve accept it. Omarchy uses `-fm cfr -f 60`.
  - `-fm content` does not work with KMS capture on Wayland.
- **Frame rate.** `-f` accepts 1–1000. gsr can capture faster than the monitor refreshes.
- **Max resolution and "scale to 1×".**
  - Pass `-s` **only when downscaling**: a box smaller than the capture.
  - For 1× on a 1.25 monitor, use the logical size (2752×1152).
  - Copy gsr's fit function so the result is even in both dimensions.

## Q9. Window recording on Hyprland

- **`-w focused` and window IDs are X11-only.** On Wayland: `"GPU Screen Recorder window capture only works in a pure X11 session…"`, exit 2. **[S capture_setup.c:355-359]**
- **`-w portal`** uses the XDPH picker. Here `hyprland-preview-share-picker` is configured with `allow_token_by_default=true`. **[V ~/.config/hypr/xdph.conf]**
  - Reusing a choice: `-restore-portal-session yes`.
  - The token goes to `~/.config/gpu-screen-recorder/restore_token` by default. Pass `-portal-session-token-filepath` to keep it in Snip's state directory.
  - The portal path shows `no_screen_share` windows as black, has no HDR, and supports `-fm content`.
  - Not tested here: it shows a dialog.
- **The portal path is fragile on this stack.** None of the fixes below is in the latest XDPH tag, v1.4.1 (cc8e5ef). **[V `git tag --contains`]**
  - Multi-plane dmabufs were dropped, which breaks gsr. Reported as [XDPH #420](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/420); dec05eba's fix is #427 (ba31964, 2026-08-29). The reported gsr error: `pipewire: the desktop portal failed to provide dmabuf (GPU) video frames…` followed by `timed out waiting for pipewire`.
  - Permanent freeze when out of buffers (#424) and during renegotiation (#425 / [#403](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/403)).
  - The portal wedges at 36% CPU after a DMA-BUF failure **on an RTX 4060 with 610.57.04** ([#434](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/434)).
  - Segfault ([#442](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/442)), EGL_BAD_MATCH from bad modifiers ([#311](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/311)), region offset ([#386](https://github.com/hyprwm/xdg-desktop-portal-hyprland/issues/386)).
  - Omarchy: "We have stopped using the portal because it had issues…" ([omarchy#5632](https://github.com/omacom/omarchy/issues/5632)); it is now opt-in with `OMARCHY_SCREENRECORD_USE_PORTAL` because of "EGL DMA-BUF modifier import" failures. **[D local script]**
- **Hyprland's own window capture.** ext-image-copy-capture with foreign-toplevel sources, plus the Hyprland toplevel mapping. Snip already binds these in `crates/snip-wayland/src/state.rs:28-47`, but gsr cannot use them.
- **Phase 1 should stay "window rectangle via a KMS region"** (REC-01). Real per-window recording means the portal as an opt-in, or the Phase-2 pipeline of our own.

Other gsr facts on this stack:
- **6.1.0 selects the GPU attached to each monitor.** This fixed the hybrid-laptop `display not found` failure from Omarchy [#7530](https://github.com/omacom/omarchy/issues/7530). 6.1.1 fixes CUDA picking the wrong GPU.
- **NVENC API mismatch on Pascal/Maxwell with 580xx drivers** was fixed in 6.0.1 (dec05eba on [omarchy#7640](https://github.com/omacom/omarchy/issues/7640)).
- **Suspend.** `gsr-nvidia.conf` sets `PreserveVideoMemoryAllocations=1` ([omarchy#8126](https://github.com/omacom/omarchy/issues/8126)). The README says Vulkan encoding avoids the CUDA-after-suspend breakage.
- **Security.** `cap_sys_admin` on the helper is a known concern ([omarchy#8245](https://github.com/omacom/omarchy/issues/8245)). 6.1.2 hardens the helper's argument checks (4de50ca).
- **Versions and repos.**
  - `-ipc` exists from **6.0.0** (2026-08-08).
  - Arch `extra` has 6.1.0 (sync DB from Sep 8); upstream is at 6.1.3.
  - The `omarchy` repo also ships **5.12.3, which has no `-ipc`**. `extra` is listed first in `/etc/pacman.conf`.

---

## Recommended argument builder (pseudocode)

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

## Known failure modes: how Snip detects and reports each

| # | Failure | How to detect | What to do or show |
|---|---|---|---|
| 1 | gsr missing, or older than 6.0 (no `-ipc`; Omarchy repo ships 5.12.3) | Preflight `--version` | "Install or update gpu-screen-recorder (6.1 or newer)." Disable Record. |
| 2 | `gsr-kms-server` without `cap_sys_admin` | Preflight `getcap` | Block before pkexec can prompt: "The capture helper lost its permission. Reinstall gpu-screen-recorder." |
| 3 | Output not recordable (HEADLESS, nested, virtual) | Not in `--list-monitors`. At runtime: exit 51 `display "X" not found`, or **exit 0** after `gsr_capture_start failed` | Grey out Record on that output. Treat "exited before the IPC reply" as failure regardless of exit code. |
| 4 | Region off-monitor or spanning monitors | exit 51 `doesn't match any monitor` | Internal bug: the snap/clip step must prevent it. Log it. |
| 5 | Socket path in use or too long | exit 1 with `another program is already listening` / `the ipc socket path is too long` | Unique short path per recording, then retry once. |
| 6 | Audio device missing at start | exit 50 with the device list | Re-enumerate. Offer "Continue without microphone". |
| 7 | Device gone mid-recording | gsr writes silence and carries on | Watch PipeWire (`pactl subscribe`). Show "Microphone disconnected". |
| 8 | NVENC/CUDA failure (after suspend, session limit, driver mismatch) | Early exit; stderr `Could not open video codec`, `failed to load cuda`, `failed to start video encoder`; codes 1/52/54 | Retry once with `-k h264_vulkan`. Otherwise show the stderr tail. |
| 9 | Capture too large for H.264 | exit 53 | Pick HEVC automatically above the H.264 maximum. |
| 10 | Monitor unplugged or DPMS off | Hyprland socket2 `monitorremoved`; stderr `failed to find monitor by name` every 2 s; gsr keeps going | Stop and finalize. "Display disconnected; recording saved." |
| 11 | Disk full or write errors | statvfs poll; stderr `failed to write frame index … No space left` (gsr keeps running) | Warn early, auto-stop at a threshold (REC-14). Never stage in tmpfs. |
| 12 | External stop (Omarchy ALT+PRINT or indicator SIGINT) | IPC EOF, exit 0, path on stdout | Treat as a normal stop. Or avoid it with the `arg0` rename (product decision). |
| 13 | gsr killed (OOM, GPU reset, SIGKILL) | Exit by signal | Recover with `-ignore_editlist 1`. "Recording recovered; last ~1 s lost." |
| 14 | snipd crash | systemd SIGTERMs gsr (KillMode=control-group), so the file is finalized | On restart, pick up the finished file from the staging state. Or run gsr in its own scope and re-attach through the socket. |
| 15 | Slow finalize | `stop` reply delayed | Show "Finishing…" after 1 s. No SIGKILL timer (omarchy#12469). |
| 16 | Stop before the first frame | stderr `not finalizing the video file because it has no video/audio data`; reply still `ok`; ffprobe fails | "Nothing recorded." Delete. |
| 17 | Padding and upscale artifacts | Plan time (snap / `size_arg`) | Snap to even and ≥ 128 px. Never pass a larger `-s`. |
| 18 | Harmless stderr noise | Known strings: `ignoring invalid SAR`, `Passing qmin/qmax`, the AAC `non monotonically increasing dts … -22` at stop, `Qavg` | Filter; never report. |
| 19 | Private windows recorded by KMS | Existing decision (D6) | Warning (unchanged). |
| 20 | Portal path (if offered) | stderr `canceled by the user`, `failed to provide dmabuf`, `timed out waiting for pipewire` | Opt-in only. Fall back to KMS with a notice. |

## Decisions for the product owner (record as D-numbers if adopted)

1. **Process name.** Rename gsr's `argv[0]` so Omarchy cannot see Snip's recordings (no foreign stops, no Omarchy indicator), or keep the default name to integrate with them.
2. **Daemon crash.** Either recordings survive a snipd crash (systemd scope plus log files plus re-attach), or a crash ends the recording cleanly (SIGTERM finalizes it). The second is simpler. Either way, architecture §3's claim needs correcting.
3. **Minimum gsr version.** 6.0.0 is required. 6.1.1 or later is recommended for the A/V alignment fix; Arch `extra` was at 6.1.0 on Sep 8.
4. **Small areas.** Either enlarge selections under 128 px to 128, or allow them and crop with a re-encode after recording.
5. **Audio codec of the master file.** AAC (compatible everywhere) or Opus.
6. **Window recording in v1.** Window rectangle only, or the portal as an opt-in.

**Doc corrections to make**
- Architecture §5.2 stages recording temp files in `$XDG_RUNTIME_DIR`, a 3.2 GB tmpfs. Use a staging directory on disk.
- Architecture §4.3 should add the ≥ 128 px rule and use `-k h264` instead of `-k auto`.
- `03-performance.md`: gsr startup is about 0.35–0.5 s, not 150–250 ms.

**Spike S7 must cover**
- Cost at full 3440×1440@60 and @144.
- Cursor plane, with the pointer visible.
- An A/V sync test with a flash and a beep.
- The paused pre-roll start.
- Stop latency on a long recording.
- The systemd scope approach for survival.