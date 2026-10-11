# Studio editing research (2026-10-10)

How Snip could get CleanShot X 5's Studio Mode on Hyprland:
- zooms, automatic and placed by hand;
- a smooth pointer, redrawn after the take, that can be hidden, resized and restyled;
- clicks and keystrokes added after the fact;
- motion blur.

**Status: the plan for milestone M4.5 (D55: Studio Mode now, in 1.0).** The owner's decisions are §14 of the design, Q1–Q14, each with options and a recommendation. Decided so far: Q1 (now, D55); Q2 and Q13 (a Studio switch on the panel, clicks kept for every take, D54); Q6 and Q10 (a zoom grows the card, Replace keeps the original, D56).

Claims are tagged as measured on the reference machine, read in source, read in CleanShot's own app files, documented, or inferred. Paths under `/tmp/claude-1000/…` point to scratch files that no longer exist: harnesses, the offscreen-render prototype, source checkouts. The results are in the report.

| Report | What it answers |
|---|---|
| [design.md](design.md) | What CleanShot X 5.0.1's Studio Mode records and edits (from its app files and walkthrough). What Screen Studio, FocuSee, Cap, OpenScreen and others do, and what their users complain about. How Snip can record the pointer apart on Hyprland: gsr `-cursor no`, `cursorpos` polling, the existing hooks, clock alignment. The input log and the studio edit. One Rust evaluator for every frame. One QML scene for the preview and the offscreen export (148–155 fps at 1440p, measured). The algorithms: pointer spring, zoom ramps, follow, auto zoom, keystrokes, motion blur. Then tests, a phased plan, spikes, risks, and the owner's decisions |
