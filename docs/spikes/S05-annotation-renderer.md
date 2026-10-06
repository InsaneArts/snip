# Spike S05: annotation renderer, preview vs export and frame rate (covers S8)

Date: 2026-10-06. Code: `spikes/s01-foundation/qml/Editor.qml`, driver `qao-test/s8_editor.py`.

## Setup
- A Qt Quick canvas with CleanShot's core object types:
  - curved and straight **arrows** (`Shape` with `CurveRenderer`, filled heads);
  - outlined and filled **rectangles**;
  - an **ellipse**;
  - a **highlighter**;
  - **text** in Outline and Box styles, including Georgian;
  - three **counters**;
  - a **pixelate** region (a low-resolution `ShaderEffectSource` with nearest sampling).
- Shown at **100 %**: a 1600×900 image in a 1280×720 logical window at scale 1.25, so 1 image px = 1 device px. The window is floated by a runtime rule at logical (400, 200), which is a whole physical pixel position.
- **Export** = `grabToImage` of the canvas. **Screen** = a capture of the output cropped to the window.

## Results

| Test | Result |
|---|---|
| **Export vs on-screen pixels** | **44 differing channel values of 4.32 M, max Δ 1, PSNR 98 dB.** Preview equals export ✅ (criterion ≤ 1 LSB) |
| Frame rate, **threaded** render loop, dragging one curved arrow every frame, 0 extra objects | **144.0 fps**, median 6.94 ms, p95 7.25 ms, max 8.2 ms ✅ |
| Same with **200 extra arrows** | **144.0 fps**, median 6.94 ms, p95 7.22 ms, max 8.1 ms ✅ |
| Same with Qt's **basic** render loop (Qt's default choice here) | **125 fps**, p95 15–16 ms (missed vblanks) ❌ |

## Findings that change the product
1. **Force `QSG_RENDER_LOOP=threaded`.** On this NVIDIA machine Qt chose the *basic* loop by default, which caps the editor at about 125 fps with stutter. The threaded loop holds a steady 144 Hz even with 200 objects.
   - The S5 soak instances run the default (basic) loop, so **a second soak with the threaded loop is required** before 1.0. Threaded rendering on NVIDIA is exactly where the known Quickshell EGL issues live.
2. **`grabToImage(cb, size)` multiplies `size` by the window's DPR.** Pass the *logical* size to get native pixels (1280×720 at 1.25 → 1600×900). Asking for 1600×900 returned 2000×1125.
3. **Export path decided:** render the editor scene offscreen (`grabToImage`, or `QQuickRenderControl` for sizes beyond the window or the GPU texture limit, using tiling). This answers architecture Q-S8: **Qt Quick nodes with Shapes and CurveRenderer, plus ShaderEffect.** Qt Canvas Painter isn't needed for parity or speed.
4. **cxx-qt QML module quirk:** a second QML file declared with `qml_file("../qml/Editor.qml")` is registered at the wrong qrc path (`…/com/snip/qml/…`), so `Editor {}` as a type fails. The spike loads it by URL. The product puts QML files inside the crate's module directory, so no `../` paths appear.
5. **QML `Window` inside a `Loader` under a non-visual root never mapped.** Create windows with `Qt.createComponent().createObject()`, or from C++, which the product does anyway.

## Not yet covered
- Live interaction (handles, hit-testing, text editing with IME: S4), undo/redo, very large canvases (tiling beyond 16k), secure pixelate built from pixels *outside* the region (needs a ShaderEffect with a custom shader), and object shadows (MultiEffect cost).
