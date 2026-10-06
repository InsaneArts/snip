#!/usr/bin/env python3
"""S8: editor preview == export? and frame times with many objects."""
import json, math, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__)); import dnd_matrix as M
SP = M.SPIKE
def raw(png, crop=None):
    vf = ["-vf", f"crop={crop}"] if crop else []
    return subprocess.run(["ffmpeg", "-v", "error", "-i", png, *vf, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], capture_output=True).stdout
def compare(a, b):
    n = len(a); se = 0; diff = 0; mx = 0
    for i in range(0, n):
        d = a[i] - b[i]
        if d: diff += 1; mx = max(mx, abs(d)); se += d * d
    psnr = float("inf") if se == 0 else 10 * math.log10(255 * 255 / (se / n))
    return {"channel_diffs": diff, "max_delta": mx, "psnr_db": round(psnr, 2)}
def run(extra_args, ws, seconds=None):
    M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{ws}" }}))')
    out = open(SP / "out/editor.jsonl", "w")
    app = subprocess.Popen([str(SP / "build/snip-spike"), "--editor", *extra_args], stdout=out, stderr=open(SP / "out/editor.err", "w"),
                           env=dict(os.environ, QT_FORCE_STDERR_LOGGING="1", **({"QSG_RENDER_LOOP": os.environ["LOOP"]} if os.environ.get("LOOP") else {})))
    return app
prev = json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
M.hypr('SNIP_ED_RULE = hl.window_rule({ match = { title = "^snip-editor$" }, float = true, size = { 1280, 720 }, move = { 400, 200 }, '
       'border_size = 0, no_shadow = true, no_blur = true, no_anim = true, no_dim = true, rounding = 0, opacity = "1 1", tag = "-default-opacity" })')
res = {}
try:
    # 1) parity
    app = run([], "name:snip-editor"); time.sleep(2.0)
    ed = M.find("snip-editor"); res["window"] = {"at": ed["at"], "size": ed["size"]} if ed else None
    M.hypr('hl.dispatch(hl.dsp.cursor.move({ x = 2700, y = 1100 }))'); time.sleep(0.3)
    M.ctl(f"export {SP}/out/editor-export.png"); time.sleep(1.0)
    M.sh("grim", "-o", "DP-2", str(SP / "out/editor-screen.png"))
    app.terminate(); app.wait()
    x, y = round(ed["at"][0] * 1.25), round(ed["at"][1] * 1.25)
    a = raw(str(SP / "out/editor-export.png")); b = raw(str(SP / "out/editor-screen.png"), f"1600:900:{x}:{y}")
    res["export_vs_screen"] = compare(a, b) if len(a) == len(b) else {"size_mismatch": [len(a), len(b)]}
    # 2) perf with 200 extra objects, animating one arrow
    for n in (0, 200):
        app = run(["--objects", str(n), "--animate"], "name:snip-editor"); time.sleep(4.0)
        app.terminate(); app.wait()
        ts = [json.loads(l)["t_ns"] for l in open(SP / "out/editor.jsonl") if '"editor_frame"' in l]
        ts = ts[len(ts) // 4:]  # drop warm-up
        dts = sorted((b - a) / 1e6 for a, b in zip(ts, ts[1:]))
        if dts:
            res[f"perf_{n}_objects"] = {"frames": len(dts) + 1, "fps": round(1000 / (sum(dts) / len(dts)), 1),
                                        "frame_ms_median": round(dts[len(dts) // 2], 2), "frame_ms_p95": round(dts[int(len(dts) * 0.95)], 2),
                                        "frame_ms_max": round(dts[-1], 2)}
finally:
    M.hypr('if SNIP_ED_RULE then SNIP_ED_RULE:set_enabled(false) end')
    M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')
print(json.dumps(res, indent=1))
