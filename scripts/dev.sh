#!/usr/bin/env bash
# Dev loop: build Snip from source and use it as your screenshot tool.
#
#   scripts/dev.sh          build, (re)start snipd from build/, load hypr/snip.lua
#   scripts/dev.sh stop     stop it, also at login; `hyprctl reload` gives Hyprland its own bindings back
#   scripts/dev.sh log      follow the daemon's log
#   scripts/dev.sh status   is it running, which keys point at it
#
# snipd runs as the user unit `snip-dev` (journalctl --user -u snip-dev), which
# starts with every login (graphical-session.target) until `stop`, with your
# real data: history in ~/.local/share/snip, Save to the export folder in
# ~/.config/snip/config.toml. Nothing is installed but the unit file and the
# launcher entry ("Snip", from app/snip.desktop, pointing at build/), and no
# config file is edited: the bindings are applied at runtime from the repo's
# snip.lua (load it from hyprland.lua to have them at login too).
set -euo pipefail
cd "$(dirname "$0")/.."
UNIT=snip-dev
UNIT_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/$UNIT.service"
DESKTOP_FILE="${XDG_DATA_HOME:-$HOME/.local/share}/applications/snip.desktop"

# the launcher's "Snip" (OMA-09), running this checkout's build
write_desktop() {
  mkdir -p "$(dirname "$DESKTOP_FILE")"
  sed -e "s|^Exec=snip |Exec=$PWD/build/snip |" -e "s|^Icon=snip$|Icon=$PWD/app/icons/snip.png|" app/snip.desktop >"$DESKTOP_FILE"
}

write_unit() {
  mkdir -p "$(dirname "$UNIT_FILE")"
  cat >"$UNIT_FILE" <<EOF
# Written by $PWD/scripts/dev.sh: Snip from the source checkout.
[Unit]
Description=Snip (snipd from $PWD/build)
PartOf=graphical-session.target
After=graphical-session.target

[Service]
ExecStart=$PWD/build/snipd
Environment=SNIP_TRACE=1 QT_FORCE_STDERR_LOGGING=1
Restart=on-failure
RestartSec=1
LimitCORE=0

[Install]
WantedBy=graphical-session.target
EOF
  systemctl --user daemon-reload
}

bind_keys() {
  local out
  out=$(hyprctl eval "dofile(\"$PWD/hypr/snip.lua\")")
  [[ $out == ok* ]] || { echo "dev.sh: loading hypr/snip.lua failed: $out" >&2; return 1; }
}

case "${1:-run}" in
  run | restart)
    [[ -f build/build.ninja ]] || cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo
    ninja -C build
    write_unit
    write_desktop
    systemctl --user enable --quiet "$UNIT"
    systemctl --user reset-failed "$UNIT" 2>/dev/null || true
    systemctl --user restart "$UNIT"
    for _ in $(seq 50); do build/snip ping >/dev/null 2>&1 && break; sleep 0.1; done
    build/snip ping >/dev/null || { echo "dev.sh: snipd did not start; see: $0 log" >&2; exit 1; }
    bind_keys
    echo "snipd running from build/ (unit $UNIT, starts at login). SUPER+SHIFT+3 fullscreen, +4 area, +5 all-in-one."
    ;;
  stop)
    systemctl --user disable --now --quiet "$UNIT" 2>/dev/null || true
    rm -f "$DESKTOP_FILE"
    hyprctl reload >/dev/null
    echo "stopped, also at login; Hyprland bindings restored from your config"
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
