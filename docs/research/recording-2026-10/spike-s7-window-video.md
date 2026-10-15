# Spike S7: one window's video (D68)

**Question.** Can Snip record a single window by itself: the window's own frames, following it as it moves, never covered by another window? gpu-screen-recorder records one window on Wayland only through the desktop portal, which fails on this NVIDIA stack (see [gpu-screen-recorder.md](gpu-screen-recorder.md), Q9). Snip would need its own pipeline: the window's frames from Hyprland's toplevel capture (as a window screenshot already takes them), then an encoder.

**Answer: yes, with room to spare.** Hyprland hands Snip a window's frames at the monitor's full refresh rate, and NVENC encodes them faster than 60 fps needs.

## How it was measured

`crates/snip-wayland/examples/window_stream.rs` captures one window (its Hyprland address) as fast as frames come, through the same one-shot `ext-image-copy-capture` session a window screenshot uses. With `--encode` it pipes each frame, BGRA, into ffmpeg's `h264_nvenc` (`-f null`: nothing is kept).

It ran on 2026-10-15 on the owner's desktop while it was idle: an RTX 4080 (driver 610.57), Hyprland 0.56, DP-2 at 3440 × 1440, 144 Hz, scale 1.25. Each run lasted 8 s.

| Window (physical px) | Frames only | Frames → NVENC | CPU, frames → NVENC |
|---|---|---|---|
| Chromium page, 1331 × 1296 | **143.9 fps**; capture p50 6.9 ms, p95 9.1 ms, max 12.9 ms | **137.6 fps** | probe 44 %, ffmpeg 44 % of a core |
| Terminal, 1079 × 623 | **144.1 fps**; p50 6.9 ms, p95 8.5 ms | **134.7 fps** | probe 43 %, ffmpeg 44 % |

Without encoding, the probe itself costs 1 % of a core.

## What it means

- **Frame rate:** frames come at the monitor's refresh rate (144 Hz here). A take at 60 fps would ask for fewer than half of them.
- **CPU:** the encoded run moved a whole BGRA frame through a pipe each time: ~7 MB per frame, about 950 MB/s at 138 fps. At 60 fps the same path would cost about 40 % of a core in all. Handing the encoder GPU buffers (dmabuf) instead of copies would remove most of that.
- **What's left to build** (if the owner wants it):
  - a persistent capture session instead of one-shot sessions per frame,
  - frame timestamps for a constant-rate video (repeat a frame when the window doesn't change),
  - sound from the same clock as gsr's,
  - what happens when the window is resized, minimised, moved to another workspace or closed mid-take,
  - Studio's input log in the window's coordinates.

Until then a window take records the window's area of the screen (REC-01).
