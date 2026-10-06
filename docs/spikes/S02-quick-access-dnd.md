# Spike S02: Quick Access card, drag-and-drop and paste (covers S3, S10, S14)

Date: 2026-10-06. Code: `spikes/s01-foundation/` (shared spike app), plus `qao-test/`. Machine: Hyprland 0.56.2, DP-2 at 1.25.

## What was built
- **QAO card prototype:**
  - a layer surface, `snip-qao`, bottom-left, keyboard `None`, non-exclusive;
  - thumbnail, hover scrim, Copy and Save pills, close button;
  - drag-out through a C++ `DragHelper` (`QDrag`, **COPY action only**). It can offer several MIME sets: `uri`, `png`, `portal`, `utf8`, `all`.
- **Captures now follow the decided lifecycle** (product §5.0): area or fullscreen capture → stored file → card. Nothing goes to the clipboard until Copy.
- **Rust core:** a `zwlr_virtual_pointer_v1` synthetic pointer (also the basis of auto-scroll) and a line-based control socket, `$XDG_RUNTIME_DIR/snip-spike.sock` (a `ctl.sock` prototype).
- **Test driver `qao-test/dnd_matrix.py`:**
  1. Opens each real app **on its own temporary workspace**, with fresh profiles for Chromium, Zen and VS Code.
  2. Drags the card into it with the virtual pointer, and pastes with Hyprland `send_shortcut`.
  3. Reads back what arrived: a local results page for the browsers, files on disk for Nautilus and VS Code, raw pty bytes for the terminals.
  4. Restores the user's workspace.

## Results: everything works with the right MIME set

| App | Drop: `text/uri-list` | Drop: + `image/png` | Drop: + portal filetransfer | Paste (`image/png`) |
|---|---|---|---|---|
| Chromium 1xx (Wayland) | ✅ arrives as a real `File` (image/png, 1600×900, decodes) | ✅ | ✅ | ✅ `image.png` 1600×900 |
| Zen (Firefox) | ✅ same | ✅ | ✅ | ✅ |
| Nautilus | ✅ copied **with the real filename** | ⚠️ **saves "Dropped Image.png"** (prefers the raw image, loses the name) | ✅ real filename | ✅ creates "Pasted image.png" |
| VS Code 1.138 | ✅ opens the image | ✅ | ✅ | ✅ saves `image.png` into the workspace |
| Ghostty 1.3.1 | ✅ path inserted (bracketed paste) | ✅ | ✅ | — (an image-only clipboard has no text) |
| kitty | ✅ path inserted | ✅ | ✅ | — |
| foot | ✅ path inserted, shell-quoted | ✅ | ✅ | — |

In every case **the source file still existed after the drop**, so COPY was honoured and no app moved the capture away.

Not automated, on purpose: **Slack and Discord**, because a stray keystroke could send a real message, and Obsidian (vault set-up). They are Electron, which uses the same Chromium Ozone/Wayland drop and paste code that passed above. Check them by hand once before release.

## Findings that change the product

1. **The drag MIME set is `text/uri-list` + `application/vnd.portal.filetransfer`, and *not* `image/png`.**
   - Offering raw `image/png` makes Nautilus discard the file reference and save a nameless "Dropped Image.png".
   - Browsers, Electron, VS Code and terminals all accept the file URI.
   - The portal type (KF6 `KUrlMimeData::exportUrlsToPortal`) is harmless and covers Flatpak receivers.
2. **Clipboard Copy = `image/png` only** works everywhere it should: Chromium, Zen, Nautilus, VS Code. Confirms SET-07.
3. **Qt's `QDrag::exec()` returned `IgnoreAction` for every drop, including ones the target accepted.** CleanShot's "Close after dragging" therefore cannot rely on the drag result.
   - Follow-up: use the Wayland source events (`dnd_drop_performed` / `dnd_finished`, which Qt does not expose), or a heuristic (drop ended over another client's surface).
   - Tracked as open item S2-1.
4. **Ghostty's "Potentially Unsafe Paste" dialog appears when the foreground app has not enabled bracketed paste.**
   - Snip can't change this, and it's correct Ghostty behaviour.
   - Normal shells and TUI apps (zsh, bash, Claude Code) enable bracketed paste and get the path inserted directly.
   - The harness first mistook this for "Ghostty rejects drops". A Nautilus-to-Ghostty control drag showed the dialog.
5. **Sensitive clipboard hint verified:** a copy offered with `x-kde-passwordManagerHint` is **not** stored in Omarchy's clipboard history (`~/.local/state/omarchy/clipboard-images/`); the same copy without it **is**. SET-07b works as designed.
6. **Clipboard watchers:** every offer is read by 3 or more readers within about 1 ms (Omarchy's two `wl-paste --watch` processes plus any other watchers). Concurrent `send` handling must be non-blocking, which is how the spike does it.

## Harness lessons (for the real test suite)
- Give each target app a **full-size window on its own workspace**. In a shared tiled layout, narrow windows put the "centre" on sidebars (Zen's tab bar, VS Code's explorer) and the drops looked like failures.
- Fresh app profiles need first-run dialogs suppressed: VS Code `chat.disableAIFeatures` plus Escape; Zen prefs in `user.js`.
- Terminals need a payload reader that enables bracketed paste (`printf '\e[?2004h'`), as a shell does.
- **Never use `pkill -f` with a pattern that appears in the calling command line.** It kills the caller. Use `pgrep -x` plus a `/proc/PID/cmdline` check.
- Ghostty launched with `-e` runs as its own process. The user's single-instance Ghostty is a different PID, so test windows can be killed safely, but check the PID first.

## Open items
- **S2-1:** a reliable "drop completed" signal for "Close after dragging".
- **Card visuals:** rounded-corner clipping of the thumbnail (needs `layer.enabled` + MultiEffect mask), the shadow, theme tokens (S17), stacking, auto-close timer, swipe gestures.
- **Input region:** limit the card window's input region to the visible card. The transparent shadow margin currently intercepts clicks.
