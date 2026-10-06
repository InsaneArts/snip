#!/usr/bin/env python3
"""S11/S16: scroll a real page with the virtual pointer, grab frames through
Snip's capture path, stitch them with Omasnap's stitcher (stitch-replay), and
verify the result by decoding every row's barcode."""
import json, os, shutil, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__)); import dnd_matrix as M

SP = M.SPIKE
REPLAY = os.environ.get("REPLAY", "/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad/omasnap/build/stitch-replay")
FR = SP / "out/scroll-frames"
STEP = float(os.environ.get("STEP", "45"))      # wheel axis units per step
SETTLE = float(os.environ.get("SETTLE", "0.35"))

def raw(png):
    probe = subprocess.run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", png],
                           capture_output=True, text=True).stdout.strip().split(",")
    w, h = int(probe[0]), int(probe[1])
    data = subprocess.run(["ffmpeg", "-v", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], capture_output=True).stdout
    return w, h, data

def decode(png, scale):
    """Per scanline: decode the 12-bit barcode + parity at known x positions."""
    w, h, d = raw(png)
    row20 = d[20 * w * 3:21 * w * 3]
    is_hdr = lambda p: abs(p[0] - 0x2d) < 30 and abs(p[1] - 0x0a) < 30 and abs(p[2] - 0x4e) < 30
    off = next((x for x in range(w) if is_hdr(row20[x*3:x*3+3])), 0)
    # effective page scale from the header height (70 CSS px): Chromium may apply its own zoom
    hy = [y for y in range(min(h, 400)) if is_hdr(d[y * w * 3 + (off + 5) * 3: y * w * 3 + (off + 5) * 3 + 3])]
    if hy: scale = (hy[-1] - hy[0] + 1) / 70.0
    xs = [off + round((20 + 12 + k * 24) * scale) for k in range(13)]
    seq = []
    header_rows, footer_rows = [], []
    for y in range(h):
        row = d[y * w * 3:(y + 1) * w * 3]
        px = lambda x: row[x * 3:x * 3 + 3]
        r, g, b = px(off + 5)
        if abs(r - 0x2d) < 30 and abs(g - 0x0a) < 30 and abs(b - 0x4e) < 30: header_rows.append(y)
        if abs(r - 0x0a) < 30 and abs(g - 0x4e) < 30 and abs(b - 0x2d) < 30: footer_rows.append(y)
        bits = []
        for x in xs:
            v = sum(px(x)) / 3
            if v < 90: bits.append(1)
            elif v > 150: bits.append(0)
            else: bits = None; break
        if bits is None: continue
        val = int("".join(map(str, bits[:12])), 2)
        if sum(bits[:12]) % 2 != bits[12]: continue
        seq.append((y, val))
    # collapse into bands
    bands = []
    for y, v in seq:
        if bands and bands[-1][0] == v and y - bands[-1][2] <= 2: bands[-1][2] = y
        else: bands.append([v, y, y])
    return w, h, bands, header_rows, footer_rows

def main():
    if FR.exists(): shutil.rmtree(FR)
    FR.mkdir(parents=True)
    M.MON = M.monitor(); M.EXT = (round(M.MON["width"] / M.MON["scale"]), round(M.MON["height"] / M.MON["scale"]))
    scale = M.MON["scale"]
    srv_root = SP / "qao-test"
    srv = subprocess.Popen(["python3", "-m", "http.server", str(M.PORT + 1), "--bind", "127.0.0.1"], cwd=srv_root,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    prev = json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
    app = subprocess.Popen([str(SP / "build/snip-spike"), "--test"], stdout=open(SP / "out/scroll-app.jsonl", "w"), stderr=subprocess.DEVNULL)
    win = None
    try:
        time.sleep(1.2)
        ws = "name:snip-scroll"
        # opaque, undimmed, unblurred: Omarchy's default opacity/dim leak the wallpaper into region captures
        if not os.environ.get("TRANSLUCENT"): M.hypr('SNIP_SCROLL_RULE = hl.window_rule({ match = { title = "SNIP-SCROLL" }, opacity = "1 override 1 override", '
               'no_dim = true, no_blur = true, tag = "-default-opacity" })')
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{ws}" }}))')
        prof = f"/tmp/snip-dnd-test/chromium-scroll"
        cmd = f"chromium --user-data-dir={prof} --no-first-run --no-default-browser-check --ozone-platform=wayland --app=http://127.0.0.1:{M.PORT + 1}/scroll.html"
        M.hypr(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "{ws}" }}))')
        for _ in range(40):
            time.sleep(0.5); win = M.find("SNIP-SCROLL")
            if win: break
        time.sleep(2.5)
        win = M.find("SNIP-SCROLL")
        # capture region = the window's content area, in physical px
        x0, y0 = round(win["at"][0] * scale), round(win["at"][1] * scale)
        w0, h0 = round(win["size"][0] * scale), round(win["size"][1] * scale)
        cx, cy = M.center(win)
        for dx in (60, 20, 0): M.move(cx + dx, cy + dx); time.sleep(0.05)
        t0 = time.time(); frames = 0; still = 0; prev_bytes = None
        while frames < 200:
            path = FR / f"frame-{frames:04d}.png"
            r = M.ctl(f"grab DP-2 {x0} {y0} {w0} {h0} {path}")
            data = path.read_bytes()
            if prev_bytes == data:
                still += 1; path.unlink()
                if still >= 3: break
            else:
                still = 0; frames += 1; prev_bytes = data
            M.ctl(f"scroll {STEP}")
            time.sleep(SETTLE)
        dt = time.time() - t0
        print(f"captured {frames} frames in {dt:.1f}s (region {w0}x{h0} @ {scale})")
    finally:
        app.terminate(); srv.terminate()
        if win: M.hypr(f'hl.dispatch(hl.dsp.window.close({{ window = "address:{win["address"]}" }}))')
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')
        M.hypr('if SNIP_SCROLL_RULE then SNIP_SCROLL_RULE:set_enabled(false) end')
    out = SP / "out/scroll-stitched.png"
    t1 = time.time()
    r = subprocess.run([REPLAY], env=dict(os.environ, OMASNAP_STITCH_REPLAY_DIR=str(FR), OMASNAP_STITCH_REPLAY_OUT=str(out)),
                       capture_output=True, text=True)
    print(f"stitch-replay rc={r.returncode} in {time.time() - t1:.2f}s; stderr tail: {r.stderr.strip().splitlines()[-3:]}")
    report(out, scale)

def report(out, scale):
    if not out.exists(): print("no stitched output"); return
    w, h, bands, hdr, ftr = decode(str(out), scale)
    bands = [b for b in bands if 0 <= b[0] < 400 and b[2] - b[1] >= 8]   # valid rows, ignore 1-2 px slivers
    # gaps between rows have no blocks -> read as all-white = a valid-looking "0"; keep only the real row 0 (first)
    bands = [b for i, b in enumerate(bands) if b[0] != 0 or i == 0]
    vals = [b[0] for b in bands]
    breaks = [(vals[i], vals[i + 1], bands[i + 1][1]) for i in range(len(vals) - 1) if vals[i + 1] != vals[i] + 1]
    miss = sorted(set(range(vals[0], vals[-1] + 1)) - set(vals)) if vals else []
    dup = sorted(v for v in set(vals) if vals.count(v) > 1)
    hts = [b[2] - b[1] + 1 for b in bands[1:-1]]
    def runs(rows):
        res, st = [], None
        for i, y in enumerate(rows):
            st = y if st is None else st
            if i + 1 == len(rows) or rows[i + 1] != y + 1: res.append((st, y)); st = None
        return res
    print(json.dumps({"stitched": f"{w}x{h}", "rows_decoded": len(set(vals)), "first": vals[0] if vals else None,
                      "last": vals[-1] if vals else None, "missing": len(miss), "missing_sample": miss[:12],
                      "dupes": dup[:12], "order_breaks": breaks[:10], "n_breaks": len(breaks), "band_height_min_max": [min(hts), max(hts)] if hts else None,
                      "header_runs": runs(hdr)[:4], "footer_runs": runs(ftr)[:4]}))

if len(sys.argv) > 1 and sys.argv[1] == '--decode-only':
    report(SP / 'out/scroll-stitched.png', 1.25)
else:
    main()
