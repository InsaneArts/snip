#!/usr/bin/env bash
# Install Snip from this checkout (M3.5): build a pacman package from
# packaging/aur/snip-screenshot-git (system-wide, so `pacman -R` removes it
# cleanly), switch off a from-source Snip (scripts/dev.sh) so only one runs,
# and start Snip now and at every login.
#
#   scripts/install.sh          install, or update to this checkout's main branch
#   scripts/install.sh remove   uninstall
#
# pacman asks for your password, and installs what Snip needs.
set -euo pipefail
cd "$(dirname "$0")/.."
PKG=snip-screenshot-git

case "${1:-install}" in
  install)
    (cd packaging/aur/$PKG && SNIP_REPO="$PWD/../../.." makepkg --syncdeps --install --force --cleanbuild)
    # one Snip at a time: the from-source one goes (scripts/dev.sh brings it back)
    systemctl --user disable --now --quiet snip-dev 2>/dev/null || true
    rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/applications/snip.desktop"
    systemctl --user daemon-reload
    systemctl --user enable --now snipd
    echo "Snip is installed: in the launcher, in the bar, and started at every login."
    ;;
  remove)
    systemctl --user disable --now --quiet snipd 2>/dev/null || true
    sudo pacman -R $PKG
    ;;
  *) sed -n '2,11p' "$0"; exit 2 ;;
esac
