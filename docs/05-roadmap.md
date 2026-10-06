# Roadmap

Status: **DRAFT**, 2026-10-06.

- Milestones are sized S (under a week), M (1–3 weeks), L (3–6 weeks) or XL (over 6 weeks) of focused work. They are not dated until the team and scope are decided.
- Feature IDs refer to [01-product-spec.md](01-product-spec.md).
- **Every milestone ends with a usable build that someone uses every day.**

## M0: De-risking spikes (M, about 2 weeks; throwaway code)

Each spike answers one question with a measurement and a go/no-go. The results are written up in `docs/spikes/Sx-*.md` and may change the architecture doc.

| ID | Question | Pass criterion | Blocks |
|---|---|---|---|
| S1 | Can a warm Qt Quick daemon show a frozen, per-output overlay fast enough? | p95 ≤ 35 ms from hotkey to presented frame (wp_presentation) on DP-2, plus a cold number | stack |
| S2 | Is the overlay pixel-exact at fractional scales? | round-trip diff == 0 at 1, 1.25, 1.5, 1.6̅, 2; transform 1 | stack |
| S3 | Does drag-out from a non-exclusive layer surface work into Firefox, Chromium, Nautilus, a terminal, and a Flatpak app? | matrix filled in; COPY action respected | QAO design |
| S4 | Does IME work on our surfaces (fcitx5 preedit, dead keys, compose) in QML `TextEdit` on Hyprland 0.56? | works in an xdg toplevel; layer-surface behaviour documented | editor |
| S5 | Is a long-lived Qt Quick process stable on NVIDIA? | 72 h soak: no crash, RSS drift ≤ 10 MB, fd drift 0 | stack, Q-S2 |
| S6 | Do empty input regions on xdg toplevels give click-through on Hyprland? | a locked pin passes clicks through | PIN-05 |
| S7 | gpu-screen-recorder as an engine: `-ipc` control, region snapping, an external `pkill`, a daemon crash and re-attach, suspend, private-window detection | every case handled; drops = 0 at 3440×1440@60 | REC |
| S8 | Annotation renderer: Qt Quick nodes vs Qt Canvas Painter (6.11 tech preview) | preview == export (pixel diff ≤ 1 LSB); 144 fps with 200 objects | ANN |
| S9 | Keystroke and click capture through the Hyprland Lua hook | keys and clicks arrive within 5 ms; `non_consuming` mouse bind doesn't break drags; the watchdog removes hooks after kill -9 | REC-09/10 |
| S10 | Clipboard paste matrix: `image/png` and file-mode offers into Chromium, Firefox, Slack, Discord, VS Code, GIMP, LibreOffice, Telegram, kitty/alacritty | matrix filled in; the default chosen from data | SET-07 |
| S11 | Scroll stitching on a real corpus (Firefox, Chromium, Ghostty, Nautilus, Electron) at scale 1.25, including XWayland | ≥ 95 % of seams correct; sticky headers handled | SCR |
| S12 | OCR bake-off on a real screenshot corpus (UI, code, dark, Georgian, Russian, CJK): Tesseract fast with preprocessing vs PP-OCRv5 ORT | error rates recorded; default confirmed | OCR |
| S13 | Rust core plus Qt UI via cxx-qt and Corrosion: build, QML list models defined in Rust, memfd frames shared zero-copy, debugging | builds in CI and from an AUR PKGBUILD; the QAO model is driven from Rust | Q-S1 |
| S14 | ext-data-control clipboard ownership from the daemon, with Omarchy's clipboard manager watching | paste works; history behaviour with and without the sensitive hint | SET-07b |
| S15 | Hide-desktop-clutter cover: does a wallpaper slice (PreserveAspectCrop, same file as omarchy-shell) line up pixel-exactly over the bar at scale 1.25 and with a multi-monitor wallpaper? | diff ≤ 1 LSB outside wallpaper transitions; no window re-layout | CAP-12 |
| S16 | Omasnap stitcher vendored as C++ behind `cxx`, driven from Rust, checked with Omasnap's `stitch-replay` corpus | replay results identical to upstream Omasnap | SCR, §10 reuse plan |
| S17 | Theme tokens: resolve every installed Omarchy theme (26 or more) into Snip tokens, with the contrast guard; render the QAO card and the toolbar in all of them | no contrast failures after the guard; screenshots reviewed | design language |

**Exit:** the decided stack (Qt Quick + Rust core) is confirmed by S1, S2, S5 and S13, or the fallback is triggered with data. Q-S2 (process split) is settled.

## M1: Capture core and the Quick Access experience (L)

**Scope:**
- Daemon, systemd unit, global shortcuts, `snip` CLI (API-01, API-03)
- Capture: CAP-01 (area), CAP-02, CAP-03, CAP-04, CAP-05 (Space then click: clean window capture), CAP-09 basic, CAP-10
- Selection: SEL-01 to SEL-07, SEL-09 to SEL-11 (crosshair and magnifier included)
- QAO: QAO-01 to QAO-11 (stacking, hover actions, drag-and-drop out, gestures, context menu)
- **Capture lifecycle (product §5.0):** the history store as the default home (HIS-02 retention, HIS-04), Copy → clipboard only, Save → export folder plus clipboard; the history *browser UI* comes in M3
- Clipboard (SET-07), save with naming (SET-03 to SET-06; After Capture defaults: only "Show QAO" on), sounds (SET-09)
- Theme tokens and the design gallery (OMA-01, design doc §6); Lua module (OMA-02); OMARCHY_SCREENSHOT_EDITOR handoff (OMA-06)

**Exit:**
- All latency budgets for these paths are met.
- The developer uses it as their only screenshot tool for a week.
- Two-monitor, mixed-scale test passes.
- The QAO card passes design review against the CleanShot reference board in a dark, a light and a gradient-border theme.

## M2: Editor, Background tool, pins (L)

**Scope:**
- The full annotate editor: ANN-01 to ANN-18, except ANN-06b
- Background tool: BG-01 to BG-06, **including Omarchy theme backgrounds**
- Window shots "With wallpaper" and "Transparent" with shadow (CAP-05 options, BG-04)
- Pins: PIN-01 to PIN-08
- Project format v1, possibly adopting Omasnap's op-log model

**Exit:**
- Preview == export.
- IME works.
- Redaction security tests pass.
- The Background tool renders every installed theme.
- Design review of the editor, Background tool and pins.

## M3: The rest of the screenshot experience (L)

**Scope:**
- Capture history UI (HIS-01 to HIS-03)
- Scrolling capture (SCR-01 to SCR-05), on the vendored Omasnap stitcher
- Self-timer (CAP-06), cursor in captures (CAP-11)
- Hide desktop clutter (CAP-12)
- OCR complete (OCR-01 to OCR-06, including QR, languages and Georgian)
- Settings window (SET-02); shortcut recorder (SET-08)
- Capture-menu entries and file handlers (OMA-03, OMA-07)

**Exit:**
- **Every P0 item in the product spec is done.**
- A feature-by-feature walk-through against the CleanShot inventory (screenshot areas) passes.
- **First public release: Snip 0.x, "the screenshot experience".**

## M4: Recording suite and remaining parity (XL)

**Scope:**
- Recording: REC-01 to REC-15, including webcam, clicks, keystrokes, GIF, safety and privacy-safe mode as decided
- Trimmer and export: VED-01 to VED-03
- Bar widget (OMA-04)
- All-In-One (CAP-07), adjustable selection (SEL-08)

**Exit:**
- 10-minute recording at 3440×1440@60 with zero drops.
- External stop, crash, suspend and lock are all handled.
- Every P1 item is done or explicitly waived.

## M5: 1.0 hardening (M)

- Multi-monitor matrix: 1, 2 and 3 outputs; mixed scales; rotation; hotplug.
- Intel and AMD laptop reference passes all budgets.
- Soak tests pass.
- Fuzzing of every parser is clean.
- AUR `snip-screenshot`, onboarding, docs, THIRD_PARTY.md (including Omasnap's MIT notice).
- **Snip 1.0 = CleanShot parity, without Cloud and Studio Mode.**

## M6: After 1.0

- Studio Mode (VED-04, VED-05; decided to be out of v1).
- Redaction suggestions (ANN-06b), URL scheme (API-02).

## Dependencies between milestones

```
M0 spikes ──▶ M1 capture + QAO ──▶ M2 editor/background/pins ──▶ M3 screenshot parity ──▶ M4 recording ──▶ M5 1.0 ──▶ M6
                       └────────────────────────────────────── (M4 can start after M1 with a second person) ─┘
```

The screenshot experience (M1–M3) comes first, as decided. Recording (M4) depends only on M1's daemon, overlay, theme and QAO layers, so a second contributor can start it in parallel.
