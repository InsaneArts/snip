# Omarchy's screen-recording UI and how Snip should fit in

Research date: 2026-10-09. Installed versions: Omarchy package 4.0.4-1 (`/usr/share/omarchy/version` still says `4.0.0.alpha`), Hyprland 0.56.2, Quickshell 0.3.1, gpu-screen-recorder (gsr) 6.1.0, mpv 0.41.0 and v4l-utils 1.32.0. I compared the local files with upstream `basecamp/omarchy` on its default branch `quattro`, pushed 2026-10-08.

Each claim carries one of these tags:
- **[verified]**: I ran it or observed it on this machine.
- **[source]**: I read it in the installed Omarchy files, the Hyprland source or the gsr man page. The path and line are given.
- **[inferred]**: I reasoned it from the source but did not run it.

Snip's docs already cover some of this, and I do not repeat it (platform doc §9 "Omarchy interop", tech-stack doc, architecture §4.3 and §6.3):
- The indicator runs `pgrep -f ^gpu-screen-recorder`, and clicking it SIGINTs every gsr.
- gsr has `-ipc`, `set-paused` and `-write-first-frame-ts`.
- Omarchy mutes the first 400 ms of audio and records with `-fm cfr -f 60`.
- The webcam is an mpv window named WebcamOverlay.

## Answers in brief

1. **How the bar shows "recording".** It runs one check: `pgrep --quiet -f ^gpu-screen-recorder`. There is no polling and no state file. The check runs again only when something calls the `omarchy.indicators refresh` IPC, or when the bar is created.
   - **Will a Snip gsr light it?** Only if gsr's `argv[0]` is the bare name `gpu-screen-recorder`, and only after the next refresh. It then stays lit after Snip stops, until another refresh.
   - **What a click does.** While lit, it SIGINTs every bare-named gsr and then shows Omarchy's own "Screen recording saved" toast with an empty file path. While not lit, it opens Omarchy's recording menu.
   - **Is there an IPC for state or the stop action?** No. Nothing can set the indicator's state or its click action.
   - **The way out.** If Snip starts gsr by its absolute path, every Omarchy recording surface ignores it [verified].
2. **Webcam overlay.**
   - It is an mpv window showing an 8:9 portrait crop from the centre of a 16:9 feed (`crop=ih*8/9:ih`).
   - Its three sizes are 18%, 25% and 33.75% of the recorded area's height.
   - It sits 40 px from the bottom-right corner of the recorded region.
   - On DP-2 full screen the sizes are 184×207, 256×288 and 346×389 logical px.
   - Its window rules are float, pin, `no_initial_focus` and `no_dim`.
3. **Recording keys.**
   - ALT+PRINT: start or stop a recording.
   - SUPER+ALT+`[` and SUPER+ALT+`]`: make the webcam smaller or larger.
   - SUPER+CTRL+C: the capture menu.
   - SUPER+ALT+comma: runs the last toast's action, which opens the last recording.
   - PRINT, SUPER+PRINT and SUPER+CTRL+PRINT are the screenshot, colour-picker and OCR keys.
4. **Pause.** Omarchy has none, either locally or upstream. gsr's pause works [verified].
5. **Folder and file name.**
   - Folder: `$OMARCHY_SCREENRECORD_DIR`, else `XDG_VIDEOS_DIR` from `~/.config/user-dirs.dirs`, else `~/Videos`. The folder must already exist.
   - File name: `screenrecording-YYYY-MM-DD_HH-MM-SS.mp4`. Omarchy post-processes the file in place.

---

## 1. The recording indicator

### 1.1 How it decides [source]

`/usr/share/omarchy/shell/plugins/bar/indicators/ScreenRecording.qml`:
- `refresh()` (lines 16-20) runs `pgrep --quiet -f ^gpu-screen-recorder`. Exit code 0 means "recording".
- It runs at `Component.onCompleted` and `onBarChanged` (22-23), and on `indicatorHost.refreshRequested` (25-29).
- The icon is 󰻂. The tooltip says "Stop recording" when active and "Screen Recording" when inactive.

`/usr/share/omarchy/shell/plugins/bar/widgets/Indicators.qml`:
- Lines 168-174: `IpcHandler { target: "omarchy.indicators"; function refresh() }` sends the refresh to every bar's instance.
- There is no Timer. Hovering does not run the check again: the inactive block always exists and hover only changes its clip width (196-217).

Who calls `omarchy-shell -q omarchy.indicators refresh` (searched with symlinks followed):
- `omarchy-capture-screenrecording`, after its output file appears (199-200) and when it stops (214, 245-247).
- `omarchy-toggle-notification-silencing` (line 6).
- `omarchy-reminder` (38, 198).
- Snip's `notifications setDnd` does **not** refresh it. The DND indicator reads the notification service property directly (`indicators/Dnd.qml:8-11`).

### 1.2 What a click does [source]

`ScreenRecording.qml:38-42`:
- **When active**, it runs `omarchy-capture-screenrecording --stop-recording`. The script (`/usr/bin/omarchy-capture-screenrecording`) then does this:
  1. `pkill -SIGINT -f "^gpu-screen-recorder"` (line 205).
  2. It waits up to 5 s (50 × 0.1 s, 208-212).
  3. It refreshes the indicators. It runs `pkill -f "WebcamOverlay"` (an unanchored pattern) and deletes `$XDG_RUNTIME_DIR/omarchy-screenrecord-region` (110-113, 214-215).
  4. If gsr is still alive, it runs `pkill -9` and shows a critical toast: "Recording process had to be force-killed. Video may be corrupted." (217-219).
  5. Otherwise it calls `finalize_recording`. For a gsr that Omarchy did not start, this returns early, because the state file `/tmp/omarchy-screenrecord-filename` does not exist (253-256).
  6. The toast still fires (222-232) [inferred, not run]. `filename` is empty, so:
     - the preview path becomes `-preview.png`;
     - ffmpeg fails quietly;
     - `omarchy-notification-send "Screen recording saved" "Open with Super + Alt + , (or click this)" --image -preview.png --exec mpv -- ""` runs.
  7. The toast's app name is `omarchy-action`, and that app name **always gets past DND** (`NotificationLogic.js:118-121`, `omarchy-notification-send:13`).
  8. Upstream `quattro` (commit b44fb74780, #8374) moved the state files to `$XDG_RUNTIME_DIR`. The behaviour is the same.
- **When inactive**, it runs `omarchy-menu toggle trigger.capture.screenrecord`, Omarchy's recording menu. That menu offers:
  - With no audio
  - With desktop audio
  - With desktop + microphone audio
  - With desktop + microphone audio + webcam (only when `omarchy-hw-webcam` finds a camera)
  - Stop Screenrecording (only while a bare-named gsr runs)

  Source: `default/omarchy/omarchy-menu.jsonc:53-63`. Inactive indicators are visible only while the user hovers over them, or when `alwaysShow` is set.

### 1.3 Other Omarchy surfaces that use the same pattern [source]
- **Capture menu**: "Stop Screenrecording" appears when `pgrep -f '^gpu-screen-recorder'` succeeds (`omarchy-menu.jsonc:55`).
- **ALT+PRINT**: `omarchy-capture-screenrecording --stop-recording || omarchy-menu toggle trigger.capture.screenrecord` (`default/hypr/bindings/utilities.lua:38`).
- **Every "start" item in Omarchy's recording menu** runs the same script. That script **stops** instead of starting while any bare-named gsr is running (lines 282-288). So choosing "With desktop audio" during a visible Snip recording stops Snip's recording.
- **`omarchy screenrecord`** (the CLI alias) and Omarchy's own AI-agent skill ("Record my screen", `default/agents/skills/omarchy/SKILL.md:293`) use the same script.

### 1.4 No IPC can set the state or the click action
- `qs ipc -n -p /usr/share/omarchy/shell show` lists `target omarchy.indicators` with only `refresh(): void` [verified].
- `omarchy-shell shell call <id> <method> <arg>` reaches only loaded panel, overlay and menu plugins (`shell.qml:1279-1291`), not bar widgets [source].
- `BarIndicator.activeOverride` exists (`Ui/BarIndicator.qml:15-16`), but nothing exposes it [source].
- A third-party plugin cannot add an entry to the built-in Indicators widget. It loads only `../indicators/<id>.qml` from its own folder. The only way in is to clone it with `omarchy plugin clone omarchy.indicators` [source].

### 1.5 Whether Omarchy sees a gsr depends only on argv[0] [verified]
- Test with dummy processes:
  - `exec -a snipresearch-dummy sleep` matched `pgrep -f '^snipresearch-dummy'`.
  - `exec -a /usr/bin/snipresearch-dummy sleep` did not match.
  - `/proc/PID/comm` stayed `sleep` in both cases.
- So a gsr started as `/usr/bin/gpu-screen-recorder` is invisible to all of the above. In Rust, `Command::new` passes the program string as `argv[0]`; `CommandExt::arg0` can set any value.
- `pgrep -x gpu-screen-reco`, `killall` and unanchored patterns still match through `comm`. Snip must still handle someone else stopping its gsr.
- gsr finds `gsr-kms-server` through `/proc/self/exe` and then PATH (from `strings /usr/bin/gpu-screen-recorder`), so `argv[0]` does not affect KMS capture. All my tests below ran gsr by its absolute path and worked.

### 1.6 How gsr behaves when stopped from outside
All tests used a 160×96 logical region at 0,0 on DP-2, NVENC H.264 and no audio. I deleted the files afterwards.

| Event | gsr exit code | File |
|---|---|---|
| SIGINT after 2 s | 0; prints the path on stdout | valid, 46 frames (1.53 s) |
| SIGUSR2 (pause) at 1.5 s, SIGINT 1 s later | 0 | valid, 32 frames: the paused time is left out |
| SIGTERM after 2 s (this is what `systemctl --user stop` sends) | 0 | valid |
| SIGHUP after 2 s | 129 | **corrupt** ("moov atom not found") |
| SIGKILL after 2 s, `-c mp4` / `-c mkv` | 137 | MP4: 88 bytes, unreadable. MKV: 0 bytes, nothing flushed yet, so the MKV result proves nothing |
| SIGINT 0.1 s or 0.3 s after spawn, before the first frame | **0**, still prints the path | **corrupt**, and no `.ts` file with `-write-first-frame-ts yes` |
| IPC `stop` from a client that keeps the socket open | 0 | reply `{"id":9,"result":"ok","data":"ipc3.mp4"}` after 12 ms |
| IPC `stop` sent through `socat` (it half-closes after stdin EOF) | 0 | the file is saved, but **no reply arrives** |

Other findings [verified]:
- **First frame.** The `.ts` file appeared 473 ms after spawn, which matches the frame counts.
- **Pause commands.** `set-paused true` sent twice returned `ok` both times; `toggle-pause` returned `ok`.
- **Socket path length.** An `-ipc` path longer than 107 characters makes gsr exit 1 at once ("the ipc socket path is too long").
- **`-region` is deprecated.** gsr 6.1.0 warns: "use -w with region directly". The tech-stack doc's `-w region -region WxH+X+Y` should become `-w WxH+X+Y`.
- **Minimum size on NVENC.** H.264 rejected 140 px wide (112 logical) with "Frame Dimension less than the minimum supported value". 146 px worked. Every region shorter than 128 px was encoded 128 px tall (padded). HEVC and AV1 accepted 130×128.
- **No leftovers.** `gsr-kms-server` exited with its parent every time. No gsr and no stray layers were left.
- **HEADLESS-1 can't be recorded with KMS.** `gpu-screen-recorder --list-capture-options` lists only `DP-2|3440x1440`, `region` and `portal`. Omarchy's `--fullscreen` would fail if HEADLESS-1 had focus.

---

## 2. Webcam overlay

### 2.1 Pipeline [source: `omarchy-capture-screenrecording:62-113`]
- **Device**: the first entry from `omarchy-capture-webcam-list`. For each `v4l2-ctl --list-devices` group, that is the first `/dev/video*` whose Device Caps include "Video Capture". This skips metadata nodes.
- **Several cameras**: `omarchy-capture-screenrecording-with-webcam` shows a picker. It always adds desktop and microphone audio.
- **Format**: the first of 640x360, 1280x720 and 1920x1080 that appears in `v4l2-ctl --list-formats-ext`, at `framerate=30` (lines 75-84). The **smallest** mode wins.
- **mpv command**:
  ```
  mpv av://v4l2:DEV --profile=low-latency --untimed --no-cache --demuxer-lavf-o=video_size=…,framerate=30 \
      '--vf=lavfi=[crop=ih*8/9:ih]' --title=WebcamOverlay --wayland-app-id=WebcamOverlay-<small|medium|large> \
      --no-border --no-audio --no-osc --osd-level=0 --really-quiet
  ```
- **Crop**: a centre crop to 8:9, so 640×360 becomes 320×360.
- **No mirroring**: there is no `hflip`.
- **No shape**: it is a plain rectangle with Hyprland's normal border. Omarchy's default is `border_size 2`, `rounding 0`; this user has 1 and 10.
- **Start-up order**:
  1. It waits up to 2 s for the window to map (40 × 50 ms, lines 98-102).
  2. It writes the region file and snaps the window to its size with `omarchy-capture-webcam-resize`.
  3. It sleeps 0.6 s before gsr starts, "or the camera is recorded sliding into its corner" (94-107). There is no `no_anim` rule.
- **Idle**: mpv's `--stop-screensaver` defaults to yes [verified via `mpv --list-options`], so a webcam recording probably keeps the screen awake as a side effect. A plain recording does not [inferred].

### 2.2 Window rules [source: `/usr/share/omarchy/default/hypr/apps/webcam-overlay.lua`]
```lua
o.window("^WebcamOverlay-small$",  { size = { "(monitor_h*4/25)", "(monitor_h*9/50)" },
                                     move = { "(monitor_w-monitor_h*4/25-40)", "(monitor_h-monitor_h*9/50-40)" } })
o.window("^WebcamOverlay-medium$", { size = { "(monitor_h*2/9)", "(monitor_h/4)" },
                                     move = { "(monitor_w-monitor_h*2/9-40)", "(monitor_h-monitor_h/4-40)" } })
o.window("^WebcamOverlay-large$",  { size = { "(monitor_h*3/10)", "(monitor_h*27/80)" },
                                     move = { "(monitor_w-monitor_h*3/10-40)", "(monitor_h-monitor_h*27/80-40)" } })
o.window({ class = "^WebcamOverlay-(small|medium|large)$", title = "^WebcamOverlay$" }, {
  tag = "-default-opacity", float = true, pin = true, no_initial_focus = true, no_dim = true, opacity = "1 1" })
```
- `o.window("string", …)` matches on `class` (`helpers.lua:142-154`).
- Upstream `quattro` drops `tag`/`opacity`. Commit 54e3b53b88 (2026-10-04, #14253) made app transparency opt-in. The rest is identical.
- Per the Hyprland wiki, `move` takes monitor-local coordinates, and "All position variables are monitor-local" (`hyprland-wiki content/configuring/core/rules/window-rules.md`). `monitor_w/h` are probably logical px [inferred].
- The comment says the dedicated app id keeps the window out of "mpv's generic centered floating rules".

### 2.3 Exact sizes
I ran the resize script's own integer arithmetic. The script is at `scratchpad/research/webcam-presets.sh`. All values are logical px.

| Anchor | small | medium | large | medium top-left |
|---|---|---|---|---|
| DP-2 full, 2752×1152 (3440×1440 @1.25) | 184×207 | 256×288 | 346×389 | 2456,824 |
| HEADLESS-1 full, 1536×864 at x=2752 | 139×156 | 192×216 | 260×292 | 4056,608 |
| 1920×1080 @1 | 172×194 | 240×270 | 324×365 | 1640,770 |
| 2560×1440 (4K @1.5) | 230×259 | 320×360 | 432×486 | 2200,1040 |
| Region 800×600+100+100 on DP-2 | 96×108 | 133×150 | 180×203 | 727,510 |
| Tall region 300×1000 (capped by its width) | 117×132 | 163×183 | 220×247 | 97,777 |

- **Heights** are 9/50, 1/4 and 27/80 of the anchor height. **Widths** are 8/9 of the height.
- **Tall, narrow regions**: if 0.3·H would not fit inside the width minus 80, the sizes scale from (W−80)·10/3 instead (`omarchy-capture-webcam-resize:78-94`).
- **Placement**: 40 px from the anchor's right and bottom edges, and at least 40 px from its top-left (137-141). The 40 px margin also appears in Omarchy's PiP rule (`apps/pip.lua`).
- **Physical size on DP-2**: medium is 320×360, which is exactly what the 640×360 feed gives after cropping. Large is 432×486, so that feed is scaled up 1.35× [inferred].

### 2.4 Moving and resizing during a recording [source]
- **Resize keys**: SUPER+ALT+`[` and SUPER+ALT+`]` step through the three sizes, and `reset` means medium. The script finds the window by its title, `WebcamOverlay`, and exits 0 when there is none (31-33).
- **Anchor**: the recorded region from `omarchy-screenrecord-region`, else the window's monitor (61-76).
- **Position resets**: every resize moves the window back to the bottom-right corner, so a position the user dragged to is lost.
- **Moving and free resizing**: only through Hyprland's generic SUPER+left-drag and SUPER+right-drag (`bindings/tiling.lua:70-71`).

### 2.5 The class+title rule and Snip's gotcha
Snip's CLAUDE.md says a Hyprland 0.56 rule that matches class *and* title never matches. The Hyprland 0.56.2 source disagrees [source]:
- The Lua binding registers every key of the `match` table (`src/config/lua/bindings/LuaBindingsConfigRules.cpp` ≈ lines 1200-1225).
- `CWindowRule::matches` returns false at the first property that fails and true otherwise (`src/desktop/rule/windowRule/WindowRule.cpp:376-479`). That is an AND.

Snip recorded the gotcha in commit a1f31fd. At that time its test instances had a different class (`SNIP_APP_ID`) and the title contained an em dash. Either one would explain the miss [inferred]. I did not check this at runtime. One field per rule is still the safe pattern.

---

## 3. Keybindings
All of these are live in `hyprctl binds` [verified]. The definitions are in `default/hypr/bindings/utilities.lua`.

| Keys | Omarchy action | Line |
|---|---|---|
| PRINT | Screenshot | 37 |
| **ALT+PRINT** | **Start or stop recording**: stops any bare-named gsr, otherwise opens the recording menu | 38 |
| SUPER+PRINT | hyprpicker colour picker | 41 |
| SUPER+CTRL+PRINT | OCR | 42 |
| **SUPER+ALT+code:34 / code:35** (`[` / `]`) | **Webcam overlay smaller / larger** | 39-40 |
| SUPER+CTRL+C | Capture menu (Screenshot, Screenrecord submenu, Text, QR, Color) | 4 |
| SUPER+ALT+comma | Run the last toast's action; the "recording saved" toast opens the file | 27 |
| SUPER+CTRL+comma | Toggle DND | 26 |
| SUPER+CTRL+PERIOD | Transcode (picks a file from `~/Pictures:~/Videos`) | 86 |
| SUPER+left-drag / SUPER+right-drag | Move / resize a window (this is how the webcam window moves) | tiling.lua:70-71 |
| RETURN, CTRL+RETURN, TAB, CTRL+TAB, arrows | Bound only while a layer named `selection` (slurp) is open | 44-82 |

- **Snip's current keys**: SUPER+SHIFT+code:12..15 (3, 4, 5, 6) are Snip's. Code:16..19 are still "Move window to workspace 7..10", so a SUPER+SHIFT+7 default would cost Omarchy another binding.
- **Free Print chords**: only four PRINT binds exist, so SHIFT+, CTRL+, CTRL+SHIFT+, ALT+SHIFT+, CTRL+ALT+, SUPER+SHIFT+ and SUPER+ALT+PRINT are all free.
- **SysRq**: ALT+PRINT is the kernel's SysRq chord. `kernel.sysrq=16` here allows only "sync", so these chords are safe.
- **Upstream**: `quattro` keeps the same recording keys.

## 4. Pause
- Omarchy has nothing for pause, restart or discard. Neither the installed scripts nor upstream `quattro` use SIGUSR2, `toggle-pause` or `set-paused` [source].
- Omarchy's indicator would still show "recording" while gsr is paused, because it only runs pgrep.
- gsr pause works: SIGUSR2 toggles it, `set-paused` can be sent twice safely, and the paused time is left out of the file (§1.6) [verified].

## 5. Folder, naming and post-processing [source]
- **Folder** (`omarchy-capture-screenrecording:21-27`):
  - The script sources `~/.config/user-dirs.dirs`, which overrides any `XDG_VIDEOS_DIR` already in the environment.
  - It then uses `${OMARCHY_SCREENRECORD_DIR:-${XDG_VIDEOS_DIR:-$HOME/Videos}}`.
  - If the folder is missing, it shows a critical toast and exits **without creating it**. The screenshot script does create its folder (`omarchy-capture-screenshot:10-14`).
  - The suggested example in `default/uwsm/default:16-17` is `$HOME/Videos/Screencasts`, the same as Snip's SET-05 default.
  - Here, `XDG_VIDEOS_DIR="$HOME/Videos"` and neither environment variable is set [verified].
- **Name**: `screenrecording-$(date +'%Y-%m-%d_%H-%M-%S').mp4`, in local time (176). A temporary `…-preview.png` thumbnail sits next to it and is deleted 2 s later.
- **Post-processing in place** (253-280):
  - It trims the first 0.1 s.
  - It re-encodes the video (libx264 veryfast, crf 20) only when `ffprobe -read_intervals %+0.2 -show_entries packet=flags` shows a `D` flag. Otherwise it stream-copies.
  - For audio: `volume=enable='lt(t,0.4)':volume=0,afade=t=in:st=0.4:d=0.05,loudnorm=I=-14:TP=-1.5:LRA=11`.
  - Then `mv` replaces the original.
- **Other gsr flags** (191): `-k auto -f 60 -fm cfr -fallback-cpu-encoding yes`, plus `-a "default_output|default_input" -ac aac` (one mixed track).
  - Monitor recordings use `-s 0x0`, or `-s 3840x2160` above 4K (115-123).
  - Regions are passed in logical coordinates as `-w WxH+X+Y`.
- **Where Omarchy looks for videos**: `omarchy-transcode` and `omarchy-menu-file` search `~/Pictures:~/Videos` recursively, newest first, and skip hidden folders (`omarchy-transcode:166`, `omarchy-menu-file:33-47`). A `~/Videos/Screencasts` subfolder shows up there; Snip's history under `~/.local/share` does not.
- **`~/.local/share/omarchy`** does not exist [verified]. Omarchy keeps its state in `~/.local/state/omarchy`: `notifications.json`, `indicators/` and `toggles/`.

## 6. Other Omarchy features a recording touches
- **DND IPC** (`plugins/notifications/Service.qml:857-878`; checked live with `qs ipc show`):
  - The target is `notifications`.
  - `setDnd <v>` turns DND on for `true`, `1`, `on` or `yes`; any other value turns it off. It returns `on` or `off`.
  - `isDnd` and `dndState` return `on`/`off`. `toggleDnd` also exists.
  - DND is saved to `~/.local/state/omarchy/notifications.json` (`{"version":3,"dnd":…}`, written with a short delay), so it survives a shell restart.
  - An `omarchy-shell` call takes about 35 ms [verified].
  - DND is **on right now** (file time 2026-10-07 01:09). That is earlier than Snip's `setDnd` code (f7c7f36, 2026-10-08), so Snip did not cause it [verified].
- **Which toasts get past DND** (`NotificationLogic.js:118-121`):
  - App name `omarchy-action`, which is `omarchy-notification-send`'s default and is what Snip's OMA-05 uses.
  - `notify-send` with critical urgency.
  - Toasts sit on the Overlay layer (`Service.qml:962`), so a KMS recording captures them [inferred].
- **`omarchy-shell` needs `OMARCHY_PATH`.** The wrapper fails without it, silently with `-q` (lines 13-17, 40). snipd gets it today from the user manager's environment (checked in `/proc/<snipd>/environ`) [verified].
- **Idle**: Omarchy's idle service is a Quickshell `IdleMonitor { respectInhibitors: true }` (`plugins/services/idle/Service.qml:251-256`).
  - Hyprland blocks idle when a `zwp_idle_inhibitor` exists on a visible surface. Inhibitors on surfaces that are not windows, such as layers, count as visible (`src/managers/input/IdleInhibitor.cpp:20-25, 35-63`) [source].
  - `omarchy-shell idle disable` saves `~/.local/state/omarchy/indicators/stay-awake` to disk and lights the Stay Awake indicator.
- **Tray**:
  - Stock Omarchy puts every tray item the user hasn't pinned into a drawer that opens on hover. It hides items whose status is Passive (`Tray.qml:154-180`).
  - This user's clone `tgomareli.tray` pins every item.
  - A left click calls `Activate`, unless the item says `ItemIsMenu` (832-841).
  - Icons whose names end in `-symbolic` are recoloured to the bar's foreground colour (770-795).
  - Snip's tray item has `ItemIsMenu=false`, a `-symbolic` icon and CONSTANT properties (`app/src/tray.h:14-35`).
- **Menu extension**:
  - `~/.config/omarchy/extensions/omarchy-menu.jsonc` is merged by id into Omarchy's menu (`Menu.qml:51`, `MenuModel.js:65-95`). The user already uses it.
  - A `when` condition is a bash test that runs every time the menu opens.
- **Plugins**:
  - `omarchy plugin add` accepts only git URLs.
  - To install from a package, symlink or copy the plugin to `~/.config/omarchy/plugins/<id>/`, then run `omarchy-shell shell rescanPlugins` and `omarchy plugin enable <id> --before omarchy.indicators` (the enable command supports `--before`).
  - Symlinked plugins load fine; the user's own are symlinked [verified].
  - The user's talkify plugin shows a working pattern: a `service` runs `<daemon> watch` (JSON lines on stdout) and reconnects on a timer, and the bar widget binds to that service.

---

## 7. Integration recommendations for Snip

1. **Start gsr by its absolute path** (`/usr/bin/gpu-screen-recorder`, found through PATH), with `-ipc` under `$XDG_RUNTIME_DIR/snip/` (under 107 characters).
   - Omarchy's indicator, its "Stop Screenrecording" item, ALT+PRINT and its toggling script then never stop Snip's recording.
   - They also never show the misleading "Screen recording saved" toast.
   - This keeps two recorders, each with its own controls.
2. **Handle being stopped from outside anyway.**
   - Treat "gsr exited while recording" as a normal stop **only if** the `.ts` file from `-write-first-frame-ts` exists. Exit 0 plus a printed path does not mean the file is valid (§1.6).
   - Keep the IPC connection open until the `stop` reply arrives; a half-closed socket loses the reply.
   - Set `PR_SET_PDEATHSIG=SIGINT` on gsr, from a long-lived thread, so it saves if snipd dies [inferred].
   - SIGTERM from `systemctl` saves the file [verified].
   - When snipd starts, look for a leftover `snip/gsr.sock` or a leftover temporary file, and recover it (REC-14).
   - A non-fragmented MP4 is lost after SIGKILL or SIGHUP. Test MKV or MPEG-TS working files with a longer recording before choosing one.
3. **Indicator (open question Q4)**: Snip's own surfaces, not Omarchy's.
   - **Zero-install path**: the tray item becomes the stop button while recording.
     - `Activate` stops the recording.
     - The icon switches to a red icon without `-symbolic` (via `NewIcon`).
     - The tooltip shows the elapsed time.
     - It needs NOTIFY properties and SNI signals. I did not check that Quickshell listens to them.
     - On stock Omarchy, onboarding can offer to pin it, because the tray drawer hides it otherwise.
   - **Optional bar plugin** (OMA-04, kinds `service` + `bar-widget`):
     - Copy talkify's pattern: `snip watch` streams JSON lines.
     - Show a red dot with mm:ss. Left click stops; a menu offers pause, restart and discard.
     - Place it with `--before omarchy.indicators`, where Omarchy's own recording dot appears.
   - **Rejected alternative**: reuse Omarchy's indicator with a bare-named gsr plus `omarchy-shell -q omarchy.indicators refresh` after the first frame and after every exit. It costs:
     - the misleading toast, which gets past DND;
     - "start" items in Omarchy's menu that stop Snip instead;
     - no timer and no paused state;
     - a stale lit icon if a refresh is missed.
4. **ALT+PRINT**: let it join the `snip_print_keys` opt-in family as `snip:record` (start or stop), as spec §7 already says.
   - Do **not** take it over only during a recording. Both bindings would fire, and Omarchy's would open its menu.
   - Omarchy's binding can't be restored afterwards, because with a Lua config `hyprctl binds` shows only `__lua`.
   - If the user declines, Snip's recordings stop from the pill, the tray, the widget or `snip record --stop`.
5. **Camera size keys**: while a camera recording runs, add SUPER+ALT+code:34 and code:35 with `hl.bind` handles, and remove each handle exactly once at stop.
   - Omarchy's binding fires too, but exits 0 when no `WebcamOverlay` window exists (resize script lines 31-33).
6. **Camera bubble defaults** (taken from Omarchy, not a product decision yet):
   - **Anchor** to the *recorded rectangle*, 40 logical px from its bottom-right corner.
   - **Sizes**: bubble height = 0.18 / 0.25 / 0.3375 of the recorded height, with Omarchy's cap for narrow regions. On DP-2 full screen: 207, 288 and 389 logical px.
   - **Corner on resize**: keep the corner the user chose; Omarchy's reset to bottom-right is a weakness.
   - **Feed**: 30 fps. Pick the *smallest* 16:9 mode whose height is at least the bubble's physical height, then centre-crop to the bubble's shape. Omarchy's 640×360 is too small for "large" on this monitor.
   - **Timing**: map the bubble before the countdown and use `no_anim`, so frame 0 needs no 0.6 s wait.
   - **Mirroring**: Omarchy does not mirror (REC-08 asks for an option).
7. **Camera window rules** (if it stays a toplevel, architecture §6.3): one match field (`xdg_tag = "^snip-camera$"`, or an ASCII title like "Snip Pins"), plus:
   ```
   float = true, pin = true, no_initial_focus = true, no_dim = true, no_anim = true,
   border_size = 0, no_shadow = true, no_blur = true, rounding = 0,
   opacity = "1 override 1 override", suppress_event = "maximize fullscreen", tag = "+snip-camera"
   ```
   - The `+snip-camera` tag lets e2e tests prove the rule matched.
   - Check in the nested e2e: whether `no_focus` still allows pointer drags, and whether a pinned floating window stays above a fullscreen app. If it doesn't, an overlay layer surface is the alternative.
8. **Folders**:
   - Keep `$XDG_VIDEOS_DIR/Screencasts` as the default (Omarchy's own suggested example) and create it when needed.
   - Decide whether to use `OMARCHY_SCREENRECORD_DIR` and `OMARCHY_SCREENSHOT_DIR` as defaults when they are set. Today Snip reads neither, and `paths::user_dir` ignores the environment.
   - Snip's `Snip … at ….mp4` names never collide with `screenrecording-…mp4`.
9. **DND, idle and toasts**:
   - Reuse `engine.rs::dnd()` (`notifications isDnd` / `setDnd`), but save "DND was off before" in `$XDG_RUNTIME_DIR/snip/`. The current in-memory copy is lost on a crash or `scripts/dev.sh` restart, and Omarchy keeps DND on across restarts.
   - Send no toasts during a recording: `omarchy-action` toasts get past DND and would be recorded.
   - Keep the screen awake with a Wayland idle inhibitor on any Snip layer surface; an empty 1×1 one is enough. Do not use `omarchy-shell idle disable`, which is saved to disk and lights an indicator.
10. **Omarchy capture menu (OMA-03)**, offered in onboarding because it edits user config:
    - Add `trigger.capture.snip-record` ("Record with Snip", `snip record`).
    - Add `trigger.capture.snip-stop` with `"when": "test -e \"$XDG_RUNTIME_DIR/snip/recording\""`. That needs a recording marker file, which also helps users' scripts.
11. **Names to avoid**:
    - `WebcamOverlay` anywhere in a command line or title. Omarchy's `pkill -f WebcamOverlay` is unanchored and would even kill an editor with a file of that name open, so call the QML file e.g. `CameraBubble.qml`.
    - A layer named `selection`, which triggers Omarchy's slurp key bindings.
    - A bare `argv[0]` of `gpu-screen-recorder`.
12. **gsr limits for the selection UI**:
    - Snap to at least about 146×128 physical px for H.264, or a short region gets padding that is not on screen.
    - Offer only the outputs gsr lists for KMS. HEADLESS-1 needs the portal backend or should be disabled.
    - Use `-w WxH+X+Y`, not the deprecated `-region`.

## 8. Decisions for the product owner (next D-numbers)
- **Q4, bar integration**:
  - A: hidden gsr plus Snip's tray and optional plugin (recommended).
  - B: reuse Omarchy's indicator, with the clashes listed in 7.3.
- **ALT+PRINT**: should it join the `snip_print_keys` opt-in, as spec §7 implies?
- **Export folder**: honour `OMARCHY_SCREENRECORD_DIR` and `OMARCHY_SCREENSHOT_DIR` when they are set?
- **Camera defaults**: shape (CleanShot's default is a circle), the size ladder (Omarchy's 18 / 25 / 33.75%), and whether to mirror by default.
- **Audio**: normalise loudness like Omarchy (−14 LUFS) or leave it as recorded? Should the start-up mute and trim always apply?

## 9. Scratch files and cleanup
- **Scripts and copies**, all under `/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/research/`:
  - `webcam-presets.sh`
  - `gsr-signal-test.sh`
  - `hyprctl-binds.txt`
  - upstream copies `upstream-*.sh|lua|qml`
  - the Hyprland sources `hl-WindowRule.cpp`, `hl-LuaRules.cpp`, `hl-IdleInhibitor.cpp`
  - the wiki page `window-rules.md`
  - gsr stderr logs in `gsr/`
- **Test recordings** were 160×96 px at the top-left corner of DP-2, with no audio and never the microphone. They are all deleted. No gsr or `gsr-kms-server` is left running, and `hyprctl layers` shows only `omarchy-background`, `omarchy-bar` and `talkify-hud`.
- I edited nothing in the repo or in `~/.config`.

## Sources
- **Omarchy, installed** (4.0.4-1):
  - `/usr/bin/omarchy-capture-screenrecording`, `-with-webcam`, `omarchy-capture-webcam-list`, `omarchy-capture-webcam-resize`, `omarchy-capture-region`, `omarchy-capture-screenshot`, `omarchy-shell`, `omarchy-notification-send`, `omarchy-toggle-notification-silencing`, `omarchy-transcode`, `omarchy-menu-file`, `omarchy-bar`, `omarchy-plugin-enable`, `omarchy-plugin-add`
  - `/usr/share/omarchy/shell/plugins/bar/indicators/ScreenRecording.qml`, `bar/widgets/Indicators.qml`, `bar/widgets/Tray.qml`, `bar/README.md`, `shell/README.md`, `shell/shell.qml`, `plugins/notifications/Service.qml`, `plugins/notifications/NotificationLogic.js`, `plugins/services/idle/Service.qml`, `plugins/menu/Menu.qml`, `plugins/menu/MenuModel.js`
  - `/usr/share/omarchy/default/hypr/apps/webcam-overlay.lua`, `apps/pip.lua`, `windows.lua`, `helpers.lua`, `bindings/utilities.lua`, `bindings/tiling.lua`; `/usr/share/omarchy/default/omarchy/omarchy-menu.jsonc`, `default/uwsm/default`, `default/agents/skills/omarchy/capture.md`
- **Omarchy, upstream**: https://github.com/basecamp/omarchy (branch `quattro`), commits b44fb74780 (#8374) and 54e3b53b88 (#14253).
- **Hyprland 0.56.2 source**:
  - https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/desktop/rule/windowRule/WindowRule.cpp
  - https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/config/lua/bindings/LuaBindingsConfigRules.cpp
  - https://github.com/hyprwm/Hyprland/blob/v0.56.2/src/managers/input/IdleInhibitor.cpp
- **Hyprland wiki**: https://github.com/hyprwm/hyprland-wiki/blob/main/content/configuring/core/rules/window-rules.md (commit c04680a283).
- **gsr 6.1.0**: man page at `…/scratchpad/gsr-man.txt` (SIGNALS and IPC sections); `strings /usr/bin/gpu-screen-recorder`; `gpu-screen-recorder --list-capture-options`.
- **Live checks**: `hyprctl binds` / `binds -j`, `hyprctl monitors all -j`, `qs ipc -n -p /usr/share/omarchy/shell show`, `omarchy-shell notifications isDnd`, `systemctl --user show-environment`, `/proc/<snipd>/environ`, `/proc/sys/kernel/sysrq`.
- **Snip repo**: `hypr/snip.lua`, `crates/snip-bridge/src/engine.rs:546-562`, `app/src/tray.h`, `crates/snip-core/src/{paths.rs,naming.rs,store.rs}`, commit a1f31fd, `docs/01-product-spec.md` §5.9 / §5.13 / §7 / §9, `docs/02-architecture.md` §4.3 / §6.3 / §8, `docs/research/platform-feasibility-hyprland.md` §9, `docs/research/tech-stack-and-prior-art.md`.