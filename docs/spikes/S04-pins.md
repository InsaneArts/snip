# Spike S04: pinned screenshots and lock (click-through) mode (covers S6)

Date: 2026-10-06. Code: [`spikes/s01-foundation/`](https://github.com/InsaneArts/snip/tree/1b9ab47a31056383be0a7b2a2d4506ceb580fa36/spikes/s01-foundation) (removed later; the link is the last commit with it) (`cpp/pinhelper.h`, pin windows in `qml/Main.qml`), driver `qao-test/s6_pin.py`.

All interactive tests ran on a dedicated temporary workspace over a Chromium test page that logs `pointerdown` and `click` events.

## Results

| Check | Result |
|---|---|
| Pin = frameless xdg toplevel titled `snip-pin` + runtime rule (`float`, `pin`, `center`, border 0, `no_shadow`, `no_blur`, `no_anim`, `no_dim`, `opacity "1 1"`, `tag -default-opacity`) | ✅ floating, pinned, centred, 480×270 |
| Stays above a **fullscreen** window | ✅ (screenshot over fullscreen Chromium) |
| Follows to **other workspaces** | ✅ (`pin` moves it to the active workspace) |
| Normal click goes to the pin, not the app below | ✅ |
| **Lock mode on the xdg toplevel** with an empty input region (`wl_surface.set_input_region(empty)` + commit, confirmed in `WAYLAND_DEBUG`) | ❌ **Hyprland 0.56.2 still routes clicks to the toplevel.** It ignores empty input regions on toplevels. |
| **Lock mode as a layer surface** at the same position (overlay layer, anchors top/left, margins = position) with an empty input region | ✅ the pin gets nothing; **Chromium below receives `pointerdown` and `click`** at the pin's centre |

## Decisions for the product (PIN-05)
- **Unlocked pin = xdg toplevel** (movable and resizable by the compositor, appears in window lists, can take keyboard focus).
- **Locked pin = layer surface** with an empty input region, placed at the toplevel's last position through layer-shell margins.
  - It's visible over fullscreen windows and on every workspace of its output, which matches "pinned".
  - Because a locked pin receives no input, **unlocking** happens through the global hotkey "Unlock pins", the bar widget, or `snip pin unlock`. This matches CleanShot, where a locked pin is also inert.
- Swapping windows keeps the image, size and opacity. The position comes from the Hyprland window cache (`snip-hypr`).

## Implementation lessons
1. **`QWindow::setMask` cannot express "no input".** An empty `QRegion` means "no mask" (full input), and regions outside the window are clipped to empty. Set an empty `wl_region` directly on Qt's `wl_surface` (`nativeResourceForWindow("surface")`), then `wl_surface_commit`. An unchanged Qt Quick scene renders no frame, so nothing else would commit it.
2. **Apply input regions only after the native surface exists.** `onVisibleChanged` fires before the layer surface is created; the call silently did nothing until it was deferred.
3. **Harness: always arrive with pointer motion.** Hyprland re-evaluates pointer focus and input regions on motion. Warping to the exact spot the cursor already occupies keeps stale focus, which first looked like "clicks don't work".
4. **Synthetic clicks only on dedicated empty workspaces.** One manual run clicked into the user's active Zen browser on their own workspace. The harness now never clicks outside its own workspaces.

## Open
- Pin resize gestures (Ctrl+scroll, corner drag), opacity by scroll, and arrow-key nudging: straightforward on the toplevel; to be built in M2.
- Exact placement of the locked layer pin on fractional scales: margins are integer *logical* pixels, so at 1.25 a locked pin can shift by up to 1 physical pixel compared with the unlocked toplevel. This is acceptable, but recheck it when building M2.
