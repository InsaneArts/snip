# Roadmap

Status: **DRAFT**, 2026-10-06.

- Milestones are sized S (under a week), M (1–3 weeks), L (3–6 weeks) or XL (over 6 weeks) of focused work. They are not dated until the team and scope are decided.
- Feature IDs refer to [01-product-spec.md](01-product-spec.md).
- **Every milestone ends with a usable build that someone uses every day.**

## M0: De-risking spikes (M, about 2 weeks; throwaway code)

Each spike answers one question with a measurement and a go/no-go. The results are written up in `docs/spikes/Sx-*.md` and may change the architecture doc.

| ID | Question | Pass criterion | Blocks |
|---|---|---|---|
| S1 | Can a warm Qt Quick daemon show a frozen, per-output overlay fast enough? | p95 ≤ 35 ms from hotkey to presented frame (wp_presentation) on DP-2, plus a cold number | stack |
| S2 | Is the overlay pixel-exact at fractional scales? | round-trip diff == 0 at 1, 1.25, 1.5, 1.6̅, 2; transform 1 | stack |
| S3 | Does drag-out from a non-exclusive layer surface work into Firefox, Chromium, Nautilus, a terminal, and a Flatpak app? | matrix filled in; COPY action respected | QAO design |
| S4 | Does IME work on our surfaces (fcitx5 preedit, dead keys, compose) in QML `TextEdit` on Hyprland 0.56? | works in an xdg toplevel; layer-surface behaviour documented | editor |
| S5 | Is a long-lived Qt Quick process stable on NVIDIA? | 72 h soak: no crash, RSS drift ≤ 10 MB, fd drift 0 | stack, Q-S2 |
| S6 | Do empty input regions on xdg toplevels give click-through on Hyprland? | a locked pin passes clicks through | PIN-05 |
| S7 | gpu-screen-recorder as an engine: `-ipc` control, region snapping, an external `pkill`, a daemon crash and re-attach, suspend, private-window detection | every case handled; drops = 0 at 3440×1440@60 | REC |
| S8 | Annotation renderer: Qt Quick nodes vs Qt Canvas Painter (6.11 tech preview) | preview == export (pixel diff ≤ 1 LSB); 144 fps with 200 objects | ANN |
| S9 | Keystroke and click capture through the Hyprland Lua hook | keys and clicks arrive within 5 ms; `non_consuming` mouse bind doesn't break drags; the watchdog removes hooks after kill -9 | REC-09/10 |
| S10 | Clipboard paste matrix: `image/png` and file-mode offers into Chromium, Firefox, Slack, Discord, VS Code, GIMP, LibreOffice, Telegram, kitty/alacritty | matrix filled in; the default chosen from data | SET-07 |
| S11 | Scroll stitching on a real corpus (Firefox, Chromium, Ghostty, Nautilus, Electron) at scale 1.25, including XWayland | ≥ 95 % of seams correct; sticky headers handled | SCR |
| S12 | OCR bake-off on a real screenshot corpus (UI, code, dark, Georgian, Russian, CJK): Tesseract fast with preprocessing vs PP-OCRv5 ORT | error rates recorded; default confirmed | OCR |
| S13 | Rust core plus Qt UI via cxx-qt and Corrosion: build, QML list models defined in Rust, memfd frames shared zero-copy, debugging | builds in CI and from an AUR PKGBUILD; the QAO model is driven from Rust | Q-S1 |
| S14 | ext-data-control clipboard ownership from the daemon, with Omarchy's clipboard manager watching | paste works; history behaviour with and without the sensitive hint | SET-07b |
| S15 | Hide-desktop-clutter cover: does a wallpaper slice (PreserveAspectCrop, same file as omarchy-shell) line up pixel-exactly over the bar at scale 1.25 and with a multi-monitor wallpaper? | diff ≤ 1 LSB outside wallpaper transitions; no window re-layout | CAP-12 |
| S16 | Omasnap stitcher vendored as C++ behind `cxx`, driven from Rust, checked with Omasnap's `stitch-replay` corpus | replay results identical to upstream Omasnap | SCR, §10 reuse plan |
| S17 | Theme tokens: resolve every installed Omarchy theme (26 or more) into Snip tokens, with the contrast guard; render the QAO card and the toolbar in all of them | no contrast failures after the guard; screenshots reviewed | design language |

### M0 status (2026-10-06)

| Spike | Status | Result | Doc |
|---|---|---|---|
| S1 warm latency | ✅ | hotkey → overlay p95 16.4 ms (cold ≈ 170 ms) | [S01](spikes/S01-foundation.md) |
| S2 pixel exactness | ✅ | 0 diff at scales 1/1.25/1.5/5⁄3/2, transforms 0/1/3/4/5, multi-monitor | [S01](spikes/S01-foundation.md) |
| S3 drag-and-drop | ✅ | 7 apps; drag offers URI + portal, no `image/png` | [S02](spikes/S02-quick-access-dnd.md) |
| S4 text input / IME | ✅ / ⚠️ | Latin and Unicode on toplevel and layer; Georgian **layout** needs a manual check; CJK IME untested | [S08](spikes/S08-text-input.md) |
| S5 NVIDIA soak (basic loop) | ⏹ stopped | 3 h, 59 trials, no crash; Pss 45 → 118 MB (inconclusive). Stopped 2026-10-06: its idle trials flashed a black overlay every 30 s | [S01](spikes/S01-foundation.md) |
| S5b soak (threaded loop) | ⏹ stopped | 2.7 h, 146 trials, no crash. Replaced by snipd's hourly `mem` trace over the week of daily use (no UI) | [S05](spikes/S05-annotation-renderer.md) |
| S6 pins | ✅ | over fullscreen, all workspaces; lock = layer surface | [S04](spikes/S04-pins.md) |
| S7 gpu-screen-recorder | ⏸ deferred | before M4 (recording) | — |
| S8 renderer | ✅ | export = preview (98 dB), 144 fps with 200 objects (threaded loop) | [S05](spikes/S05-annotation-renderer.md) |
| S9 keystroke hook | ⏸ deferred | before M4 | — |
| S10 paste matrix | ✅ | `image/png` paste works in Chromium, Zen, Nautilus, VS Code | [S02](spikes/S02-quick-access-dnd.md) |
| S11 stitching | ✅ | 400/400 rows, sticky header and footer once (opaque and translucent) | [S06](spikes/S06-scrolling-capture.md) |
| S12 OCR | ✅ | Tesseract fast + clean-up: 0 % Georgian, ≤ 2.5 % UI/Russian, 8 % code | [S07](spikes/S07-ocr.md) |
| S13 Rust + Qt build | ✅ | CMake/cxx-qt/Corrosion; offline AUR PKGBUILD builds and passes the self-test | [S01](spikes/S01-foundation.md) |
| S14 clipboard ownership | ✅ | ext-data-control; sensitive hint skips Omarchy history | [S02](spikes/S02-quick-access-dnd.md) |
| S15 hide-desktop cover | ✅ | visually seamless (46 dB), no re-layout | [S03](spikes/S03-themes-and-hide-desktop.md) |
| S16 Omasnap reuse | ⏹ superseded | Built as-is in the spike; replaced by D27 (Snip writes its own stitcher) | [S06](spikes/S06-scrolling-capture.md) |
| S17 theme tokens | ✅ | 26 themes, contrast ≥ 6.7:1; QScreen ownership crash found and fixed | [S03](spikes/S03-themes-and-hide-desktop.md) |
| R18 multi-monitor input | ✅ | a drag on the second monitor and Esc both route correctly | [S01](spikes/S01-foundation.md) |

**Exit:** the decided stack (Qt Quick + Rust core) is confirmed by S1, S2, S5 and S13, or the fallback is triggered with data. Q-S2 (process split) is settled.

## M1: Capture core and the Quick Access experience (L)

**Scope:**
- Daemon, systemd unit, global shortcuts, `snip` CLI (API-01, API-03)
- Capture: CAP-01 (area), CAP-02, CAP-03, CAP-04, CAP-05 (Space then click: clean window capture), CAP-09 basic, CAP-10
- Selection: SEL-01 to SEL-07, SEL-09 to SEL-11 (crosshair and magnifier included)
- QAO: QAO-01 to QAO-11 (stacking, hover actions, drag-and-drop out, gestures, context menu)
- **Capture lifecycle (product §5.0):** the history store as the default home (HIS-02 retention, HIS-04), Copy → clipboard only, Save → export folder plus clipboard; the history *browser UI* comes in M3
- Clipboard (SET-07), save with naming (SET-03 to SET-06; After Capture defaults: only "Show QAO" on), sounds (SET-09)
- Theme tokens and the design gallery (OMA-01, design doc §6); Lua module (OMA-02); OMARCHY_SCREENSHOT_EDITOR handoff (OMA-06)

**Exit:**
- All latency budgets for these paths are met.
- The developer uses it as their only screenshot tool for a week.
- Two-monitor, mixed-scale test passes.
- The QAO card passes design review against the CleanShot reference board in a dark, a light and a gradient-border theme.

### M1 status (2026-10-06, branch `m1`)

Built: `snipd` (Rust core + Qt Quick UI in one process), `snip` CLI, `hypr/snip.lua`, systemd unit, `snip-screenshot-git` PKGBUILD. End-to-end suite `tests/e2e/e2e.py` (11 cases on a headless output) passes.

| Area | Status | Notes |
|---|---|---|
| Daemon, unit, shortcuts, CLI (API-01, API-03) | ✅ | `snip area/fullscreen/window/previous/text/all-in-one/open/copy/save/list/restore/close-all/status/reload/quit`; exit codes 0/1/2/3. The full CleanShot URL-API surface (`--x --y --w --h --action`) is still to add. |
| CAP-01, 03, 04, 05, 09, 10 | ✅ | Click = window or monitor (smallest wins); Space = clean toplevel capture with Hyprland corners and shadow; OCR copies text. |
| CAP-02 variants | ⚠️ | Available through After Capture rules; separate "& Copy / & Save" bindable actions still to add. CAP-04 "all monitors" variant missing. |
| SEL-01…07, 09…11 | ✅ | p95 17.6 ms hotkey → every overlay presented (warm); first overlay after start ≈ 225 ms (budget 300). Exact crops at scales 1/1.25/1.5/2 and on a rotated output. The size in logical px is a setting. |
| QAO-01…07, 10 | ✅ | Hover keys (Ctrl+C/S/W/E, Space) verified with real key events; focus returns to the app. Copy closes the card (Alt keeps it). |
| QAO-08 gestures | ⚠️ | Touchpad swipes implemented, untested (no touchpad on the dev machine). |
| QAO-09 menu | ✅ | Copy, Save/Show in Files, Save As… (the desktop's save dialog via xdg-desktop-portal; Ctrl+Shift+S, also in the editor), Annotate, Pin, Open With…, Extract Text, Rotate Left, Flip Horizontal, Scale to 1×, Resize…, Preview, Print… (the Print portal), Trash, Close, Close All, Save All, Hide. |
| QAO-11 | ✅ | Edits in Snip's editor update the card (M2). |
| Lifecycle, history retention | ✅ | Capture → history only; Copy → clipboard; Save → folder + clipboard; hourly sweep; "Never" deletes on card close. |
| SET-03…07, 09 | ✅ | Rules, template naming (with the UTC and `@2x` options), export folder, PNG/JPG/WebP, clipboard modes, shutter sound. |
| OMA-01, 02, 06 | ✅ | Live theme (inotify + `configreloaded`), Lua module, `snip open/edit FILE`. |
| Design gallery | ✅ | `snipd --gallery`; renders in [docs/design/m1-gallery](design/m1-gallery/). |
| Snip UI never in captures | ✅ | Cards drawn transparent until the frame is presented (~0.4 ms), then the capture runs (decision D18). |
| QR and barcodes (OCR-03, 05) | ✅ | Capture Text and Extract Text read QR codes first (pure-Rust `rqrr`), then Data Matrix, Aztec and PDF417 (`rxing`); 1D barcodes with `ocr.barcodes`. Values are copied as sensitive. A link in a code or the text asks "Open link?" (Open, Always, Never). |
| Self-timer (CAP-06) | ✅ | `snip area/fullscreen --delay N`; shortcuts `snip:timer-area`, `snip:timer-fullscreen` (`capture.timer_seconds`). Area: select, count down, capture live. The countdown is never in the capture; the hotkey cancels it. |
| Cursor on/off (CAP-11) | ✅ / ⏳ | `capture.show_cursor` and `--cursor` / `--no-cursor`; Hyprland paints it per request (checked in its 0.56.2 source). Live check pending on the test output. |
| Searchable history (HIS-05) | ✅ index / ⏭ UI in M3 | Every capture is read in the background (OCR + QR, `nice 19`, ~120 ms each) into SQLite FTS5; `snip search TEXT` with prefix matching, Georgian included. |

**Exit criteria:** latency ✅; two-monitor mixed-scale ✅ (DP-2 at 1.25 plus a headless output at 1–2 and rotated); design review ⏳ (gallery ready); a week of daily use ⏳ started 2026-10-06 from source (`scripts/dev.sh`; the package is for later). snipd logs `mem` hourly, which doubles as the long-running soak.

**Dev loop:** `scripts/dev.sh` builds and runs `build/snipd` as the user unit `snip-dev` with real data, and loads `hypr/snip.lua` at runtime; `scripts/dev.sh stop` restores Hyprland's own bindings; `scripts/dev.sh log` follows the trace.

**Follow-ups found in M1:** OCR picks up junk on busy translucent backgrounds and reads Georgian there poorly (try colour-cluster binarisation; link Tesseract in-process); ~~pre-warm the overlay to remove the first-use cost~~ (done: overlays map once, invisibly, at start; first overlay ≈40 ms instead of ≈120–145 ms); window decoration takes ~40 ms on large windows (blur the shadow at quarter resolution).

## M2: Editor, Background tool, pins (L)

**Scope:**
- The full annotate editor: ANN-01 to ANN-18, except ANN-06b
- Background tool: BG-01 to BG-06, **including Omarchy theme backgrounds**
- Window shots "With wallpaper" and "Transparent" with shadow (CAP-05 options, BG-04)
- Pins: PIN-01 to PIN-08
- Project format v1 (Snip's own op-log format)

### M2 status (2026-10-07)

| Area | Status | Notes |
|---|---|---|
| Editor window (ANN-01, 14, 15, 19) | ✅ first slice | CleanShot's layout ([docs/design/editor.md](design/editor.md)); zoom (fit, steps, Ctrl+scroll at the pointer), pan; undo/redo; Save, Save As, Copy, Drag Me, Pin, Open With… (the share button: the desktop's app chooser); edits written back on their own. `snip edit FILE`, `snip edit --clipboard` (an image, or a copied PNG file) and the `snip:edit-clipboard` shortcut; the card's pencil and double-click. |
| Objects (ANN-02…05, 08…10) | ✅ | Arrow (Standard, Fancy, Curved, Double; drag its middle to bend it), rectangle, filled rectangle, ellipse, line, text (7 styles, wrap width, emoji from Omarchy's list), counter (1,2,3 / A,B,C / I,II,III, start value), pencil (smoothing), highlighter (snaps to OCR text lines, opacity); move, handles, Shift axis lock, Alt-drag duplicates, Ctrl+C/V/D, nudge, delete; object shadows. |
| Pixelate / Text shape / Blur / Black out (ANN-06, D26, D30) | ✅ | Pixelate is the default. Text shape (D30) shows word-shaped blocks of ink, never pixel values. Tiles come only from pixels outside the region plus seeded noise (`snip-core/redact.rs`): e2e exports two images that differ only in the hidden text and gets identical bytes; the hidden text also leaves the history search. Intensity (Low…Maximum) keeps that at every level. Natural blur is a real blur, labelled Not secure. |
| Spotlight (ANN-07) | ✅ | Rounded rectangle or ellipse; dim 25–85 %; spotlights merge; objects drawn on top stay bright. |
| Crop (ANN-11) | ✅ | CleanShot's crop bar: ratio, typed W × H, canvas fill (automatic edge colour, transparent, white, black, any), rotate, flip, resize in px or %, Revert to Original, Cancel, Crop. Handles, move, snapping to edges, outward crop that grows the canvas; exported exactly. |
| Background tool (BG-01…06) | ✅ | Gradients (2 from the Omarchy theme), the theme's wallpapers, blurred wallpaper or screenshot, plain colours and transparent; padding, inset, shadow, corners, ratio, alignment; the image stays pixel-exact inside. Presets with Apply Previous Settings, Save Changes and delete (BG-03); your own wallpapers (+ or drop) and any plain colour (BG-01). Auto-balance (BG-02). Values typed or stepped with ↑/↓ (BG-05). Apply to New Screenshots: area, window and fullscreen shots get the preset before their card, still editable. Shift held as the selection ends (or a window is clicked) skips it (D32). |
| Add image (ANN-12) | ✅ | From a file (Ctrl+I, or dropped), Take Screenshot (Ctrl+Shift+I: the editor steps aside for an area or window selection) or the clipboard (Ctrl+V); placed in the middle at up to half the canvas, with handles. Dropped near an edge, an image joins the canvas on that side at its width or height and the canvas grows (combine images). The window grows with the canvas. Moved objects snap to the canvas's and each other's edges and middles, with guides. |
| Colours (ANN-13) | ✅ | CleanShot's 10 swatches, 10 saved colours (My Colors, kept across editors), saturation/brightness, hue, alpha and hex; the eyedropper picks anywhere on screen through the frozen overlay with the magnifier. |
| Annotate settings (ANN-17, 18) | ✅ | `[annotate]` in the config: `object_shadows` (on), `inverse_arrows`, `smooth_drawing` (on), `auto_expand` (objects past the canvas grow it, one undo step), `remember_background_tool`, `always_on_top` (Hyprland pin). |
| Pins (PIN-01…08) | ✅ | `snip pin [ID]`, the card's pin (Ctrl+P), the editor's pin button. A floating, pinned window (every workspace of its monitor, above fullscreen) at the capture's own size: drag to move, scroll to fade, Ctrl+scroll or a corner drag to zoom, middle-click to close; hover chrome (close, zoom); menu: Copy, Save, Annotate, Close, Close All Pins. Arrow keys nudge (Shift: 10 px), a grip drags the image out, `snip:pins-toggle` / `snip:pins-close` and `snip pins toggle|close`. Lock (click-through, fades under the pointer; `snip pins unlock`). Capture Area & Pin (`snip area-pin`, `snip:area-pin`) pins the area in place; `snip pin FILE`. Look from `[pins]`: corner_radius, shadow, border. |
| Preview == export | ✅ | `Platform.grabExact`: the same scene, one item unit per pixel at any fractional scale; e2e checks every pixel outside the drawn object, and undo restores all of them. |
| Project format (ANN-16) | ✅ | `edit.json` (objects) and `original.png` next to the capture; reopening keeps objects editable; rotate/flip flatten. Save As `.snip` (a zip: original, objects, their files, preview, thumbnail) and `snip edit/open FILE.snip` bring it back editable. |

**Exit:**
- Preview == export.
- IME works.
- Redaction security tests pass.
- The Background tool renders every installed theme.
- Design review of the editor, Background tool and pins.

## M3: The rest of the screenshot experience (L)

**Scope:**
- Capture history UI (HIS-01 to HIS-03)
- Scrolling capture (SCR-01 to SCR-05), on Snip's own stitcher in Rust, tested against a recorded replay corpus
- Self-timer (CAP-06), cursor in captures (CAP-11)
- Hide desktop clutter (CAP-12)
- OCR complete (OCR-01 to OCR-06, including QR, languages and Georgian)
- Settings window (SET-02); shortcut recorder (SET-08)
- Capture-menu entries and file handlers (OMA-03, OMA-07)

### M3 status (2026-10-07; order from D34: history, settings, scrolling capture)

| Area | Status | Notes |
|---|---|---|
| History window (HIS-01, 03, 05) | ✅ | `snip history`, `snip:history`: a band across the top of the screen with large thumbnails, newest first; filters All, Screenshots and Projects; ←/→ or the wheel; a click selects a capture and a second click deselects it; its Restore button or Enter restores it to Quick Access (the band stays; D39), double-click or Ctrl+E annotates, Space previews, Ctrl+C copies, drag out; typing searches the text in captures; Esc clears the search, then closes. Backspace deletes after asking ("Don't ask again"), Clear History…, and a right-click menu (Restore, Annotate, Pin, Copy, Open With…, Show in Files, Delete). Retention (HIS-02) is set in Settings › Advanced. |
| Settings window (SET-02) | ✅ | `snip settings [TAB]`, `snip:settings`: CleanShot's tabs (General, Shortcuts, Quick Access, Screenshots, Annotate, Advanced, About) over every key of config.toml. A change is written at once, keeping the file's comments, and only if the file stays valid; a hand edit of the file shows in the window at once. History's size and Clear History… (HIS-02) are under Advanced. Recording's tab comes with M4. |
| Shortcut recorder (SET-08) | ✅ | Settings › Shortcuts lists every action with its keys. Click and press keys to record one (Esc cancels, Backspace or × clears). A key that already runs something asks first ("Use Anyway" takes it over). Restore Defaults brings snip.lua's keys back. Keys are kept in config.toml `[shortcuts]` and bound by snipd (docs/02-architecture.md, "Shortcuts"). |
| Scrolling capture (SCR-01…05) | ✅ | `snip scroll`, `snip:scroll`: choose the area (or Space and a window), then Start Capture (Enter) and scroll, or Auto-Scroll; Done (Enter), Esc cancels. A frame around the area, and a panel beside it with the image growing, its height, "too fast" and "very large" (over 30,000 px) warnings, and tips the first time. Auto-Scroll turns the wheel a notch a frame, pauses while the pointer is out of the frame, and ends when the page stops moving. Frames come from the window's own buffer when the area is inside one window. Snip's own stitcher: rows kept by content position, sticky header and footer once. Horizontal scrolling isn't done yet. |

**Exit:**
- **Every P0 item in the product spec is done.**
- A feature-by-feature walk-through against the CleanShot inventory (screenshot areas) passes.
- No public release yet (D40): it waits for recording.

### M3 P0 sweep (from the 2026-10-08 audit, D40)

What the 2026-10-08 audit of the spec found unbuilt, in batches:

- **Capture variants and the CLI:** ✅
  - Capture Area & Copy / & Save / & Annotate as bindable actions, which skip the After Capture rules by default (CAP-02). The After Capture "Pin to the screen" rule now pins.
  - `snip fullscreen --all` and "Capture all monitors", composited at the largest scale (CAP-04).
  - `snip area --x --y --w --h --action` (API-01).
  - `snip text --file / --clipboard` (OCR-06).
- **Text recognition:** ✅
  - Keep line breaks, a setting, plus the actions Capture text with line breaks / as one line (OCR-02).
  - Barcodes besides QR: Data Matrix, Aztec and PDF417 always, 1D opt-in (OCR-03).
  - A link in a code or the text asks "Open link?": Open, Always, Never (OCR-03, OCR-05).
- **Polish:**
  - ✅ UTC and `@2x` in file names (SET-04); history keeps each capture's scale.
  - ✅ Alt+Save asks where (QAO-03), and Ctrl+Alt+S. Tests never open the desktop's file dialogs now.
  - ✅ Scale to 1×, Resize… and Print… on cards (QAO-09); Print in the editor (ANN-15).
  - ✅ Resizing a pin from its corners (PIN-04).
  - ✅ History filters (HIS-01): All, Screenshots, Projects.
  - ✅ The size in logical px, as a setting (SEL-03).
- **Desktop:**
  - ✅ Hide desktop clutter while capturing, opt-in (CAP-12): the bar's band becomes the wallpaper, and pins stay out.
  - ✅ The live toggle (`snip desktop hide|show|toggle`, the `desktop-toggle` key, the bar menu): the cover on screen, DND, and pins and cards hidden (CAP-12).
  - A notification when the QAO is off (OMA-05).
- **Background tool:** ⏭ Snip's own wallpaper pack (BG-06 as changed in D41) is deferred (D42); the tool keeps the wallpaper in use, the theme's and the user's own.
- **To check on real hardware** (the product owner): the touchpad swipe on cards (QAO-08), the pointer in captures (CAP-11), the Georgian layout in the text tool (ANN-05).

## M3.5: Snip as a product (M; D36, planned, not built yet)

Snip should feel like an app, not a set of commands. The CLI stays complete for scripts and agents.

**Scope:**
- **Logo and icon set:** ✅ the app icon (`app/icons/snip.png`, from the product owner's design) and a one-colour bar icon the bar tints to its colour (`app/icons/snip-tray-symbolic.svg`). Still to do: the icon at every size, for the launcher.
- **Bar icon and menu** (OMA-08, D35): ✅ a tray icon (StatusNotifierItem). A click opens CleanShot's menu under the bar: the captures with their keys, History, Restore Recently Closed, Settings…, Quit Snip.
- **Launcher** (OMA-09): ✅ "Snip" is searchable. It opens Settings (`snip settings`), and its actions start Capture Area, Fullscreen, Window, Scrolling Capture, Text and History. `snip` starts Snip first if it isn't running (quit from its menu, say). `scripts/dev.sh` installs the entry for this checkout.
- **Easy install while the repository is private:** ✅ `scripts/install.sh` builds a pacman package from `packaging/aur/snip-screenshot-git` and installs it (`remove` uninstalls). It switches off a from-source Snip (`scripts/dev.sh` does the reverse, so one Snip runs at a time), and enables `snipd.service`, so Snip starts now and at every login. The launcher entry and icon come with the package.
- **First-run setup** (OMA-02): ✅ on Snip's first start a Welcome window opens (and with `snip welcome`). It shows Snip's keys; **Use these keys** adds one `dofile(…/snip.lua)` line to hyprland.lua and the keys work at once, or it says the config already loads them. It also has a start-at-login switch (for the systemd unit running Snip) and points to the bar icon. Get Started closes it for good.
- The public release, the AUR package and the landing page come last, when the whole product is ready (D40).

**Exit:**
- On a fresh Omarchy, one command installs Snip.
- Snip appears in the launcher and in the bar.
- Its menu opens Settings and starts every capture.
- Snip starts at login, and its keys work.

## M3.9: Clean-up (D40)

Before recording:
- refactoring, and cleaning the codebase;
- performance issues;
- a cleanliness audit.

## M4: Recording suite and remaining parity (XL)

**Scope:**
- Recording: REC-01 to REC-15, including webcam, clicks, keystrokes, GIF, safety and privacy-safe mode as decided
- Trimmer and export: VED-01 to VED-03
- Bar widget (OMA-04)
- All-In-One (CAP-07), adjustable selection (SEL-08)

**Exit:**
- 10-minute recording at 3440×1440@60 with zero drops.
- External stop, crash, suspend and lock are all handled.
- Every P1 item is done or explicitly waived.

## M5: 1.0 hardening (M)

- Multi-monitor matrix: 1, 2 and 3 outputs; mixed scales; rotation; hotplug.
- Intel and AMD laptop reference passes all budgets.
- Soak tests pass.
- Fuzzing of every parser is clean.
- AUR `snip-screenshot`, onboarding, docs, THIRD_PARTY.md.
- **Snip 1.0 = CleanShot parity, without Cloud and Studio Mode.**

## M6: After 1.0

- Studio Mode (VED-04, VED-05; decided to be out of v1).
- Redaction suggestions (ANN-06b), URL scheme (API-02).

## Dependencies between milestones

```
M0 spikes ──▶ M1 capture + QAO ──▶ M2 editor/background/pins ──▶ M3 screenshot parity ──▶ M4 recording ──▶ M5 1.0 ──▶ M6
                       └────────────────────────────────────── (M4 can start after M1 with a second person) ─┘
```

The screenshot experience (M1–M3) comes first, as decided. Recording (M4) depends only on M1's daemon, overlay, theme and QAO layers, so a second contributor can start it in parallel.
