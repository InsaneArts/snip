# Product spec: Snip

Status: **DRAFT; product direction decided 2026-10-06**. The open questions left are in §9.

**Decided**
- **Product:** the CleanShot X experience, UX and UI, matched feature for feature (the inventory is our feature list), except Cloud. Every part is Snip's own; nothing depends on Omasnap (D27).
- **Name:** **Snip**.
- **Scope:** Studio Mode is **out of v1**.
- **Stack:** **Rust core plus Qt UI**.
- **Licence:** **GPL-3.0-or-later**.
- **Recording default:** the fast KMS backend, with private-window warnings.
- **Look:** it must be **as beautiful as CleanShot** and **follow Omarchy themes** ([06-design-language.md](06-design-language.md)). It uses a CleanShot-style proportional UI font, and its corners follow Hyprland's rounding, with an override.
- **Capture lifecycle** (§5.0):
  - A capture goes **into Snip's history only**, never into your folders.
  - **Copy** puts it on the clipboard only.
  - **Save** writes it to the default folder **and** puts it on the clipboard.
- **Onboarding** *offers* to take over the PRINT keys; it never does so silently.

Reference material:
- [research/cleanshot-x-feature-inventory.md](research/cleanshot-x-feature-inventory.md): what CleanShot X 5.0.1 does, with sources.
- [research/platform-feasibility-hyprland.md](research/platform-feasibility-hyprland.md): what Hyprland 0.56 and Omarchy 4 allow.

## 1. Summary

Snip is a screenshot and screen-recording tool for Omarchy (Arch Linux + Hyprland). It matches CleanShot X feature for feature and interaction for interaction, except for CleanShot Cloud.

It should feel native to Omarchy:
- It follows the current Omarchy theme.
- It uses Omarchy's keybinding style, notifications, bar and menus.
- It is keyboard-first.

It should also be faster than today's Omarchy capture chain (`hyprpicker`, `slurp`, `grim`, `wl-copy`, `tensaku`). Today each of those runs as its own process and passes PNGs through the disk. Snip replaces that chain with one resident daemon.

> **Name and packaging.**
> - The product is **Snip**, the command is `snip`, and the daemon is `snipd`.
> - The AUR name `snip` is taken by an abandoned snippet manager (v1.2.0, 2023, 0 votes) that probably installs `/usr/bin/snip`. So our package is **`snip-screenshot`** (plus `-git`), with `conflicts=('snip')`.
> - "Mathpix Snip" is an unrelated OCR product with a similar name. We don't use Mathpix branding, and that is the only care needed.
> - The repository folder is still named `OpenShotX`; rename it whenever convenient.

### 1.1 Competitive context: Omasnap is Omarchy's new default

On 2026-09-21 Omarchy merged [omacom/omarchy#9446 "Use Omasnap for screenshots"](https://github.com/omacom/omarchy/pull/9446) into its next release branch (`quattro`). [Omasnap](https://github.com/omacom/omasnap):
- is MIT, C++/Qt6 Widgets plus LayerShellQt, at v1.21.0 with 562 stars;
- is opinionated and Hyprland-only;
- already ships smart selection, a floating preview with drag-out, pins, a recents shelf of 5, editable vector annotations, secure redaction, OCR, scrolling capture with auto-scroll, backdrops, and crash-safe working documents.

By design it has **no** screen recording, **no** settings UI and **no** history browser. Its multi-monitor support is weak (issues #164, #166). Every Omarchy user will soon have it bound to PRINT.

**Decision: Omasnap is the competitor to beat, and Snip shares no code with it (D27).** Omasnap is opinionated and minimal by design (no settings, history, recording, All-In-One or After Capture rules; weak multi-monitor). Snip wins with:
  - CleanShot's UI and interaction model: QAO stacking with hover actions and drag-out, capture history, the All-In-One bar, Space-to-window, self-timer, hide desktop clutter
  - the full annotate tool set and Background tool, **with Omarchy-theme backgrounds**
  - pins with lock mode
  - settings and After Capture rules
  - multi-monitor support
  - then the recording suite.

Other Omarchy and Hyprland projects in this space, as of 2026-10:

| Project | What it is |
|---|---|
| **Tensaku** | Rust/GTK4, MPL-2.0. The current Omarchy editor, about to be replaced by Omasnap. |
| **OmniShot** ([joshdaws/omnishot](https://github.com/joshdaws/omnishot)) | Python, MIT, v0.2. Explicitly CleanShot-inspired and aimed at Omarchy 4.0.2. Has scrolling capture, OCR/QR, H.264 and GIF recording, and a timeline. |
| **ChalKak** | AUR. A Hyprland editor with PaddleOCR v5. |
| **[FifthRooter/OpenShotX](https://github.com/FifthRooter/OpenShotX)** | Rust, also "inspired by CleanShot X" (57 stars). Its name is why we dropped the "OpenShotX" working name. |

None of them combines a resident daemon, the full CleanShot recording suite, multi-monitor support, and a settings and history UI.

## 2. Goals and non-goals

### Goals
1. **Parity with CleanShot X 5.0** in every area below except Cloud. Studio Mode comes after v1. Where we cannot match CleanShot exactly because of Wayland or Linux, §6 records the closest equivalent we will build instead.
2. **As beautiful as CleanShot, in Omarchy's theme.** CleanShot's layout, motion and polish are rendered in the active Omarchy theme's colours, borders, corners and materials ([06-design-language.md](06-design-language.md)).
3. **Instant feel.** The hotkey-to-frozen-overlay time is under 35 ms with a warm daemon (about 2–5 refreshes at 144 Hz, mostly the compositor's frame copy), a capture is stored in history within 30 ms of mouse release, and Copy is effectively instant (≤ 5 ms). [03-performance.md](03-performance.md) defines the budgets.
4. **Omarchy-native.** Colours and fonts come from the active Omarchy theme and change with it. Keybinds are Lua snippets in Omarchy's style. Status appears in the Omarchy bar, and notifications and do-not-disturb go through omarchy-shell.
5. **Safe by default.** Redaction cannot be reversed. Private windows stay private. The app takes no permanent elevated permissions, and it sends no data over the network.
6. **Automation.** A CLI and IPC surface covers everything CleanShot's `cleanshot://` URL API does, plus more.

### Non-goals
- CleanShot Cloud: uploads, share links, cloud history, teams, "Upload & copy link".
- Raycast AI Chat integration.
- macOS-only features: notch auto-crop, TopNotch and PixelSnap integrations, Quick Look extensions, Share-sheet extensions, licence management.
- X11 sessions.
- Compositors other than Hyprland **in v1**. The capture layer gets a portability seam (§3 of the architecture doc), but v1 is tested only on Hyprland ≥ 0.54.
- Sending data over the network: no telemetry, update checks or uploads. Updates come through pacman/AUR.

## 3. Users and core jobs

| User | Job | What success looks like |
|---|---|---|
| Developer filing a bug or PR | Capture a region, mark it up, paste it into GitHub, Slack or Linear | Hotkey, drag, Ctrl+V in the browser, under 3 seconds |
| Technical writer | Consistent window shots with background and shadow; numbered steps | A background preset applied automatically; counters; a reusable project file |
| Content creator | Record a demo with mic, webcam, keystrokes and clicks; trim; export MP4 or GIF | Start recording in 2 keystrokes; no dropped frames at 60 fps; clean audio from the first frame |
| Anyone | Grab text from an image or video frame | Text on the clipboard with the layout kept |
| Anyone | Keep a reference visible while working | Pin a capture above all windows and make it click-through |

## 4. Product principles

These are taken from CleanShot's UX (inventory §11) and adapted to Omarchy:

1. **One gesture to a result.** A hotkey, a drag and a release give a capture, with no dialogs. **Every capture is kept in Snip's history. Nothing lands in your folders or on your clipboard until you Copy or Save** (or turn on an After Capture rule that does).
2. **Defer the decision.** The Quick Access Overlay (QAO) holds the result without stealing focus, so the user picks copy, save, edit, pin or drag later.
3. **Everything drags.** QAO cards, the editor's drag handle, pins and history items all drag into other apps as real files.
4. **Three paths to every action:** mouse, keyboard (single letters in overlays, Ctrl-chords elsewhere) and drag. Tooltips show the keys.
5. **Modifiers invert defaults instead of adding modes.** CleanShot's Option key maps to **Alt** on Linux. CleanShot's Cmd key maps to **Ctrl** in app contexts; Super stays reserved for Hyprland.
6. **Remember everything.** Last area, ratio, tool, colour, arrow style, text size, save folder and recording options.
7. **Forgiveness.** Undo and redo everywhere, a capture history, re-editable project files, recordings recovered after a crash, and a confirmation before anything is lost.
8. **Stay out of the shot.** Our own UI never appears in a capture or recording unless it is meant to: keystrokes, click highlights and webcam are meant to be recorded.
9. **Precision on demand.** Crosshair, magnifier, typed sizes, ratio lock, arrow-key nudging and snapping are all available, and none are forced.
10. **Omarchy first.** Keyboard-driven, themed, scriptable, and configured with files as well as a settings UI.

## 5. Feature catalogue

Priorities (re-ranked 2026-10-06 to match the features named as most important):
- **P0: the CleanShot screenshot experience.** Built first (M1–M3) and polished to CleanShot's level:
  - capture area, window (Space then click), fullscreen, previous area
  - self-timer, scrolling capture, OCR
  - hide desktop clutter
  - Quick Access Overlay with drag-and-drop
  - capture history
  - annotation editor (text, arrows, shapes and the rest)
  - Background tool with Omarchy-theme backgrounds and padding
  - pin to screen
- **P1: the rest of 1.0 = full CleanShot parity** (M4): the recording suite, All-In-One, and the remaining parity items.
- **P2: after 1.0:** Studio Mode (out of v1, decided) and extras.

The IDs are referenced by the roadmap and will become issues.

### 5.0 Capture lifecycle (decided 2026-10-06)

This rule applies to every capture type: screenshot, scroll capture, recording, GIF.

| Moment | What happens | Where the bytes live |
|---|---|---|
| **Capture** (release, timer fires, recording stops) | Added to **Snip's capture history**, and the QAO card appears | `~/.local/share/snip/history/<id>/`, Snip's private store with retention rules (HIS-02). Not your Pictures or Videos folder, **not the clipboard**. |
| **Copy** (QAO, editor, pin, history, `Ctrl+C`) | Put on the clipboard: `image/png` for images, a file reference for videos | Clipboard only. Nothing is written to your folders. |
| **Save** (QAO, editor, history, `Ctrl+S`) | Written to the **default export folder** (SET-05) with the name template, **and** put on the clipboard | The export folder plus the clipboard. Afterwards the clipboard and drags refer to the saved file. |
| **Save As** | Asks for location and format; also copies | The chosen path plus the clipboard |
| **Drag out** | Exports a file to Snip's drag cache (not your folders) and offers it to the drop target | `~/.cache/snip/drag/` |
| **Close card / auto-close** | The card goes away; the capture **stays in history** | Restore it from history or "Restore last capture" |

What follows from this:
- **Default After Capture rules (SET-03):** Screenshot: *Show QAO* ✓, Copy ✗, Save ✗, Annotate ✗, Pin ✗. Recording: *Show QAO* ✓, all others ✗. Users who want CleanShot's auto-copy or auto-save can turn those rules on.
- **Closing a card never loses work**, so the CleanShot warning "Your media will be lost" appears only when history is set to *Never*.
- **Clipboard-history privacy is solved by default.** Nothing reaches Omarchy's clipboard history until the user copies or saves on purpose, usually after redacting.
- Recordings are written into the history store while recording. Save copies them to `$XDG_VIDEOS_DIR` and never moves them, so history keeps a playable copy until retention removes it.

### 5.1 Capture modes (CAP)

| ID | Feature | Pri | Behaviour on Snip |
|---|---|---|---|
| CAP-01 | Capture Area | P0 | Freeze, select, and capture on release (no adjust step, like CleanShot). **The whole display is free space**: no window highlight and no dimming until a drag starts, and a click without dragging does nothing (decided 2026-10-07, D23). |
| CAP-02 | Capture Area & {Copy, Save, Annotate, Pin, OCR} | P0 | Separate bindable actions that follow §5.0: *& Copy* → clipboard only; *& Save* → export folder plus clipboard. They skip the After Capture rules when "ignore After Capture for variants" is on (CleanShot setting). |
| CAP-03 | Capture Previous Area | P0 | Re-captures the last rectangle with no UI. It is stored in physical pixels and remembers which monitor it was on. If that monitor is gone, it falls back to the focused one with a toast. |
| CAP-04 | Capture Fullscreen | P0 | Captures the monitor under the cursor. A separate "all monitors" variant composites every monitor at the largest scale. |
| CAP-05 | Capture Window | P0 | **Holding Space** in the area overlay picks windows, with a hover highlight; click captures, releasing Space returns to area (D23). Space held *while dragging* moves the selection (SEL-07). `snip window` opens directly in window mode. The capture comes from the window's own buffer through the ext toplevel source, so it is never occluded and works for windows on other workspaces when they are picked from the window list. Options: **With wallpaper** (padding) or **Transparent** (alpha). Shift inverts the choice. Corner mask and shadow are drawn by us (platform doc §11). **With the desktop** (D37, `capture.window_desktop`): the window as it is on screen plus 48 px of what surrounds it (wallpaper, other windows, the bar), opaque, instead of the clean window; Shift when clicking gives the other look. |
| CAP-06 | Self-Timer | P0 | Area or fullscreen after N seconds, with a countdown overlay drawn on a no-input layer that is **not** in the frame. Interval 2–15 s, default 5. Includes the cursor if "show cursor" is on. |
| CAP-07 | All-In-One | P1 | One hotkey opens a floating mode bar: Area (A), Fullscreen (F), Window (Space/W), Scrolling (S), Timer (T), Text (O), Record (R). The selection stays adjustable, with a W×H field, an aspect-ratio menu, and Ctrl+C to copy at once. It remembers the last selection. |
| CAP-08 | Scrolling Capture | P0 | See §5.8. |
| CAP-09 | Capture Text (OCR) | P0 | See §5.7. |
| CAP-10 | Freeze screen | P0 | **On by default** (CleanShot defaults it off). On Wayland the freeze is free, it is how we guarantee our overlay never lands in the capture, and Omarchy users already expect it. It can be turned off in settings: live selection then uses a re-capture after the overlay unmaps, which is slower. |
| CAP-11 | Cursor in captures | P0 | Off for area, window and OCR. Optional for fullscreen and self-timer, as in CleanShot. When it is on, the cursor is drawn from the cursor-session image at the recorded position, which works even with a hardware cursor. |
| CAP-12 | **Hide desktop clutter** (CleanShot's "Hide desktop icons", reinterpreted) | P0 | Omarchy has no desktop icons; its clutter is the bar, notifications, our own pins and cards, and any desktop widgets. We use **CleanShot's own technique**: cover the clutter with the wallpaper, without removing it. (1) An overlay-layer surface covers the bar's area with the matching slice of the current Omarchy wallpaper, scaled exactly as omarchy-shell draws it (`PreserveAspectCrop`). The bar keeps its reserved space, so **tiled windows do not re-layout**. (2) Notifications go to DND. (3) Pins and QAO cards are set to opacity 0. It applies to fullscreen captures, "With wallpaper" window shots and recordings, through a setting ("while capturing") plus a toggle hotkey and CLI (`snip desktop hide|show|toggle`). **Verified in spike S03: visually seamless (PSNR ≈ 46 dB vs the real wallpaper, mipmapped), not bit-exact**, because omarchy-shell renders the wallpaper through its own pipeline. Screenshots apply the cover to the frozen frame; recordings use the live overlay surface. Never toggle the bar to hide it (Omarchy's toggle can strand the bar off-screen). |

### 5.2 Selection overlay (SEL)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| SEL-01 | Per-monitor frozen overlay that appears instantly | P0 | p95 under 35 ms from hotkey to first presented frame when warm; under 300 ms cold. No fade animation (a layer rule ships with the app). |
| SEL-02 | Dim outside, clear inside, 1-physical-pixel accent border | P0 | Uses the theme accent colour. |
| SEL-03 | Dimensions pill (W × H in **physical** px; logical px on hover or in settings) | P0 | Shows ratio when a ratio is locked. |
| SEL-04 | Crosshair mode: Off / Always / While Ctrl is held | P0 | Coordinate readout next to the cursor. |
| SEL-05 | Magnifier loupe with pixel grid and hex colour of the centre pixel | P0 | Needs crosshair (CleanShot rule). Pressing **C** copies the hex value (our addition, which covers part of hyprpicker's job). |
| SEL-06 | Window hover highlight (while Space is held, or in `snip window`) | P0 | Smallest rectangle wins. Tab and Shift+Tab, and arrow keys, pick windows explicitly in any mode; Enter captures the picked window (or the monitor when none is picked). |
| SEL-07 | Modifiers while dragging, CleanShot's (D32): Alt = square, Ctrl = resize from centre, Space (held) = move the selection; Shift held at the release skips an auto-applied Background preset | P0 | |
| SEL-08 | Adjustable selection (All-In-One and recording) | P1 | 8 handles, arrow-key nudge (1 px; Shift = 10 px; Ctrl = resize), snap to window and monitor edges, typed W×H, ratio presets (Free, 1:1, 4:3, 3:2, 16:9, 5:4, 9:16, custom). |
| SEL-09 | Esc cancels; Enter confirms (or records the full screen in the recording overlay) | P0 | Pressing the capture hotkey again while the overlay is up cancels it, or cycles mode (decision Q6). |
| SEL-10 | Multi-monitor | P0 | A selection stays on one monitor. Each monitor gets its own overlay. A selection never crosses monitors (Wayland capture and encoders require one monitor; CleanShot behaves the same way). |
| SEL-11 | Exact pixel mapping at fractional scales | P0 | The selection is held in physical pixels. Guarantee: what you see inside the border is exactly what you get (platform doc §2). |

### 5.3 Quick Access Overlay (QAO)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| QAO-01 | Thumbnail card in the bottom-left or bottom-right corner, size slider, on the monitor under the cursor ("move to active screen") | P0 | Slides in quickly; never takes keyboard focus on appear. |
| QAO-02 | Hover actions: Copy, Save (centre pills), Close, Pin, Annotate or Edit video (corners) | P0 | No Upload. Copy and Save follow §5.0 (Save = folder plus clipboard). After a Save the card shows a "Saved" state with **Show in Files**. |
| QAO-03 | Keyboard while hovered: Ctrl+C, Ctrl+S, Ctrl+W, Ctrl+E, Space (preview) | P0 | Needs on-demand keyboard focus while hovered. Alt modifies Copy (keep the card), Save (ask for location) and Drag (keep the card). |
| QAO-04 | Drag the card out as a file (Copy action only) | P0 | Offers `text/uri-list` plus the portal file-transfer type for Flatpak apps, and no raw `image/png` (Nautilus would save it nameless). Verified in 7 apps in spike S02. The file lives in a persistent cache until it is saved or cleaned up. "Close after dragging" needs a reliable drop signal (S02 open item S2-1). |
| QAO-05 | Double-click opens the editor (images) or the trimmer (videos) | P0 | |
| QAO-06 | Stacking: newest on top, newest-capture indicator, up to N visible (default 5, rest collapsed into "+k") | P0 | Save All and Close All. Close All needs no confirmation, because everything stays in history. It asks only when history is set to *Never*. |
| QAO-07 | Auto-close after an interval, with action **Close** (default; the capture stays in history) or **Save and Close** | P0 | Hovering pauses the timer. |
| QAO-08 | Swipe right or left to dismiss; swipe down to hide all; a hotkey brings them back | P0 | Touchpad scroll gestures on the surface. Ctrl+W and the close button work everywhere. |
| QAO-09 | Context menu: Annotate, Pin, Rotate, Flip, Scale to 1×, Resize…, Extract text, Preview, Show in Files, Open With ▸, Save As…, Print…, Move to Trash, Hide, Close, Close All, Save All | P0 | "Show in Files" uses `org.freedesktop.FileManager1`. "Open With" uses the MIME database. |
| QAO-10 | Restore the last closed capture (hotkey) | P0 | Backed by history. |
| QAO-11 | Edits made in the editor update the card | P0 | |

### 5.4 Annotate editor (ANN)

A normal floating window (Hyprland rule: float, centred, with optional pin / always-on-top). The full tool list and keys follow inventory §4.2, with Cmd mapped to Ctrl.

| ID | Feature | Pri |
|---|---|---|
| ANN-01 | Canvas with zoom (fit, 50/100/200 %, Ctrl+±, Ctrl+1), Space-drag to pan, scroll to pan, checkerboard under transparency, HiDPI-correct rendering | P0 |
| ANN-02 | Select/Move (V): handles, Shift locks an axis, Alt-drag duplicates, Ctrl+C/V copies objects (also between images), Ctrl+D duplicates, Delete removes | P0 |
| ANN-03 | Rectangle (R), Filled rectangle (F), Ellipse (E), Line (L); Shift for perfect shapes; optional object shadow; corner roundness | P0 |
| ANN-04 | Arrow (A): Standard, Fancy, Curved (with a control point), Double; Alt inverts direction while drawing | P0 |
| ANN-05 | Text (T): 7 styles (Standard, Box, Rounded, Rounded Box, Outline, Mono, Mono Box), size, resize from the corner, wraps into a paragraph, emoji picker, **IME input that works** | P0 |
| ANN-06 | **Pixelate / Blur tool (P)**, CleanShot's: drag over an area and it is pixelated or blurred to hide what is there. Styles: **Pixelate** (default, as in CleanShot), **Blur**, **Black out** (solid). Pixelate and Blur *look* like CleanShot's but are **secure by construction**: their pixels come from outside the region or from seeded random tiles, so nothing of the hidden content survives (pixelated text can otherwise be recovered: Hill et al. 2016, Depix). A "Natural blur (not secure)" style for softening backgrounds is labelled as such. Intensity slider and keyboard; the area stays movable and resizable while editing; flattened on export. Research: processing doc §4. Decided 2026-10-07 (D26). | P0 |
| ANN-06b | ~~Redaction suggestions~~ (automatic detection of emails, card numbers, keys): **not planned** (D26) | — |
| ANN-07 | Spotlight (H): rounded rectangle or ellipse, adjustable dim | P0 |
| ANN-08 | Counter (C): 1,2,3 / A,B,C / I,II,III, start value, renumbered on delete, always on top | P0 |
| ANN-09 | Pencil (D) with smoothing | P0 |
| ANN-10 | Highlighter (M) with **smart text-line snapping** (uses OCR line boxes) | P0 |
| ANN-11 | Crop and Resize (K): ratio presets, snap to edges, outward crop grows the canvas and fills it with the detected background colour, rotate and flip, resize in px or % with "flatten" warning | P0 |
| ANN-12 | Add image: drop a file, paste, or "take a new screenshot". Drop zones around the canvas (above, below, left, right) plus snapping guides; the canvas grows to fit | P0 |
| ANN-19 | **Edit any image**: open a file or **the image on the clipboard** in the editor (`snip edit FILE`, `snip edit --clipboard`, a bindable action), and act as Omarchy's screenshot editor (OMA-06). The most-voted Linux request (flameshot#240) | P0 |
| ANN-13 | Colour picker: 10 swatches, 10 saved colours, HSV and alpha, hex, **eyedropper that picks from anywhere on screen** (uses our freeze overlay) | P0 |
| ANN-14 | Undo/redo for every operation (Ctrl+Z, Ctrl+Shift+Z) | P0 |
| ANN-15 | Output: **Ctrl+S Save** = export folder plus clipboard (§5.0); Ctrl+Shift+S Save As (PNG, JPG, WebP, **project file**; also copies); **Ctrl+Shift+C Copy** = clipboard only, then closes (Alt keeps the editor open); "Drag me" handle; Pin; Print. Edits are written back to the history entry automatically. | P0 |
| ANN-16 | **Project format** (re-editable, non-destructive): see the architecture doc. Opening a capture from the QAO or history keeps its objects editable. | P0 |
| ANN-17 | Auto-expand canvas when drawing outside it (setting) | P0 |
| ANN-18 | Settings: inverse arrow, smooth drawing, object shadows, remember background tool, always on top | P0 |

### 5.5 Background tool (BG)

| ID | Feature | Pri |
|---|---|---|
| BG-01 | Sources: built-in gradients (we design our own set of 20, not CleanShot's), current Omarchy wallpaper, custom images, blurred wallpaper or screenshot, plain colour, transparent | P0 |
| BG-06 | **Omarchy theme backgrounds**, our signature addition. A "Themes" section builds backgrounds from every installed Omarchy theme (system `/usr/share/omarchy/themes/*` and user `~/.config/omarchy/themes/*`). For each theme: (a) its **signature gradient** from `hyprland_active_border` (for example `e20342 → c8e967` in Aetheria); (b) gradients from palette pairs (accent with background, accent with color4/5/6…); (c) solid `background` and `accent`; (d) its wallpapers (`backgrounds/`). A "**Follow current theme**" choice (also in presets) re-resolves the colours when the theme changes, so an auto-applied preset always matches the desktop. Theme previews (`preview.png`) label each group. | P0 |
| BG-02 | Padding, inset (with colour), auto-balance, shadow, corner radius, aspect ratio (Auto, 1:1, 4:3, 3:2, 16:9, 5:4, 9:16, portrait), 9-point alignment | P0 |
| BG-03 | Named presets; "apply automatically to every screenshot"; Shift held when the selection ends (or a window is clicked) skips the preset (D32) | P0 |
| BG-04 | Window shots made with "With wallpaper" stay editable (background can be swapped or removed later) | P0 |
| BG-05 | Precise numeric input: hover shows the value, click to type, ↑/↓ to adjust, Tab to commit | P0 |

### 5.6 Pin to screen (PIN)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| PIN-01 | Pin from the QAO, the editor, history, the Capture Area & Pin hotkey, "Pin last", or "Pin file…" | P0 | |
| PIN-02 | Always on top, on every workspace of its monitor, drawn **above fullscreen windows** | P0 | Floating, pinned toplevel with an xdg tag rule (platform doc §7). |
| PIN-03 | Hover chrome: close, lock, drag-out handle | P0 | |
| PIN-04 | Move by dragging and with arrow keys; resize with Ctrl+scroll and from the corners; opacity with scroll; middle-click closes | P0 | |
| PIN-05 | Lock: click-through and hide on hover | P0 | **Spike S04:** Hyprland 0.56 ignores empty input regions on xdg toplevels, but honours them on layer surfaces. So a locked pin becomes a layer surface at the same position with an empty input region, and Chromium underneath was verified to receive the clicks. Unlock through a hotkey, the bar widget or the CLI, since a locked pin gets no input (as in CleanShot). |
| PIN-06 | Context menu: Copy, Save, OCR, Annotate, Close, Close All | P0 | |
| PIN-07 | Global hotkeys: Toggle all pins, Close all pins | P0 | |
| PIN-08 | Look: rounded corners, shadow, border (settings) | P0 | |

### 5.7 Text recognition (OCR)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| OCR-01 | Select a region and copy the recognised text; toast "Text copied" or "No text detected" | P0 | On-device only. |
| OCR-02 | Keep line breaks: setting plus separate hotkeys (with or without line breaks) | P0 | |
| OCR-03 | QR and barcode detection: show the decoded value; "Open link?" with an Always/Never preference | P0 | 2D codes by default; 1D codes opt-in (they false-positive on dense UI). Values are copied as **sensitive**, because OTP URIs are secrets and Omarchy's clipboard history skips sensitive entries. |
| OCR-04 | Languages: automatic, or a pinned list. **Georgian, Cyrillic and CJK supported** | P0 | **Spike S07:** Tesseract + `tessdata_fast` + upscale/invert + TSV clean-up gives 0 % CER on Georgian, 2.5 % on Russian, 0–2 % on UI text, 6.6 % on mixed Georgian/English, at about 50–65 ms per region. CJK still needs testing. |
| OCR-05 | Link detection in the result (setting) | P0 | |
| OCR-06 | OCR from a file, the clipboard, a pin, a QAO card or a video frame | P0 | |

### 5.8 Scrolling capture (SCR)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| SCR-01 | Select the scrollable region, press Start, scroll by hand; a live stitched preview sits beside the region | P0 | The preview and HUD take no input and are never inside the region. |
| SCR-02 | Auto-scroll (vertical and horizontal): we generate the scroll events; moving the cursor out pauses and moving it back resumes; it stops at the end of the content | P0 | Uses the wlr virtual pointer. Esc and Enter come through temporary binds. |
| SCR-03 | Detects sticky headers and footers and stitches them only once | P0 | Snip's own stitcher (D27). The bar it must meet was set in spike S06: a 400-row barcode page stitched with 0 missing or duplicated rows; header and footer exactly once. Capture from the window's own buffer (Omarchy's window translucency otherwise leaks the wallpaper in and makes stitching 7× slower). |
| SCR-04 | Warnings: "slow down", "very large" (more than 30 000 px tall) | P0 | |
| SCR-05 | Onboarding card on first use (what to select, what breaks stitching) | P0 | |

### 5.9 Screen recording (REC)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| REC-01 | Area, monitor or window recording. The selection overlay supports Enter for full screen, Space for window, typed size, ratio lock and remembered area | P1 | Window recording means "record the window's rectangle" in P0; true per-window capture comes later (P2). |
| REC-02 | Recorder toolbar before start: Video / GIF, microphone picker, system audio toggle, camera picker, highlight clicks, show keystrokes, settings | P1 | |
| REC-03 | Optional 3-2-1 countdown | P1 | |
| REC-04 | Dim outside the region, or show a border only | P1 | The dim and border sit **outside** the encoded rectangle with a 2 px safety margin. |
| REC-05 | Controls pill: stop and timer, pause/resume, restart, discard, audio level | P1 | Placed **outside the region**. For full-monitor recordings, the controls sit on another monitor or are hidden, and the bar indicator plus hotkeys are used instead (the controls cannot be hidden from the recording, see the platform doc §3). |
| REC-06 | Bar indicator with elapsed time; click to stop | P1 | Omarchy already has a recording indicator, so we either integrate with it or ship a bar widget plugin (decision Q4). |
| REC-07 | Microphone and system audio: one mixed track, or separate tracks; mono option; no pop at the start | P1 | |
| REC-08 | Webcam overlay: circle, square or rectangle; size; drag to a corner; mirror; click to toggle full-screen camera | P1 | It is an on-screen window, so it is recorded as seen. |
| REC-09 | Highlight clicks: colour (theme accent), size, outline or filled, animated | P1 | Uses the Hyprland Lua input hook (platform doc §10). |
| REC-10 | Show keystrokes: size, dark or light, 6 positions, all keys or modifier combos only, held modifiers stay visible, repeat counter | P1 | Same hook. Muted while a password-manager window is focused or the session is locked. |
| REC-11 | Do not disturb while recording (restore the previous state afterwards) | P1 | omarchy-shell `setDnd`. |
| REC-12 | MP4 (H.264; HEVC or AV1 optional), fps 24/30/60/native (≤ 144), max resolution, scale to 1× | P1 | |
| REC-13 | GIF: fps, width, quality, optimise | P1 | |
| REC-14 | Safety: low-disk auto-stop, recover after a crash, warnings for muted or missing mic, Bluetooth HFP warning | P1 | |
| REC-15 | **Privacy-safe mode**: windows Omarchy marks private (1Password, Bitwarden) are blacked out | P1 | The default fast backend (KMS) cannot do this. See decision Q5. |

### 5.10 Video editor (VED)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| VED-01 | Trimmer: in and out handles, thumbnail strip, frame stepping, play/pause on Space | P1 | |
| VED-02 | Export: quality, resolution, mute or keep audio, estimated size, Replace or Save as new, export as GIF | P1 | |
| VED-03 | Cuts: split at playhead (B / Ctrl+B), delete segments | P1 | |
| VED-04 | **Studio Mode**: record the screen without the cursor, plus event logs (cursor path, clicks, keys) and the camera as a separate stream, saved as a project | P2 | **Out of v1 (decided 2026-10-06).** The v1 recording design keeps the door open (architecture §6.4). |
| VED-05 | Studio editor: auto zooms and manual zooms (follow-cursor or fixed), smooth cursor, cursor size and style, click ripple, keystrokes after the fact, camera layout, background and aspect ratio, motion blur, hardware export | P2 | |

### 5.11 Capture history (HIS)

| ID | Feature | Pri |
|---|---|---|
| HIS-01 | A strip across the top of the screen with large thumbnails; filters (All, Screenshots, Videos, GIFs, Projects); arrow keys, Space to preview, Enter restores to the QAO with objects still editable | P0 |
| HIS-02 | Retention: 1 day, 3 days, **1 week (default)**, 1 month, Forever, or Never (Never turns history off and brings back the close confirmations). Settings show how much space history uses and offer "Clear history". | P0 |
| HIS-03 | Delete with Backspace (optional confirmation); Clear all | P0 |
| HIS-04 | History is **the default home of every capture** (§5.0). It stores originals plus project data (never just the flattened export) in `~/.local/share/snip/history/`. Unsaved captures are not in the user's folders. | P0 |
| HIS-05 | **Search history by the text in captures** (and window title / app): every capture is read in the background (OCR + QR, lowest CPU priority, local only) into a full-text index; `snip search TEXT` now, the history UI search box in M3. Setting `ocr.index_history` | P0 |

### 5.12 Settings, configuration and automation (SET, API)

| ID | Feature | Pri | Notes |
|---|---|---|---|
| SET-01 | Config file `~/.config/snip/config.toml` as the source of truth, reloaded live | P0 | Omarchy convention: config lives in files and the user can edit it. |
| SET-02 | Settings window with tabs: General, Shortcuts, Quick Access, Screenshots, Recording, Annotate, Advanced, About | P0 | It writes the same TOML file. |
| SET-03 | After Capture rules (Screenshot column, Recording column): show QAO, copy, save, annotate, pin, open video editor. **Defaults follow §5.0: only "Show QAO" is on.** At least one rule must stay on. A setting "Save also copies to clipboard" (default **on**) controls the decided Save behaviour. | P0 |
| SET-04 | File name template with tokens (`%y %m %n %d %w %H %M %S %p %t %a %r %i`), illegal-character stripping, UTC option, `@2x` suffix option | P0 | `%t` is the window title and `%a` the app class, both from Hyprland. |
| SET-05 | Export folder (default `$XDG_PICTURES_DIR/Screenshots`, `$XDG_VIDEOS_DIR/Screencasts`) | P0 | |
| SET-06 | Formats: PNG (default), JPG, WebP; optional lossless JPEG XL | P0 | |
| SET-07 | Clipboard mode: Image (default) / File / Image + File; plus **the file path as text** (default off since D38; it was on, D24) | P0 | Copy offers `image/png` and the file's path as `text/plain`: image-aware apps paste the image, text-only targets (terminals, plain fields) paste the path instead of nothing (D24). |
| SET-07b | "Keep captures out of clipboard history" | P1 | Mostly not needed, because nothing is copied automatically (§5.0). Kept as an opt-in for users who turn on auto-copy: copies are then offered with `x-kde-passwordManagerHint`, which Omarchy's clipboard history skips. Default **off**. |
| SET-08 | Shortcuts: a recorder that writes Hyprland Lua binds, detects conflicts with Omarchy's binds, and restores defaults | P1 | Wayland apps cannot grab keys, so shortcuts are written to Hyprland config (architecture doc). |
| SET-09 | Sounds: shutter and recording start, stop, pause (bundled, themable, can be turned off) | P0 | |
| API-01 | `snip <command> [args]` CLI covering the full CleanShot URL API (`capture-area --x --y --w --h --monitor --action`, `pin --file`, `record-screen`, `capture-text --file`, `open-annotate`, `add-qao --file`, `open-history`, `restore-recent`, `open-settings --tab`…) | P0 | |
| API-02 | `snip://` URL scheme handler, with consent on first use from each caller | P2 | Low value on Linux; the CLI covers it. |
| API-03 | JSON output (`--json`) and a stable exit-code contract for scripting | P0 | |

### 5.13 Omarchy integration (OMA)

| ID | Feature | Pri |
|---|---|---|
| OMA-01 | Follows the theme: reads `~/.local/state/omarchy/current/theme/colors.toml` and the font; changes live on theme switch | P0 |
| OMA-02 | Keybinding module `require("snip")` for `hyprland.lua`, offered during onboarding, which replaces Omarchy's PRINT bindings | P0 |
| OMA-03 | Entries in the Omarchy capture menu (`SUPER+CTRL+C`) | P1 |
| OMA-04 | Bar widget: recording timer and stop button; optional capture button | P1 |
| OMA-05 | Notifications through omarchy-shell (with image and click action) as a fallback when the QAO is off | P0 |
| OMA-06 | Works as `OMARCHY_SCREENSHOT_EDITOR`, so Omarchy's own flow can open our editor | P0 |
| OMA-07 | `.desktop` handlers: "Annotate with Snip", "Pin with Snip" for images | P1 |
| OMA-08 | **Bar icon** (D35): a tray icon (StatusNotifierItem) with Snip's logo, drawn in the bar's colours. A click opens CleanShot's menu, in CleanShot's order (its MainMenu.nib): All-In-One, Capture Area, Capture Previous Area, Capture Fullscreen, Capture Window, Self-Timer, Scrolling Capture, Capture Text (each with its key); Open…, Open from Clipboard, Pin to the Screen…, Capture History…, Restore Recently Closed; Settings…, About Snip, Quit Snip. Record Screen, Hide Desktop Icons and Check for Updates join when Snip has them | P0 |
| OMA-09 | **In the launcher**: "Snip" opens Settings, starting snipd if it isn't running. Its actions start captures and open History | P0 |

## 6. Linux reinterpretations: where we differ from CleanShot on purpose

| CleanShot (macOS) | Snip (Omarchy) | Why |
|---|---|---|
| Cmd / Option keys | Ctrl / Alt | Super belongs to Hyprland |
| System-wide hotkeys set inside the app | Hyprland Lua binds generated by us, dispatched to the daemon with **no process spawn** (`hl.dsp.global`) | Wayland apps cannot grab keys |
| Freeze screen off by default | On by default | Free on Wayland, and keeps our UI out of captures |
| Click (no drag) = nothing | Click = window or monitor under the cursor | Omarchy users' muscle memory |
| Controls pill can sit inside the recording area (excluded by macOS) | Controls are always placed **outside** the region | Hyprland can only paint hidden surfaces black, and KMS capture can't hide them at all |
| Copy = "File & Image" | Copy = image/png; "Copy as file" is a separate action | Electron and terminal apps paste the path instead of the image when text types are offered |
| Hide desktop icons (a wallpaper window covers the icons) | Hide desktop clutter: a wallpaper slice covers the bar, plus DND, plus our pins and cards hidden (CAP-12) | No desktop icons on Omarchy. Covering instead of hiding avoids re-layout, the same trick CleanShot uses. |
| Built-in gradient backgrounds | Our gradients **plus every installed Omarchy theme's colours and wallpapers** (BG-06) | Screenshots match the user's desktop |
| Retina @2x | Physical pixels by default; "scale to 1×" and an `@{scale}x` suffix when scale ≠ 1 | Fractional scales (1.25, 1.5, 1.6̅) are common |
| Quick Look (Space) | Built-in preview window | No system Quick Look |
| Share sheet, Mail, Raycast AI | Open With ▸, Show in Files | |
| Do Not Disturb | omarchy-shell `setDnd` (state saved and restored) | |
| Keystroke and click display through Accessibility permission | Hyprland Lua input hook, installed only while recording | No global input on Wayland; avoids the `input` group and root |
| "Launch at login" | Hyprland autostart or a systemd user unit | |

## 7. Default keybindings

**Decided 2026-10-06: macOS muscle memory.** CleanShot users press Cmd+Shift+3 and Cmd+Shift+4, and Snip keeps those on Super.

| Keys | Action | Notes |
|---|---|---|
| `SUPER+SHIFT+3` | **Capture Fullscreen** (monitor with focus) | Replaces Omarchy's *Move window to workspace 3* (`code:12`) |
| `SUPER+SHIFT+4` | **Capture Area** (click = window; Space = window mode) | Replaces Omarchy's *Move window to workspace 4* (`code:13`) |
| `PRINT` | Capture Area | Omarchy's habit; offered during onboarding (Q3) |
| `ALT+PRINT` | Record screen (pressed again: stop) | Omarchy's habit |
| `SUPER+CTRL+PRINT` | Capture Text | Omarchy's habit |
| `SUPER+PRINT` | Unchanged (hyprpicker colour picker) | |
| `SUPER+SHIFT+5` | **All-In-One** (decided 2026-10-06; macOS Cmd+Shift+5) | Replaces Omarchy's *Move window to workspace 5* (`code:14`) |
| unbound by default | scrolling, previous area, self-timer, pin last, history, restore last, toggle pins, hide or show overlays | |

**Conflict handling (onboarding, Q3 "offer"):**
- Omarchy binds `SUPER+SHIFT+1…0` to *Move window to workspace N*. Taking 3, 4 and 5 removes three of those.
- Onboarding shows this, offers the swap, and keeps *Move window silently to workspace 3/4/5* (`SUPER+SHIFT+ALT+3/4/5`) untouched.
- It offers an alternative set (for example `SUPER+CTRL+SHIFT+3/4/5`) for users who rely on moving windows to workspaces 3, 4 and 5.
- The binds are written as `hl.unbind("SUPER + SHIFT + code:12")` plus `hl.bind(..., hl.dsp.global("snip:fullscreen"))` in Snip's Lua module, so removing `require("snip")` restores Omarchy's defaults.
- In spike S01 they were applied at runtime with exactly these calls, and `hyprctl reload` restores Omarchy's bindings.

Inside overlays (no Hyprland binds needed, because the overlay has keyboard focus): Esc, Enter, Space, Tab, arrow keys, single-letter modes in All-In-One, Ctrl+C.

## 8. Success metrics (measured, not surveyed)

- Hotkey to first overlay frame: p95 under 35 ms warm, under 300 ms cold. The floor is one compositor commit plus its readback, about 7–25 ms.
- Mouse release to capture persisted in history: p95 under 30 ms for 3440×1440. Copy action to `image/png` on the clipboard: p95 under 5 ms.
- Recording at 3440×1440@60 on the RTX 4080: 0 dropped frames over 10 minutes, under 5 % of one CPU core.
- Daemon idle: ≤ 50 MB PSS without a GPU context, ≤ 120 MB PSS while kept warm (measured 46 / 106 MB in spike S01); 0 % CPU and no wakeups while idle.
- Zero crashes of the user's desktop shell caused by Snip (an architectural requirement; see the architecture doc).

## 9. Product questions

### Decided (2026-10-06)

| # | Question | Decision |
|---|---|---|
| Q1 | Name | **Snip** (AUR package `snip-screenshot`; see §1) |
| Q2 | Studio Mode in 1.0? | **Out of v1** |
| Q5 | Recording default | **gpu-screen-recorder KMS** (fast), with private-window warnings; privacy-safe mode later (REC-15) |
| Q8 | Licence | **GPL-3.0-or-later** |
| Q10 | Strategy towards Omasnap | **Independent competitor** (D27): the full CleanShot experience, no shared code (§1.1) |
| Q3 | Replace or coexist with Omarchy's PRINT binds | Onboarding **offers** to rebind (with one-click revert); never automatic |
| Q11 | Clipboard and history | **Capture → history only; Copy → clipboard only; Save → export folder + clipboard** (§5.0) |
| Q12 | UI font | **CleanShot style:** a proportional UI font with tabular figures (design doc §5) |
| Q13 | Corners | **Follow Hyprland `rounding`**, with an override setting (Follow / Rounded / Square) |

### Still open

4. **Bar integration.** Ship an omarchy-shell bar-widget plugin, or reuse Omarchy's `pgrep gpu-screen-recorder` recording indicator? Proposal: our own widget (recording timer, stop and capture buttons), since Omarchy's indicator stops *every* gsr process.
6. **Second press of the hotkey while the overlay is open.** Cancel (Omarchy today) or cycle mode (Area → Window → Fullscreen)? Proposal: cancel.
7. **Settings UI depth.** Proposal: the full CleanShot-style settings window (now P0, M3) plus the TOML file.
9. **Distribution.** AUR first. Flatpak is not workable (layer shell, Hyprland IPC).
