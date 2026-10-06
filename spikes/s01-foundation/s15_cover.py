#!/usr/bin/env python3
"""S15: does a wallpaper slice cover the bar pixel-exactly (no re-layout)?"""
import json, os, subprocess, time
OUT = "DP-2"
def sh(*a): return subprocess.run(a, capture_output=True, text=True)
def hypr(lua): sh("hyprctl", "eval", lua)
def layers(): return json.loads(sh("hyprctl", "layers", "-j").stdout)[OUT]["levels"]
def raw(png, w, h):
    return subprocess.run(["ffmpeg", "-v", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], capture_output=True).stdout
mon = next(m for m in json.loads(sh("hyprctl", "monitors", "-j").stdout) if m["name"] == OUT)
W, H, S = mon["width"], mon["height"], mon["scale"]
bar = next(l for l in layers()["2"] if l["namespace"] == "omarchy-bar")
bar_px = round(bar["h"] * S)
prev = json.loads(sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
hypr('hl.dispatch(hl.dsp.focus({ workspace = "name:snip-cover" }))'); time.sleep(0.8)
hypr('hl.dispatch(hl.dsp.cursor.move({ x = 1700, y = 700 }))')
# NB: omarchy-toggle-bar "on" sets the bar-off flag = HIDES the bar; "off" shows it.
bar_was_hidden = os.path.exists(os.path.expanduser("~/.local/state/omarchy/toggles/bar-off"))
try:
    sh("omarchy-toggle-bar", "on"); time.sleep(1.5)          # hide -> ground truth
    sh("grim", "-o", OUT, "out/s15-truth.png")
    sh("omarchy-toggle-bar", "off"); time.sleep(1.5)         # show
    sh("grim", "-o", OUT, "out/s15-bar.png")
    app = subprocess.Popen(["./build/snip-spike", "--cover", str(bar["h"])] + os.environ.get("COVER_ARGS", "").split(), stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    for line in app.stdout:
        if "cover_ready" in line: break
    sh("grim", "-o", OUT, "out/s15-cover.png")
    app.terminate()
finally:
    sh("omarchy-toggle-bar", "on" if bar_was_hidden else "off")
    hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')
t, b, c = (raw(f"out/s15-{n}.png", W, H) for n in ("truth", "bar", "cover"))
row = W * 3
def diff(a, x, r0, r1):
    n = mx = 0
    for y in range(r0, r1):
        sa, sb = a[y*row:(y+1)*row], x[y*row:(y+1)*row]
        if sa == sb: continue
        for i in range(0, row):
            d = abs(sa[i] - sb[i])
            if d: n += 1; mx = max(mx, d)
    return n // 3 if n else 0, mx
print(f"bar: {bar['w']}x{bar['h']} logical = {bar_px} px rows @ scale {S}")
print("bar region, truth vs bar visible  :", diff(t, b, 0, bar_px), "(channel diffs/3, max delta)")
print("bar region, truth vs cover        :", diff(t, c, 0, bar_px))
print("rest of screen, truth vs cover    :", diff(t, c, bar_px, H))

import math
def psnr(a, x, r0, r1):
    se = 0; n = 0
    for y in range(r0, r1):
        sa, sb = a[y*row:(y+1)*row], x[y*row:(y+1)*row]
        for i in range(0, row, 1):
            d = sa[i] - sb[i]; se += d * d
        n += row
    mse = se / n
    return float("inf") if mse == 0 else 10 * math.log10(255 * 255 / mse)
print(f"bar region PSNR truth vs cover: {psnr(t, c, 0, bar_px):.1f} dB;  seam rows {bar_px-2}..{bar_px+1}: {diff(t, c, bar_px-2, bar_px+2)}")
