#!/usr/bin/env python3
"""Summarise spike timing JSONL: hotkey -> capture -> Qt -> first frame swapped."""
import json
import statistics as st
import sys

events = [json.loads(l) for l in open(sys.argv[1]) if l.startswith("{")]
trials, cur = [], None
for e in events:
    ev = e["ev"]
    if ev == "hotkey_pressed":
        cur = {"hotkey": e["t_hotkey_ns"], "delivery_ms": e["delivery_ms"]}
        trials.append(cur)
    elif cur is None:
        continue
    elif ev == "capture_complete" and e.get("purpose") == "Freeze":
        cur["capture"] = e["t_ns"]
        cur["capture_ms"] = e["total_ms"]
    elif ev == "qml_show":
        cur.setdefault("show", e["t_ns"])
    elif ev == "frame_swapped":
        cur.setdefault("swap", e["t_ns"])

done = [t for t in trials if "swap" in t and "capture" in t]
if not done:
    sys.exit("no complete trials")

def ms(a, b):
    return (b - a) / 1e6

def summary(name, xs):
    xs = sorted(xs)
    p95 = xs[min(len(xs) - 1, int(round(0.95 * (len(xs) - 1))))]
    print(f"{name:<34} median {st.median(xs):7.2f}  p95 {p95:7.2f}  min {xs[0]:7.2f}  max {xs[-1]:7.2f}  (ms)")

print(f"trials: {len(done)} complete of {len(trials)} hotkeys  (first = cold window, excluded below)")
first, warm = done[0], done[1:]
print(f"cold first trial: hotkey->swap {ms(first['hotkey'], first['swap']):.2f} ms")
if warm:
    summary("hotkey delivery (compositor->us)", [t["delivery_ms"] for t in warm])
    summary("capture (issue->all ready)", [t["capture_ms"] for t in warm])
    summary("hotkey -> frames ready", [ms(t["hotkey"], t["capture"]) for t in warm])
    summary("frames ready -> frame swapped", [ms(t["capture"], t["swap"]) for t in warm])
    summary("HOTKEY -> FRAME SWAPPED", [ms(t["hotkey"], t["swap"]) for t in warm])
    print("(on-screen = swapped + up to one refresh: <= 6.9 ms at 144 Hz)")
