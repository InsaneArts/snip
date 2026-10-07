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
        # The test instance's editor windows open on the test output, without
        # taking focus. One rule: Hyprland 0.56 drops `float` from another
        # matching rule when this one moves the window.
        ws = self.monitor()["activeWorkspace"]["id"]
        hypr_eval(f'hl.window_rule({{ match = {{ class = "^snip-e2e$" }}, workspace = "{ws} silent", float = true, center = true, '
                  f'no_initial_focus = true, opacity = "1 1" }})')

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


class Skip(Exception):
    """The environment can't run this case right now (said why); not a failure."""


def case(fn):
    CASES.append(fn)
    return fn


def area(s, x0, y0, x1, y1, during=None):
    p = s.bg("area")
    time.sleep(0.5)
    s.drag(x0, y0, x1, y1, during=during)
    return finish(p)


@case
def first_overlay_is_warm(s, env):
    """SEL-01: overlays map once, invisibly, at start, so the instance's first session
    appears about as fast as the rest (it took ~145 ms without that)."""
    assert not s.traces("overlay_presented"), "run this case first"
    p = s.bg("area")
    assert wait_for(lambda: s.traces("overlay_presented"), 3)
    s.move(400, 300); s.key("Escape")
    finish(p)
    begin = s.traces("capture_begin")[0]["t_ns"]
    shown = s.traces("overlay_presented")[0]["t_ns"]
    ms = (shown - begin) / 1e6
    print(f"      first overlay: {ms:.1f} ms")
    assert ms < 100, ms


@case
def first_card_is_warm(s, env):
    """The Quick Access window also maps once at start: the first card starts moving about
    as soon as later ones do (it took ~90 ms without that)."""
    assert not s.traces("qao_motion"), "run this case before any card"
    png = s.tmp / "first-card.png"
    make_text_png(png, ["first card"])
    t0 = time.monotonic_ns()                        # the same clock as the traces
    rec = open_card(s, png)
    assert wait_for(lambda: [e for e in s.traces("qao_motion") if e["id"] == rec["id"]], 3)
    ms = ([e for e in s.traces("qao_motion") if e["id"] == rec["id"]][0]["t_ns"] - t0) / 1e6
    print(f"      first card moving after {ms:.1f} ms")
    s.cli("close-all"); time.sleep(0.6)
    assert ms < 60, ms


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
    s.move(400, 300)   # Escape goes to the overlay under the pointer
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
        # Hyprland sometimes paints no pointer into any capture while a headless
        # output exists (grim -c neither): then there is nothing of Snip's to test.
        g0, g1 = t.tmp / "g0.png", t.tmp / "g1.png"
        sh("grim", "-o", real["name"], str(g0)); sh("grim", "-c", "-o", real["name"], str(g1))
        if ImageChops.difference(Image.open(g0).convert("RGB"), Image.open(g1).convert("RGB")).getbbox() is None:
            raise Skip("Hyprland paints no pointer into captures right now (grim -c shows none either)")
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
def rotate_and_flip(s, env):
    """QAO-09 Rotate Left / Flip Horizontal rewrite the capture and its saved file."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "marker.png"
    im = Image.new("RGB", (300, 150), "white")
    im.paste((255, 0, 0), (0, 0, 30, 30))           # red marker, top-left
    im.save(png)
    rec = open_card(s, png); time.sleep(0.4)
    saved = Path(s.json("save", rec["id"])["result"])
    def get():
        return next(r for r in s.json("list", "-n", "20")["result"] if r["id"] == rec["id"])
    s.debug("transform", rec["id"], "rotate-left")
    assert wait_for(lambda: (get()["width"], get()["height"]) == (150, 300), 5), get()
    a = Image.open(get()["source"]).convert("RGB")
    assert a.getpixel((5, 295))[0] > 200 and a.getpixel((5, 5))[1] > 200, "marker should move to the bottom-left"
    assert Image.open(saved).size == (150, 300), "saved file not rewritten"
    s.debug("transform", rec["id"], "flip-horizontal")
    assert wait_for(lambda: len(s.traces("transformed")) >= 2, 5)
    b = Image.open(get()["source"]).convert("RGB")
    assert b.getpixel((145, 295))[0] > 200 and b.getpixel((5, 295))[1] > 200, "marker should move to the bottom-right"


def noise_png(path, w, h, seed=7):
    """Random pixels: any resampling or colour change in a round trip shows."""
    import random
    from PIL import Image
    rnd = random.Random(seed)
    Image.frombytes("RGB", (w, h), bytes(rnd.randrange(256) for _ in range(w * h * 3))).save(path)


def editor_state(s):
    n = len(s.traces("editor_state"))
    s.debug("ui", "editor", "state")
    assert wait_for(lambda: len(s.traces("editor_state")) > n, 3), "no editor state"
    return s.traces("editor_state")[-1]


def editor_flush(s):
    n = len(s.traces("edited"))
    s.debug("ui", "editor", "flush")
    assert wait_for(lambda: len(s.traces("edited")) > n, 5), "the edit was not written back"


def editor_window():
    return next((c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"].startswith("Snip Editor")), None)


def close_editor(s):
    s.debug("ui", "editor", "close")
    assert wait_for(lambda: editor_window() is None, 5), "the editor did not close"


@case
def editor_round_trip(s, env):
    """ANN-15/16/19: snip edit FILE opens the editor; the export is pixel-exact outside the
    drawn object at a fractional scale; undo restores every pixel; the original and the
    objects are kept, so reopening keeps them editable."""
    from PIL import Image
    png = s.tmp / "noise.png"
    noise_png(png, 333, 217)                        # odd sizes: 333 px is not a whole number of logical px at 1.25
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5), "no editor window"
    w = editor_window()
    assert w["floating"] and w["monitor"] == env.monitor()["id"], w
    assert wait_for(lambda: editor_state(s)["ready"], 3)
    assert not s.status()["cards"], "snip edit shows no card"

    s.debug("ui", "editor", "rect", 100, 60, 120, 80)
    editor_flush(s)
    orig, out = Image.open(png).convert("RGB"), Image.open(rec["source"]).convert("RGB")
    assert out.size == orig.size, out.size
    inside = lambda x, y: 80 <= x <= 240 and 40 <= y <= 160
    diff = [(x, y) for y in range(217) for x in range(333) if not inside(x, y) and out.getpixel((x, y)) != orig.getpixel((x, y))]
    assert not diff, f"{len(diff)} pixels changed outside the rectangle, e.g. {diff[:3]}"
    r, g, b = out.getpixel((100, 100))                # on the rectangle's left edge
    assert r > 200 and g < 90, (r, g, b)
    hist = Path(rec["source"]).parent
    assert (hist / "original.png").exists() and json.loads((hist / "edit.json").read_text())["items"][0]["kind"] == "rect"

    s.debug("ui", "editor", "undo")
    editor_flush(s)
    assert Image.open(rec["source"]).convert("RGB").tobytes() == orig.tobytes(), "undo must restore every pixel"
    s.debug("ui", "editor", "redo")
    editor_flush(s)
    close_editor(s)

    s.debug("annotate", rec["id"])                  # the card's pencil / double-click
    assert wait_for(lambda: editor_window(), 5), "annotate did not open the editor"
    st = editor_state(s)
    assert [i["kind"] for i in st["items"]] == ["rect"] and st["undo"] == 0, st
    close_editor(s)


@case
def editor_from_card(s, env):
    """QAO-11: double-clicking a card opens Snip's editor on that capture, and the edit
    reaches the card's capture."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "card-edit.png"
    Image.new("RGB", (500, 300), "white").save(png)
    rec = open_card(s, png); time.sleep(0.6)
    mon = env.monitor()
    H = mon["height"] / mon["scale"]
    cw, ch = 280, round(280 * 0.705)
    s.move(24 + cw / 2, H - 24 - ch / 2); time.sleep(0.3)
    s.click(); time.sleep(0.05); s.click()
    assert wait_for(lambda: editor_window(), 5), "double-click did not open the editor"
    assert editor_state(s)["id"] == rec["id"]
    s.debug("ui", "editor", "rect", 100, 100, 200, 100)
    editor_flush(s)
    source = next(r for r in s.json("list", "-n", "20")["result"] if r["id"] == rec["id"])["source"]
    assert Image.open(source).convert("RGB").getpixel((100, 150))[1] < 120, "the edit did not reach the capture"
    assert rec["id"] in s.status()["cards"], "the card should stay"
    close_editor(s)


@case
def editor_draws_with_pointer(s, env):
    """Drawing an arrow and moving it with the pointer; Copy writes back, copies and closes."""
    from PIL import Image
    png = s.tmp / "white.png"
    Image.new("RGB", (400, 260), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5)
    assert wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)

    def at(ix, iy):                                   # image px -> output-local logical px
        return (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])

    s.drag(*at(60, 200), *at(300, 60), steps=6)
    time.sleep(0.2)
    st = editor_state(s)
    assert len(st["items"]) == 1 and st["items"][0]["kind"] == "arrow", st["items"]
    a = st["items"][0]
    assert abs(a["x1"] - 60) <= 3 and abs(a["y2"] - 60) <= 3, a
    s.drag(*at(120, 165), *at(120, 215), steps=4)     # grab it a quarter along, move it down 50
    time.sleep(0.2)
    b = editor_state(s)["items"][0]
    assert len(editor_state(s)["items"]) == 1 and abs((b["y1"] - a["y1"]) - 50) <= 3 and abs(b["cy"] - a["cy"] - 50) <= 3, (a, b)
    s.drag(*at(180, 180), *at(180, 230), steps=4)     # its middle is the bend handle
    c = editor_state(s)["items"][0]
    assert abs(c["y1"] - b["y1"]) < 1 and c["cy"] - b["cy"] > 80, (b, c)
    n = len(s.traces("copied"))
    s.debug("ui", "editor", "flush", "copy")
    assert wait_for(lambda: len(s.traces("copied")) > n and editor_window() is None, 5), "Copy did not copy and close"
    px = Image.open(rec["source"]).convert("RGB").getpixel((round(c["x2"]), round(c["y2"])))  # the arrow's tip
    assert px[0] > 200 and px[1] < 90, px


@case
def editor_keyboard(s, env):
    """ANN-05/14: the Text tool types where you click and Enter commits; tool keys, undo,
    redo and Escape work. Keys are delivered inside snipd (never through the seat)."""
    from PIL import Image
    png = s.tmp / "keys.png"
    Image.new("RGB", (500, 260), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.move(*at(250, 200)); s.click(); time.sleep(0.2)       # focus the editor
    s.debug("ui", "editor", "keys", "t")
    assert wait_for(lambda: editor_state(s)["tool"] == "text", 2), editor_state(s)["tool"]
    s.move(*at(60, 100)); s.click(); time.sleep(0.2)
    assert editor_state(s)["editingText"], "clicking with the Text tool should start typing"
    s.debug("ui", "editor", "keys", "Hello", "snip{Return}")
    st = editor_state(s)
    assert [i.get("text") for i in st["items"]] == ["Hello snip"] and not st["editingText"], st
    s.debug("ui", "editor", "keys", "{ctrl+z}")
    assert editor_state(s)["items"] == []
    s.debug("ui", "editor", "keys", "{ctrl+shift+z}")
    assert len(editor_state(s)["items"]) == 1
    s.debug("ui", "editor", "keys", "r")
    assert editor_state(s)["tool"] == "rect"
    editor_flush(s)
    px = Image.open(rec["source"]).convert("RGB")
    assert any(px.getpixel((x, y))[1] < 100 for x in range(60, 200) for y in range(85, 115)), "the text should be drawn"
    s.debug("ui", "editor", "keys", "{Escape}")     # deselect
    s.debug("ui", "editor", "keys", "{Escape}")     # close
    assert wait_for(lambda: editor_window() is None, 5), "Escape with nothing selected closes the editor"


@case
def editor_redaction_is_secure(s, env):
    """ANN-06 / D26: Pixelate, Blur and Black out never carry the hidden content. Two
    images that differ only inside the regions export identically, and the hidden text
    leaves the history search."""
    from PIL import Image, ImageDraw, ImageFont
    try:
        font = ImageFont.truetype("/usr/share/fonts/noto/NotoSans-Regular.ttf", 26)
    except OSError:
        font = ImageFont.load_default(26)
    regions = [(40, 40, 360, 50, "pixelate"), (40, 120, 360, 50, "blur"), (40, 200, 360, 50, "black")]
    outs = []
    for secret in ("hunter2 swordfish", "Q7XK9 mangosteen"):
        im = Image.new("RGB", (480, 300), (236, 240, 247))
        d = ImageDraw.Draw(im)
        d.text((14, 8), "visible header", fill=(30, 30, 30), font=font)
        for (x, y, w, h, _) in regions:
            d.text((x + 8, y + 10), secret, fill=(10, 10, 10), font=font)
        png = s.tmp / f"secret-{len(outs)}.png"
        im.save(png)
        rec = s.json("edit", str(png))["result"]
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
        if not outs:
            assert wait_for(lambda: s.json("search", "swordfish")["result"], 8), "the secret should be searchable first"
        for (x, y, w, h, style) in regions:
            s.debug("ui", "editor", "redact", x, y, w, h, style, 42)
        editor_flush(s)
        close_editor(s)
        outs.append(rec)
    a, b = (Image.open(r["source"]).convert("RGB") for r in outs)
    assert a.tobytes() == b.tobytes(), "the hidden content changed the redacted image"
    px = [a.getpixel((x + w // 2, y + h // 2)) for (x, y, w, h, _) in regions]
    assert px[2] == (0, 0, 0) and all(abs(c - 238) < 40 for c in px[0]), px   # black out; tiles from the light surroundings
    assert wait_for(lambda: not s.json("search", "swordfish")["result"], 8), "redacted text is still searchable"


@case
def editor_crop(s, env):
    """ANN-11: a crop exports exactly that part of the image; resetting brings back all of it."""
    from PIL import Image
    png = s.tmp / "crop.png"
    noise_png(png, 301, 203, seed=3)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "crop", 51, 37, 199, 101)
    s.debug("ui", "editor", "rect", 60, 50, 40, 30)
    editor_flush(s)
    orig = Image.open(png).convert("RGB")
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (199, 101), out.size
    assert out.getpixel((150, 80)) == orig.getpixel((201, 117)), "cropped pixels must be exact"
    assert out.getpixel((9 + 20, 13)) != orig.getpixel((60 + 20, 50)), "the rectangle is drawn in the crop"
    s.debug("ui", "editor", "crop", "reset")
    s.debug("ui", "editor", "delete", 0)
    editor_flush(s)
    assert Image.open(rec["source"]).convert("RGB").tobytes() == orig.tobytes()

    # with the pointer: the crop tool's bottom-right handle, dragged in by 100 x 50
    s.debug("ui", "editor", "tool", "crop")
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.drag(*at(301, 203), *at(201, 153), steps=5)
    time.sleep(0.2)
    c = editor_state(s)["crop"]
    assert c and c["x"] == 0 and c["y"] == 0 and abs(c["w"] - 201) <= 2 and abs(c["h"] - 153) <= 2, c
    close_editor(s)


@case
def editor_from_clipboard(s, env):
    """ANN-19: snip edit --clipboard opens the clipboard's image (from another app), or a
    copied PNG file, in the editor; text alone is a clean error."""
    from PIL import Image
    png = s.tmp / "clip me.png"
    noise_png(png, 120, 80, seed=11)
    try:
        subprocess.run(["wl-copy", "--type", "image/png"], stdin=open(png, "rb"), check=True)
        time.sleep(0.3)
        rec = s.json("edit", "--clipboard")
        assert rec["ok"], rec
        assert wait_for(lambda: editor_window(), 5), "no editor for the clipboard image"
        assert Image.open(rec["result"]["source"]).convert("RGB").tobytes() == Image.open(png).convert("RGB").tobytes()
        close_editor(s)

        card = open_card(s, png)                      # Snip's own copy: it reads its own selection
        s.cli("copy", card["id"])
        time.sleep(0.3)
        rec = s.json("edit", "--clipboard")
        assert rec["ok"] and wait_for(lambda: editor_window(), 5), rec
        close_editor(s)

        uri = "file://" + str(png).replace(" ", "%20")
        subprocess.run(["wl-copy", "--type", "text/uri-list", uri + "\r\n"], check=True)
        time.sleep(0.3)
        rec = s.json("edit", "--clipboard")
        assert rec["ok"] and rec["result"]["title"] == "clip me", rec
        assert wait_for(lambda: editor_window(), 5)
        close_editor(s)

        subprocess.run(["wl-copy", "just some text"], check=True)
        time.sleep(0.3)
        r = s.cli("edit", "--clipboard", check=False)
        assert r.returncode == 2 and "no image" in r.stderr, (r.returncode, r.stderr)
    finally:
        subprocess.run(["wl-copy", "--clear"], check=False)


@case
def save_as(s, env):
    """QAO-09 / ANN-15 Save As: the format follows the extension, the file becomes the
    capture's saved file and is copied. (The portal dialog itself is checked by hand: it
    opens on the user's monitor and takes the keyboard.)"""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "saveas.png"
    make_text_png(png, ["save me as"])
    rec = open_card(s, png); time.sleep(0.3)
    n = len(s.traces("copied"))
    out = s.json("debug", "save-as", rec["id"], str(s.tmp / "picked.webp"))
    assert out["ok"] and out["result"].endswith("picked.webp"), out
    assert (s.tmp / "picked.webp").read_bytes()[8:12] == b"WEBP"
    saved = next(r for r in s.json("list", "-n", "20")["result"] if r["id"] == rec["id"])["saved"]
    assert saved == str(s.tmp / "picked.webp"), saved
    assert wait_for(lambda: len(s.traces("copied")) > n, 2), "Save As also copies"


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
    failed, skipped = [], []
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
            except Skip as e:
                skipped.append(fn.__name__)
                print(f"  SKIP {fn.__name__}: {e}")
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
    print((f"{len(failed)} failed" if failed else "all passed") + (f" ({len(skipped)} skipped: {', '.join(skipped)})" if skipped else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
