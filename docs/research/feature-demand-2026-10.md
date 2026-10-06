# What people ask screenshot apps for (October 2026)

Date: 2026-10-07. Four parallel research passes covered macOS (CleanShot X, Shottr, Xnapper, Snagit, Monosnap, Skitch), Linux (Flameshot, Spectacle, GNOME, ksnip, Satty, Swappy, Gradia, Hyprshot, grimblast, slurp), Windows (ShareX, Greenshot, PowerToys, Snipping Tool, Snagit, Lightshot) and the Omarchy/Hyprland community.

**Evidence quality.**
- **Votes:** GitHub 👍 counts are real votes, sorted by reactions with `gh`. They are the backbone of this ranking.
- **KDE Bugzilla:** voting is off, so CC and duplicate counts stand in for votes.
- **Reddit:** reachable only through archives. Upvotes are archive snapshots or unverified.
- **No boards:** CleanShot and Shottr have no public vote boards. "Shipped after requests" is inferred from changelogs.
- **Search budget:** every pass ran out of web-search budget, so coverage of forums and reviews is partial.

Numbers like "👍137" are thumbs-up on the named issue.

## 1. The ranked demand, across ecosystems

| # | Request | Strongest evidence | In Snip's plan |
|---|---|---|---|
| 1 | **Scrolling / long capture**, auto and manual, horizontal too | flameshot#1130 👍137 (its most-wanted feature); PowerToys#3388 👍34; ShareX #7334 👍29 and others (people want manual scroll back); 14 Mac Reddit titles; Omarchy D#14175 "coming from macOS, I miss scrolling screenshots" | ✅ SCR-01…05 (P0, M3), manual + auto + sticky headers |
| 2 | **Open or edit an existing image, paste one from the clipboard** | flameshot#240 👍176, **the most-voted Linux request**; Gradia#282, Satty#127, Greenshot#107 (open clipboard image by hotkey) | ⚠️ partial: OMA-06 `snip open/edit FILE`, ANN-12 add image. No "annotate the clipboard image" action yet |
| 3 | **Screen recording and GIF** (webcam, audio, pause) | flameshot#172 👍126; PowerToys#143 👍86 (closed when Windows built it in); CleanShot 5.0 Studio Mode post +271; Omarchy #4286, #3699, #7479 | ✅ REC/VED (P1, M4) |
| 4 | **Pick a window in the selection UI** | flameshot#5 👍122, #395 👍38, #334 👍34 | ✅ hold Space (D23), clean toplevel capture |
| 5 | **Cursor on or off in captures** | flameshot#604 👍119 (include it); Hyprshot#75 👍31 and grimblast#60 👍17 (hide it) | ⚠️ CAP-11 is P1; today the cursor is always hidden |
| 6 | **Copy to the clipboard without saving a file** | GNOME 5208 👍84 (125 comments); Gradia, ksnip, Spectacle | ✅ the capture lifecycle (D10) is exactly this |
| 7 | **OCR**, more languages, plus **QR** | flameshot#702 👍61, #511 👍22; Omarchy#1918 (its top capture request); PowerToys Text Extractor; CleanShot's most-used feature in r/macapps | ✅ OCR basic shipped (M1); QR is OCR-03 (P0, not built) |
| 8 | **Upload to your own storage** (S3, SFTP, custom uploader) with a share link | flameshot#159 👍42, #50 👍40, #499 👍29, #27 👍21; ShareX's whole identity; Shottr S3 (+95); Monosnap SFTP | ❌ out of scope today ("no Upload") |
| 9 | **HDR- and 10-bit-correct captures** (no washed-out colours) | ShareX#6688 👍124 (280 comments, fixed with tone mapping in 2026-07); Hyprshot#100 👍13; hyprpicker#16 👍12; Spectacle 502053; Omarchy#2981 | ❌ not addressed |
| 10 | **Fractional scaling, HiDPI and multi-monitor correctness** | flameshot#564 👍140, #2364 👍87, #2978 (Hyprland) 👍103; Spectacle 462860 (15 duplicates); Omasnap#170 (blurry preview) | ✅ Snip's core strength: pixel-exact at 1–2× and rotated, tested |
| 11 | **Editor zoom, crop, editable objects, step counter** | ShareX#2325 👍83; swappy#55 👍72; flameshot#272 👍40, #187 👍27; Spectacle crop (26 duplicates) | ✅ ANN-01, ANN-11, ANN-02/16, ANN-08 (M2) |
| 12 | **Repeat the last region** | flameshot#417 👍48, #1509 👍14 | ✅ CAP-03 (built) |
| 13 | **Formats and quality**: WebP, AVIF, JPEG XL, and fast PNG | ShareX#6090 👍41, #5250 👍32; flameshot#506 👍36; Omarchy#10049 (1.6 s per shot on 6K because of PNG compression) | ✅ PNG (fast) / JPG / WebP; JXL optional; AVIF missing |
| 14 | **Capture history / library, searchable** | 21 Mac Reddit titles ("Is there a screenshot app with a library?"); Shottr reviews ask for history with OCR search; Snagit's library is a reason to stay | ⚠️ HIS-01…04 (M3); no text search |
| 15 | **Backgrounds, padding, shadow, rounded corners** ("beautify") | Xnapper's whole product; Shottr Backdrop (+66); flameshot#313 👍23, #690 👍16 | ✅ BG-01…06 (M2), with Omarchy themes |
| 16 | **Pin to screen** | Spectacle 470369 (cc19); flameshot#954 👍21; the "top favourite" CleanShot feature in one thread; ShareX users keep asking for pin follow-ups | ✅ PIN-01…08 (M2) |
| 17 | **Delay / self-timer** | flameshot#124 👍25; GNOME 6674; Omarchy #1911, PR#13824 | ✅ CAP-06 (P0), not built yet |
| 18 | **Redaction that really hides content**, auto-redact | flameshot#121, Satty#90; Xnapper's auto-redact praised; HN warns pixelation can be reversed; Snagit Smart Redact (2025) | ✅ ANN-06 secure redaction; ANN-06b suggestions are P2 |
| 19 | **Pixel ruler / measurement, colour picker, dimensions while selecting** | Shottr's signature features; Omarchy D#6960, D#12526 ("80 % of the time I just want to check the size"); Omasnap ships a ruler | ⚠️ dimensions ✅, hex picker ✅; no ruler / measure mode |
| 20 | **Screenshots into AI coding agents** (paste into Claude Code, copy the path) | Omarchy #99, #4362, #13070; DHH's `rsyncing` helper for "remote AI runners" | ✅ D24: Copy also offers the file path as text |

Smaller asks: emoji/stickers (flameshot#764 👍25); cut out the middle of a long image (ShareX#4513, shipped; Omasnap has a cut tool); combine screenshots / grow the canvas (ANN-12 ✅); aspect-ratio presets (SEL-08 ✅); keystroke and click overlays in recordings (REC-09/10 ✅); Screen Studio-style cursor zoom (Studio Mode, out of v1 by D3); shortcuts for keyboards without PrintScreen (our SUPER+SHIFT+3/4/5 ✅).

## 2. What users complain about (and what Snip can win on)
- **Pricing changes and subscriptions:** CleanShot's yearly update fee, Snagit's move to subscription-only, Monosnap's paywall, and Shottr called "nagware". Snip is GPL and free.
- **Abandonment and breakage after OS updates:** Skitch, Monosnap and a quiet Shottr year.
- **Bloat and a non-native feel:** Snagit "feels like a Windows app"; CleanShot has "too many editing buttons".
- **Privacy:** ShareX uploaded by default until users pushed back (#6561 👍42). Shottr "calls home". CleanShot's cloud feels forced. Snip is local-first with no network by default.
- **Wayland-specific:**
  - portal prompts and failed captures (flameshot#3700 👍93);
  - clipboard bugs (flameshot#4116 👍59);
  - fractional scaling;
  - HDR;
  - NVIDIA/hybrid recording (Omarchy#2706);
  - frozen or stuck overlays (Omarchy#5468, #5506);
  - slow editor start-up (Satty#132).

## 3. The Omarchy picture: Omasnap is now the default
- **Omasnap** (github.com/omacom/omasnap, C++, MIT, about 560★, created 2026-08-10) replaced Tensaku as Omarchy's screenshot tool in PR #9446, merged 2026-09-21, after v4.0.4.
- Its README lists:
  - capture before the overlay maps;
  - a 10 s floating preview with pin, drag-out, copy and edit;
  - pins;
  - a shelf of the last 5 captures;
  - scrolling capture, OCR, backdrops, editable vector layers;
  - a cut tool, a pixel ruler and secure redaction.
- Its credits name Shottr, Satty and Flameshot.
- Snip already reuses Omasnap's engine code (D1, `stitch.cpp`).

**Where Snip is still distinct:**
- CleanShot-fidelity UX and motion.
- A themed, compositor-native design.
- A searchable history as the home of every capture.
- A full recording suite. Omasnap and Omarchy keep recording in shell scripts plus gpu-screen-recorder, and recording on NVIDIA is a known pain point.
- Pixel-exact HiDPI and multi-monitor, verified with tests (Omasnap#170 reports blurry previews).
- AI-agent hand-off.

**Maintainer stance** to respect when proposing Omarchy integration:
- Minimal configuration: "too much configuration for something like this" (#3701).
- Capture first, edit later (v3.4.0).
- One smart key.
- Small functions over menus.
- Privacy: QR results never touch disk (#6679).

## 4. Recommendations
1. **Annotate from the clipboard or a file** (#2, the most-voted Linux request): `snip edit --clipboard`, a bindable action, and "Open file…". Small, and should land with the M2 editor.
2. **OCR-indexed history search** (#14, #7): OCR every capture in the background and search history by its text ("find the screenshot with that error"). A clear differentiator from Omasnap's 5-item shelf. M3, P1.
3. **Cursor include/hide toggle** (#5): raise CAP-11 to P0. It is cheap, since the capture protocol already paints cursors on request.
4. **HDR / 10-bit capture** (#9): add as a risk and P1 item. Request 10-bit shm formats where the output is 10-bit or HDR, and tone-map to sRGB on export. Check the user's monitor first.
5. **Share through *your own* target, opt-in** (#8): not CleanShot Cloud. A post-capture "run command" hook (rclone, S3, 0x0.st, scp) that copies the returned URL. Local-first by default. **Needs a product decision**: the brief excluded "cloud", and this is user-owned sharing.
6. **Pixel ruler / measure mode** (#19): Shottr's signature feature, which Omasnap also has. P2 → P1, in the overlay or the editor.
7. **Auto-redaction suggestions** (#18): raise ANN-06b from P2 to P1.
8. **AVIF export**, a cut-middle tool for scroll captures, and emoji/stickers stay P2.
