# Spike S01: foundation (covers S1, S2, S13; starts S5)

Date: 2026-10-06. Code: [`spikes/s01-foundation/`](https://github.com/InsaneArts/snip/tree/1b9ab47a31056383be0a7b2a2d4506ceb580fa36/spikes/s01-foundation) (removed later; the link is the last commit with it) (throwaway). Machine: RTX 4080 (nvidia-open 610), Hyprland 0.56.2, DP-2 3440×1440@144 at scale 1.25.

## Question
Can a **Rust core plus Qt Quick UI**, running as one resident process, do Snip's hot path fast enough and pixel-exactly? The hot path:
1. hotkey;
2. capture every output in memory;
3. show a frozen overlay per output;
4. select;
5. crop, encode, put on the clipboard.

## What was built
- **Rust core** (`rust/src/`):
  - its own Wayland connection ("connection A") on its own thread (calloop);
  - `ext-image-copy-capture-v1`, with a fresh session per capture into memfd `wl_shm` buffers. The buffers are pre-faulted and marked `MADV_DONTDUMP`;
  - `hyprland-global-shortcuts-v1`, bound as `hl.dsp.global("snipspike:area")`;
  - `ext-data-control-v1` for clipboard ownership;
  - `ext-idle-notify-v1` for the soak;
  - PNG encoding with the `png` crate (`Compression::Fast`).
- **cxx-qt 0.10 bridge:**
  - a `SnipCore` QML singleton owned by Rust, with signals queued onto the Qt thread;
  - plain `extern "Rust"` functions the C++ render code calls (`frame_info`, `mark_time`).
- **C++ `FrozenFrame` QQuickItem:**
  - wraps the Rust-owned memfd mapping in a `QImage` **without copying**;
  - uploads once and samples 1:1 with nearest filtering;
  - timestamps `frameSwapped`.
- **QML:** one LayerShellQt overlay window per screen (layer overlay, all anchors, exclusive keyboard, namespace `snip-select`), created up front and mapped on demand. Selection, dimming and a W×H label in physical pixels.
- **Build:** CMake plus cxx-qt-cmake 0.10 (Corrosion) plus Qt 6.11.

## Results

### S2: pixel exactness. ✅ PASS
Capture, show the frozen overlay, re-capture the composited output while the overlay is up, then diff:

```
{"output":"DP-2","w":3440,"h":1440,"diff_pixels":0,"max_channel_delta":0,"bbox":null}
```

**0 of 4,953,600 pixels differ at fractional scale 1.25.** Qt Wayland's fractional-scale and viewporter path plus 1:1 nearest sampling gives an exact frozen frame. It was re-verified after every code change.

Not yet covered: scales 1.5, 5/3 and 2, rotated outputs, multi-monitor. The only monitor here is 1.25. These need a temporary headless output (see "Next").

### S1: hotkey-to-overlay latency (warm process). ✅ PASS (target p95 ≤ 35 ms)
30 trials through the **real Hyprland global-shortcut path** (`hyprctl eval 'hl.dispatch(hl.dsp.global(...))'`). The first trial is cold and excluded. Timestamps are on CLOCK_MONOTONIC, which is the same clock Hyprland uses for the shortcut event (`steady_clock`).

| Stage | Median | p95 | Max |
|---|---|---|---|
| Hotkey delivery, compositor to our process (no spawn) | **0.07 ms** | 0.10 | 0.12 |
| Capture of 3440×1440 (issue to all outputs ready) | 8.9 ms | 10.8 | 11.4 |
| Frames ready to first frame swapped (Qt: map, upload 20 MB, render) | 2.9 ms | 5.7 | 5.9 |
| **Hotkey to frame swapped** | **12.1 ms** | **16.4** | 16.6 |

On-screen time is the swap plus at most one refresh (6.9 ms at 144 Hz), so **p95 is ≤ 23 ms from keypress to photons**. For comparison, Omarchy's current script sleeps a fixed 100 ms after starting hyprpicker, before slurp even appears.

Notes:
- The capture time is dominated by waiting for Hyprland's next commit (≤ 6.9 ms at 144 Hz) plus its readback. It will be about 2× on 60 Hz monitors (≈ 17–20 ms); still within budget.
- `createTextureFromImage` returns in 0.02 ms because RHI defers the real upload into the frame. The upload is inside the 2.9 ms.

### Cold path. ✅ within budget (≤ 300 ms)

| Step | Time |
|---|---|
| `exec` to core ready (Qt app, QML loaded, Wayland bound, shortcut registered) | **70 ms** (QGuiApplication 4 ms after unsetting the gtk3 platform theme) |
| First overlay show in a fresh process (window creation, EGL and GL init) | about 100 ms |
| **Total cold, process start to frozen overlay** | **≈ 170 ms** |

### Copy path. ✅ PASS (target ≤ 30 ms to clipboard)
Copying a 1600×900 region from the frozen frame:

| Step | Time |
|---|---|
| Crop + BGRX→RGB | 1.8–2.3 ms |
| PNG encode (fdeflate) | 9.8–10.9 ms (994 KB) |
| **Copy to clipboard offered** | **12.4–13.2 ms** |
| First `send` from clipboard watchers | < 0.2 ms after the offer |

- `wl-paste` returned a valid 1600×900 RGB PNG with correct colours and orientation (checked visually).
- The selection was read **four times concurrently** (Omarchy's two `wl-paste --watch` processes, the user's Jolt watcher, and our paste), which confirms the multi-reader `send` handling.
- Observed: an unrelated `wl-copy` from another process replaced the clipboard within about a second of one test. This is not caused by Snip, but it is a reminder that clipboard ownership can be taken at any time and the UI must never assume it still holds the selection.

### Memory and fds (no leak over 65 shows)

| State | PSS | Private (anon) | RSS | fds |
|---|---|---|---|---|
| Started, overlay never shown (no GPU context) | **46 MB** | 26 MB | 128 MB | 30 |
| After first show (GPU context and EGL loaded) | 104 MB | 48 MB | 200 MB | 47 |
| After 15 / 35 / 65 shows | 108 / 108 / **106 MB** | 48 / 48 / 48 MB | 204 MB | 47 |

- Memory **plateaus after the first show**. Most of the RSS is shared library pages (Qt, NVIDIA driver) that omarchy-shell also maps.
- The earlier budget "idle ≤ 40 MB RSS" was wrong for an NVIDIA machine and is replaced by a PSS budget (performance doc §3).
- The real design choice is **keep the GPU context warm (about 106 MB PSS, 12 ms hotkey path)** or **release it when idle (about 46 MB PSS, +about 90 ms on the next capture)**. Proposal: keep warm for 10 minutes after the last use, then release. This is configurable.

### S13: Rust core plus Qt UI build. ✅ PASS locally (CI/PKGBUILD pending)
- CMake, cxx-qt-cmake 0.10, Corrosion, cargo and Qt 6.11 build in about 50 s cold and seconds incrementally.
- Rust-owned QML singleton, queued signals across threads, and C++ calling Rust directly all work.
- Zero-copy frame sharing from Rust to a C++ QQuickItem works.
- Gotchas found, both one-line fixes:
  1. The bridge `namespace` also applies to imported `QString`. Put `#[namespace = ""]` on the extern C++ block.
  2. cxx-qt 0.10 no longer camelCases names. Add `#[auto_cxx_name]` to the `extern "RustQt"` block.
- **AUR packaging verified ([`packaging/aur/snip-spike-git/PKGBUILD`](https://github.com/InsaneArts/snip/tree/1b9ab47a31056383be0a7b2a2d4506ceb580fa36/packaging/aur/snip-spike-git/PKGBUILD)):**
  - All downloads happen in `prepare()`: `cargo fetch --locked`, plus cxx-qt-cmake as a git source.
  - `build()` runs **offline**: `CARGO_NET_OFFLINE=true` and `FETCHCONTENT_SOURCE_DIR_CXXQT` with `FETCHCONTENT_FULLY_DISCONNECTED=ON`.
  - `makepkg` built the package in **59 s**: 1.8 MB, a 5.4 MB stripped binary. **The packaged binary passes `--selftest` (0 differing pixels).**
  - Still to do: a clean-chroot build (`extra-x86_64-build`), and the CI job once the repository has a remote.

## S5: NVIDIA soak. ⏳ RUNNING
- Started 2026-10-06 19:35 as the transient user unit `snip-soak.service`.
- It runs **only while the user is idle for 2 minutes or more** (ext-idle-notify): one capture and overlay every 30 s, and memory and fds logged every minute. That covers screen lock, DPMS-off and resume without disturbing active use.
- Logs: `~/.local/state/snip-spike/soak.jsonl` and `soak.err`.
- Stop with `systemctl --user stop snip-soak`.
- Pass criteria (performance doc §5): no crash, RSS drift ≤ 10 MB, fd drift 0, over at least 72 h.

## Decisions and doc changes from this spike
1. **Stack confirmed:** Rust core plus Qt Quick, one process, two Wayland connections. Nothing found that triggers the GTK fallback.
2. **Performance budgets updated:** the PSS-based idle budget, and the keep-warm policy.
3. **Startup:** unsetting `QT_QPA_PLATFORMTHEME` is confirmed worthwhile (QGuiApplication in 4 ms).

## Round 1b: multi-monitor, scales and transforms (2026-10-06)

**Method.** `multiscale.sh` adds a temporary headless output (`hyprctl output create headless SNIPTEST`, 1920×1080) next to DP-2. For each scale and transform it runs `--selftest`, which captures every output, shows the overlays, re-captures and diffs per output. Then it removes the output.

**Final results** (after the fixes below):

| Second output | DP-2 (1.25) | SNIPTEST |
|---|---|---|
| scale 1, 1.25, 1.5, 5/3, 2 (transform 0) | 0 * | **0** |
| 1.25, transform 1 (90°) | 0 * | **0** |
| 1.5, transform 3 (270°) | 0 * | **0** |
| 2, transform 4 (flipped) | 0 * | **0** |
| 5/3, transform 5 (flipped-90), cursor hidden | **0** | **0** |

\* With a headless output present, DP-2 showed exactly **309 px in a 20×29 box at the cursor position**. With `cursor:invisible` set, the diff was **0**. Hyprland falls back to a **software cursor**, which is baked into captures, and the overlay changes its shape (arrow to crosshair). This confirms the software-cursor edge case from the platform research.

**Bugs found and fixed (all carry over to the product):**
1. **Every overlay landed on the first monitor.** The cause: QML's `Qt.application.screens` yields `QQuickScreenInfo` wrappers. Neither `Window.screen` nor LayerShellQt's `screen` (which wants a `QScreen*`) placed the layer surface correctly. **Fix:** pass the real `QScreen*` (C++ `ScreenLookup.byName`) to `LayerShell.Window.screen`. The product creates its overlay windows from C++, one per `QScreen`.
2. **Rotated and flipped outputs were fully wrong.** The captured buffer is in the panel's native orientation (`frame.transform` = the transform applied to the contents). **Fix:** draw with a geometry node whose texture coordinates are `rotate_ccw^k(flip?(u,v))`. All transforms tested (0, 1, 3, 4, 5) are exact.
3. Minor: a context-property object must outlive the QML engine (bindings re-evaluate during teardown).

**Still open for the product:**
- **Crop and export on rotated outputs.** Map the selection to buffer coordinates and un-rotate the cropped pixels. The display is correct; the copy path in the spike still assumes transform 0.
- **Software-cursor policy.** When Hyprland renders the cursor in software (tearing, `no_hardware_cursors`, or a headless or virtual output present), monitor captures contain the cursor. The plan:
  - for the one capture frame, set `cursor:invisible` (eval, about 1–2 ms) or force hardware cursors, as Omarchy does, then restore;
  - when "show cursor" is on, draw the cursor ourselves from the ext cursor session.
- ~~Input routing across monitors~~ **done (R18, `qao-test/r18_multimon.py`):**
  - Overlays on DP-2 (1.25) and a headless second output (1.5), both exclusive-keyboard.
  - A virtual-pointer drag of 400×200 logical **on the second monitor** produced a **600×300** capture: the correct output, scale and crop.
  - The overlay closed on release, and in a second round **Esc** reached the overlay and closed it.
  - Harness note: the spike's `--test` mode first inherited the bench auto-close timer, which made a false pass look real. Fixed and re-verified.

## Next (Round 1 leftovers)
- ~~Scales 1.5, 5/3 and 2 and rotated outputs~~ done (Round 1b).
- Multi-monitor input routing (R18), moved to Round 2 (synthetic input).
- `wp_presentation` feedback for exact photon time (currently swap plus ≤ 1 refresh).
