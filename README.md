# Snip

CleanShot X's screenshot workflow for [Omarchy](https://omarchy.org) and Hyprland. Snip has an instant frozen selection overlay and floating Quick Access cards. It reads text and QR codes and keeps a searchable capture history. It follows your Omarchy theme and Hyprland's corner rounding.

**Status:** M1 (capture core and Quick Access) is in daily use from source. M2 has started with the editor; backgrounds and pins follow; history UI and scrolling capture in M3; recording in M4 ([roadmap](docs/05-roadmap.md)).

## What works today

- **Capture:**
  - area, fullscreen, previous area;
  - window: hold Space and click for the clean window, with Hyprland's corners and a shadow;
  - Capture Text, which reads OCR and QR codes into the clipboard.
- **Selection overlay:**
  - pixel-exact at any scale, including rotated monitors;
  - crosshair, magnifier with hex colour;
  - Alt (square), Ctrl (from centre), Space while dragging (move); Shift as you let go skips an auto-applied Background preset (CleanShot's keys).
- **Quick Access cards:**
  - hover to frost, then Copy or Save;
  - drag out, context menu, preview, auto-close;
  - CleanShot's motion, measured from its demos.
- **Capture lifecycle:** a capture goes to Snip's history only. Copy puts it on the clipboard. Save writes it to your folder and also copies it. Snip's own UI is never in a capture.
- **Editor:** CleanShot's Annotate tools: arrows (four styles; drag the middle to bend), rectangles, ellipses, lines, text in seven styles with emoji, counters, pencil, a highlighter that snaps to text lines, spotlight, Pixelate/Text shape/Blur/Black out that really hide what is under them, crop and resize, added images and the Background tool; with undo, zoom, Copy, Save, Save As and Drag Me. Open it from a card (pencil or double-click), with `snip edit FILE`, or on whatever image is on the clipboard with `snip edit --clipboard`. Edits go back to the capture and stay editable.
- **History search:** `snip search connection refused` finds captures by the text in them.
- **Scrolling capture:** `snip scroll` (or bind `snip:scroll`). Choose the area, then press Enter and scroll the page yourself, or let Auto-Scroll do it; Enter again finishes. You get one tall capture, stitched by Snip's own stitcher.
- **History window:** `snip history` (or bind `snip:history`) shows past captures across the top of the screen. Type to search their text, Enter restores one to Quick Access, double-click annotates it, drag it out.
- **First start:** a Welcome window walks you through three steps. **Keys:** Snip's keys, with Print Screen's if you tick it; the lines go into hyprland.lua only if you click. **Try it:** your first capture, and what its card does. **Ready:** start at login, the bar icon, `snip` in a terminal. `snip welcome` shows it again.
- **In the launcher:** search "Snip" to open Settings; its actions start each capture and History. Snip starts if it isn't running.
- **In the bar:** Snip's icon sits in Omarchy's bar. A click opens its menu: every capture with its key, History, Settings…, Quit.
- **Settings:** `snip settings` (or bind `snip:settings`) opens CleanShot's settings tabs. They write `~/.config/snip/config.toml` and keep your comments; editing the file by hand works too, live. Shortcuts records keys for every action and binds them in Hyprland.
- **Formats:** PNG, JPG and WebP.

## Install

On Omarchy (Arch), from this checkout:

```sh
scripts/install.sh          # build a pacman package, install it, start Snip now and at every login
scripts/install.sh remove   # uninstall
```

pacman asks for your password and installs what Snip needs. Snip then appears in the launcher and in the bar. Its keys come with Snip's Hyprland module (`/usr/share/snip/hypr/snip.lua`).

## Run it from source

Dependencies (Arch/Omarchy):

```sh
sudo pacman -S --needed rust cmake ninja corrosion qt6-base qt6-declarative qt6-wayland qt6-shadertools layer-shell-qt kcoreaddons sqlite tesseract tesseract-data-eng
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
| SUPER+SHIFT+6 | Edit the image on the clipboard |

The `snip` CLI talks to the running daemon: `snip area`, `snip text`, `snip edit FILE`, `snip copy`, `snip save`, `snip list`, `snip search TEXT`, `snip --help`.

## Releases

Every merge to `main` is a release on GitHub (D33): the next patch version, notes made from the merged pull requests, and the source. `scripts/release.sh minor` (or the Release workflow run by hand) bumps the minor version instead. `snip --version` says which one you run.

## Docs

- [docs/00-README.md](docs/00-README.md): start here (decision log, document map)
- [docs/01-product-spec.md](docs/01-product-spec.md): every feature with its ID
- [docs/02-architecture.md](docs/02-architecture.md): process model, Wayland, Qt, storage

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). The editor's Rounded text styles use [Nunito](https://github.com/googlefonts/nunito) (SIL Open Font License, `app/fonts/OFL-Nunito.txt`).
