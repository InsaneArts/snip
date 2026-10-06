# Spike S08: text input for annotations (covers S4)

Date: 2026-10-06. Code: `--textinput [toplevel|layer]` in the spike, driver `qao-test/s4_text.py`.

## Setup
- A QML `TextEdit` in (a) an xdg toplevel (where the editor's text tool lives) and (b) an overlay layer surface with **exclusive** keyboard focus (where All-In-One and size fields live).
- The session: fcitx5 running with only the `keyboard-us` input method, `QT_IM_MODULE=fcitx`, and Hyprland layouts `us,ge`.

## Results

| Input path | xdg toplevel | Layer surface (exclusive) |
|---|---|---|
| Latin text through a virtual keyboard (`wtype "Hello Snip 123"`) | ✅ exact | ✅ exact |
| Unicode through a virtual keyboard: Georgian `ქართული`, `✓`, `é`, emoji `😀` | ✅ exact | ✅ exact |
| Georgian **layout** (`hyprctl switchxkblayout all 1`) plus keys sent by Hyprland's `send_shortcut` with Georgian keysyms | ⚠️ arrived as Latin `abgd` | ⚠️ same |
| IME preedit events | none observed (`keyboard-us` has no preedit) | none |

**Why row 3 is a test-method problem, not a Snip finding:** the result was identical with `QT_IM_MODULE=fcitx`, `wayland` and unset. So it isn't Qt's IM module; Hyprland's synthetic `send_shortcut` key doesn't carry the switched layout group to the client. The real path (physical keyboard → Hyprland → fcitx5 input-method grab → app) can't be reproduced without a uinput device.

## Manual check requested (10 seconds)
1. In `spikes/s01-foundation`, run: `./build/snip-spike --textinput`
2. A small window titled `snip-text` appears.
3. Switch to the Georgian layout as usual and type a few letters, then try `--textinput layer`.
4. Expected: Georgian letters appear in both. If Latin appears, Snip must pass the layout group through (or depend on fcitx5's layout handling) before M2.

## Conclusions
- Text entry into Qt Quick `TextEdit` works on both the toplevel and the exclusive layer surface, including non-Latin Unicode and emoji. Annotation text can be edited in place on either surface type.
- CJK IME composition (preedit) is still untested: no Pinyin or Mozc engine is configured here. It's a P1 check before release for users of those IMEs.
