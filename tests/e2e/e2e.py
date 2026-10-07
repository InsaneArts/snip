#!/usr/bin/env python3
"""End-to-end tests for snipd on the live Hyprland session.

Everything happens on a temporary headless output with its own workspace, in
an isolated snipd instance (own XDG dirs and ctl socket), driven by Snip's
virtual pointer and test hooks. The user's monitors only see the selection
overlay flash for the area cases (overlays cover every output).

    tests/e2e/e2e.py [--build build] [--only NAME ...] [--keep]

Exit code 0 when every case passes. Cleanup always runs: the daemon quits,
test windows close, the headless output is removed, the pointer and focus
go back to where they were, and no snip layer may be left mapped.
"""

import argparse
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = "SNIPE2E"
UID = os.getuid()


def sh(*args, check=True, **kw):
    return subprocess.run(args, check=check, capture_output=True, text=True, **kw)


def hypr_eval(lua):
    return sh("hyprctl", "eval", lua, check=False).stdout.strip()


def hypr_json(what):
    return json.loads(sh("hyprctl", "-j", what).stdout)


class Snip:
    """An isolated snipd plus its CLI."""

    def __init__(self, build):
        self.build = Path(build)
        self.tmp = Path(tempfile.mkdtemp(prefix="snip-e2e-"))
        self.run_dir = Path(f"/run/user/{UID}/snip-e2e-{os.getpid()}")
        self.export = self.tmp / "export"
        self.env = dict(os.environ)
        self.env.update(
            XDG_CONFIG_HOME=str(self.tmp / "cfg"),
            XDG_DATA_HOME=str(self.tmp / "data"),
            XDG_CACHE_HOME=str(self.tmp / "cache"),
            SNIP_RUNTIME_DIR=str(self.run_dir),
            SNIP_APP_ID="snip-e2e",   # never trigger (or be triggered by) the user's daemon
            SNIP_OUTPUTS=OUT,         # overlays only on the test output
            SNIP_TRACE="1",
            QT_FORCE_STDERR_LOGGING="1",
        )
        (self.tmp / "cfg/snip").mkdir(parents=True)
        self.write_config("")
        self.proc = None
        self.log = self.tmp / "snipd.err"

    def write_config(self, extra):
        (self.tmp / "cfg/snip/config.toml").write_text(
            f'[general]\nexport_dir = "{self.export}"\nsounds = false\n{extra}\n'
        )

    def start(self):
        self.proc = subprocess.Popen(
            [str(self.build / "snipd")], env=self.env, stdout=subprocess.DEVNULL, stderr=open(self.log, "w")
        )
        for _ in range(50):
            if self.cli("ping", check=False).returncode == 0:
                return
            time.sleep(0.1)
        raise RuntimeError("snipd did not start")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.cli("quit", check=False)
            try:
                self.proc.wait(3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        shutil.rmtree(self.run_dir, ignore_errors=True)

    def cli(self, *args, check=True, timeout=30):
        r = subprocess.run([str(self.build / "snip"), *args], env=self.env, capture_output=True, text=True, timeout=timeout)
        if check and r.returncode != 0:
            raise RuntimeError(f"snip {' '.join(args)}: exit {r.returncode}: {r.stderr.strip()}")
        return r

    def json(self, *args):
        r = self.cli("--json", *args, check=False)
        return json.loads(r.stdout) if r.stdout.strip() else {"ok": False, "error": r.stderr}

    def bg(self, *args):
        """Start a blocking CLI call (a capture waiting for the selection)."""
        # overlays only on the test output: the user's monitors stay untouched
        if args and args[0] in ("area", "window", "text") and "--monitor" not in args:
            args = (*args, "--monitor", OUT)
        return subprocess.Popen(
            [str(self.build / "snip"), "--json", *args], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )

    def debug(self, *args):
        self.cli("debug", *map(str, args))

    def move(self, x, y, out=OUT):
        self.debug("move", out, int(x), int(y))
        time.sleep(0.03)

    def click(self, right=False):
        self.debug("rdown" if right else "down")
        time.sleep(0.03)
        self.debug("rup" if right else "up")

    def key(self, name, kind="key"):
        self.debug("ui", kind, name)
        time.sleep(0.03)

    def drag(self, x0, y0, x1, y1, steps=4, during=None):
        self.move(x0, y0)
        self.debug("down")
        for i in range(1, steps + 1):
            if during and i == steps // 2:
                during()
            self.move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps)
        self.debug("up")

    def traces(self, ev=None):
        out = []
        for line in self.log.read_text(errors="replace").splitlines():
            if line.startswith("{"):
                try:
                    j = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if ev is None or j.get("ev") == ev:
                    out.append(j)
        return out

    def status(self):
        return self.json("status")["result"]


def finish(p, timeout=10):
    out, _ = p.communicate(timeout=timeout)
    return json.loads(out), p.returncode


def png_size(path):
    b = Path(path).read_bytes()[:24]
    return int.from_bytes(b[16:20], "big"), int.from_bytes(b[20:24], "big")


def make_text_png(path, lines):
    """A white image with dark text: OCR fixtures."""
    from PIL import Image, ImageDraw, ImageFont
    try:
        font = ImageFont.truetype("DejaVuSans.ttf", 30)
    except OSError:
        font = ImageFont.load_default(30)
    im = Image.new("RGB", (900, 70 + 56 * len(lines)), "white")
    d = ImageDraw.Draw(im)
    for i, line in enumerate(lines):
        d.text((28, 28 + i * 56), line, fill="black", font=font)
    im.save(path)


def open_card(s, path):
    """Show a file as a card (snip open) and return its record."""
    return json.loads(s.cli("--json", "open", str(path)).stdout)["result"]


def layer_mapped(namespace):
    return any(l["namespace"] == namespace for v in hypr_json("layers").values() for ls in v["levels"].values() for l in ls)


def wait_for(fn, timeout=5.0, step=0.1):
    end = time.time() + timeout
    while time.time() < end:
        v = fn()
        if v:
            return v
        time.sleep(step)
    return fn()


# ------------------------------------------------------------------ environment

class Env:
    def __init__(self):
        self.prev_focus = hypr_json("activewindow").get("address")
        self.prev_cursor = hypr_json("cursorpos")
        self.windows = []

    def add_output(self, scale=1.25, transform=0):
        if not any(m["name"] == OUT for m in hypr_json("monitors")):
            sh("hyprctl", "output", "create", "headless", OUT)
            time.sleep(0.5)
        self.configure(scale, transform)

    def configure(self, scale, transform=0):
        hypr_eval(f'hl.monitor({{ output = "{OUT}", mode = "1920x1080@60", position = "auto", scale = {scale}, transform = {transform} }})')
        time.sleep(0.8)

    def monitor(self):
        return next(m for m in hypr_json("monitors") if m["name"] == OUT)

    def spawn_window(self, app_id, text):
        ws = self.monitor()["activeWorkspace"]["id"]
        cmd = f"foot --title={app_id} --app-id={app_id} sh -c 'printf \"{text}\"; sleep 3600'"
        hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "{ws} silent" }}))')
        for _ in range(40):
            c = [c for c in hypr_json("clients") if c["class"] == app_id]
            if c:
                self.windows.append(c[0]["pid"])
                time.sleep(0.4)
                return c[0]
            time.sleep(0.1)
        raise RuntimeError(f"window {app_id} did not appear")

    def cleanup(self):
        for pid in self.windows:
            try:
                os.kill(pid, 15)
            except ProcessLookupError:
                pass
        # Put the pointer back only if the tests left it on the test output;
        # if the user has moved it since, leave it where they put it.
        try:
            mon = self.monitor()
            cur = hypr_json("cursorpos")
            ours = mon["x"] <= cur["x"] < mon["x"] + mon["width"] / mon["scale"]
        except StopIteration:
            ours = False
        sh("hyprctl", "output", "remove", OUT, check=False)
        time.sleep(0.3)
        c = self.prev_cursor
        if ours:
            hypr_eval(f'hl.dispatch(hl.dsp.cursor.move({{ x = {c["x"]}, y = {c["y"]} }}))')
        if self.prev_focus:
            hypr_eval(f'hl.dispatch(hl.dsp.focus({{ window = "address:{self.prev_focus}" }}))')


def stray_layers(pid):
    layers = hypr_json("layers")
    return [l for v in layers.values() for ls in v["levels"].values() for l in ls if l["namespace"].startswith("snip") and l["pid"] == pid]


# ------------------------------------------------------------------ cases

CASES = []


def case(fn):
    CASES.append(fn)
    return fn


def area(s, x0, y0, x1, y1, during=None):
    p = s.bg("area")
    time.sleep(0.5)
    s.drag(x0, y0, x1, y1, during=during)
    return finish(p)


@case
def area_exact_size(s, env):
    """500x300 logical at 1.25 -> exactly 625x375 physical, file matches."""
    r, code = area(s, 100, 100, 600, 400)
    assert code == 0 and r["ok"], r
    rec = r["result"]
    assert (rec["width"], rec["height"]) == (625, 375), rec
    assert png_size(rec["source"]) == (625, 375)


@case
def modifiers(s, env):
    """Shift = square, Alt = from centre, Space held = move (SEL-07)."""
    r, _ = area(s, 100, 100, 300, 200, during=lambda: s.key("Shift", "keydown"))
    s.key("Shift", "keyup")
    assert (r["result"]["width"], r["result"]["height"]) == (250, 250), r
    r, _ = area(s, 400, 400, 450, 430, during=lambda: s.key("Alt", "keydown"))
    s.key("Alt", "keyup")
    assert (r["result"]["width"], r["result"]["height"]) == (125, 75), r
    p = s.bg("area")
    time.sleep(0.5)
    s.move(100, 100); s.debug("down"); s.move(150, 150); s.move(200, 200)
    s.key("space", "keydown"); s.move(250, 225); s.move(300, 250); s.key("space", "keyup")
    s.move(310, 255); s.move(320, 260); s.debug("up")
    r, _ = finish(p)
    assert (r["result"]["width"], r["result"]["height"]) == (150, 137), r
    last = s.traces("finish_area")[-1]
    assert last["rect"] == [250, 188, 150, 137], last


@case
def area_is_free_space(s, env):
    """Area mode: no window highlight, a click without a drag does nothing."""
    w = env.win
    lx, ly = w["at"][0] - env.monitor()["x"], w["at"][1]
    p = s.bg("area"); time.sleep(0.5)
    s.move(lx + w["size"][0] / 2, ly + w["size"][1] / 2); s.click()
    time.sleep(0.4)
    still_open = p.poll() is None
    overlay = s.status()["overlay"]
    s.key("Escape")
    assert still_open and overlay == "area", f"a plain click must not capture (cli running: {still_open}, overlay: {overlay})"
    r, code = finish(p)
    assert code == 1, r


@case
def space_held_picks_window(s, env):
    """Hold Space + click = the clean window (CAP-05); release Space = area again."""
    w = env.win
    lx, ly = w["at"][0] - env.monitor()["x"], w["at"][1]
    p = s.bg("area"); time.sleep(0.5)
    s.move(lx + 60, ly + 60)
    s.key("space", "keydown"); s.click(); s.key("space", "keyup")
    r, code = finish(p)
    assert code == 0 and r["result"]["kind"] == "window", r
    W, H = png_size(r["result"]["source"])
    assert W > w["size"][0] * 1.25 and H > w["size"][1] * 1.25, (W, H)  # clean buffer + shadow padding
    # Space pressed and released before dragging: still an area drag
    p = s.bg("area"); time.sleep(0.5)
    s.move(lx + 60, ly + 60); s.key("space", "keydown"); s.key("space", "keyup")
    s.drag(lx + 60, ly + 60, lx + 260, ly + 160)
    r, code = finish(p)
    assert r["result"]["kind"] == "area" and (r["result"]["width"], r["result"]["height"]) == (250, 125), r


@case
def keyboard_flow(s, env):
    """Tab + Enter picks a window; Esc and the hotkey again cancel (exit 1)."""
    p = s.bg("area"); time.sleep(0.5)
    s.move(3, 860); s.key("Tab"); s.key("Return")
    r, _ = finish(p)
    assert r["result"]["kind"] == "window", r
    p = s.bg("area"); time.sleep(0.5); s.key("Escape")
    r, code = finish(p)
    assert code == 1 and r["error"] == "cancelled", r
    p = s.bg("area"); time.sleep(0.5)
    hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:area"))')
    r, code = finish(p)
    assert code == 1, r


@case
def previous_area(s, env):
    r, _ = area(s, 200, 200, 400, 300)
    prev = s.json("previous")
    assert prev["ok"] and (prev["result"]["width"], prev["result"]["height"]) == (r["result"]["width"], r["result"]["height"]), prev


@case
def window_capture(s, env):
    """Space + click: the clean toplevel buffer, rounded, with a shadow."""
    w = env.win
    lx, ly = w["at"][0] - env.monitor()["x"], w["at"][1]
    p = s.bg("window"); time.sleep(0.5)
    s.move(lx + 50, ly + 50); s.click()
    r, code = finish(p)
    assert code == 0 and r["result"]["kind"] == "window", r
    W, H = png_size(r["result"]["source"])
    assert W > w["size"][0] * 1.25 and H > w["size"][1] * 1.25, (W, H)  # shadow padding


@case
def ocr_text(s, env):
    """Capture Text over known text (CAP-09)."""
    w = env.win
    lx, ly = w["at"][0] - env.monitor()["x"], w["at"][1]
    p = s.bg("text"); time.sleep(0.5)
    s.drag(lx + 2, ly + 2, lx + 330, ly + 40)
    r, code = finish(p, timeout=15)
    assert code == 0 and "Snip window capture test" in r["result"], r


@case
def lifecycle(s, env):
    """Capture -> history only; Copy -> clipboard; Save -> folder + clipboard; restore."""
    s.cli("close-all"); time.sleep(0.3)
    rec = s.json("fullscreen", "--monitor", OUT)["result"]
    assert not list(s.export.glob("*.png")), "capture must not write to the export folder"
    s.cli("copy", rec["id"])
    time.sleep(0.2)
    types = sh("wl-paste", "-l", check=False).stdout.split()
    assert "image/png" in types and "text/plain;charset=utf-8" in types, types
    path = sh("wl-paste", "-n", "-t", "text/plain;charset=utf-8", check=False).stdout
    assert path.endswith(".png") and png_size(path) == (1920, 1080), path   # text targets get the file
    saved = s.json("save", rec["id"])["result"]
    assert Path(saved).exists() and Path(saved).parent == s.export, saved
    assert Path(saved).name.startswith("Snip ") and saved.endswith(".png"), saved
    s.cli("close-all"); time.sleep(0.4)
    assert s.status()["cards"] == []
    s.cli("restore"); time.sleep(0.4)
    assert s.status()["cards"] == [rec["id"]], s.status()["cards"]


@case
def cards_not_in_captures(s, env):
    """Snip's cards never appear in a capture (CleanShot parity)."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    s.move(1200, 300)
    for _ in range(3):
        rec = s.json("fullscreen", "--monitor", OUT)["result"]
        time.sleep(0.4)
    im = Image.open(rec["source"]).convert("RGB")
    strip = im.crop((22, 600, 32, 1060))   # the cards' left edge (accent ring on the newest)
    reds = [p[0] for p in strip.get_flattened_data()]
    assert len(s.status()["cards"]) == 3
    assert max(reds) < 100 or statistics.mean(reds) < 30, "a card was captured"


@case
def latency(s, env):
    """Hotkey -> every overlay presented: p95 < 35 ms warm (SEL-01)."""
    times = []
    for i in range(12):
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:area"))')
        time.sleep(0.35)
        s.key("Escape")
        time.sleep(0.25)
    ev = s.traces()
    shortcuts = [e for e in ev if e["ev"] == "shortcut" and e.get("id") == "area"]
    for sc in shortcuts:
        pres = [e for e in ev if e["ev"] == "overlay_presented" and e["t_ns"] > sc["t_ns"]]
        if not pres:
            continue
        epoch = pres[0]["epoch"]
        same = [p for p in pres if p["epoch"] == epoch]
        times.append((same[-1]["t_ns"] - sc["t_ns"]) / 1e6)
    warm = sorted(times[1:])
    p95 = warm[int(0.95 * (len(warm) - 1))]
    print(f"      hotkey->presented ms: first {times[0]:.1f}, warm median {statistics.median(warm):.1f}, p95 {p95:.1f}")
    assert p95 < 35, warm


@case
def menu_closes_on_any_click(s, env):
    """The card menu closes on a click on another card or anywhere outside the panel,
    and that click does nothing else."""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "menu.png"
    make_text_png(png, ["menu test"])
    open_card(s, png); time.sleep(0.5)
    open_card(s, png); time.sleep(0.7)
    mon = env.monitor()
    H = mon["height"] / mon["scale"]
    cw, ch = 280, round(280 * 0.705)
    x = 24 + cw / 2
    y_old = H - 24 - ch / 2                  # oldest card, in the corner
    y_new = H - 24 - ch - 16 - ch / 2        # newest card, on top of it
    s.move(x, y_new); time.sleep(0.25); s.click(right=True)
    assert wait_for(lambda: layer_mapped("snip-dismiss"), 2), "menu did not open"
    s.move(x, y_old); time.sleep(0.2); s.click()
    assert wait_for(lambda: not layer_mapped("snip-dismiss"), 2), "a click on another card did not close the menu"
    s.move(x, y_new); time.sleep(0.25); s.click(right=True)
    assert wait_for(lambda: layer_mapped("snip-dismiss"), 2), "menu did not reopen"
    s.move(mon["width"] / mon["scale"] - 200, 300); time.sleep(0.2); s.click()
    assert wait_for(lambda: not layer_mapped("snip-dismiss"), 2), "a click outside the panel did not close the menu"
    assert len(s.status()["cards"]) == 2, "the closing click must not act on a card"


@case
def cursor_toggle(s, env):
    """--cursor paints the pointer into a capture, --no-cursor leaves it out (CAP-11).
    Hyprland draws no pointer on headless outputs (grim -c shows none either), so this
    case captures the first real monitor with its own instance: no card, no clipboard,
    files deleted afterwards."""
    from PIL import Image, ImageChops
    real = next(m for m in hypr_json("monitors") if m["name"] != OUT)
    prev = hypr_json("cursorpos")
    t = Snip(s.build)
    t.env["SNIP_RUNTIME_DIR"] = str(t.run_dir) + "-cursor"
    t.run_dir = Path(t.env["SNIP_RUNTIME_DIR"])
    t.env["SNIP_APP_ID"] = "snip-e2e-cursor"
    t.env.pop("SNIP_OUTPUTS", None)
    t.write_config('[after_capture]\nscreenshot = ["save"]\n')
    cfg = (t.tmp / "cfg/snip/config.toml")
    cfg.write_text(cfg.read_text().replace("sounds = false\n", "sounds = false\nsave_also_copies = false\n"))
    try:
        t.start()
        lx, ly = real["width"] / real["scale"] / 2, real["height"] / real["scale"] / 2
        t.debug("move", real["name"], int(lx), int(ly)); time.sleep(0.4)
        a = t.json("fullscreen", "--monitor", real["name"], "--no-cursor")["result"]
        b = t.json("fullscreen", "--monitor", real["name"], "--cursor")["result"]
        assert t.status()["cards"] == [], "no card may appear on the user's monitor"
        ia, ib = Image.open(a["source"]).convert("RGB"), Image.open(b["source"]).convert("RGB")
        x, y = round(int(lx) * real["scale"]), round(int(ly) * real["scale"])
        near = ImageChops.difference(ia.crop((x - 10, y - 10, x + 60, y + 60)), ib.crop((x - 10, y - 10, x + 60, y + 60))).getbbox()
        assert near is not None, "no pointer in the --cursor capture"
    finally:
        t.stop()
        shutil.rmtree(t.tmp, ignore_errors=True)
        hypr_eval(f'hl.dispatch(hl.dsp.cursor.move({{ x = {prev["x"]}, y = {prev["y"]} }}))')


@case
def extract_text_feedback(s, env):
    """Extract Text on a card: text on the clipboard, and the card shows the result."""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "ocr.png"
    make_text_png(png, ["error: connection refused", "listening on port 5432"])
    rec = open_card(s, png); time.sleep(0.5)
    s.debug("extract", rec["id"])
    st = wait_for(lambda: [e for e in s.traces("card_status") if e["id"] == rec["id"]], 10)
    assert st and st[-1]["status"] == "text-copied", st
    assert st[-1]["detail"].endswith("characters"), st
    text = sh("wl-paste", "-n", check=False).stdout
    assert "connection refused" in text and "5432" in text, text


@case
def qr_on_card(s, env):
    """Extract Text reads a QR code exactly and copies it as sensitive."""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "qr.png"
    sh("qrencode", "-s", "8", "-o", str(png), "https://github.com/tornikegomareli/snip")
    rec = open_card(s, png); time.sleep(0.5)
    s.debug("extract", rec["id"])
    st = wait_for(lambda: [e for e in s.traces("card_status") if e["id"] == rec["id"]], 10)
    assert st and st[-1]["status"] == "qr-copied", st
    assert sh("wl-paste", "-n", check=False).stdout == "https://github.com/tornikegomareli/snip"
    assert "x-kde-passwordManagerHint" in sh("wl-paste", "-l", check=False).stdout.split()


@case
def self_timer(s, env):
    """CAP-06: --delay counts down on screen, then captures live; the countdown is
    never in the capture; the capture hotkey cancels a running countdown."""
    from PIL import Image, ImageChops
    s.cli("close-all"); time.sleep(0.3)
    s.move(5, 5); time.sleep(0.2)
    ref = s.json("fullscreen", "--monitor", OUT)["result"]
    s.cli("close-all"); time.sleep(0.4)
    # fullscreen after 2 s
    t0 = time.time()
    p = s.bg("fullscreen", "--monitor", OUT, "--delay", "2")
    time.sleep(0.6)
    assert layer_mapped("snip-countdown"), "no countdown on screen"
    sh("grim", "-o", OUT, str(s.tmp / "countdown.png"))
    r, code = finish(p, timeout=10)
    took = time.time() - t0
    assert code == 0 and 1.9 < took < 3.5, (code, took)
    a = Image.open(ref["source"]).convert("RGB"); b = Image.open(r["result"]["source"]).convert("RGB")
    cx, cy = a.width // 2, a.height // 2
    assert ImageChops.difference(a.crop((cx - 80, cy - 80, cx + 80, cy + 80)), b.crop((cx - 80, cy - 80, cx + 80, cy + 80))).getbbox() is None, "the countdown is in the capture"
    s.cli("close-all"); time.sleep(0.4)
    # area: select first, then count down, then capture that area live
    p = s.bg("area", "--delay", "2")
    time.sleep(0.5)
    s.drag(200, 200, 600, 400)
    time.sleep(0.4)
    assert layer_mapped("snip-countdown"), "no countdown after the selection"
    r, code = finish(p, timeout=10)
    assert code == 0 and (r["result"]["width"], r["result"]["height"]) == (500, 250), r
    s.cli("close-all"); time.sleep(0.4)
    # the hotkey cancels a running countdown
    p = s.bg("fullscreen", "--monitor", OUT, "--delay", "3")
    time.sleep(0.6)
    hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:area"))')
    r, code = finish(p, timeout=5)
    assert code == 1 and r["error"] == "cancelled", r
    assert wait_for(lambda: not layer_mapped("snip-countdown"), 2), "countdown still on screen"


@case
def scales_and_transforms(s, env):
    """Exact physical crops at every scale, and on a rotated output (SEL-11)."""
    for scale, transform in [(1, 0), (1.5, 0), (2, 0), (1.25, 1)]:
        env.configure(scale, transform)
        mon = env.monitor()
        sc = mon["scale"]
        p = s.bg("area"); time.sleep(0.6)
        s.drag(40, 40, 240, 140)
        r, code = finish(p)
        exp = (round(240 * sc) - round(40 * sc), round(140 * sc) - round(40 * sc))
        got = (r["result"]["width"], r["result"]["height"]) if r.get("ok") else r
        assert got == exp, f"scale {sc} transform {transform}: got {got}, want {exp}"
    env.configure(1.25, 0)


# ------------------------------------------------------------------ main

def pointer_idle(seconds=1.5):
    """The suite drives the seat's one pointer and grabs the keyboard: only
    run it while nobody is using the machine."""
    pos = hypr_json("cursorpos")
    for _ in range(int(seconds / 0.25)):
        time.sleep(0.25)
        if hypr_json("cursorpos") != pos:
            return False
    return True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--build", default=str(ROOT / "build"))
    ap.add_argument("--only", nargs="*")
    ap.add_argument("--keep", action="store_true", help="keep the temp dir")
    ap.add_argument("--force", action="store_true", help="run even if the pointer is moving")
    a = ap.parse_args()
    if not a.force and not pointer_idle():
        print("the pointer is in use: run the e2e suite while the machine is idle (or --force)")
        return 3

    env = Env()
    s = Snip(a.build)
    failed = []
    try:
        env.add_output()
        env.win = env.spawn_window("snip-e2e-target", "Snip window capture test\\nfn main() {}\\n")
        s.start()
        time.sleep(0.5)
        for fn in CASES:
            if a.only and fn.__name__ not in a.only:
                continue
            t0 = time.time()
            try:
                fn(s, env)
                print(f"  PASS {fn.__name__} ({time.time() - t0:.1f}s)")
            except Exception as e:
                failed.append(fn.__name__)
                print(f"  FAIL {fn.__name__}: {e}")
            s.cli("close-all", check=False)
            time.sleep(0.2)
    finally:
        pid = s.proc.pid if s.proc else None
        s.stop()
        env.cleanup()
        if pid:
            left = stray_layers(pid)
            if left:
                failed.append("stray_layers")
                print(f"  FAIL stray layers left mapped: {left}")
        shutil.rmtree(s.run_dir, ignore_errors=True)
        if a.keep or failed:
            print(f"  logs: {s.tmp}")
        else:
            shutil.rmtree(s.tmp, ignore_errors=True)
    print(f"{len(failed)} failed" if failed else "all passed")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
