# Studio editing for Snip: research and technical design

Draft, 2026-10-10, for the product owner and whoever builds it. Nothing here is built.

This document proposes how Snip gets the "studio" half of CleanShot X 5's recording:
- zooms, both automatic and placed by hand;
- a smooth, redrawn pointer that can be hidden, resized and restyled;
- clicks and keystrokes shown after the fact;
- motion blur.

The choices the owner has to make are in §14. Where a fact comes from is marked:
- **measured**: run on the reference machine (i5-13600KF, RTX 4080, Hyprland 0.56.2, gsr 6.1.0, Qt 6.11.2, ffmpeg 9.0.1);
- **source**: read in the code, with a path and line;
- **vendor**: read in CleanShot's own app files (never run) or its videos;
- **documented**: from a web page;
- **inferred**.

---

## 0. In short

1. **What CleanShot does.** CleanShot X 5.0.1 (the newest version, 18 Sep 2026) has a third record button, **Record in Studio Mode**.
   - **Recording.** A studio take records the screen without the pointer. It samples the pointer 60 times a second (its position, and which of 32 cursor shapes shows) and logs its buttons and wheel. It also logs keys, the front window and text-field frames.
   - **Editing.** The editor opens as soon as the take stops. It places zooms from clicks and pointer dwell (150 % or 200 %). It redraws the pointer in one of three styles, 1–5× its size, with Natural or Smooth motion, a press effect and ripples. It can hide the pointer when it is idle or while typing, shows keystrokes in 9 positions, and adds motion blur.
   - **Standard recordings** get the same editor, but only with cuts, zooms placed by hand, background, motion blur and audio.
2. **On Hyprland, Snip can do the same without a capture pipeline of its own.**
   - **The pointer out of the video.** gsr's `-cursor no` leaves it out whenever Hyprland draws it on the hardware cursor plane, which it does on the owner's machine (measured). The pointer is baked into the picture when Hyprland falls back to a software cursor: VRR or a second GPU on NVIDIA, tearing, Omarchy's screen zoom, mirroring. Snip checks for these at the start and while recording.
   - **Positions.** Snip polls `j/cursorpos` 120 times a second on its own thread. A request costs 30 µs (p99 126 µs, measured). Positions never travel over socket2, which broadcasts to every listener.
   - **Clicks and keys.** Clicks, releases and the wheel come from non-consuming Hyprland binds, as Snip's clicks already do. Keys come from the existing key hook: command keys only, plus a "typing" marker that carries no content.
   - **The pointer's shape** isn't available in Hyprland 0.56.2. Nothing exposes it, and the ext-image-copy-capture cursor session always sends a blank image (source). Snip v1 draws one arrow from the user's cursor theme.
   - **Alignment.** gsr's file clock is CLOCK_MONOTONIC minus the pauses, and frame j sits at exactly j/fps. Its first-frame file gives a CLOCK_MONOTONIC anchor. The expected error is under one frame.
3. **Rendering.**
   - **One evaluator.** A Rust module, `snip_core::studio`, turns a recording's input log and its edit into a *frame state* for any time t: the view (zoom), the pointer, ripples and keystrokes.
   - **Preview.** The editor's preview is today's QML stage with a pointer and overlays added, driven by the shown frame's own timestamp.
   - **Export.** The **same QML scene is rendered offscreen** (QQuickRenderControl), fed by an ffmpeg decoder and feeding an NVENC encoder. A prototype measured 148–155 fps at 2560×1440 and 113 fps at 3440×1440, with sub-pixel smooth motion. Preview equals export by construction.
4. **Algorithms**, from CleanShot's constants (vendor) and Cap's open code:
   - **Zoom ramps** take at most 0.75 s (Smooth) or 0.4 s (Dynamic).
   - **Two zooms that touch** pan from one to the other instead of zooming out in between.
   - **Follow-cursor zooms** move only when the pointer leaves a dead zone.
   - **Auto zoom** groups clicks that come within 3 s of each other, at 1.5× or 2×, as CleanShot does.
     - Unlike CleanShot, the view is fully zoomed 0.3 s *before* the first click. The top complaint about other tools is zooms that arrive late.
     - A lone click or a resting pointer starts no zoom. Zooming on a resting pointer gave half false hits in OpenScreen's test.
     - Zooms less than 1.5 s apart pan into each other, so the view never bounces out and back in.
   - **The pointer** is smoothed by a damped spring run once over the whole path, with a phase lead so it doesn't lag, and it lands exactly on every click.
5. **The smallest valuable first step** is zooms placed by hand in today's editor, for every recording (no change to recording), exported through the new renderer. The rest follows in order:
   1. studio takes with the redrawn pointer;
   2. auto zoom and follow;
   3. clicks and keystrokes after the fact;
   4. polish (motion blur, styles);
   5. the camera as its own stream.
6. **What to prototype first:**
   - the offscreen export inside snipd's build;
   - how the pointer feels (the spring presets) on a real log;
   - one live check that `-cursor no` and the alignment hold on the owner's screen. This one records the screen, so it needs the owner's OK.

---

## 1. Scope and words

**Where this stands.**
- D3 keeps Studio Mode out of v1, and the spec has VED-04/05 as P2.
- Architecture §6.4 sketched "Studio Mode without our own capture pipeline": gsr `-cursor no`, event logs, offscreen Qt Quick, FFmpeg. This document turns that sketch into a design backed by measurements.

**Two assumptions of the sketch change:**
- Positions come from IPC polling, not from the ext cursor session. Hyprland never fills that session's image, and its positions are whole logical px with no timestamps.
- Keystrokes keep today's privacy filter (§4.4).

**New words** (to add to GLOSSARY.md):

| Word | Meaning |
|---|---|
| **Studio take** | A take recorded without the pointer, with an input log beside it |
| **Input log** | `input.jsonl`: pointer positions, buttons, wheel, filtered keys, typing markers, pauses, and spans where the pointer was baked into the video |
| **View** | The part of the recording that the export shows at a moment. ("Camera" already means the webcam bubble in Snip, REC-08.) |
| **Zoom** | A block on the editor's zoom track: from, to, level, follow or fixed |
| **Studio edit** | A recording's `edit.json`: cuts, audio, background, zooms, pointer, keys, motion blur. It plays the part a screenshot's `edit.json` plays for its objects (ANN-16) |
| **Frame state** | What the renderer needs for one output frame: the source time, the view, the pointer (position, scale, opacity, shape, velocity), ripples, keystrokes |

---

## 2. What the others do

### 2.1 CleanShot X 5.0.1, the target

**Sources:**
- The official 5.0.1 DMG (SHA-256 `85ab95c8…1cf1`, it matches the Homebrew cask), read statically: Swift types, fields, constants and disassembly. It was never run.
- The 7-minute walkthrough's captions (youtu.be/fUB1s3RaD7c).
- The changelog and reviews.

5.0.1 is the newest version: the cask, the changelog and the update server all stop there.

**The flow:**
- The recorder panel gets a third button, **Record in Studio Mode**, with a "What is Studio Mode?" info button (inventory §6.1).
- When a studio take stops, **the editor opens at once**: "the recording is done and … we are now in the brand new video editor" (walkthrough 01:04).
- An export appears as a normal recording card. The project can be saved and reopened later (05:32–05:46).
- On macOS, Studio Mode needs the Accessibility permission (vendor).

**What a studio take records** (vendor):
- The screen video without the pointer: `setShowsCursor: NO`, behind a flag inferred to be Studio.
- Mouse events, each with x, y, an event type, the **cursor type** and a time. The position is polled every 1/60 s.
  - Event types: moved; left, right or other button down, up or dragged; scroll wheel.
  - The cursor type is one of 32 shapes, read from the window server's cursor seed.
- Keys: down, up, modifier changes, and display text.
- The front window's app and frame every 0.2 s, and the focused text field's frame.
- The camera, into its own `camera.mov`, with a time offset.

**The project** (vendor):
- A `.cleanshotvideo` package holding `meta.json` (the format version), `document.json` (the project), `mouse_events.bin`, `files/video.mov`, `files/camera.mov`, `files/background.tiff`, `files/wallpaper.tiff` and `previews/preview.jpg`.
- It is saved periodically while recording, and the editor keeps an undo history.

**Auto zoom** (`FLSmartZoomAnalyzer`; the constants are vendor, how they combine is inferred):
- It runs once, when the project is made.
- Its inputs are mouse and window events, **not keys**.
- **Levels:** 200 % for dense groups, 150 % for others.
- **Timing:** lead-in 0.3 s, trail 0.8 s; each zoom 2–60 s long; gaps under 3 s merged.
- **Grouping:** neighbours within ±3 s and 0.25 of the frame diagonal; peaks at least 5 s apart; dwell of 3 s or more within 5 % of the width.
- Zooms start in Follow Cursor, at (0.5, 0.5).
- No settings, and no way to regenerate.

**Zooms by hand** (vendor; walkthrough 01:30–02:13):
- Click or drag on the zoom track. Drag a zoom's ends or its middle. Backspace deletes.
- Right-click: Apply Zoom Level to All, Remove All Zooms, Split Zoom at Playhead.
- Level 100–400 %.
- **Follow Cursor** or **Manual**. Manual shows a thumbnail with a draggable spotlight that snaps to the centre lines.
- Two zooms pushed together pan from one to the other.
- **Animation** is one setting for the whole project: **Smooth** (default) or **Dynamic**. A ramp lasts min(half the zoom, 0.75 s for Smooth or 0.4 s for Dynamic). The table is vendor; that it gives the ramp is inferred.

**Pointer** (vendor; walkthrough 02:44–03:31):
- Show.
- **Style:** System, Modern or Touch. Internally there are five sets: legacy, tahoe, goldengate, modern, touch.
- **Size:** 1–5, default 2.
- **Motion:** Natural (the default, "a very slight cleanup on the arcs and paths") or Smooth. Both use a One Euro filter.
- **Press effect:** on by default.
- **Ripple:** 8 colours, outline or filled. It is on only if the recorder's Highlight Clicks was on.
- **Hide when not moving:** off. **Hide when typing:** on.
- Drawn from CleanShot's own image sets, one image per shape.

**Keystrokes** (vendor):
- Show; size 0.5–2 (default 1); Dark (default) or Light.
- 9 positions, bottom centre by default.
- **Command keys only** (default) or all keys.
- The settings are copied from the recorder's.

**Camera** (vendor):
- A separate file.
- Show; size 0.5–2; circle, square, rectangle or vertical; 9 positions; flip.
- **Shrink on zoom:** on by default.
- No full-screen camera segments.

**Background and aspect ratio** (vendor):
- None, gradients, wallpapers (blurred too), colours or images.
- Padding up to 30 %, shadow 0–100, corners up to 0.4. No inset.
- Window takes get the first gradient automatically, with 10 % padding, shadow 50 and the window's own corner radius.
- Ratios: Auto (the recording's own), 1:1, 5:4, 4:3, 3:2, 16:9, 4:5, 3:4, 9:16.

**Motion blur** (vendor): Off, Low, **Medium** (default), High = 0, 0.5, 1, 2. What it blurs is not confirmed.

**Audio** (walkthrough 03:54–04:06; vendor):
- Microphone and system audio can each be switched on after recording: "Did you forget to enable system audio … You can turn it on after the fact".
- Volume up to 200 %.

**Export** (vendor):
- Resolution: Original, 4k down to 240p, or Custom.
- Quality: Very low to Best. Audio on or off. An estimated file size.
- **H.264 only, no GIF.**
- Export Current Frame as Image.
- A passthrough when nothing changed.

**Standard takes in the editor** (vendor; walkthrough 06:02):
- Cuts, zooms by hand, background, aspect ratio, motion blur and audio work.
- These say "…only available for videos recorded in Studio Mode": the pointer, keystrokes, camera and Follow Cursor.

**Not known:**
- whether the background's padding stays visible inside a zoom;
- the exact easing curve;
- the One Euro parameters;
- the delay before an idle pointer hides.

### 2.2 Screen Studio, FocuSee, Rapidemo and others

**What the field agrees on** (documented):
- **Record the pointer apart and draw it again later.**
  - Screen Studio "records your screen with cursor hidden and then re-adds it in post production". It swaps in "a high quality, vector version" so the pointer stays sharp at 4K.
  - Camtasia (`.trec`), Tella and Cursorful's desktop apps also keep the pointer as data beside the video.
- **Auto zoom is driven by clicks.**
  - Screen Studio documents clicks only, and FocuSee, Rapidemo, Tella and Cursorful zoom on clicks too.
  - Only Camtasia's SmartFocus (moves, clicks, the active window, typing) and the open Screenize also read context.
- **Springs drive both the zoom and the pointer.** Products show presets, sometimes with tension, friction and mass sliders.

| | **Screen Studio 4** (macOS; 4.0 shipped 2026-10-01) | **FocuSee** (macOS, Windows) | **Rapidemo** (Windows) | **Camtasia 2025** | **Tella** |
|---|---|---|---|---|---|
| Pointer recorded apart | Yes, then redrawn as a vector cursor | Clicks and moves tracked | Clicks; vector cursors | `.trec` keeps cursor data | Yes |
| Auto zoom from | Clicks only; one global switch | Clicks ("AI" picks level and angle) | Clicks | SmartFocus: moves, clicks, active window, typing; meant for recordings over 30 s | Clicks and moves; nearby windows merged; intensity presets |
| Zoom level | Per zoom; keys 0–9; apply to all | 1–4× | Adjustable | Zoom-n-Pan | 100–350 % |
| Zoom animation | "Focused" or "Smooth"; Instant; shortest zoom 0.1 s. Until 2024: Slow, Mellow, Quick, Rapid, plus tension, friction and mass | Slow (default), Medium, Fast, Rapid, plus tension, friction and mass | Timeline | Drag the duration | Gentle, Moderate, Snappy |
| Follow the pointer | Yes | "Follow actions" (3D Motion) | Yes | Pans | Auto follows; Manual stays fixed |
| Pointer smoothing | On by default, can be turned off per clip; Smooth, Medium, Rapid or None; custom physics removed | Level, speed presets, physics | "Fluid glide" | Cursor Smoothing | Yes |
| Size, idle, styles | Slider; "Hide cursor if it's not moving"; macOS and Touch styles; cursor sets; "Always use default system cursor" | Size; idle time and fade can be set; several styles | Size, colour, shape | Colour, glow, shadow | 1.5–3×; hide when inactive; macOS, Windows and Touch sets |
| Click effects | Ripple (redesigned in 4.0), Circle, Shockwave; click sounds | Effects and sound | — | Burst, Rings, Ripple, Scaling, Sound, Warp and more | Ripple, spring |
| Keystrokes | Shortcuts found automatically; single keys optional, skipping keys pressed close together (typing); keycaps (4.0) | Shortcuts and keys; style, size, position | — | Keystroke callouts (older docs) | — |
| Motion blur | Pointer, zoom and pan; the advanced settings were removed in 4.0 | Zoom, screen and pointer; default 50 | Zoom and pointer | Pointer | Subtle to Strong |
| Frame and ratio | Wallpaper, gradient, colour, image; padding, corners, inset, shadow. Ratios: Auto, 16:9, 9:16, 1:1, 4:3, 3:4, 4:5 | Yes; presets | Yes | — | — |
| Webcam | Size, roundness, mirror; **shrinks during a zoom**; a layout track (full, default, hidden). 4.0 adds split screen, face tracking, hide during silence, background removal. | 6 presets; full, PiP or hidden over time; shrinks in zooms | Overlay, side by side, 3D | — | — |
| Export | MP4, GIF; 24, 30 or 60 fps; "up to 3× faster" in 4.0. A reviewer's 2-minute take still took 6 minutes. | MP4, GIF, 480p–4K | MP4, GIF, 4K60 | — | — |

**Others:**
- **Canvid**: spring presets Molasses, Default, Gentle, Stiff; smoothing on by default.
- **Cursorful**: auto zoom needs **2 or more clicks within 3 s**, "to avoid motion sickness".
- **ScreenSnap Pro**: 1–4×; ease-in and ease-out up to 1000 ms each; follows by default.

**Open code that plans zooms** (read at the commits given; licences matter):

| Project | Licence | Auto zoom | Pointer |
|---|---|---|---|
| OpenScreen | MIT | §2.3. No zoom on a resting pointer: its own test found 8 false hits in 16 candidates (`cursor-track.ts`) | Offline spring at 240 Hz; a pull onto click points (0.25 s in, 0.1 s hold, 0.25 s out) |
| Recordly (fork of OpenScreen) | AGPL-3.0 plus branding terms: ideas only | Clicks at most 2.5 s apart, ±0.5 s. It rates a click by what the pointer does in the next 2 s: a still pointer suggests a text field; a downward move, a dropdown; a sideways drag, a text selection. | An exact spring, k = 100/s, c = 21, m = s |
| Screenize | Apache-2.0 | It classifies activity (typing, clicking, navigating, scrolling, reading, idle), each with its own zoom range (typing 2.0–2.8×, clicking 1.5–2.5×, scrolling 1.3–1.5×). Its camera arrives early: 0.4 s before typing, 0.15 s before a click. | Catmull-Rom spline, then a spring (ζ 0.9, response 0.06 s, quicker when fast). Camera dead zone 75 % (60 % while typing). |
| ScreenArc | GPL-3.0 | Clicks under 3 s apart; 1.0 s before to 0.9 s after; at least 3 s; 1.5× | — |
| screenstudio-alt | MIT | Clicks at most 2.5 s apart; typing bursts get their own zoom | Gaussian path (σ 4 frames), resting points snapped |

**What users complain about, and the fix:**

| Complaint | Fix |
|---|---|
| **Zooms start too late and end too late** (Screen Studio's feedback board) | Be fully zoomed *before* the first click (OpenScreen 0.5 s), and hold 1–1.5 s after the last |
| **Dozens of zooms on long takes** | Merge clicks within 3 s; require 2 clicks (Cursorful); keep 1.5 s wide between zooms; "Remove all zooms" in plain sight (Screen Studio hid it) |
| **"Yo-yo"**: out and back in between close zooms | Pan between zooms 1.5 s apart or less |
| **Motion sickness** | Moderate depths (1.5–1.8×); log-scale interpolation; an ease that starts at zero speed |
| **The smoothed pointer lags behind** what the buttons do | A phase lead; a pull onto click points; smoothing off per clip |
| **Physics sliders removed** (power users missed them) | 3–4 presets, with the physics in an advanced panel |
| **Slow exports** | Render on the GPU |
| **Two pointers**, when hiding the real one fails | Detect a baked pointer (§4.1) |

### 2.3 Cap and OpenScreen, the open references

**Cap** (github.com/CapSoftware/Cap, read at `877d3a3`, 2026-10-10). Its licence is AGPL-3.0, except the capture crates, which are MIT. Snip takes ideas from it, never code. Its bundled cursor SVGs are redrawn Apple and Microsoft art.

**Recording:**
- It records the screen without the cursor on macOS, Windows and X11.
- **On Wayland it embeds the cursor** (portal `Embedded`) and records **no cursor, click or key data** ([#2419](https://github.com/CapSoftware/Cap/issues/2419)). That is the gap Snip can fill on Hyprland.
- It polls position, buttons and keys every 16 ms.
- It saves each new cursor image once, by hash, as a PNG with a fractional hotspot.
- Positions are relative to the captured rectangle. Times count from the pipeline's start. There is one folder per pause segment.

**Editing:**
- Edits live in `project-config.json`, saved atomically 250 ms after the last change.
- Undo keeps snapshots of the whole configuration. A drag is one step, and the limit is 100.

**One wgpu renderer for preview and export.** Everything is a pure function of t, computed once per edit.
- **Zoom:** three springs (amount, centre and "activity"). Stiffness 200, damping 40, mass 2.25, re-aimed every 8 ms.
- **Cursor:** a fixed-step spring. The presets (tension/mass/friction) are Mellow 470/3/70, Fast 380/1/30, Smooth 80/2.5/28 and Slow 200/2.25/40.
  - A **phase lead** of friction ÷ tension (149 ms for Mellow) removes the lag.
  - A shake filter drops tiny reversals.
  - The cursor gets stiffer up to 175 ms before a click and snaps to the click up to 500 ms ahead.
- **Click:** the cursor shrinks to 0.8× with a 130 ms ease. An optional ripple ring has radius 1 − (1 − t)³ and alpha (1 − t)^1.5 over 0.6 s.
- **Idle hide:** after at least 0.5 s (default 2 s), with a 400 ms fade.
- **Motion blur:** one pass. The cursor and the screen get a 21-tap directional blur, and a 13-tap radial blur when the zoom dominates.
- **What zooms:** the whole recording card, corners and shadow included, grows over a still background. The cursor grows with it, and the camera shrinks to 0.7.

**Auto zoom** (on by default):
1. Each click becomes [t − 0.3 s, t + 2.5 s].
2. Clicks in the last 1 s are ignored.
3. Gaps of 2.5 s or less merge.
4. Each zoom is 2× in follow mode. The camera re-aims only when the cursor leaves a cluster box of 50 % × 70 % of the view, and a focus within 25 % of an edge puts the view flush against that edge.

**Lessons from its issues:**
- The smoothed cursor lagged 149 ms until the phase lead ([#1649](https://github.com/CapSoftware/Cap/issues/1649) → [#1978](https://github.com/CapSoftware/Cap/pull/1978)).
- Keystrokes followed output time and ran past cuts ([#2118](https://github.com/CapSoftware/Cap/pull/2118), [#1725](https://github.com/CapSoftware/Cap/issues/1725)).
- The cursor was a different size in preview and export until both sized it from the source resolution ([#629](https://github.com/CapSoftware/Cap/issues/629) → [#633](https://github.com/CapSoftware/Cap/pull/633)).
- On area recordings the cursor was misplaced until positions were mapped through the crop ([#453](https://github.com/CapSoftware/Cap/issues/453)). The hotspot was ignored ([#311](https://github.com/CapSoftware/Cap/issues/311)).
- Follow-cursor zoom jumps in 9:16, and users ask for a fixed area ([#1401](https://github.com/CapSoftware/Cap/issues/1401)).
- **Inferred bugs:**
  - Auto zoom adds up click times across pauses without offsets, so zooms after a pause land at the wrong time.
  - The drag spring profile never fires on real recordings: button numbers are 1-based, and the tests used 0.

**OpenScreen** (github.com/getopenscreen/openscreen, MIT, read at `77cf022` and `3ee6a52`; the original repo is archived).

**Capture on Linux:**
- It asks the portal for cursor *metadata*. That works with the GNOME and KDE portals, not with xdph.
- Clicks come from evdev, which needs the `input` group.

**Cursor:**
- It is smoothed offline at 240 Hz with a spring.
- It keeps two tracks: the raw track draws the cursor, and the smoothed track drives the camera, with a 50 % dead zone.

**Zoom:**
- 1.25–5×, with the scale interpolated in log space.
- The ramp lasts 0.6 s + 0.55·ln(scale).
- Two zooms 1.5 s apart or less pan instead of zooming out and back in.

**Auto zoom suggestions from clicks:**
- It stays wide for 2.5 s at the start and 1 s at the end.
- It merges clicks within 3 s, with a lead of 0.5 s and a tail of 1.5 s.
- A zoom holds for at least 1.8 s, and at least 1.5 s stays wide between zooms.
- The depth is chosen so the click group fits with a 12 % margin.
- Zooms the user placed are never moved.

### 2.4 Linux and Hyprland today

- **Cap**, on Wayland: no cursor data (above).
- **OpenScreen**: the portal's cursor metadata plus evdev. Neither works on Hyprland as it is: xdph has no metadata, and evdev needs the `input` group.
- **xdph 1.4.1** offers only the Hidden and Embedded cursor modes, so no PipeWire cursor metadata (source `Screencopy.cpp:672`). Its current master is the same.
- **gsr** asks the portal for metadata when it can, then draws the cursor itself, but it never writes the cursor out.

**Other Linux tools:**

| Tool | What it does | Pointer and input | On Hyprland |
|---|---|---|---|
| OBS + obs-zoom-to-mouse | Zoom and follow on a hotkey, live | X11 `XQueryPointer` | Misses native Wayland pointers |
| hypr-obs-mouse-follow (GPL-3.0) | Follow and zoom in OBS, live | **Polls `hyprctl cursorpos`**, the same source as §4.2 | Hyprland only |
| Zoominator (OBS plugin) | Zoom, follow, click halo, live | — | X11 |
| Recordly, ScreenArc | Studio editors | uiohook / X11 | Recordly: "Cursor hiding is not supported" on Linux; ScreenArc's tracking is X11 only |
| Screenix (commercial) | Recorder with auto zoom | — | "Doesn't work on Arch" (weak source) |
| Kooha, GPU Screen Recorder UI | Simple recorders | Pointer on or off | Burned-in pointer only |
| Kdenlive, Shotcut | Video editors | No pointer data | Zoom only by keyframes placed by hand |

**Keystroke tools on Wayland:**
- showmethekey (Apache-2.0, in Arch) reads libinput through a root helper.
- wshowkeys is setuid root, and was archived in 2025-12.
- screenkey is X11 only, and keyviz has no Linux release.

All of them draw on screen while recording, so keys are burned into the video and can't be added afterwards.

**Two attempts that fell short on Hyprland:**
- **OpenScreen** refuses its editable-cursor mode on Hyprland, because xdph has no cursor metadata (`portal.rs:104-111`).
- **OpenScreen PR #157**, which would have read the cursor from Hyprland's IPC, was closed without merging.

**As far as we found, nothing on Hyprland records the pointer separately from the screen today.** Snip's existing Hyprland hooks give it clicks and keys without root or the `input` group, which no other Linux tool has.

### 2.5 What Snip takes

| Topic | Taken from | Snip's default |
|---|---|---|
| Flow | CleanShot, then D54 | A Studio switch on the panel, off until turned on and remembered between takes (D54). When a studio take stops, the editor opens (Q3). Every take keeps its clicks, so any recording gets auto zooms (D54). |
| Auto zoom | CleanShot's grouping and levels; OpenScreen's guards (wide at the start and end, never move the user's zooms); Cursorful's "2 clicks"; the complaints above (fully in before the click; no dwell) | On for studio takes |
| Zoom ramps | CleanShot | Smooth 0.75 s, Dynamic 0.4 s, one setting for the project |
| Zoom levels | CleanShot | 1–4×. Auto zoom picks 1.5×, or 2× for dense clicks. |
| Follow | Cap | A dead zone of 50 % × 70 % of the view; edge snap at 25 % |
| Pointer motion | Cap's spring with a phase lead and click snapping | Natural |
| Pointer size | CleanShot | 1–5 × the theme's size, default 2 |
| Press effect and ripple | Cap's shapes, CleanShot's defaults | Press on. Ripple on if Highlight Clicks was on. |
| Hide pointer | CleanShot | When typing: on. When idle: off (2 s). |
| Keystrokes | CleanShot, plus Snip's live HUD rules | Bottom centre, command keys only |
| Motion blur | CleanShot's levels, Cap's one-pass method | Medium |
| What zooms | Cap | The card grows over a still background |
| Undo and saving | Cap | Snapshots; saved 300 ms after the last change, atomically |

---

## 3. Design overview

```
Panel: Studio switch on ─▶ record_go (studio)
   ├─ gsr -cursor no ─────────────────────────────▶ recording/<stamp>.mp4 (+ <stamp>.mp4.ts)
   ├─ pointer poller: j/cursorpos at 2 × fps ─┐
   ├─ Hyprland hooks: buttons, wheel, keys ───┼──▶ recording/<stamp>.input.jsonl
   └─ watch: gsr pauses, software-cursor spans ┘
Stop ─▶ history/<id>/: original.mp4 · input.jsonl · edit.json (defaults + auto zooms) · poster.png
     ─▶ the video editor opens
          ├─ Rust: studio::Studio::open(log, edit) → tracks (pointer path, view path, ripples, keys)
          ├─ Preview: QML stage; a frame relay sets t = the shown frame's time → the frame state
          └─ Export, Replace or Save as New:
               ffmpeg decodes the kept frames → snipd --render-video renders the same QML offscreen
               → ffmpeg encodes with NVENC (+ today's audio graph)
               ─▶ history/<id>/source.mp4: what cards, Copy, Save, drag and Quick Look use
```

**The seams this adds:**
- **The input log.** A pure Rust reader and writer in `snip_core::studio`, tested with `cargo test`.
- **The evaluator.** `Studio::frame(t)`, a pure function used by both the preview and the export.
- **The renderer.** `StudioFrame.qml`, the one scene that both the editor and `snipd --render-video` load.

---

## 4. Capture

### 4.1 A video without the pointer

**How gsr handles it (source).**
- In KMS mode, gsr-kms-server sends gsr every plane with a framebuffer, for every frame (`kms_server.c:356-469`).
- `-cursor` only decides whether gsr draws the cursor plane over the frame (`kms.c:1011-1024`). With `-cursor no`, the cursor stays out.
- A *software* cursor is already part of the primary plane, and nothing can remove it.

**When Hyprland draws a software cursor** (source, Hyprland 0.56.2):

| Condition | Code | The owner's machine (measured) |
|---|---|---|
| Actively tearing | `Monitor.cpp:2294` | `allow_tearing` false |
| `cursor:no_hardware_cursors = 1` | `:2300` | 2 (auto) |
| `no_hardware_cursors = 2` (auto): NVIDIA **and** (two GPUs **or** VRR active on any monitor) | `:2303` | NVIDIA, one card, VRR off, so **hardware** |
| `cursor:zoom_factor ≠ 1`: every monitor goes software. Omarchy's zoom keys change this. | `Renderer.cpp:2119-2126` | 1 |
| The output is the source of a mirror | `Monitor.cpp:1395` | no mirrors |
| The cursor image is bigger than the plane (256×256 on NVIDIA) | `PointerManager.cpp:452` | 30–48 px |
| *Check:* is the hardware plane in use? | — | Yes: Hyprland's log says "Cursor buffer imported into KMS" (256×256 dumb buffers) |

**What Snip does:**
- **At the start of a studio take**, it reads:
  - `j/getoption` for `cursor:no_hardware_cursors`, `cursor:zoom_factor` and `cursor:invisible`;
  - `j/monitors` for the recorded output's `vrr`, `mirrorOf` and `activelyTearing`, and `vrr` on any monitor;
  - the GPU count and vendor from `/sys/class/drm/card*/device/vendor`.

  If any condition holds, the pointer will be baked in. What happens then is decision Q8.
- **While recording**, once a second (on the hooks' existing heartbeat thread), it rereads `zoom_factor` and `activelyTearing`. It writes `baked` spans into the input log, so the editor never draws a second pointer over one that is already in the picture.
- **Snip never changes the user's `no_hardware_cursors`.** Hyprland applies a change only at the next cursor change anyway (source).
- **Accepted:** drag-and-drop icons are surfaces, so they are always in the frame.
- **Verified on screen (spikes S2 and S2b, 2026-10-12).** On the owner's DP-2, with none of the conditions above, Hyprland's auto mode drew the pointer in software: `-cursor no` still had it. With `no_hardware_cursors` forced to 0 it was left out. Aquamarine opened two DRM backends for the one card: a suspect for the auto rule's "two GPUs". D57: a studio take forces it to 0 and puts it back. Forced on a nested session's `WAYLAND-` output, Hyprland 0.56 crashes at the next pointer move, so Snip does it only when every output is a DRM one.

### 4.2 Pointer positions

| Source | Rate and timing | Cost | Verdict |
|---|---|---|---|
| **Snip polls `j/cursorpos`** (socket1) | As fast as asked. Each sample is stamped by Snip. | **30 µs p50, 126 µs p99 at 120 Hz**; capacity about 73,000 requests/s (measured). Less than 0.4 % of one core on Hyprland's main thread. | **Use**: twice the frame rate, at most 240 Hz |
| A Lua timer + `hl.get_cursor_pos()` + socket2 `custom>>` | Timer period plus loop latency; drifts. Lua has no sub-second clock, so it can't stamp samples. | Every event wakes every socket2 client: today `socat`→bash (Omarchy's monitor watch), quickshell (omarchy-shell) and snipd. A client that stops reading is cut off after about 342 events: 2.9 s of stall at 120 Hz (measured). | **No.** It could get omarchy-shell disconnected. |
| ext-image-copy-capture **cursor session** | A position on every commit of the source's monitor, when it changed. Whole **logical** px (not buffer px as the spec says). No time. | A Wayland object on connection A | Possible later. Its image is always blank (§4.5). |
| A Hyprland plugin on the event bus (`input.mouse.move`, `.button`, `.axis` with libinput times) | Exact | About 150 lines of C++, rebuilt for every Hyprland or library update. A stale plugin can crash the session. | **No for v1** |
| The portal's cursor metadata | — | xdph has none | Not available |

**How the poller works:**
- It runs on its own thread while a studio take records, and not while it is paused.
- Each sample is one connection with one request, written straight after connecting. (A client that connects and doesn't write blocks Hyprland for up to 5 s: `HyprCtl.cpp:2264`.)
- The sample is stamped with CLOCK_MONOTONIC at the midpoint of the round trip.
- A line goes into the log only when the position changed, so an idle pointer costs nothing on disk.
- **Values** are global logical px, floored to integers (`HyprCtl.cpp:1291`). Snip maps them to video px as V = (G + 0.5 − area origin) × scale. The 0.5 centres the floored value. At scale 1.25 a step is 1.25 physical px, and smoothing hides it.
- **The pointer outside the recorded area** (or on another output) is logged anyway. The evaluator clips it at the card's edge and doesn't draw it outside.

### 4.3 Buttons, drags and the wheel

- **Presses.** Snip already has non-consuming binds on `MOD3 + MOD5 + mouse:272/273` (`crates/snip-hypr/src/recording.lua`). At a press they read `hl.get_cursor_pos()`, whose floats are exact.
- **Releases.** Studio takes add `release = true` twins, so every press has a release. That gives the press effect, and drags with their paths (from §4.2).
- **The wheel.** Non-consuming `mouse_up` / `mouse_down` binds give *markers* only: they fire for mouse wheels, never touchpads, and at most once per `binds:scroll_event_delay` (300 ms) (source `KeybindManager.cpp:446-475`). Auto zoom uses them to tell scrolling (reading) from clicking.
- **Transport.** socket2 `custom>>` lines, as today: a few events a second at most. Snip stamps them when they arrive. The hop is 3 µs (earlier research).
- **One fix carried along.** socket2 events now hand off to a channel. Today `record_input` takes the recording's lock on the event thread, so a slow gsr stalls it (review gap 12).

### 4.4 Keys, typing and privacy

- **The hook.** The existing `hl.on("input.keyboard.key")` hook sends command keys only:
  - modifiers;
  - keys pressed with Ctrl, Alt or Super;
  - Esc, Tab, Enter, arrows, F-keys and similar.

  It sends nothing while a password manager or the polkit prompt has the focus, and nothing while paused. A lock pauses the recording (REC-14). That stays the default for studio takes.
- **New: a typing marker** with no content. When a key outside that set goes down, the Lua sends `snip:<sid>:t` **at most once per 500 ms**, with the same private-window check. It gives:
  - "hide the pointer while typing" (CleanShot's default; Omarchy already hides the real pointer on key press with `cursor:hide_on_key_press`);
  - typing-aware auto zoom.

  It doesn't reveal what was typed, and at 500 ms it doesn't reveal typing rhythm either.
- **"All keys"** (CleanShot's option) means writing every key into a file in history, including passwords typed into ordinary windows. It is a choice made *before* recording, never on by default (Q5).
- **Timestamps.** The key hook's own time is libinput's (CLOCK_MONOTONIC ms, 32-bit), but arrival stamps are within microseconds, so Snip uses its own clock for every event.
- **Side finding.** `hyprctl -j locked` exists in 0.56.2 (`HyprCtl.cpp:1967`). The pause-on-lock check could poll it instead of parsing `solitaryBlockedBy`.

### 4.5 The pointer's shape

**What Hyprland 0.56.2 offers (source):**
- **Nothing exposes the shape.** Hyprland knows the shape name a client asked for through cursor-shape-v1 (`m_lastCursorData.name` / `.shape`), and the names of its own resize and move cursors. No hyprctl command, Lua function or socket2 event exposes them.
- **The cursor session's image is always blank.**
  - `CCursorshareSession::render` draws only if the cursor has *both* a buffer and a surface (`CursorshareSession.cpp`, render).
  - `setCursorBuffer` and `setCursorSurface` each reset the other first (`src/pointer/PointerManager.cpp:127-200, 241-262`).
  - So the image is always transparent. Checked in the source for this document. Hyprland's main branch is the same.
- **Clients that set a surface cursor** (XWayland, games) give only pixels and a hotspot.

**Snip's plan:**
1. **v1: one pointer image for the whole take.** It is the user's theme "default" arrow, or Snip's own art (Q7).
   - **Finding the theme**, in this order:
     - `HYPRCURSOR_THEME`/`_SIZE`;
     - `XCURSOR_THEME`/`_SIZE`;
     - gsettings;
     - "default"/24.
   - **On the owner's machine** that is XCursor **Adwaita**, 24 logical px (30 physical at 1.25). Its sizes run 24–96 and its licence is CC-BY-SA-3.0 OR LGPL-3.0.
   - **Rendering.** Snip draws from the theme's largest size (96 px), so the pointer stays sharp up to about 96 px on screen. The default size 2 inside a 2× zoom is 120 px at 1.25, a slight enlargement. Larger than that needs an SVG: a hyprcursor SVG theme, or Snip's own set (Q7).
   - **Loading.** Rust's `xcursor` crate (MIT) parses XCursor files; its pixels are premultiplied BGRA. A hyprcursor reader (zip, manifest, PNG/SVG) is about 150 lines.
2. **Later: a guess from the cursor session's hotspot events.** Adwaita's hotspots differ by shape at 24 px: default (3, 1), pointer (7, 5), text (11, 12) (inferred). That tells an arrow from a hand from an I-beam well enough. It must be checked in the nested Hyprland first (spike S6).
   - **Checked (S6, 2026-10-13):** a pointer cursor session sends a `hotspot` event at each change, without a capture session, 30–60 ms after the move: (3, 1) over the desktop, (11, 12) over a terminal's text, (7, 5) over a button, in logical px at scale 1.25. Built: a studio take logs them, and the editor draws the theme's shape whose hotspot at the take's size matches.
3. **The real fix is upstream.** Hyprland could send a shape event or a `j/cursor` request returning `m_lastCursorData.name`, and fix the session's render. With that, Snip could redraw every shape as CleanShot does. Proposing it is public work in the owner's name (Q12).
4. **Until then**, a user who needs exact shapes (I-beams over text) records a standard take, which keeps the real pointer.

### 4.6 Clocks and alignment

**gsr's clocks** (source, the same in 6.1.0 and master):
- **The recording clock** is CLOCK_MONOTONIC minus paused time (`utils.c:26`, `recording_clock.c:41-56`).
  - Nothing is captured while paused, and timestamps stay continuous, so **pauses are removed from the file**.
- **Constant frame rate.** With `-fm cfr`, frame j has pts exactly j in a time base of 1/fps (`recorder.c:622`). It shows the screen as sampled at about S + (j + 1)·T, where:
  - S is when recording started (`gsr_recorder_run`);
  - T = 1/fps.
- **`-write-first-frame-ts`** writes `monotonic_µs \t realtime_µs` when the **first encoded packet reaches the muxer** (`encoder.c:24-37, 181`). gsr's code calls the value `evdev_compatible_ts`: it exists to line input up with video. With NVENC it lands at about S + 3T (earlier measurement).
- **Pauses.** gsr prints `gsr info: Paused` / `Unpaused` when it applies a pause, even with `-v no` (`recorder.c:668`). Snip already sends gsr's stderr to a log file. It watches that file and stamps each line when it appears. The fallback is the time of Snip's own IPC request, which is off by −T to +20 ms.

**The mapping** (all times CLOCK_MONOTONIC):

```
video_time(t) = t − S − T − Σ (Rᵢ − Pᵢ)  for every pause [Pᵢ, Rᵢ] with Rᵢ ≤ t
S ≈ ts_mono − k·T    (k = 3 for NVENC; calibrated per encoder in spike S3)
events with Pᵢ ≤ t < Rᵢ are dropped
```

**Expected error:**
- About ±2 ms, plus up to one refresh of how old the screen content is (7 ms at 144 Hz). That is under one frame at 60 fps.
- A pointer one frame early or late, or a ripple, can't be seen.
- Each pause edge adds −T to +20 ms if it falls back to the IPC time.

**Fake recorder.** For tests, the test-pattern recorder starts its clock at once, so S is the take's start.

**Better anchors, if spike S3 shows they are needed:**
- **A pre-roll.** Start gsr, queue a pause, and take S as the moment the unpause is answered. But the start leaks one frame (review C1).
- **An observe-only gsr plugin** (`-p lib.so`) whose `draw()` runs once per capture. It gives exact capture times, but costs an RGBA pass, and a crash in it kills the recording.

### 4.7 The input log

`input.jsonl` is written beside the take while it records, in `~/.local/share/snip/recording/`. It is on disk and flushed every second, as gsr's fragments are. Then it moves into history with the video.

**Why JSON lines, not binary:**
- One line per event, append-only.
- A torn last line (a crash) is simply dropped.
- Easy to read and diff.
- Small next to the video: moves are logged only when they change, about 25 bytes a line, roughly 1–3 MB per 10 minutes of active mouse use (inferred).
- CleanShot writes a binary log and Cap writes JSON. Either works.

```jsonc
{"format":"snip-input","version":1,"clock":"monotonic_us","gsr_ts":[1234567890,1760000000000000],
 "fps":60,"encoder":"nvenc","output":"DP-2","scale":1.25,"area":[0,0,2752,1152],"video":[3440,1440],
 "cursor":{"theme":"Adwaita","size":24,"hide_on_key_press":true},"keys":"commands","baked":false}
{"t":1234560000,"m":[1201,644]}               // pointer moved (global logical px, floored)
{"t":1234590000,"b":272,"d":1,"p":[1201.4,644.9]}   // left button down, exact position
{"t":1234710000,"b":272,"d":0,"p":[1203.0,646.2]}   // released
{"t":1235000000,"w":"down"}                    // wheel marker
{"t":1235400000,"k":54,"d":1}                  // a command key (C with Ctrl held)
{"t":1235900000,"ty":1}                        // typing happened (no content)
{"t":1236000000,"pause":1}                     // gsr applied a pause (stderr line)
{"t":1238000000,"pause":0}
{"t":1239000000,"baked":1}                     // the pointer went software (zoom, tearing)
```

**Loading.** Snip reads the raw file and converts it into video px and video time when the editor opens. The file itself is never rewritten. Unknown record types are skipped, so newer Snips can add records.

### 4.8 What changes in Snip's code (capture side)

| Where | Change |
|---|---|
| `snip_core::record::Spec` | `cursor: false` for a studio take (the field exists) |
| `snip_core::studio::log` (new, pure) | The log's types, writer and reader, time mapping, and the software-cursor checks as pure functions over `hyprctl` JSON. Unit-tested. |
| `snip_core::take` | Keep `.ts` contents and gsr's pause lines before stop deletes them (`take.rs:243`) |
| `snip_bridge::engine::record` | A studio take: the panel's Studio switch (D54), the poller thread, hooks with releases, wheel and typing, the 1 s check for software cursors, moving the log into history, recovering a log left behind with its take, and opening the editor at stop. socket2 events go through a channel (review gap 12). The live HUD draws **no** rings or keys during a studio take: they are in the log, and drawn ones would be baked in twice. Every take, the switch off too (D54): the poller and the click hooks, no keys. |
| `snip_hypr::keys::recording_lua` / `recording.lua` | Release binds, wheel binds, the typing marker |
| `snip_core::store` | `ingest_media` takes the log and the first `edit.json`; `original.mp4` (§5.1) |

---

## 5. The studio edit

### 5.1 Files in history

The layout mirrors a screenshot's (ANN-16, D28: "the capture keeps its original and its objects, so they stay editable"):

| File in `history/<id>/` | Standard recording | Studio take |
|---|---|---|
| `source.mp4` (or `.gif`) | The recording, or the result of its last Replace | The rendered result. It doesn't exist until the first render. |
| `original.mp4` | The recording as recorded, kept from the first Replace on | The take without the pointer |
| `edit.json` | The edit, once one has been exported | The studio edit, written at stop (defaults plus auto zooms) |
| `input.jsonl` | — | The input log |
| `poster.png` | The first frame | The first frame of the render; until then the take's own first frame |

**What changes:**
- **Replace stops destroying the recording** (Q10). It renders `source.mp4` again from `original.mp4` and `edit.json`, and reopening the editor brings the edit back.
- **Save as New** makes a new capture. Its `original.mp4` is a hard link to the old one where the filesystem allows, else a copy. It gets its own `edit.json` and its own render.
- **Everything else in Snip** keeps using `source.mp4`: cards, history, Copy, Save, drag and Quick Look.
- **A studio take not yet rendered** is in history at once (with a "Studio" badge), as every capture is (D10). Copy, Save and drag render it first, with progress on its card (Q3).
- **Disk.** A studio take keeps two videos once rendered. History's retention (HIS-02) and `snip status` count both.

### 5.2 The schema

Times are **source milliseconds**: the recording's own time, as the editor's cuts already are. A cut never shifts a zoom.

```jsonc
{
  "format": "snip-video-edit", "version": 1,
  "trim": [0, 61234], "cuts": [[12000, 15500]],
  "audio": { "tracks": [1.0, 0.8], "mono": false },
  "background": null,                 // the editor's background object today (D50)
  "aspect": null,                     // null = the recording's own; "16:9" … CleanShot's list
  "zooms": [
    { "id": "z1", "from": 4200, "to": 9100, "level": 2.0,
      "follow": true, "focus": [0.42, 0.31], "auto": true }
  ],
  "zoom_style": "smooth",             // smooth (0.75 s ramps) | dynamic (0.4 s)
  "motion_blur": "medium",            // off | low | medium | high = 0, 0.5, 1, 2
  "pointer": { "show": true, "size": 2.0, "style": "theme", "motion": "natural",
               "press": true, "ripple": { "color": "accent", "filled": false },
               "hide_idle": false, "hide_typing": true },
  "keys": { "show": true, "all": false, "size": 1.0, "look": "dark", "position": "bottom-center" },
  "export": { "height": null, "quality": 23, "gif": null }
}
```

- **On a standard recording** (the Studio switch off), `pointer` and `keys` are absent or ignored: its pointer is in the picture. Zooms, auto zooms and follow work from its log (D54). The editor greys the Pointer and Keystrokes sections out: "Turn on Studio on the recording panel to change the pointer".
- **Versions.** Readers refuse a newer `version`. Unknown fields are kept when the file is saved again, as `.snip` projects do (architecture §5.3).

### 5.3 Saving and undo

- **Saving.** `edit.json` is saved 300 ms after the last change, through `store::atomic_write` (Cap does it at 250 ms). Closing the editor never loses an edit.
- **Undo** works as the screenshot editor's does (`Editor.qml:48-78`): a list of whole-document snapshots.
  - The edit is small, under 50 KB even with hundreds of zooms.
  - A slider or handle drag is one step: the snapshot is taken at the press and committed at the release.
  - Limit 100. Ctrl+Z, and Ctrl+Shift+Z or Ctrl+Y.

---

## 6. Evaluating a frame (`snip_core::studio`)

One pure Rust module. Given the input log and the edit, `Studio::open` builds the *tracks*. `Studio::frame(t)` reads them for any source time t. The preview and the export both call it, so both draw the same thing at the same t.

### 6.1 Spaces and time

| Space | Unit | Used for |
|---|---|---|
| G | Global logical px (Hyprland's layout) | IPC and Lua positions; gsr's `-w WxH+X+Y` |
| V | Video px: the recorded area's physical px, (0, 0) at its top-left | The edit, the tracks, the frame state |
| C | Canvas px of the export (background, padding, aspect ratio) | The renderer |

**Time:**
- Output time maps to source time through the kept parts (trim and cuts). That mapping moves from QML (`VideoEditor.qml:60-76`) into Rust, because the export needs it.
- Tracks are built on the **video timeline**, where pauses are already removed. Cap gets this wrong.

### 6.2 The pointer

All of it is built once into a 240 Hz table. Every step is deterministic.

1. **Clean.**
   - Drop a sample that reverses direction with both steps under 1.5 % of the frame within 100 ms. This is Cap's shake filter. Screen Studio has a similar "Remove cursor shakes" option, its method unpublished.
   - Keep at most 120 samples a second.
   - A gap over 67 ms means the pointer was idle, not that it travelled slowly.
2. **Smooth.** A damped spring (stiffness k, mass m, damping c) chases the raw path. It runs at fixed 1/240 s steps from the start of the take.
   - **Phase lead.** It chases the raw position c/k seconds *ahead*: a spring that follows a moving target trails it by c/k. That removes the lag (Cap #1978).
   - **Presets, as starting points for spike S5:**

     | Mode | k / m / c | Lag removed | Basis |
     |---|---|---|---|
     | Natural (default) | 380 / 1 / 30 | 79 ms | Cap's "Fast" |
     | Smooth | 470 / 3 / 70 | 149 ms | Cap's "Mellow" |
     | Off | Raw positions, joined by straight lines | — | |

   - **Alternatives.**
     - CleanShot uses a One Euro filter, an adaptive low-pass. Offline it can run forwards and then backwards to remove its lag.
     - Screenize gives the spring a short response (0.06 s) that shortens further at speed.

     The spring is chosen because Cap shows how to make it land on clicks. Lag is the most common complaint about smoothed pointers (§2.2).
3. **Land on clicks.**
   - Up to 500 ms before a press, the spring's target is the press point.
   - Within 175 ms it switches to a stiff profile (530 / 1 / 40).
   - From press to release the pointer sits on the logged press position, unless it is a drag (more than 8 px of travel). A drag follows the raw path with a stiff spring (1000 / 1 / 40).
   - Snip's tests must use **real button numbers** (272/273). Cap's drag profile is dead because its tests used 0.
   - OpenScreen's simpler alternative pulls the smoothed pointer onto the click point: 0.25 s in, 0.1 s hold, 0.25 s out. Spike S5 compares the two.
4. **Hide** (all opacity changes are smoothstep fades):
   - **Typing:** fade out over 150 ms at a typing marker. Show again when the pointer moves more than 4 px.
   - **Idle**, if on: fade out over 400 ms after 2 s without movement. Fade back in so it is fully visible by the next move.
   - **Outside the recorded area:** clipped at the card's edge, so it slides off the edge rather than popping. Not drawn outside.
   - **In a `baked` span:** not drawn, because the video already has it.
5. **Size.** Base size = the theme size × the recording's scale (24 × 1.25 = 30 px on DP-2). Drawn size = base × the size setting (1–5, default 2) × the view's zoom. The pointer belongs to the card, so it grows with a zoom (as Cap does).
6. **Press effect.** 0.8× while a button is down, eased over 130 ms at each edge (Cap).
7. **Ripple.**
   - A ring at each press, at its exact logged position.
   - Its radius grows from 16 to 56 px × scale, with 1 − (1 − p)³ over 0.6 s. Its alpha is (1 − p)^1.5.
   - Colour: accent for the left button, warning for the right, as the live HUD does today.
   - Outline 3 px, or filled at 30 %.
   - It lives in the card, so it grows with a zoom.

### 6.3 The view (zooms)

**A zoom** is `{from, to, level 1–4, follow | fixed focus}`. Zooms never overlap; they may touch.

**Ramps.**
- Each zoom ramps in at its start and out at its end, *inside* the block: the block on the timeline is exactly when the view is not at 1×.
- A ramp lasts min(half the zoom, 0.75 s) for Smooth and min(half the zoom, 0.4 s) for Dynamic (CleanShot).
- Spike S5 also tries OpenScreen's ramp, which lengthens with depth (0.6 s + 0.55·ln(level)), against motion sickness.
- Easing (both to be tuned in spike S5):
  - Smooth uses smootherstep, 6p⁵ − 15p⁴ + 10p³.
  - Dynamic uses a critically damped spring's curve, 1 − (1 + ωt)·e^(−ωt) with ωT = 7, scaled to reach 1 at the ramp's end (OpenScreen's).
  - Both start at zero speed. OpenScreen dropped a curve that "started at full speed on the first frame".

**The scale** is interpolated in log space, so a zoom feels even (as OpenScreen does).

**A fixed zoom** zooms about the one point that stays still on screen. For views A (centre cA, scale sA) and B (cB, sB):

```
p    = (cA·sA − cB·sB) / (sA − sB)      // the point that stays put
c(s) = p − (p − cA)·sA / s              // centre at any scale between them
```

- Equal scales are a pure pan: interpolate the centre with the same easing.
- The formula was checked numerically for this document (`/tmp/claude-1000/…/studio/homothety.py`): p stays on the same output pixel all the way.

**Follow:**
- **Pre-aim.** While the view is still at 1×, the centre jumps straight to its target, so the zoom scales toward the pointer (Cap).
- **Dead zone.** Inside the zoom, the target centre moves only when the smoothed pointer leaves a box of 50 % × 70 % of the view, and then just far enough to bring it back in.
- **Spring.** The centre follows its target with a spring (200 / 2.25 / 40: ζ ≈ 0.94, settles in about 0.45 s; Cap's default), stepped at 240 Hz.
- **Zoom-out.** The centre holds still during the ramp out (Cap).

**Zooms that touch:**
- The view goes straight from the first zoom's state to the second's over the second's ramp, without zooming out. Scale and centre ease together.
- A gap shorter than both ramps added together is treated as touching (OpenScreen pans at ≤ 1.5 s).

**Staying inside:**
- The view stays inside the card: the card always covers the canvas while zoomed, unless the zoom is too small for that.
- A focus within 25 % of an edge puts the view flush against that edge (Cap's edge snap).

**What zooms** (Q6). The recording's card grows — the video, its rounded corners, its shadow, the pointer and the ripples — over a background that stays still. Once the level passes 1 + 2 × the padding (1.2× at 10 % padding), the card covers the whole canvas and the background is gone.
- Keystrokes (and later the camera) sit on the canvas, so they don't zoom.
- While a padding or corner slider is dragged, the preview drops out of the zoom so the frame can be seen (CleanShot's `skipZoomRendering`).

### 6.4 Auto zoom

Snip's own rules: CleanShot's grouping and levels (vendor), OpenScreen's guards, and the fixes for what users complain about elsewhere (§2.2). They are deterministic and unit-tested on recorded click sequences. Every constant lives in one table in the code, to be tuned with the owner in spike S5.

1. **Events.** Left and right presses (not middle), the start and end of each drag, typing markers and wheel markers, all on the video timeline.
   - **Ignore** presses in the last 1.0 s. The click that stops a whole-screen take lands on the bar (D52), and the video never shows its result.
2. **Groups.** A press joins the current group when it comes within 3 s of the group's last press and within 0.25 × the frame diagonal of the group's centre (CleanShot's neighbour radii). Otherwise it starts a new group.
3. **What earns a zoom.**
   - **A group of 2 or more presses**, or one press followed by typing within 2 s.
   - **A lone click doesn't zoom.** This keeps long takes from getting dozens of zooms. Cursorful requires the same, and CleanShot's minimum peak score of 1 suggests it does too (inferred).
   - **A resting pointer doesn't zoom.** CleanShot zooms on 3 s of dwell, but OpenScreen measured 8 false hits in 16 candidates and dropped it. Screen Studio, Cap and Tella zoom on clicks alone. The dwell rule stays in the code, off by default.
4. **Timing.**
   - The view is **fully zoomed 0.3 s before the first press**: from = first press − 0.3 s − the ramp in. CleanShot starts its zoom 0.3 s before the click, which leaves it still moving at the click, and "zooms start too late" is the top complaint elsewhere.
   - It holds until 1.0 s after the last press, then ramps out: to = last press + 1.0 s + the ramp out.
   - Typing inside a zoom pushes its hold to the last typing marker + 1.0 s.
   - A zoom is 2–60 s long (CleanShot); longer ones are split.
   - Wheel markers lasting more than 1 s end a zoom early: scrolling is reading.
5. **Level.**
   - 2.0× for a group of 3 or more presses, else 1.5× (CleanShot's levels).
   - Lowered, down to 1.25×, until the group's box fits in the view with a 12 % margin (OpenScreen).
   - Mode: **follow**, as CleanShot's auto zooms are. With the dead zone, a group that doesn't move keeps the view still anyway.
6. **Merging and spacing.**
   - Zooms less than 3 s apart merge (CleanShot).
   - If their groups don't fit one view, they become two zooms that touch, and the view pans between them.
   - Separate zooms keep at least 1.5 s at 1× between them ("less reads as a yo-yo", OpenScreen); closer ones are made to touch.
   - The video opens wide for at least 2 s and ends wide for at least 1 s.
7. **Re-running.**
   - Results are marked `auto: true`. Editing one turns it into a user zoom.
   - **Re-run auto zoom** (a Snip extra) replaces only `auto: true` zooms. Zooms the user placed are never moved.
   - **Remove All Zooms** sits in plain sight in the zoom lane's menu. Screen Studio hid it, and users complained.
   - Auto zoom runs on standard takes too, from the clicks every take keeps (D54).
8. **Typing-only stretches.** CleanShot follows text-field frames from Accessibility, which Wayland doesn't offer. Instead, a typing stretch of 2 s or more with no zoom open makes one at the last press. That needs the press within the previous 10 s and the pointer not to have moved more than 10 % of the width since: that is where the caret probably is.
9. **Later ideas** (not v1):
   - Recordly rates a click by what the pointer does next: a still pointer suggests a text field, a downward move a menu, a sideways drag a text selection. That could set the level.
   - Screenize gives each activity its own depth.

### 6.5 Keystrokes

Input: key events from the log. Labels come from `snip_hypr::keys::key_label`, by US position, as the live HUD does. A non-US layout can mislabel letters there too.

**The rules** are today's live HUD rules (`RecordHud.qml:295-321`), now as a pure function of t:
- A combo (held modifiers + a key, or a special key on its own) shows from its press. It stays 1.5 s after the last press, or until a different combo replaces it.
- The same combo again within 1.5 s shows "×N".
- Modifiers held on their own show as "Ctrl +" after 150 ms, so a quick tap doesn't flicker.
- Fades: in over 120 ms, out over 200 ms.

**All keys** (only if they were logged, Q5): printable keys join into a running text. It shows the last 24 characters and stays 1.5 s after the last key.

**Settings:**
- show: on if the recorder's Show Keystrokes was on (CleanShot's rule);
- size 0.5–2;
- Dark or Light;
- 9 positions, bottom centre by default;
- command keys or all keys.

**Placement.** The keystrokes sit on the canvas, not inside the zoom. Their items are cut at cuts and trims (Cap #1725).

### 6.6 Motion blur

The levels are Off, Low, Medium and High = 0, 0.5, 1, 2, with Medium as the default (CleanShot).

- **The pointer.** Smear length = the pointer's travel this frame, in canvas px, × the level × fps/60, capped at 480 px (Cap). It is drawn as 8 copies along that path with falling opacity, or as a directional-blur ShaderEffect on the pointer's layer. Spike S1 measures which is cheaper.
- **The view.** When the view moves between frames, the card gets a directional blur along the move (21 taps). When the scale change dominates it gets a radial blur (13 taps). Nothing under 1 px moves, and never across a cut (Cap).
- **Phasing.** Pointer blur arrives in Phase 5, and view blur after it if spike S1 shows the cost is small. Both live in `StudioFrame.qml`, so they are in the preview and the export alike.

### 6.7 Determinism and cost

- **Tracks** are arrays sampled at 240 Hz. `frame(t)` interpolates linearly between two samples. No QML animation and no wall clock is involved, so a frame at t is the same in the preview, in a seek and in the export.
- **Build time.** Rebuilding after an edit means a spring run of 144,000 steps for 10 minutes, a few milliseconds (inferred). Only the tracks the edit touches are rebuilt.
- **Memory** is about 30 MB per hour of recording at 240 Hz (pointer 6 floats, view 3).

---

## 7. The preview in the editor

The QML structure. It builds on the rendering research's prototype (measured) and today's stage (`VideoEditor.qml:436-470`):

```
StudioFrame.qml — the canvas, at the export's size (the editor fits it into the stage with one Scale, as today)
 ├─ BackgroundLayer                still: never zoomed
 ├─ RectangularShadow              at the card's zoomed rectangle
 ├─ card                           the card's zoomed rectangle, clipped to the canvas;
 │   │                             layer + the MultiEffect rounded mask, as today
 │   ├─ VideoOutput                at the card's zoomed rectangle (it may reach past the canvas)
 │   ├─ ripples                    at their positions through the view
 │   └─ pointer Image              the theme's image at 96 px, or an SVG; mipmap: true
 ├─ keystroke pill                 on the canvas, never zoomed
 └─ (Phase 6) camera
```

**The view is applied as geometry, not as a `Scale` transform.** Rust gives the card's zoomed rectangle in canvas px. So:
- the rounded-mask layer is never bigger than the canvas;
- the video is sampled once, at its final size;
- the pointer and ripples are drawn at their final size, so they stay sharp.

The rendering prototype used a `Scale` inside a clipped card. Both are sub-pixel exact (measured jitter 0.007 px), and spike S1 settles the choice.

This scene lives in one file, **`StudioFrame.qml`**, split out of `VideoEditor.qml`'s stage. The editor and `snipd --render-video` both load it.

**Which time it shows:**
- A small C++ **frame relay** owns the `QVideoSink` that `MediaPlayer` writes to.
- On the GUI thread, it passes each frame to the `VideoOutput`. In the same slot it sets `t = frame.startTime()` and asks Rust for that frame state. The picture and the overlays then reach the scene graph in the same sync.
- `MediaPlayer.position` is *not* the shown frame. It updates at 20 Hz and is the time that was asked for (earlier research §2.3).
- Seeks follow the frame that actually lands.
- Audio sync comes free: the player paces video to the audio clock, and the overlays follow the video.

**The timeline:**
- A **zoom lane** goes above the frames, as in CleanShot.
- Blocks show "2×".
- Click or drag on empty lane to add a zoom: 2 s at the playhead, at the last level used.
- Drag a block's middle to move it, or its ends to resize it. Delete or Backspace removes it.
- Right-click: Apply Level to All, Remove All Zooms, Split Zoom at Playhead.
- Selecting a zoom opens its popover: a level slider (1–4); Follow or Manual; for Manual, a thumbnail with a spotlight to drag, which snaps to the centre lines.

**The sidebar** gets CleanShot's sections:
- Cursor (Pointer);
- Keystrokes;
- Audio and Background (today's);
- Animations: zoom style and motion blur;
- later, Camera.

**Shimmer.** Video textures have no mipmaps, so a 3440 px video drawn about 1,300 px wide can shimmer while a zoom moves. Fixes: the planned all-intra scrubbing proxy (already in the plan, "a scrubbing proxy"), or `layer.mipmap`.

---

## 8. The export

### 8.1 The options, measured

| Approach | Build | Speed (2560×1440 / 3440×1440) | Quality | Preview = export | Verdict |
|---|---|---|---|---|---|
| **A. ffmpeg graphs only**: libplacebo for the zoom, cursor tiles baked in Rust and placed with `overlay` | Medium, **plus a second renderer** in QML for the preview, kept in step forever | ~150 fps at 1.1 cores / 138 fps (measured) | Zoom sub-pixel with spline36. The pointer only through workarounds. No text or shadow inside libplacebo. Zoom motion blur cuts it to 58 fps. | Approximate: two renderers | **Fallback** |
| **B. The same QML, rendered offscreen** (QQuickRenderControl) | Medium: about 500 lines of C++ (render control, frame relay, pipes), plus the Rust frame state, plus splitting `StudioFrame.qml` out | **148–155 fps / 113 fps end to end**, 3.9–5.3 cores including software decode (measured) | Sub-pixel exact (jitter 0.007 px). The pointer stays sharp above the zoom. The video is sampled bilinearly: about 30 % softer than spline36 at 2×, fixable with a bicubic shader. | **Exact by construction** | **Chosen** |
| C. A Rust wgpu compositor (Cap's) | Extra large: Cap's crate is 83 files and 1.75 MB | GPU-fast, but the preview needs a readback or dma-buf sharing | Best control | Only if the preview shows its frames | No |
| D. libmpv preview + ffmpeg export | Medium; a new dependency | As A | As A | Closer than A, but each edit rebuilds the graph | No |

**How the ffmpeg-only filters measured** (rendering research): a zoom from 1× to 2× over 10 s, with the jitter RMS of tracked dots:

| Filter | Jitter |
|---|---|
| `zoompan` | **1.24 px** (it rounds to whole and even pixels) |
| `scale` + `crop` | 0.41 px |
| libplacebo | 0.005 px |
| Qt Quick `Scale` | 0.007 px |

**An ffmpeg 9.0.1 bug:** `sendcmd` to libplacebo's `crop_*`/`pos_*` replies "Success" and does nothing. Per-frame data can reach libplacebo only as expressions in t.

### 8.2 The chosen pipeline

```
ffmpeg -i original.mp4 -filter_complex "<today's cut graph, video only>" -f rawvideo -pix_fmt yuv420p pipe:1
   │ frames in output order
   ▼
snipd --render-video <spec.json>      (a child mode of snipd, like --gallery: its own GPU context, off the daemon)
   loads StudioFrame.qml in QQuickRenderControl (GL, the editor's backend), at the export size
   for frame n: t_src = kept-parts map (n / fps) → Studio::frame(t_src) → QVideoSink::setVideoFrame(decoded n)
               → render → read back (RGBA; later an NV12 pass) → write to the encoder
   ▼
ffmpeg -f rawvideo -pix_fmt rgba -s WxH -r fps -i pipe:0 -i original.mp4
       -filter_complex "<today's audio graph>" -map 0:v -map [ac]
       -c:v h264_nvenc -preset p4 -rc vbr -cq N -movflags +faststart out.mp4
   (libx264 when NVENC can't, as today: media.rs export fallback)
```

- **Child mode.** Running the renderer as `snipd --render-video` keeps 3–6 ms of render work per frame, and a second GPU context, off the daemon's GUI thread. A crash in it can't take snipd down.
- **Readback** is synchronous at `endFrame()`. Threads overlap the reading and writing pipes.
- **Colour.** With RGBA input, NVENC tags the stream bt470bg, even when bt709 is asked for. Players that read the tag show it correctly. Better: a small RGBA→NV12 (BT.709) shader pass before readback. It also makes the readback 2.7× smaller (Cap does the same).
- **GIF.** Render the MP4, then run today's two-pass GIF recipe on it (`media.rs:164-238`).
- **Progress.** Frames written ÷ total, sent through today's `VideoExportStatus`.
- **Exports with nothing to draw** (no zooms, no pointer, no background) can keep today's ffmpeg graph, which runs at 254 fps (measured baseline). That is Snip's version of CleanShot's passthrough.
- **Export Current Frame as Image** (CleanShot) becomes one render of one frame to PNG, as a new capture (Phase 5).
- **New gotchas for CLAUDE.md:**
  - `QT_QPA_PLATFORM=offscreen` has no RHI, so the child needs a Wayland connection, though it never maps a window.
  - `focus` is a FINAL property in Qt 6.11, as `right` is.
  - The Vulkan headers aren't installed, so stay on GL.

---

## 9. Performance budget

| Stage | Budget | Basis |
|---|---|---|
| The logger during a take | ≤ 0.5 % of a core in snipd, ≤ 0.4 % of Hyprland's main thread, no GPU | 30 µs per request at 120 Hz (measured) |
| Log size | 1–3 MB per 10 minutes of active use | estimate |
| Opening a studio take | Tracks built in under 50 ms for 10 minutes; the first frame shown in under 300 ms | estimate |
| The preview | The video's own fps at 1440p; overlays on the shown frame; an edit visible within 50 ms | prototype |
| The export | At least 100 fps at 2560×1440 (1.6× real time at 60 fps); at least 90 fps at 3440×1440 | measured 148–155 / 113 fps |
| Idle | Nothing: no polling outside a studio take (performance doc, "Idle CPU") | |

---

## 10. Tests

**Phase 1** is tested at one seam, the studio evaluator in `cargo test` (the owner's choice, 2026-10-11).

**Rust unit tests** (`cargo test -p snip-core`):

| Area | What they check |
|---|---|
| Input log | Reading a torn last line; skipping unknown records |
| Time mapping | Pauses, CFR, the encoder delay, the fake recorder |
| The pointer | It lands on every press within one frame. No lag at constant speed (the phase lead). The shake filter; typing and idle hiding; leaving the area; baked spans. Real button numbers only. |
| Zooms | The view stays inside the card. Ramp lengths. Touching zooms pan. The follow dead zone. Edge snap. |
| Auto zoom | Fixtures: one press; a burst; two groups far apart; the press that stops the take dropped; across pauses; user zooms left alone |
| Keystrokes | Combos, ×N, held modifiers, cut at cuts |
| Determinism | `frame(t)` is the same whatever order times are asked in |

**The fidelity test.** Grab `StudioFrame.qml` in the editor at time t, and compare it with `--render-video`'s frame at t. PSNR must be at least 45 dB, with text anti-aliasing masked.

**e2e** (nested Hyprland, `SNIP_RECORDER=fake`, run with `--only`):
- A studio take where the harness moves and clicks the pointer. Real pointer input works in the nested session; keys from wtype don't fire binds there.
- Check the log's moves and presses.
- Open the editor, and probe the pointer item at a frame with `where()`.
- Export 1 s, and check the pointer's pixels in the file.

**By hand,** with the owner's OK: one tiny live take with `-cursor no` (S2), and the alignment flash (S3).

---

## 11. Phased plan

Each phase is several small PRs, each reviewable and shippable on its own (CLAUDE.md). PR titles become release notes.

| Phase | What the user gets | Work | Needs | Main risk | Done when |
|---|---|---|---|---|---|
| **0. Spikes** (about a week) | — | S1–S6 below | Owner's OK for S2 and S3 (they record the screen) | — | Each question answered with a number |
| **1. Zooms you place, for every recording** (smallest valuable step) | Zooms on today's recordings: the zoom lane and popover; level 1–4; Manual focus with the spotlight thumbnail; Smooth or Dynamic; touching zooms pan; the edit kept (`edit.json`, a non-destructive Replace, Q10); undo | `studio` view path and tests → `StudioFrame.qml` and the frame relay → the zoom lane → `snipd --render-video` → `edit.json` and undo | S1 | Export speed and colour | The golden fidelity test passes; at least 100 fps at 1440p; the owner likes the ramps |
| **2. Studio takes and the redrawn pointer** | the Studio switch on the panel (D54); the editor opens at stop; the Pointer section: show, size, Natural or Smooth or Off, press effect, hide when typing or idle; the theme's arrow, sharp at any size | The log (writer, reader, mapping) → the poller and hooks → software-cursor checks and baked spans → history files and recovery → pointer track and tests → the Pointer section | S2, S3, S4, S5 | A baked pointer on other setups; how the smoothing feels | A live studio take on the owner's machine exports with a smooth pointer that lands on every click within a frame |
| **3. Auto zoom and follow** | Zooms placed on every new take (D54); Follow Cursor zooms; Re-run Auto Zoom | Auto zoom and fixtures → follow dead zone and edge snap → typing and wheel rules | Phase 2 | Zooms that feel wrong | Fixture tests pass; the owner rates the zooms on 5 real takes |
| **4. Clicks and keys after the fact** | Ripples (colour, outline or filled); the Keystrokes section (size, look, 9 positions, command or all keys) | Ripple and keystroke tracks → the two sections → the live HUD off during studio takes | Phase 2 | Key labels on non-US layouts | Keys and ripples line up with the video in the export |
| **5. Polish** | Motion blur; Snip's own pointer set; a guess at hand and I-beam from hotspots (if S6 works); CleanShot's aspect-ratio list; window takes get a background automatically; Export Current Frame as Image | — | S1 cost numbers; S6 | Cost of the blur shader | — |
| **6. The camera as its own stream** | Studio takes record the webcam to `camera.mp4` instead of drawing it on screen. Layout after the fact: size 0.5–2, shape, 9 positions, flip, shrink on zoom. | — | — | Recording the camera alongside gsr: CPU, sync | — |

---

## 12. What to prototype first

| Spike | Question | How | Needs |
|---|---|---|---|
| **S1. The renderer inside snipd** | Does `snipd --render-video` reach ≥ 100 fps at 1440p with the real `BackgroundLayer`, the MultiEffect mask and a pointer? Is the preview grab equal to the export? What does motion blur cost? | Turn the rendering research's prototype (`/tmp/claude-1000/…/studio/render/qt/`) into the child mode; add the NV12 pass; run the golden test | — |
| **S2. No pointer in the video** | Does `-cursor no` leave the pointer out on DP-2? Does Omarchy's zoom bring it back? | Record a 160×128 area under the pointer, with and without the flag | **The owner's OK** (it records the screen) |
| **S3. Alignment** | What is k (S = ts − k·T) for NVENC and x264? How big is the pause-edge error? | Flash a small white square at a known CLOCK_MONOTONIC time inside a tiny area; find its frame; pause once | **The owner's OK** |
| **S4. The logger's cost** | CPU of snipd and Hyprland, and frame pacing, with 120 Hz polling during a 5-minute take | Poll during a fake take; `perf stat`, Hyprland's frame times | — |
| **S5. How the pointer feels** | Which spring presets and ramp easings look right? | A QML playground replaying a real log (from S4) beside the raw path; the owner picks | The owner's eye |
| **S6. Shape guess** | Do cursor-session hotspot events arrive on 0.56.2, and separate arrow, hand and I-beam? | A client on connection A in the nested Hyprland | — |

---

## 13. Risks

| Risk | Effect | Likelihood | Mitigation |
|---|---|---|---|
| The cursor goes software (VRR or a second GPU on NVIDIA, tearing, zoom) | The pointer is baked into the video, and a redrawn one would double it | Medium on other machines; low here | Check at the start and every second; `baked` spans; decision Q8 |
| Export speed or colour on other GPUs (AMD/Intel: no NVENC, VAAPI) | Slow exports, or a wrong colour tag | Medium | x264 fallback as today; the NV12 pass; a VAAPI encoder later |
| Alignment off by more than a frame (encoder delay differs, pause edges) | A pointer slightly ahead of or behind hover effects | Low | Calibrate k per encoder (S3); gsr's stderr pause lines; the gsr plugin as a last resort |
| No shape information | An arrow over text fields and links looks a little wrong | Certain (v1) | The theme's arrow; the hotspot guess (S6); upstream (Q12); a standard take keeps the real pointer |
| Keystroke privacy | Passwords in a file | Low with the defaults | Command keys only; the typing marker carries no content; "all keys" is opt-in, before recording |
| Every take logs its clicks and pointer positions (D54) | A record of where you clicked, beside every recording | Certain | No keys outside studio takes; kept in the recording's history folder and deleted with it |
| Hyprland updates change Lua or IPC | Hooks or the poller stop working | Medium | Snip's existing `reinstall`; IPC is tolerant JSON; a version probe; the fake e2e covers the hooks |
| Disk: two videos per studio take | History grows | Medium | Render on demand (Q3); retention counts both; Save copies only `source.mp4` |
| gsr changes (6.1.3 flushes the end, timing) | Alignment and the last frames shift | Medium | Re-measure after updates (S3 harness) |
| A fractional scale (gsr truncates, Snip rounds) | Under 1 px of offset | Low | Smoothing hides it; replicate gsr's rounding in `V = …` |
| Long takes (an hour or more) | Track memory and build time | Low | 240 Hz is about 30 MB per hour; build tracks lazily per chunk if needed |
| The socket2 event thread stalls (today's gap 12) | Lost clicks and keys | Medium | The channel hand-off in Phase 2 |

---

## 14. Decisions for the owner

Each question has options and a recommendation. Recorded answers become D-numbers in `docs/00-README.md`.

**Q1. When does Studio arrive?** D3 keeps it out of v1.
- (a) After 1.0, as decided.
- (b) **Zooms placed by hand (Phase 1) in v1, the rest after 1.0.**
- (c) All of it in v1.

**Decided (D55, 2026-10-11): (c), now and in 1.0:** "Studio arrives now, I want to start development on studio".

**Q2. How is a studio take started?**
- (a) **A third button on the panel, "Record in Studio", as CleanShot has.** A key such as Shift+Enter could go with it.
- (b) A Studio switch on the panel.
- (c) Every video is a studio take when possible.

**Decided (D54, 2026-10-11): (b),** a Studio cell on the panel with Mic, System audio, Camera, Clicks and Keys, off until switched on and remembered between takes. The owner weighed Studio as every recording's default ("if u dont want zooming u will turn it off from editor"); it would make every Copy, Save and drag wait for a render.

**Q3. What happens when a studio take stops?**
- (a) **The editor opens (CleanShot), and the take is in history at once; Copy, Save and drag render it first, with progress on the card.**
- (b) As (a), and Snip also renders it in the background with the defaults.
- (c) Only a card; the editor opens on a double-click.

*Recommend (a).*

**Decided (D58, 2026-10-12): (a).**

**Q4. Auto zoom on new studio takes?**
- (a) **On, as CleanShot does, with Remove All Zooms one click away.**
- (b) Off.
- (c) A setting, default on.

*Recommend (a).* A setting can come later if people ask.

**Decided (D54, 2026-10-11): (a),** auto zoom on every take.

**Q5. Which keys are logged in a studio take?**
- (a) **Command keys only (today's filter), plus the typing marker with no content.**
- (b) As (a), with "All keys" as an opt-in chosen before recording, with a warning.
- (c) None.

*Recommend (a) now and (b) in Phase 4.* Passwords typed into ordinary windows would end up in history with "All keys".

**Decided (D62, 2026-10-13): (a),** shortcuts only; no "All keys".

**Q6. What does a zoom enlarge?**
- (a) **The recording's card (video, corners, shadow, pointer) grows over a still background. The padding leaves the canvas as it zooms in, as in Cap.** CleanShot probably does the same: it drops out of the zoom while a padding slider is dragged, so the padding can be seen.
- (b) Only the video inside its card, so the padding and corners stay.

**Decided (D56, 2026-10-11): (a),** the card grows over a still background.

**Q7. Pointer art?**
- (a) **The user's own cursor theme (Adwaita here), drawn at a large size.**
- (b) Plus Snip's own set: a licence-clean arrow, hand and I-beam drawn for Snip.
- (c) Bundle an open theme (for example Bibata, GPL-3.0).

*Recommend (a) first, then (b) in Phase 5.* Never Apple's or Microsoft's art.

**Decided (D59, 2026-10-12): (a)** first; Snip's own set can follow in Phase 5.

**Q8. What if the pointer can't be left out** (software cursor at the start)?
- (a) **Say so on the panel ("VRR is on: the pointer will be in the video") and record the studio take with the pointer baked in. Zooms, keys and clicks still work; the Pointer section is greyed out.**
- (b) Refuse Studio for that take.
- (c) Switch Hyprland to hardware cursors during the take.

*Recommend (a).* Never (c): it changes the user's own setting, and brings back NVIDIA's VRR problems.

**Decided (D57, 2026-10-12): a hardware pointer for the take,** not a baked one: (c), or another way to the same end. Spike S2 found the pointer in the video with `-cursor no` on DP-2 although none of §4.1's conditions held (Aquamarine had opened two DRM backends for the one card: a suspect for the auto rule's "two GPUs"). How Snip gets the hardware plane, and whether forcing `cursor:no_hardware_cursors = 0` for the take does it, is spike S2b: a first try was inconclusive, the pointer hidden by a key press.

**Q9. Pointer defaults.**
- (a) **CleanShot's: size 2, Natural, press effect on, hide when typing on, hide when idle off, ripple only if Highlight Clicks was on.**
- (b) Snip's own.

*Recommend (a).*

**Decided (D60, 2026-10-12): (a),** CleanShot's.

**Q10. Replace keeps the original** (`original.mp4` + `edit.json`, as screenshots do with D28).
- (a) **Yes: edits stay editable.** It costs one more video once Replace is used.
- (b) No: Replace overwrites, as today.

**Decided (D56, 2026-10-11): (a), yes:** edits stay editable.

**Q11. Sound in studio takes.** CleanShot lets you turn system audio on after the take, so it records it anyway.
- (a) **Record only what the panel says, as today.**
- (b) Always record the microphone and system audio as separate tracks, and choose later.

*Recommend (a).* Recording sound the user switched off would surprise them.

**Q12. Upstream work in the owner's name.**
- (a) **Propose a Hyprland change: a cursor-shape event or request, and a fix for the cursor session's blank image. Snip could then redraw every shape.**
- (b) Not now.

*Recommend (a), once Phase 2 ships.*

**Q13. An input log for standard takes too** (clicks and positions, no keys)? That would allow auto zoom and follow on any recording, with the real pointer still in the picture.
- (a) **No: only studio takes, as CleanShot does.**
- (b) Yes.

**Decided (D54, 2026-10-11): (b), yes.** Every take keeps its clicks and pointer positions (no keys), so the editor can place auto zooms on any recording; the card still shares it as recorded.

**Q14. The camera in studio takes.**
- (a) **Keep today's bubble, drawn on screen and baked in, until Phase 6.**
- (b) Record it to its own file from Phase 2 on.

*Recommend (a).*

**Decided (D61, 2026-10-13): Phase 6 now, its own file.** The bubble shows while the take is set up, to frame yourself, and goes once recording starts (it would be in the video too); the editor places the camera.

---

## 15. Sources

**CleanShot:**
- The 5.0.1 DMG: https://updates.getcleanshot.com/v3/CleanShot-X-5.0.1.dmg (SHA-256 `85ab95c8a4be4fb650a94ec087a7e7d89f4e6bda9c18611cdd44ab6a3edb1cf1`; its main binary is `4876cf92…f167`). Read statically: `/tmp/claude-1000/…/studio/cs5/fields2.txt` (the type and field dump), `dis_x86.txt`.
- https://formulae.brew.sh/api/cask/cleanshot.json
- https://cleanshot.com/changelog
- The walkthrough: https://www.youtube.com/watch?v=fUB1s3RaD7c (its captions are in `/tmp/claude-1000/…/studio/cs5/yt/walkthrough.txt`)
- The teaser: https://www.youtube.com/watch?v=PzlKjndagGU
- https://www.layerpath.com/compare/cleanshot-vs-screen-studio
- https://www.podfeet.com/blog/2026/09/cleanshot-5/
- Snip's own inventory: `docs/research/cleanshot-x-feature-inventory.md` §6.5

**Cap and OpenScreen:**
- https://github.com/CapSoftware/Cap at `877d3a3`; issues and PRs as linked in §2.3; the cap-v0.6.0 release, Linux since 2026-09-15.
- https://github.com/getopenscreen/openscreen at `77cf022`

**Other products** (fetched 2026-10-10):
- **Screen Studio:**
  - Guides: https://screen.studio/guide/auto-zoom, /guide/cursor, /guide/animations, /guide/shortcuts, /guide/camera, /guide/background, /guide/aspect-ratio
  - Changelog: https://www.screen.studio/changelog (4.0, 2026-10-01)
  - The 2024 animation docs: https://web.archive.org/web/2024/https://screen.studio/guide/animations-motion
  - The vector cursor: https://web.archive.org/web/2024/https://screen.studio/guides/custom-macos-cursor-size
  - Feedback: https://hub.screen.studio/p/custom-settings-for-default-automatic-zoom-range, /p/remove-all-zooms-button, /p/option-to-disable-all-screenstudios-mouse-features-or-fix-mouse, /p/add-back-cursor-animation-physics-customization-as-an-optional-toggle
  - Review: https://daveswift.com/screen-studio/
- **FocuSee:** https://focusee.imobie.com/guide/zoom-in-effect.htm, /guide/cursor-effect.htm, /guide/motion-blur.htm, /guide/keyboard-shortcuts.htm, /guide/camera-layout.htm; review https://daveswift.com/focusee/
- **Rapidemo:** https://getrapidemo.com/
- **Camtasia:** https://www.techsmith.com/learn/tutorials/camtasia/cursor-effects/, /animations/
- **Tella:** https://www.tella.com/blog/how-to-add-auto-zoom-to-screen-recordings, https://www.tella.com/blog/animated-cursors-for-screen-recordings, https://docs.tella.com/docs/api-reference/clips/auto-generate-zooms-from-mouse-activity.md
- **Canvid:** https://www.canvid.com/support/editing/animation
- **Cursorful:** https://cursorful.com/
- **ScreenSnap Pro:** https://www.screensnap.pro/documentation/video-zoom
- **The 1€ filter:** https://cristal.univ-lille.fr/~casiez/1euro

**Open code** (cloned in `/tmp/claude-1000/…/studio/competitors/`):
- getopenscreen/openscreen @3ee6a52 (`src/lib/ai-edition/timeline/zoom-suggestions.ts`)
- webadderallorg/Recordly @535d1cf
- syi0808/screenize @027be96
- tamnguyenvan/screenarc @cefc586
- connerkward/screenstudio-alt-skill @61a0b30
- BlankSourceCode/obs-zoom-to-mouse @53dc1a5

**Linux tools:**
- https://github.com/Itz-Hex/hypr-obs-mouse-follow
- https://github.com/mmlTools/zoominator
- https://github.com/getopenscreen/openscreen/pull/157
- https://github.com/AlynxZhou/showmethekey
- https://github.com/ammgws/wshowkeys
- https://github.com/SeaDve/Kooha
- https://gitlab.gnome.org/GNOME/mutter/-/issues/2341

**Hyprland 0.56.2** (`v0.56.2`, efb5099):
- `src/debug/HyprCtl.cpp:1291-1306` (cursorpos), `:2232-2316` (socket1), `:1967` (locked)
- `src/managers/screenshare/CursorshareSession.cpp` (render)
- `src/pointer/PointerManager.cpp:127-262, 288-457`
- `src/output/Monitor.cpp:2290-2307`
- `src/render/Renderer.cpp:2119-2126, 2849-2906`
- `src/managers/EventManager.cpp:126-192`
- `src/config/lua/LuaEventHandler.cpp:266-303`
- `src/managers/KeybindManager.cpp:446-475`
- The Lua stubs: `/usr/share/hypr/stubs/hl.meta.lua`

**gpu-screen-recorder** (master `7f9405e`; the cursor and clock code is identical in 6.1.0):
- `src/encoder/encoder.c:24-37, 181`
- `src/recorder/recorder.c:622, 668, 813`
- `src/capture/kms.c:1011-1024`
- `kms/server/kms_server.c:356-469`
- `src/capture/portal.c:205`

**xdg-desktop-portal-hyprland 1.4.1:** `src/portals/Screencopy.cpp:110-118, 672, 903-910`

**Measurements for this document** (scratch: `/tmp/claude-1000/…/studio/`):
- `hypr/cursorpos_bench.py`, `hypr/sockbuf.py`: IPC cost, socket2 buffer
- `render/qt/` (the offscreen prototype), `render/ff/` (ffmpeg jitter and throughput harnesses)
- `homothety.py` (the zoom formula)

**Snip:**
- `docs/02-architecture.md` §6.4, §5.3
- `docs/design/recording.md`
- `docs/research/recording-2026-10/` (review.md: C1, gaps 11 and 12)
- `crates/snip-core/src/{take,record,media,store}.rs`
- `crates/snip-hypr/src/{lib,keys}.rs`, `recording.lua`
- `crates/snip-bridge/src/engine/{record,video}.rs`
- `app/qml/{VideoEditor,RecordHud,Editor}.qml`
