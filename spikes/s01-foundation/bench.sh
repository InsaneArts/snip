#!/usr/bin/env bash
# S1 warm-path latency: fire the real Hyprland global shortcut N times while the
# spike runs in --bench mode (overlay auto-closes 120 ms after it is presented).
set -euo pipefail
cd "$(dirname "$0")"
N=${1:-30}
OUT=out/bench-$(date +%H%M%S).jsonl
mkdir -p out

QT_FORCE_STDERR_LOGGING=1 timeout $((N + 20)) ./build/snip-spike --bench >"$OUT" 2>out/bench.err &
APP=$!
sleep 1.5
for i in $(seq 1 "$N"); do
  hyprctl eval 'hl.dispatch(hl.dsp.global("snipspike:area"))' >/dev/null
  sleep 0.6
done
sleep 0.5
kill "$APP" 2>/dev/null || true
wait "$APP" 2>/dev/null || true
python3 analyze.py "$OUT"
