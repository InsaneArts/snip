# Quick Access card: CleanShot's design and motion, measured

Date: 2026-10-07. Source: CleanShot's own demo media on cleanshot.com (`/video/home/quickaccess.mp4`, 1360×960 at 60 fps; `/video/features/temporarily_hide.mp4`; `/img/features/quick_actions.png`, `drag_and_drop.png`). Frames were extracted with ffmpeg and measured per frame (edge positions and contrast against the empty background). The media is reference material only and is not in the repository.

## Layout
- **Stacking:** a new card appears **on top** of the stack; the oldest stays in the corner. Removing a card lets the ones above glide down.
- **Card:** fixed size, aspect ≈ **0.705** (254×179 in the video), the image fills it (aspect-fill). Corner radius ≈ 8 % of the width (Snip follows Hyprland's rounding instead, decision D14).
- **Newest indicator:** a solid accent ring ≈ 2 pt hugging the card outside its edge; shown only when there is more than one card.
- **Spacing** between cards ≈ 7 % of the card width; soft drop shadow.

## Hover
- The thumbnail **frosts**: heavy blur plus a tint (dark navy in dark mode, a darker mid-tone in light mode). The content is barely recognisable.
- **Copy** and **Save**: light-grey capsules with near-black semibold text, stacked in the centre.
- Four light-grey **circles** with dark glyphs in the corners: close (top-left), pin (top-right), annotate (bottom-left), upload (bottom-right). Snip: close, more (…), annotate, extract text; pin moves to top-right with pins in M2.
- Transition ≈ **150 ms**.
- Status states (upload's "Uploading…", "Link copied") are drawn on the frosted card with a check. Snip uses the same treatment for Copied, Saved and Reading text.

## Motion

| Event | Measured | Snip |
|---|---|---|
| Entrance | slides in from the screen edge by ≈ 22 % of the card width, **+10 % overshoot at ~150 ms, settled ≈ 220–250 ms**; no scale, no vertical motion | `cubic-bezier(0.40, 1.40, 0.40, 1.10)` over 270 ms (rms error 4.5 % against the measured curve) |
| Entrance fade | linear 0 → 1 in ≈ 180–200 ms | linear 180 ms (rms 2.3 %) |
| Exit (close, auto-close, after a drop) | slides back toward the edge, accelerating, while fading, ≈ 200–270 ms | `InCubic` 220 ms, linear fade 200 ms |
| While dragged | the card stays, dimmed to about half opacity; it closes after a successful drop ("Close after dragging") | same; Alt keeps it |
| Hide all (swipe down) | the whole stack slides down off the bottom edge; a small tab stays to bring it back | 340 ms `InOutCubic`, tab peeks 14 px and lifts on hover |
| Reflow | cards above a removed one glide down | 300 ms `OutCubic` |

Measured entrance (normalised position, 16.7 ms steps): 0.07, 0.36, 0.57, 0.79, 0.93, 1.04, 1.07, 1.11, 1.11, 1.11, 1.07, 1.05, 1.00.
