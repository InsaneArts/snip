# Spike S01: foundation (throwaway)

Rust core (own Wayland connection: ext-image-copy-capture, hyprland global shortcuts,
ext-data-control, ext-idle-notify) + cxx-qt bridge + Qt Quick/LayerShellQt overlay.
Results: [docs/spikes/S01-foundation.md](../../docs/spikes/S01-foundation.md).

```sh
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build

# runtime-only Hyprland setup (gone on config reload)
hyprctl eval 'hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })'
hyprctl eval 'hl.unbind("SUPER + SHIFT + code:12")'   # Omarchy: move window to workspace 3
hyprctl eval 'hl.unbind("SUPER + SHIFT + code:13")'   # Omarchy: move window to workspace 4
hyprctl eval 'SNIPSPIKE_FULL = hl.bind("SUPER + SHIFT + code:12", hl.dsp.global("snipspike:fullscreen"), { description = "Snip: capture fullscreen" })'
hyprctl eval 'SNIPSPIKE_AREA = hl.bind("SUPER + SHIFT + code:13", hl.dsp.global("snipspike:area"), { description = "Snip: capture area" })'
# undo everything: hyprctl reload

./build/snip-spike              # SUPER+SHIFT+4: drag, release = copy (Esc cancels); SUPER+SHIFT+3: fullscreen -> clipboard
./build/snip-spike --selftest   # capture, show, recapture, pixel diff, copy test
./bench.sh 30                   # warm hotkey latency via the real global-shortcut path
./build/snip-spike --soak       # idle-gated soak (used by the snip-soak user unit)
```

Set `QT_FORCE_STDERR_LOGGING=1` to see QML warnings on stderr.
