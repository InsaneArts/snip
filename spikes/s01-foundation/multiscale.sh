#!/usr/bin/env bash
# S2 across scales/transforms: add a headless output, run --selftest for each
# configuration (capture -> overlay -> recapture -> diff on every output).
set -uo pipefail
cd "$(dirname "$0")"
NAME=SNIPTEST
mkdir -p out
LOG=out/multiscale.txt
: >"$LOG"

cleanup() { hyprctl output remove "$NAME" >/dev/null 2>&1 || true; }
trap cleanup EXIT

hyprctl output create headless "$NAME" >/dev/null
sleep 0.5

run_case() {
  local scale=$1 transform=$2
  hyprctl eval "hl.monitor({ output = \"$NAME\", mode = \"1920x1080@60\", position = \"auto\", scale = $scale, transform = $transform })" >/dev/null
  sleep 0.8
  # keep the cursor on DP-2: headless outputs draw a software cursor into captures
  hyprctl eval 'hl.dispatch(hl.dsp.cursor.move({ x = 1700, y = 700 }))' >/dev/null
  local actual
  actual=$(hyprctl monitors -j | jq -c --arg n "$NAME" '.[] | select(.name==$n) | {scale, transform, w:.width, h:.height}')
  local report
  report=$(QT_FORCE_STDERR_LOGGING=1 timeout 15 ./build/snip-spike --selftest 2>>out/multiscale.err | grep verify_report | sed 's/.*"report"://; s/}$//')
  echo "requested scale=$scale transform=$transform actual=$actual" | tee -a "$LOG"
  echo "  $report" | tee -a "$LOG"
}

for s in 1 1.25 1.5 1.666667 2; do run_case "$s" 0; done
run_case 1.25 1
run_case 1.5 3
run_case 2 4
