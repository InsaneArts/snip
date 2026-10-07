# The editor (Annotate), as CleanShot does it

Source: cleanshot.com's `markup.mp4` demo (1700×1200, 60 fps, 27.6 s), studied frame by frame on 2026-10-07. Spec: ANN-01…19. Decisions: D26 (secure Pixelate/Blur).

## Layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ (crop)(+img)(bg) │ ➤ □ ■ ○ ╱ ↙ (A) ▦ ▭ ❶ ✎ ◎ │ ●▾  20 pt▾  A▾   [Save as…] │  toolbar
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│                    the image, on a light backdrop                    │  canvas
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│ 100% ▾                  ≡ Drag Me ≡                 ⟲  ⇪  📌  ⧉  ☁  │  bottom bar
└──────────────────────────────────────────────────────────────────────┘
```

- **Toolbar:** a light bar (`#ebe0ea` over the wallpaper tint).
  - Tools sit in white capsule groups with thin separators.
  - The selected tool is a filled blue circle (`#006dff`) with a white glyph.
  - Each tool keeps its own options. The options group changes with the tool: colour swatch with a chevron, stroke width (line glyph), "20 pt" for text, "123" style for counters, a lock for crop.
- **Save as…:** the blue primary button, top right.
- **Bottom bar:** slightly darker (`#d3cbd4`).
  - Zoom percentage dropdown (left).
  - "Drag Me" pill with grip lines (centre): drags the result out.
  - Action circles (right): rotate, share, pin, copy, cloud.

## Objects
- **Arrow:** pink-red `#f1285f` by default.
  - Thick rounded stroke and a filled triangular head about 3.2× the stroke wide.
  - Drawn straight. Dragging its middle bends it into a curve; there is no separate curved tool.
  - Selected: a pale blue halo around the whole shape, plus solid blue handle dots (`#0177ff`, white ring) at both ends; the middle handle appears on hover.
- **Text:** bold, in the current colour.
  - While typing: a pale blue box with a blue caret and handles on the left and right edges (wrap width), and a square at the bottom right (size).
  - A blue smiley above the box opens emoji.
- **Counter:** a filled circle in the current colour with a white bold number, about 1.6× the text cap height, with a soft shadow. Clicks add 1, 2, 3…
- **Highlighter:** a translucent yellow band.
- **Colour dropdown:** a vertical white capsule under the swatch.
  - Ten swatches: black, red, orange, yellow, green, teal, blue, purple, pink, white.
  - A rainbow "custom" swatch below a hairline.
  - The current colour has a ring.

## Motion
- Tool switches and selection halos are instant.
- Dropdowns fade and drop in about 120 ms.
- Zoom animates (about 200 ms) when changed from the menu or keys; it is direct under the pointer with Ctrl+scroll.

## Snip's take (first slice)

![The editor](editor/editor.jpg)

- Same layout and object look, themed. Chrome comes from Omarchy theme tokens: surface, text, accent for the selected tool and the primary button. The annotation palette is CleanShot's ten colours, with pink as the default.
- Hyprland draws the window frame, so Snip has no traffic lights.
- Tools: Select (V), Arrow (A), Rectangle (R), Text (T), Counter (N), Pixelate/Blur/Black out (P, D26), Crop (C).
- Primary button: **Save** (Ctrl+S, export folder plus clipboard, §5.0). Bottom right: Copy (Ctrl+Shift+C, closes the editor).
- Edits go back to the capture's history entry automatically (ANN-15).
  - The capture keeps its original and its objects next to it (`original.png`, `edit.json`), so reopening keeps everything editable (ANN-16).
  - Rotating or flipping a capture flattens it.
- Export is the same scene rendered offscreen at native size (`Platform.grabExact`), so the preview equals the export (spike S05).
