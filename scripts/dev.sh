#!/usr/bin/env bash
# Dev loop: build Snip from source and use it as your screenshot tool.
#
#   scripts/dev.sh          build, (re)start snipd from build/, load hypr/snip.lua
#   scripts/dev.sh stop     stop it; `hyprctl reload` gives Hyprland its own bindings back
#   scripts/dev.sh log      follow the daemon's log
#   scripts/dev.sh status   is it running, which keys point at it
#
# snipd runs as the transient user unit `snip-dev` (journalctl --user -u snip-dev)
# with your real data: history in ~/.local/share/snip, Save to the export
# folder in ~/.config/snip/config.toml. Nothing is installed and no config
# file is edited: the bindings are applied at runtime from the repo's snip.lua.
set -euo pipefail
cd "$(dirname "$0")/.."
UNIT=snip-dev

bind_keys() {
  local out
  out=$(hyprctl eval "dofile(\"$PWD/hypr/snip.lua\")")
  [[ $out == ok* ]] || { echo "dev.sh: loading hypr/snip.lua failed: $out" >&2; return 1; }
}

case "${1:-run}" in
  run | restart)
    [[ -f build/build.ninja ]] || cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
    ninja -C build
    systemctl --user stop "$UNIT" 2>/dev/null || true
    systemctl --user reset-failed "$UNIT" 2>/dev/null || true
    systemd-run --user --quiet --unit="$UNIT" --collect \
      -p Restart=on-failure -p RestartSec=1 -p LimitCORE=0 \
      --setenv=WAYLAND_DISPLAY="${WAYLAND_DISPLAY:?}" \
      --setenv=XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:?}" \
      --setenv=HYPRLAND_INSTANCE_SIGNATURE="${HYPRLAND_INSTANCE_SIGNATURE:?}" \
      --setenv=SNIP_TRACE=1 --setenv=QT_FORCE_STDERR_LOGGING=1 \
      "$PWD/build/snipd"
    for _ in $(seq 50); do build/snip ping >/dev/null 2>&1 && break; sleep 0.1; done
    build/snip ping >/dev/null || { echo "dev.sh: snipd did not start; see: $0 log" >&2; exit 1; }
    bind_keys
    echo "snipd running from build/ (unit $UNIT). SUPER+SHIFT+3 fullscreen, +4 area, +5 all-in-one."
    ;;
  stop)
    systemctl --user stop "$UNIT" 2>/dev/null || true
    hyprctl reload >/dev/null
    echo "stopped; Hyprland bindings restored from your config"
    ;;
  log) exec journalctl --user -u "$UNIT" -f -o cat ;;
  status)
    systemctl --user --no-pager --lines=0 status "$UNIT" || true
    hyprctl binds -j | python3 -c '
import json, sys
for b in json.load(sys.stdin):
    if b["description"].startswith("Snip"): print("  bound:", b["description"])'
    ;;
  *) sed -n '2,13p' "$0"; exit 2 ;;
esac
