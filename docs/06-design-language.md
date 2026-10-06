# Design language: CleanShot's polish in Omarchy's colours

Status: **DRAFT**, 2026-10-06. §5 lists the open design questions.

The goal is a direct requirement from the product owner: Snip must be **as beautiful as CleanShot X** and must **follow Omarchy themes**. These pull in different directions:
- CleanShot's look is macOS-native: SF Pro, vibrancy, large soft shadows, rounded corners.
- Omarchy's look is themeable and terminal-flavoured: monospace type, flat or glass surfaces, square corners by default, 26 or more themes including light ones.

The rule that resolves this:

> **Take CleanShot's layout, hierarchy, density, motion and interaction states. Take Omarchy's colours, borders, corners, materials and type tokens.**
> The screenshot is always the hero. Chrome recedes until it is hovered or needed.

## 1. What makes CleanShot look good (things we copy)

1. **Content first.** Capture chrome is minimal: a thin border, a small dimension pill, actions that appear on hover. The QAO card is mostly the image.
2. **Floating, layered surfaces.** Cards and toolbars float with soft, wide shadows and a hairline inner border, over a translucent and blurred material.
3. **Compact pill controls.** Actions are small rounded pills or circular icon buttons with clear hover and pressed states. Labels are short. Keyboard hints appear in tooltips.
4. **One icon family.** Line icons of one stroke weight, optically aligned, crisp at every scale.
5. **Motion with intent.**
   - Overlays appear **instantly**.
   - Cards **slide in with a spring**.
   - Hover actions **fade in fast**.
   - Dismissals **follow the finger and keep their velocity**.
   - Nothing is animated for its own sake.
6. **Generous but exact spacing** on a consistent grid. Numbers line up (tabular figures).
7. **Polished defaults for output.** Wallpaper and shadow on window shots, balanced padding, good gradients. The *exported image* is beautiful too, not only the UI.

## 2. What Omarchy provides (sources we read; never hard-code)

| Source | Tokens |
|---|---|
| `~/.local/state/omarchy/current/theme/colors.toml` | `accent`, `foreground`, `background`, `cursor`, `selection_*`, `color0`–`color15`, `hyprland_active_border` (often a **two-colour gradient**, a theme's signature), `mode` (light or dark) |
| `…/theme/shell.toml` | Surface tokens with **alpha companions** for `[bar]`, `[popups]`, `[tooltip]`, `[notifications]`, `[launcher]`, `[menu]`; `[controls]` states (normal, hover, focus, selected, pressed: fill alpha, border, width); `[font] base-size`; `[spacing] scale`; border references such as `hyprland.active-border` |
| Hyprland options | `decoration:rounding` + `rounding_power` (Omarchy default **0**; this machine uses 10 and 2.5, a superellipse), `general:gaps_out`, blur settings |
| Fontconfig | `monospace` → JetBrains Mono Nerd Font (the Omarchy chrome font) |
| Themes on disk | `/usr/share/omarchy/themes/*`, `~/.config/omarchy/themes/*`: `colors.toml`, `backgrounds/`, `preview.png`, optional `light.mode` (catppuccin-latte, flexoki-light, rose-pine, lupine and white are light) |
| Live updates | inotify on the theme directory, plus a hook in `~/.config/omarchy/hooks/theme-set.d/` |

## 3. Token mapping

The Rust `snip-core::theme` service resolves tokens and exposes them to QML as a `Theme` singleton. Every value has a fallback, so a broken or partial theme still renders well.

| Snip token | Omarchy source | Fallback | Used by |
|---|---|---|---|
| `surface.panel` (+alpha) | `[popups].background` / `background-alpha` | `background` @ 0.90 | editor toolbars, tool popovers, settings, history strip |
| `surface.card` (+alpha) | `[notifications].background` / alpha | `surface.panel` | QAO cards, toasts, recorder controls pill |
| `surface.tooltip` | `[tooltip]` | `surface.panel` @ 0.97 | tooltips with shortcut hints |
| `scrim` (+alpha) | `[launcher].scrim` / `scrim-alpha` | `background` @ 0.45 | dimming outside the selection and outside the recording area |
| `text.primary` | the section's `text` | `foreground` | labels |
| `text.secondary` | `text.primary` @ 0.62 | — | hints, secondary labels |
| `accent` | `accent` | `color4` | selection border, primary buttons, focus, click ripple, counters' default |
| `border.signature` | `hyprland_active_border` (gradient) | `accent` | panel borders when the theme uses gradient borders; optional selection border style |
| `control.*` (normal, hover, focus, selected, pressed) | `[controls]` | derived from accent | every button, toggle, segmented control |
| `radius.window` | `decoration:rounding` × scale (superellipse with `rounding_power`) | 0 | panels and cards (§5 Q-D2) |
| `radius.control` | `min(radius.window, 6)` | 0 | pills, buttons |
| `font.ui` | **Inter** (OFL, bundled; the closest open match to CleanShot's SF Pro), **tabular figures** for every number (dimensions, coordinates, timers) | system `sans-serif` | all chrome (decided: CleanShot style) |
| `font.mono` | fontconfig `monospace` (the Omarchy font) | — | only the "Mono" and "Mono Box" text-annotation styles and code-like readouts (hex colour values) |
| `font.size.base` | `[font] base-size` | 12 | type scale (rem root) |
| `space.unit` | 4 px × `[spacing] scale` | 4 px | layout grid |
| `shadow.float` | not in themes. Ours, tuned per light or dark mode | dark: `0 12 40 rgba(0,0,0,.45)`; light: `0 10 30 rgba(0,0,0,.18)` | floating cards, pins, window shots |
| `semantic.danger/success/warning` | **chosen from the palette by hue** (nearest to red, green or amber with sufficient contrast) | fixed accessible red, green, amber | delete, recording dot, warnings |

**Why semantic colours are chosen by hue:** ANSI slots are not semantic across themes. In Aetheria, `color1` is **green** (`#c8e967`). Picking "the palette colour closest to red" keeps a stop button red in every theme.

**Contrast guard:** every text and surface pair is checked for WCAG contrast at runtime. Below 4.5:1 (3:1 for large text and icons), the text colour is moved along lightness until it passes. Themes vary widely, so this is required.

**Light themes:** `mode`/`light.mode` switches shadow strength, scrim colour and checkerboard tones. No component has a hard-coded "dark" look.

## 4. Rules for specific surfaces

- **Selection overlay**
  - The scrim uses `scrim`. The border is 1 physical px in `accent`, or optionally `border.signature`.
  - The dimension pill uses `surface.tooltip` with `font.numeric`.
  - The magnifier has a `radius.window` frame and a 1 px pixel grid in `text.primary` @ 0.15.
  - No animation on appear.
- **QAO card**
  - `surface.card` over the screenshot thumbnail, which fills the card, with a `radius.window` clip and `shadow.float`.
  - On hover, a gentle scrim (`scrim` @ 0.35) fades in under the actions over 120 ms.
  - Copy and Save are pills in the middle; Close, Pin and Edit are round icon buttons in the corners.
  - The card slides in from the screen edge with a spring (about 260 ms, slightly underdamped). The newest card shows a 2 px accent indicator.
- **Editor**
  - The canvas sits on a neutral backdrop with a checkerboard under transparency.
  - The toolbar is on top and the bottom bar shows zoom, drag-out and copy.
  - Tools are icon buttons using `control.*` states. The selected tool uses `control.selected`.
  - Tool options open as a popover (`surface.panel`) under the toolbar, never as a modal dialog.
- **Background tool**
  - A right-hand panel with sections: Presets, **Themes (Omarchy)**, Gradients, Wallpapers, Colours, plus layout sliders.
  - The Themes section shows one swatch row per installed theme (signature gradient, palette pairs, wallpapers), headed by the theme's name and preview.
- **Pins:** no chrome until hover. Then close and lock buttons fade in at the corners, and a drag handle shows. `radius.window`, `shadow.float`, plus an optional hairline border.
- **Recorder:** the controls pill uses `surface.card`. The record dot is `semantic.danger`. The timer is in `font.numeric`.
- **Annotation colours are not themed by default.** Screenshots go to other people, and arrows must look right everywhere. Omasnap follows the same rule.
  - The palette is CleanShot's 10 standard swatches plus "My colours".
  - An optional **Theme** row adds the current theme's accent and palette.
  - The default annotation colour is a standard red; "use theme accent" is a setting.
- **Exports:** gradients, wallpapers and shadows are rendered with the shared beautify shader (architecture §6.10) in sRGB-encoded blending, so they match the preview exactly.

## 5. Open design questions

| ID | Question | Options | Recommendation |
|---|---|---|---|
| Q-D1 | **UI font** | — | **Decided 2026-10-06: CleanShot style.** A proportional UI font (Inter, bundled) with tabular figures everywhere; Omarchy monospace only for the Mono annotation styles. A user setting can override the UI font. |
| Q-D2 | **Corners** | — | **Decided 2026-10-06: follow Hyprland `decoration:rounding`** (and `rounding_power`), with an override setting: Follow / Rounded / Square. Default Omarchy (rounding 0) stays square; rounded setups get matching superellipse corners. |
| Q-D3 | **Material** | (a) blur behind panels and cards when the user's shell surfaces are blurred (Snip layer rule `blur` for `snip-*` namespaces); (b) always opaque | **(a)**: match the shell. Opaque is the fallback. |
| Q-D4 | **Icon set** | Lucide (ISC), Tabler (MIT), Phosphor (MIT), or custom | Lucide as the base, 1.5 px stroke, rendered as SVG at device scale. Custom icons for the arrow and redaction styles. |
| Q-D5 | **Sounds** | silent by default / CleanShot-like shutter on | A soft original shutter sound, on by default, plus 2 alternatives and Off |

## 6. How we make sure it is beautiful (process)

1. **Reference board** (internal only, not shipped): CleanShot screenshots of each surface, next to our mock-ups.
2. **Design gallery:** `snipd --gallery` renders every component in every state. It can cycle through **all installed Omarchy themes** and scales (1, 1.25, 1.5, 2).
3. **Visual regression:** CI renders the gallery offscreen for a fixed theme set (dark, light, gradient-border, square, rounded) and compares the result against golden images.
4. **Contrast tests:** every theme on disk passes the contrast guard; failures are reported per token.
5. **Design review gate:** each milestone's exit criteria include a side-by-side review against the reference board for the surfaces it ships.
