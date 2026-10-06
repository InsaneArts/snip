# Snip

CleanShot X's screenshot workflow for [Omarchy](https://omarchy.org) and Hyprland. Snip has an instant frozen selection overlay and floating Quick Access cards. It reads text and QR codes and keeps a searchable capture history. It follows your Omarchy theme and Hyprland's corner rounding.

**Status:** M1 (capture core and Quick Access) is in daily use from source. The editor, backgrounds and pins come in M2; history UI and scrolling capture in M3; recording in M4 ([roadmap](docs/05-roadmap.md)).

## What works today

- **Capture:**
  - area, fullscreen, previous area;
  - window: hold Space and click for the clean window, with Hyprland's corners and a shadow;
  - Capture Text, which reads OCR and QR codes into the clipboard.
- **Selection overlay:**
  - pixel-exact at any scale, including rotated monitors;
  - crosshair, magnifier with hex colour;
  - Shift (square), Alt (from centre), Space while dragging (move).
- **Quick Access cards:**
  - hover to frost, then Copy or Save;
  - drag out, context menu, preview, auto-close;
  - CleanShot's motion, measured from its demos.
- **Capture lifecycle:** a capture goes to Snip's history only. Copy puts it on the clipboard. Save writes it to your folder and also copies it. Snip's own UI is never in a capture.
- **History search:** `snip search connection refused` finds captures by the text in them.
- **Formats:** PNG, JPG and WebP. Live config in `~/.config/snip/config.toml`.

## Run it from source

Dependencies (Arch/Omarchy):

```sh
sudo pacman -S --needed rust cmake ninja corrosion qt6-base qt6-declarative qt6-wayland layer-shell-qt kcoreaddons sqlite tesseract tesseract-data-eng
```

Then:

```sh
scripts/dev.sh          # build, run snipd as a user unit, bind SUPER+SHIFT+3/4/5
scripts/dev.sh stop     # stop it and restore Hyprland's own bindings
```

| Keys | Action |
|---|---|
| SUPER+SHIFT+3 | Capture fullscreen |
| SUPER+SHIFT+4 | Capture area (hold Space for a window) |
| SUPER+SHIFT+5 | All-In-One (area for now) |

The `snip` CLI talks to the running daemon: `snip area`, `snip text`, `snip copy`, `snip save`, `snip list`, `snip search TEXT`, `snip --help`.

## Docs

- [docs/00-README.md](docs/00-README.md): start here (decision log, document map)
- [docs/01-product-spec.md](docs/01-product-spec.md): every feature with its ID
- [docs/02-architecture.md](docs/02-architecture.md): process model, Wayland, Qt, storage

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
