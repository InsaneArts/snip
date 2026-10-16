# Spike S06: scrolling capture on Omasnap's stitcher (covers S11, S16)

Date: 2026-10-06. Code: [`spikes/s01-foundation/qao-test/scroll.html`](https://github.com/InsaneArts/snip/blob/1b9ab47a31056383be0a7b2a2d4506ceb580fa36/spikes/s01-foundation/qao-test/scroll.html), `qao-test/s11_scroll.py`, spike `grab`/`scroll` control commands. Omasnap at upstream commit `6771b19` (MIT).

## Method
1. A synthetic long page, **400 rows**, each carrying its index as a **12-bit black/white barcode plus a parity bit**, with a **sticky header** (70 CSS px) and a **fixed footer** (48 CSS px). Opened in Chromium on a dedicated workspace.
2. Scrolled with Snip's **virtual pointer** (wheel axis events). Each frame is grabbed through **Snip's own capture path**: a fresh ext-image-copy-capture session, cropped to the window, written as PNG.
3. Frames stitched by **Omasnap's stitcher** (`stitch.cpp`, built from source with its `stitch-replay` harness).
4. The result is verified by **decoding every row's barcode**. Any duplicated, missing or out-of-order row, or a repeated header or footer, is a failure.

## Results

| Case | Frames | Capture | Stitch | Verification |
|---|---|---|---|---|
| Opaque window (rule: opacity 1, no dim, no blur) | 40 (39 forward, 0 skipped) | 15.4 s (fixed 0.35 s settle per wheel step) | **2.3 s** | ✅ **all 400 rows in order, 0 missing, 0 duplicated, 0 order breaks**, row heights uniform (47–48 px), header once at top, footer once at bottom. 3348×25089 px |
| **Omarchy default translucent window** (wallpaper visible through it) | 40 (39 forward, 0 skipped) | 16.1 s | **17.1 s** (7× slower) | ✅ identical: 400 rows in order, header and footer once |

**S16 (reuse Omasnap's engine): ✅.** `stitch.cpp` builds unchanged with our toolchain (Qt 6.11, C++23). It needs only QtCore and QtGui (`QImage`), so it fits the **C++ side** of Snip. The Rust core drives the session (virtual-pointer scrolling, frame capture, settle detection) and passes frames to the stitcher as `QImage`s wrapping memfd buffers (zero copy). Its `ManualCapture` decision loop (seeded, kept, pending, ambiguous, unmatchable, wrong direction, full) maps directly onto CleanShot's live status UI.

## Findings that change the product
1. **Scroll-capture from the window's own buffer** (ext toplevel source), not a screen region, whenever the selection lies inside one window. A screen-region capture includes Omarchy's default window opacity and dim, so the wallpaper shimmers through. Stitching still works, but takes 7× longer, and the stitched image contains the wallpaper. The toplevel buffer has no compositor effects.
2. **Settle detection replaces fixed sleeps.** This spike waited a fixed 0.35 s per step; the product compares consecutive frames (and uses frame arrival) to scroll as fast as the page settles.
3. **Chromium may render at its own scale.** The test page rendered at about 1.667, not the output's 1.25, because of the user's Chromium configuration. The stitcher doesn't care, but any layout assumptions (for example in tests) must measure scale from the content.
4. **The stitch output path should avoid Qt's PNG encoder** (it dominates the 2.3 s for a 3348×25089 image). The product hands the finished image to the Rust fdeflate path (spike S01: about 1 GP/s) and, for tall images, streams tiles.

## Not yet covered
- Horizontal scrolling, auto-scroll end-of-page detection in the product loop (here: three identical frames), lazy-loading pages, pages with animations or videos, real-world sites (GitHub, docs), XWayland apps, and very long pages near the size budget.
- The live preview UI and the "slow down" and "unmatchable" warnings.
