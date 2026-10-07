# Snip

CleanShot X's workflow for Omarchy/Hyprland: one daemon (`snipd`, Rust core plus Qt Quick UI) and a CLI (`snip`). Read [docs/00-README.md](docs/00-README.md) before product or design work: it holds the decision log (D-numbers) and the map of the spec, architecture and roadmap. Words used here are defined in [GLOSSARY.md](GLOSSARY.md).

## 1. Think Before Coding

Don't assume. Don't hide confusion. Surface tradeoffs.

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.

## 2. Simplicity First

Minimum code that solves the problem. Nothing speculative.

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that wasn't requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## 3. Surgical Changes

Touch only what you must. Clean up only your own mess.

When editing existing code:

- Don't "improve" adjacent code, comments, or formatting.
- Don't refactor things that aren't broken.
- Match existing style, even if you'd do it differently.
- If you notice unrelated dead code, mention it - don't delete it.

When your changes create orphans:

- Remove imports/variables/functions that YOUR changes made unused.
- Don't remove pre-existing dead code unless asked.

The test: Every changed line should trace directly to the user's request.

## 4. Goal-Driven Execution

Define success criteria. Loop until verified.

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## Workflow

- **Simplicity** is the mindset for every change: the smallest diff that meets the goal, deep modules behind small interfaces.
- **Commit** every fix and every feature as its own small, atomic commit (one logical change; it builds and its tests pass).
- **Pull requests**: write every PR with the `/pr` skill. Split big tasks into several PRs, each reviewable and shippable on its own; one milestone is many PRs.
- **Releases**: every merge to `main` publishes a GitHub release (`.github/workflows/release.yml` runs `scripts/release.sh`): PR titles become the release notes, so write them for users.
- **Bugs**: use `/diagnosing-bugs` and get a red test or reproduction first. **New behaviour with tests**: `/tdd`.
- **Product decisions** belong to the user: ask, then record the answer as the next D-number in `docs/00-README.md` and update the spec. The roadmap's status tables track what is built.

## Build, run, test

- `scripts/dev.sh` builds and (re)starts `snipd` from `build/` as the user unit `snip-dev`, with the user's real data, and loads `hypr/snip.lua` at runtime. `stop` restores Hyprland's own bindings; `log` follows the trace.
- Unit tests: `cargo test -p snip-core -p snip-ipc -p snip-hypr -p snip_bridge`.
- End to end: `tests/e2e/e2e.py` runs in a **nested Hyprland** (a hidden window on a special workspace), so it never changes the user's monitors, pointer, keyboard focus or clipboard and can run while they work. `--only CASE` runs one case. `--live` is the old way (a headless output in the user's session): avoid it, since output hot-plug crashes GTK4 apps like Ghostty.
- UI review: `snipd --gallery --output NAME --out DIR` renders every surface for chosen themes.
- `SNIP_TRACE=1` prints JSON trace lines (timing, sessions, per-frame card motion).

## The live desktop

Development and testing happen on the user's real Hyprland session. Testing there is pre-approved, as long as it stays clean and invisible:

- Run test instances inside the e2e harness's nested Hyprland, isolated with `SNIP_RUNTIME_DIR`, `SNIP_APP_ID`, `SNIP_OUTPUTS` and XDG dirs (the harness does this). Never add or remove outputs in the user's session: GTK 4.22 apps (Ghostty) crash on it.
- Synthetic pointer or keyboard input only while the user is idle (`pointer_idle()`), and never while a fullscreen app or game has focus.
- Afterwards: remove headless outputs, check `hyprctl -j layers` for stray `snip-*` layers, and leave nothing running that draws on screen.
- Ask before pushing, publishing, or editing the user's own config files (`~/.config/hypr`, `~/.config/snip`).

## Gotchas

What cost us hours before:

- **QML scoping**: a property named like an outer `id` shadows it (`qa: qa` silently broke every card position). Name properties differently from ids.
- **QML value types** (`rect`, `point`) are live references: copy the fields before changing state that resets them.
- **Windows** created by a `Loader` never map: create them with `Component.createObject`.
- **QObjects returned to QML** need `CppOwnership`, or the GC deletes Qt's real `QScreen`. Pass the real `QScreen*` to LayerShellQt (`Platform.screen(name)`).
- **Hyprland fades unmapped layers** out for ~150 ms, even with `no_anim`. Clear content first, then unmap. Snip's UI goes transparent and waits for `Platform.notifyNextFrame` before any capture.
- **A `visible` binding that is false for even one instant** unmaps the layer, and Hyprland then fades out a snapshot of it. Change state so the window stays mapped throughout; never reset everything and then re-enable it in the same handler.
- **Physical pixel mapping** belongs in reactive QML (`toPhys` in `Overlay.qml`). A cached C++ call keeps a stale scale from before the first configure.
- **Render loop**: force `QSG_RENDER_LOOP=threaded`, because Qt picks the basic loop on NVIDIA (125 fps cap). Unset `QT_QPA_PLATFORMTHEME` (gtk3 costs ~90 ms at start).
- **cxx-qt**: the bridge crate name must be snake_case (`snip_bridge`) or its headers are not exported. A Rust staticlib carries no native link deps, so link them in CMake (sqlite3).
- **Tracing**: `trace::js()` returns a quoted JSON string, so format it with `{}`.
- **e2e pointer input** goes through Hyprland, so a click's release can arrive after a later `snip debug` state request: after a click or drag, `wait_for` its effect instead of reading the state once.
- **Hyprland window rules** match the *whole* title: `^Snip Pin` misses "Snip Pin 1" (use `^Snip Pin .*`). A floating window can't resize itself either: ask Hyprland (`snip_hypr::resize_window`). The nested e2e Hyprland floats our windows even with no rule, so a test proves a rule matched through a `tag` it sets (`hyprctl` lists it as `name*`).
- **Hyprland 0.56 window rules**: a rule matching `class` and `title` together never matches, nor does a `title` pattern with non-ASCII characters; and when one rule moves a window (`workspace`, `monitor`), `float` from another matching rule is dropped. Use one ASCII field per rule, and put everything a window needs in one rule.
- **Grabbing an item** (`grabToImage`) renders `round(size × dpr)` texels: ask `Platform.grabExact` for exact pixels at a fractional scale.
- **Nested Hyprland on NVIDIA** can't use headless outputs (aquamarine asks for linear buffers); the harness uses the nested window's own output, kept drawing by `render_unfocused` (15 fps).
- **`color` is taken** on a QML `Window`: a custom property with that name fails to compile ("Property value set multiple times").
