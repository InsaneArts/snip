#!/usr/bin/env python3
"""S12: OCR bake-off on text rendered in Chromium and grabbed through Snip's
capture path (known ground truth). Tesseract variants + RapidOCR (PP-OCR)."""
import json, os, re, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__)); import dnd_matrix as M

SP = M.SPIKE
SCR = "/tmp/claude-1000/-home-tgomareli-Development-OpenShotX/94444474-3bec-4ca4-a91e-2416404303b7/scratchpad"
FAST = f"{SCR}/tessdata_fast"
OUT = SP / "out/ocr"
LANG = {"ui-light": "eng", "ui-dark": "eng", "small": "eng", "code": "eng", "kat": "kat", "rus": "rus", "mixed": "kat+eng"}

def truth():
    html = (SP / "qao-test/ocr.html").read_text()
    js = html[html.index("const S = window.SAMPLES = {") + len("const S = window.SAMPLES = "):]
    js = js[:js.index("};") + 1]
    return json.loads(re.sub(r",\s*}", "}", js))

def lev(a, b):
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]

def norm(s): return re.sub(r"[ \t]+", " ", re.sub(r"\n+", "\n", s.strip())).strip()

def cer(got, want, keep_indent=False):
    if keep_indent:
        g = "\n".join(l.rstrip() for l in got.strip("\n").splitlines() if l.strip())
        w = "\n".join(l.rstrip() for l in want.splitlines() if l.strip())
    else:
        g, w = norm(got), norm(want)
    return round(100 * lev(g, w) / max(1, len(w)), 1)

def ff(src, dst, vf):
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", src, "-vf", vf, dst], check=True)

def mean_luma(png):
    d = subprocess.run(["ffmpeg", "-v", "error", "-i", png, "-vf", "scale=64:-1,format=gray", "-f", "rawvideo", "-"], capture_output=True).stdout
    return sum(d) / max(1, len(d))

def tess(png, lang, tessdata=None, psm=6, threads1=False, tsv=False):
    env = dict(os.environ)
    if threads1: env["OMP_THREAD_LIMIT"] = "1"
    cmd = ["tesseract", png, "-", "--oem", "1", "--psm", str(psm), "-l", lang, "-c", "preserve_interword_spaces=1"]
    if tessdata: cmd[1:1] = ["--tessdata-dir", tessdata]
    if tsv: cmd += ["-c", "tessedit_create_tsv=1"]
    t = time.perf_counter(); r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    return r.stdout, (time.perf_counter() - t) * 1000

MIN_CONF = 35

def indent_from_tsv(tsv):
    """Rebuild code indentation from word boxes (left / median char width)."""
    rows = [l.split("\t") for l in tsv.splitlines()[1:] if l.count("\t") >= 11]
    words = [r for r in rows if r[0] == "5" and r[11].strip() and float(r[10]) >= MIN_CONF]
    if not words: return ""
    cw = sorted(int(r[8]) / max(1, len(r[11])) for r in words)[len(words) // 2]
    minleft = min(int(r[6]) for r in words)
    lines = {}
    for r in words:
        key = (int(r[2]), int(r[3]), int(r[4]))
        lines.setdefault(key, []).append(r)
    out = []
    for key in sorted(lines, key=lambda k: min(int(w[7]) for w in lines[k])):
        ws = sorted(lines[key], key=lambda w: int(w[6]))
        ind = round((int(ws[0][6]) - minleft) / cw)
        out.append(" " * ind + " ".join(w[11] for w in ws))
    return "\n".join(out)

def rapid(png, engine):
    t = time.perf_counter(); r = engine(png); dt = (time.perf_counter() - t) * 1000
    txts = getattr(r, "txts", None) or []
    boxes = getattr(r, "boxes", None)
    if boxes is not None and len(txts):
        order = sorted(range(len(txts)), key=lambda i: (round(float(boxes[i][0][1]) / 12), float(boxes[i][0][0])))
        txts = [txts[i] for i in order]
    return "\n".join(txts), dt

def grab_samples():
    OUT.mkdir(parents=True, exist_ok=True)
    M.MON = M.monitor(); M.EXT = (round(M.MON["width"] / M.MON["scale"]), round(M.MON["height"] / M.MON["scale"]))
    sc = M.MON["scale"]
    srv = subprocess.Popen(["python3", "-m", "http.server", str(M.PORT + 2), "--bind", "127.0.0.1"], cwd=SP / "qao-test",
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    prev = json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
    app = subprocess.Popen([str(SP / "build/snip-spike"), "--test"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    win = None
    try:
        time.sleep(1.2)
        ws = "name:snip-ocr"
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{ws}" }}))')
        M.hypr('SNIP_OCR_RULE = hl.window_rule({ match = { title = "SNIP-OCR" }, opacity = "1 override 1 override", no_dim = true, no_blur = true, tag = "-default-opacity" })')
        cmd = f"chromium --user-data-dir=/tmp/snip-dnd-test/chromium-ocr --no-first-run --no-default-browser-check --ozone-platform=wayland --app=http://127.0.0.1:{M.PORT + 2}/ocr.html"
        M.hypr(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "{ws}" }}))')
        for _ in range(40):
            time.sleep(0.5); win = M.find("SNIP-OCR [")
            if win: break
        time.sleep(2.0); win = M.find("SNIP-OCR [")
        layout = json.loads(win["title"][len("SNIP-OCR "):])
        x0, y0 = round(win["at"][0] * sc), round(win["at"][1] * sc)
        M.ctl(f"grab DP-2 {x0} {y0} 400 40 {OUT}/marker.png")
        # page scale from the 100 CSS px marker
        d = subprocess.run(["ffmpeg", "-v", "error", "-i", f"{OUT}/marker.png", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], capture_output=True).stdout
        row = d[5 * 400 * 3:6 * 400 * 3]
        xs = [x for x in range(400) if abs(row[x*3] - 0x2d) < 32 and abs(row[x*3+1] - 0x0a) < 32 and abs(row[x*3+2] - 0x4e) < 32]
        ps = (xs[-1] - xs[0] + 1) / 100.0
        for k, x, y, w, h in layout:
            M.ctl(f"grab DP-2 {x0 + xs[0] + round(x * ps)} {y0 + round(y * ps)} {round(w * ps)} {round(h * ps)} {OUT}/{k}.png")
        return ps
    finally:
        app.terminate(); srv.terminate()
        if win: M.hypr(f'hl.dispatch(hl.dsp.window.close({{ window = "address:{win["address"]}" }}))')
        M.hypr('if SNIP_OCR_RULE then SNIP_OCR_RULE:set_enabled(false) end')
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')

def main():
    ps = grab_samples() if "--reuse" not in sys.argv else float(sys.argv[-1])
    T = truth()
    print(f"page scale {ps:.3f} (device px per CSS px)")
    init_best = tess(str(OUT / "marker.png"), "eng")[1]; init_fast = tess(str(OUT / "marker.png"), "eng", FAST)[1]
    print(f"process+model init on a tiny image: best {init_best:.0f} ms, fast {init_fast:.0f} ms (a warm daemon pays this once)")
    rows = []
    rapid_engine = None
    try:
        from rapidocr import RapidOCR
        rapid_engine = RapidOCR()
    except Exception as e:
        print("rapidocr unavailable:", e)
    for k in T:
        src = str(OUT / f"{k}.png")
        lang = LANG[k]
        dark = mean_luma(src) < 110
        up = str(OUT / f"{k}-up.png"); ff(src, up, "scale=iw*2:ih*2:flags=lanczos,format=gray")
        upi = str(OUT / f"{k}-upi.png"); ff(src, upi, "scale=iw*2:ih*2:flags=lanczos,format=gray" + (",negate" if dark else ""))
        r = {"sample": k, "dark": dark}
        ki = k == "code"
        for name, png, td in (("best", src, None), ("fast", src, FAST), ("fast+2x", up, FAST), ("fast+2x+inv", upi, FAST)):
            txt, ms = tess(png, lang, td, threads1=True)
            r[name] = (cer(txt, T[k], ki), round(ms - (init_fast if td else init_best)))
        tsv, _ = tess(upi, lang, FAST, threads1=True, tsv=True)
        rebuilt = indent_from_tsv(tsv)
        r["fast+2x+inv+tsvclean"] = (cer(rebuilt, T[k], ki), None)
        if k in ("code", "mixed"): print(f"--- {k} rebuilt:\n{rebuilt}")
        if rapid_engine and k not in ("kat", "mixed"):
            txt, ms = rapid(src, rapid_engine); r["rapidocr"] = (cer(txt, T[k], ki), round(ms))
        rows.append(r)
        print(json.dumps(r, ensure_ascii=False))
    (SP / "out/ocr-results.json").write_text(json.dumps(rows, indent=1, ensure_ascii=False))

main()
