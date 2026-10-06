# Spike S01: foundation (throwaway)

Rust core (own Wayland connection: ext-image-copy-capture, hyprland global shortcuts,
ext-data-control, ext-idle-notify) + cxx-qt bridge + Qt Quick/LayerShellQt overlay.
Results: [docs/spikes/S01-foundation.md](../../docs/spikes/S01-foundation.md).

```sh
cmake -B build -G Ninja -DCMAKE_BUILD_TYPE=Release && cmake --build build

# runtime-only Hyprland setup (gone on config reload)
hyprctl eval 'hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })'
hyprctl eval 'SNIPSPIKE_BIND = hl.bind("SUPER + ALT + PRINT", hl.dsp.global("snipspike:area"))'

./build/snip-spike              # interactive: SUPER+ALT+PRINT, drag, release = copy; Esc cancels
./build/snip-spike --selftest   # capture, show, recapture, pixel diff, copy test
./bench.sh 30                   # warm hotkey latency via the real global-shortcut path
./build/snip-spike --soak       # idle-gated soak (used by the snip-soak user unit)
```

Set `QT_FORCE_STDERR_LOGGING=1` to see QML warnings on stderr.
