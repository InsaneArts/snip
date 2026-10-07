# Snip planning docs

**Snip** is the CleanShot X experience for **Omarchy** (Arch + Hyprland). It matches CleanShot's features, UX and UI, **except CleanShot Cloud**. It has a Rust core and a Qt Quick UI, shares no code with Omasnap (D27), and is licensed GPL-3.0-or-later.

Phase: **specification. No product code yet.**
Last updated: 2026-10-06.

## Documents

| Doc | What it answers |
|---|---|
| [01-product-spec.md](01-product-spec.md) | What we build: decisions, principles, about 120 features with IDs and priorities, Linux reinterpretations, keybindings, open product questions |
| [02-architecture.md](02-architecture.md) | How we build it: stack, process model (Rust core plus Qt UI in one daemon), hot paths, data model, subsystems, packaging, the **Omasnap reuse plan** (§10) and the **code layout** (§11) |
| [03-performance.md](03-performance.md) | Latency, throughput and resource budgets, and how we measure them |
| [04-risks-and-edge-cases.md](04-risks-and-edge-cases.md) | Risk register and an edge-case catalogue with expected behaviour |
| [05-roadmap.md](05-roadmap.md) | De-risking spikes S1–S17, then M1–M6: screenshot experience first, recording after |
| [06-design-language.md](06-design-language.md) | How Snip is as beautiful as CleanShot **and** follows Omarchy themes: the token mapping, rules per surface, open design questions |

### Research (source material, with citations)

- [What people ask screenshot apps for (Oct 2026)](research/feature-demand-2026-10.md): demand ranked by GitHub votes across macOS, Linux, Windows and Omarchy, mapped to this spec
These keep the old working name "OpenShotX" in places. They are historical records and are not edited.

| Doc | Contents |
|---|---|
| [research/cleanshot-x-feature-inventory.md](research/cleanshot-x-feature-inventory.md) | CleanShot X 5.0.1: every feature, shortcut, setting and URL-API command. **This is our feature checklist.** |
| [research/platform-feasibility-hyprland.md](research/platform-feasibility-hyprland.md) | Hyprland 0.56.2 protocols and source behaviour, recording pipelines, input, clipboard, DnD, NVIDIA |
| [research/tech-stack-and-prior-art.md](research/tech-stack-and-prior-art.md) | Toolkit comparison with measured start-up costs; lessons from Omasnap, Tensaku, Satty, Flameshot, Spectacle, Kooha and others |
| [research/processing-pipelines.md](research/processing-pipelines.md) | OCR, QR, scroll stitching, encoders, redaction security, GIF, video trimming, beautify rendering, memory, measured on the reference machine |

## Decision log

| # | Decision | Status |
|---|---|---|
| D1 | Product = the CleanShot X experience (UX and UI parity, without Cloud). (The original "built on Omasnap's engine and code" is replaced by D27.) | ✅ decided 2026-10-06 |
| D2 | Name **Snip**; command `snip`, daemon `snipd`; AUR package `snip-screenshot` (the AUR name `snip` is taken) | ✅ decided |
| D3 | Studio Mode **out of v1** | ✅ decided |
| D4 | UI: **Qt 6 Quick/QML** with LayerShellQt | ✅ decided |
| D5 | Language: **Rust core plus C++/QML UI** (cxx-qt, Corrosion); C++-only fallback if spike S13 fails | ✅ decided |
| D6 | Recording default: gpu-screen-recorder KMS, with private-window warnings | ✅ decided |
| D7 | Licence: **GPL-3.0-or-later** | ✅ decided |
| D8 | Priority order: screenshot experience (M1–M3) before recording (M4) | ✅ decided |
| D9 | Onboarding **offers** to take over the PRINT keys (with revert); never silently | ✅ decided |
| D10 | Capture lifecycle: **capture → Snip history only; Copy → clipboard only; Save → export folder + clipboard** (product §5.0) | ✅ decided |
| D11 | Settings UI depth | open: product Q7 (proposal: full CleanShot-style window plus TOML) |
| D12 | Second hotkey press while the overlay is up | open: product Q6 (proposal: cancel) |
| D13 | UI font: **CleanShot style**: proportional (Inter) with tabular figures | ✅ decided |
| D14 | Corners: **follow Hyprland rounding**, with an override (Follow / Rounded / Square) | ✅ decided |
| D15 | Process split (single process vs engine plus UI) | open: arch Q-S2, decided by spike S5 |
| D16 | Supported Hyprland versions | open: arch Q-S7 (proposal: ≥ 0.56) |
| D17 | Second hotkey press while the overlay is up **cancels** it (resolves D12 / product Q6 for M1) | ✅ decided in M1 |
| D18 | Snip's own UI never appears in a capture: cards are drawn **transparent until that frame is presented**, then the capture runs. Unmapping is not enough: Hyprland fades unmapped layers out for ~150 ms even with `no_anim` (measured). Overlays close the same way: content first, unmap 60 ms later | ✅ decided in M1 |
| D19 | OCR languages: English first; retry with the models for the user's **other keyboard layouts** (`input:kb_layout`, e.g. `ge` → `kat`) only when the read is unconfident; keep the more confident result. Mixing all languages at once misreads Latin as Georgian | ✅ decided in M1 |
| D20 | Quick Access keyboard (QAO-03): the stack takes the keyboard (exclusive) only while the pointer rests on it (180 ms dwell), and gives it back on leave | ✅ decided in M1 |
| D21 | Copy on a card closes it (CleanShot behaviour; Alt+Copy keeps it); Save keeps it and shows a Saved state | ✅ decided in M1 |
| D22 | ~~Until Snip's editor (M2), Annotate and double-click open `general.external_editor` (default `tensaku-edit`)~~ Superseded by D28 | ✅ decided in M1 |
| D23 | Area overlay = **free space**: no window highlight or dimming until a drag; a plain click does nothing. Windows are picked **only while Space is held** (click = clean window capture); Space held during a drag moves the selection. Replaces "click captures the window or monitor" | ✅ decided 2026-10-07 |
| D25 | From the feature-demand research: **in** — edit an existing or clipboard image (ANN-19, M2), video/GIF recording (stays M4), cursor on/off (CAP-11 → P0), QR (OCR-03), searchable history (HIS-05). **Not now** — HDR/10-bit capture, pixel ruler, sharing to the user's own upload target | ✅ decided 2026-10-07 |
| D27 | **Snip is independent of Omasnap.** No Omasnap code is copied, ported or translated; every part (including the scrolling-capture stitcher, scroll injection and the edit-document format) is Snip's own design. Omasnap is the main competitor (Omarchy's default since 2026-09-21); reading its public feature list for competitive analysis is fine | ✅ decided 2026-10-07 |
| D26 | Annotation keeps CleanShot's **Pixelate/Blur tool** (drag an area to hide it; Pixelate default, Blur, Black out), secure by construction. **No auto-redact** (automatic detection of sensitive text) | ✅ decided 2026-10-07 |
| D28 | Annotate, the card's pencil and double-click open **Snip's own editor**. `general.external_editor` (default empty) still sends captures to another editor for those who want one. Edits go back to the capture's history entry automatically; the capture keeps its original and its objects, so they stay editable (ANN-15/16) | ✅ decided 2026-10-07 |
| D29 | SUPER+SHIFT+6 opens the clipboard's image in the editor (`snip:edit-clipboard`), next to the 3/4/5 capture keys; it replaces Omarchy's "move window to workspace 6" as 3/4/5 do | ✅ decided 2026-10-07 |
| D30 | Pixelate gains a **Text shape** style (product owner: "we can create text shaped blocks as well"): pixelated text in CleanShot's look. Unlike Pixelate it reads inside the region, but only the paper colour, one ink colour, the line height and how much ink each block holds (none, some, much). Blocks are just over half the line height, so word shapes and roughly where tall letters are show; letters don't (at a third of the line, 19 px text was readable). Pixelate stays the default and shows nothing of the content | ✅ decided 2026-10-07 |
| D31 | Project files use the **`.snip`** extension (a stored zip: original, objects, their files, preview, thumbnail; architecture §5.3) | ✅ decided 2026-10-07 |
| D32 | **CleanShot's capture keys**: Shift held when a selection ends (or a window is clicked) **skips an auto-applied Background preset** (and still inverts a window shot's background); **Alt squares** the selection; resizing **from the centre moves to Ctrl** (free while dragging). Hotkeys that include Shift (SUPER+SHIFT+3/4/5) don't count: only Shift held at the release | ✅ decided 2026-10-07 |
| D33 | **Releases**: every merge to `main` is a GitHub release (a patch version bump, notes generated from the merged PRs, the source archive; no binaries). The AUR comes later, when the repository is public | ✅ decided 2026-10-07 |
| D34 | **Next**: the history window, the settings window with a shortcut recorder, then scrolling capture. Screen recording waits for its own design pass | ✅ decided 2026-10-07 |
| D35 | **Snip lives in the bar**, as CleanShot lives in the macOS menu bar (product owner: "the snip need to live in the menu bar like the screenshots is doing on the Mac OS"). It is a tray icon with Snip's logo; omarchy-shell hosts the tray (StatusNotifierItem). A click opens CleanShot's menu: the captures with their keys, Open History…, Restore Recently Closed, Settings…, Quit Snip (OMA-08). This replaces the spec's "Omarchy has no tray-first model" | ✅ decided 2026-10-07 |
| D36 | **Snip becomes a product** (M3.5, planned, not built yet): its own logo and icon set, the bar icon and menu (D35), "Snip" in the launcher (OMA-09), and an easy install. The AUR and a landing page come when the repository goes public (D33). The CLI stays complete for scripts and agents: every feature remains a `snip` command (API-01) | ✅ decided 2026-10-07 (plan only) |
| D24 | Copy also offers the **file path as text** (`clipboard.text_path`, default on): pasting into terminals (SUPER+V in Ghostty sends a text paste) or plain fields gives the path instead of nothing; image-aware apps still get the image | ✅ decided 2026-10-07 |
