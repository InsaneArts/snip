# Glossary

The shared language of this project. Use these words in code, commits, PRs and conversation.

- **Capture**: one screenshot (or, from M4, recording). Born in **history**; the user's folders only see it on **Save**.
- **Capture lifecycle**: capture → history only; **Copy** → clipboard only; **Save** → export folder and clipboard (decision D10).
- **History**: Snip's private store of every capture (`~/.local/share/snip/history`, SQLite index with full-text search of the text in each capture).
- **Card**: a Quick Access card, the floating thumbnail after a capture. The **stack** is all cards; the newest is on top. CleanShot calls it the Quick Access Overlay (**QAO**).
- **Frost**: a card's hover look, the blurred, tinted thumbnail under the Copy/Save buttons.
- **Status**: a result drawn on a frosted card ("Copied", "Text copied", "No text found").
- **Overlay**: the full-screen selection surface, one per **output** (monitor). Its **session** lasts from the hotkey to finish or cancel.
- **Frozen frame**: the capture the overlay shows, taken before the overlay maps.
- **Window mode**: picking windows in the overlay, while Space is held (or `snip window`).
- **Clean window capture**: a window's own buffer (toplevel capture), never occluded.
- **Physical px / logical px**: buffer pixels vs Hyprland layout pixels (scale 1.25: 1 logical = 1.25 physical). Selections are held in physical px of the **upright** frame (output transform undone).
- **snipd / snip**: the daemon (Rust core plus Qt Quick UI, one process) / the CLI that talks to it over `ctl.sock`.
- **Engine**: the Rust QObject the QML talks to (`crates/snip-bridge`).
- **Take**: one run of the screen recorder, from its start to a finished file (`snip_core::take`): gpu-screen-recorder, or ffmpeg's test pattern in tests. A **recording** is a take plus what Snip turns on around it (the HUD, Do Not Disturb, the desktop cover, the click and key hooks).
- **Connection A / B**: Snip's own Wayland connection (capture, clipboard, shortcuts) / Qt's (all surfaces).
- **Theme tokens**: colours, radius and fonts resolved from the current Omarchy theme (`Theme.qml`).
- **Test instance**: an isolated snipd (own XDG dirs, `SNIP_RUNTIME_DIR`, `SNIP_APP_ID`, `SNIP_OUTPUTS`) on a temporary headless output.
- **D-number / M-number / spec ID**: a decision in `docs/00-README.md` / a milestone in `docs/05-roadmap.md` / a feature in `docs/01-product-spec.md` (`CAP-01`, `QAO-04`…).
