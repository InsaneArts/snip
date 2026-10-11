#!/usr/bin/env python3
"""End-to-end tests for snipd, in a Hyprland of their own.

The suite starts a nested Hyprland inside the user's session, as a window
on a hidden special workspace. Every test monitor, window, pointer motion,
key and clipboard write happens in there: the user's monitors never change
(GTK 4.22 apps crash on output hot-plug), their pointer never moves and the
suite can run while they work. Inside it, cases use a temporary headless
output and an isolated snipd instance (own XDG dirs and ctl socket), driven
by Snip's virtual pointer and test hooks.

    tests/e2e/e2e.py [--build build] [--only NAME ...] [--keep] [--live]

--live runs against the user's own session instead (the old way: it adds a
headless output there and moves their pointer, so only while they're away).

Exit code 0 when every case passes. Cleanup always runs: the daemon quits,
the nested Hyprland exits, and no snip layer may be left mapped.
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
OUT = "SNIPE2E"          # the test output: a headless one (--live), or the nested Hyprland's own
UID = os.getuid()


def sh(*args, check=True, **kw):
    return subprocess.run(args, check=check, capture_output=True, text=True, **kw)


class Nested:
    """A Hyprland for the suite, nested in the user's session as a hidden window.

    Its own output (WAYLAND-1, the window) is the test output: headless outputs
    can't allocate buffers when nested on NVIDIA (aquamarine 0.15 asks for
    linear ones). render_unfocused keeps the hidden window drawing."""

    OUTPUT = "WAYLAND-1"

    CONFIG = """hl.config({
  xwayland = { enabled = false },
  misc = { disable_hyprland_logo = true, disable_splash_rendering = true },
  animations = { enabled = false },
})
"""

    def __init__(self):
        self.dir = Path(tempfile.mkdtemp(prefix="snip-e2e-hypr-"))
        self.proc = None

    def start(self):
        """Start it; returns the environment that points tools and apps at it."""
        # its window (app id "aquamarine") goes to a special workspace that is never shown
        hypr_eval('hl.window_rule({ match = { class = "^aquamarine$" }, workspace = "special:snip-e2e silent", no_initial_focus = true, '
                  'render_unfocused = true, float = true, size = { 1920, 1080 } })')
        before = {i["instance"] for i in json.loads(sh("hyprctl", "instances", "-j").stdout)}
        (self.dir / "hyprland.lua").write_text(self.CONFIG)
        env = dict(os.environ, HYPRLAND_NO_SD_VARS="1", HYPRLAND_NO_SD_NOTIFY="1", HYPRLAND_NO_CRASHREPORTER="1")
        env.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
        self.proc = subprocess.Popen(["Hyprland", "--config", str(self.dir / "hyprland.lua")], env=env,
                                     stdout=open(self.dir / "hyprland.out", "w"), stderr=subprocess.STDOUT)
        for _ in range(100):
            time.sleep(0.1)
            new = [i for i in json.loads(sh("hyprctl", "instances", "-j").stdout) if i["instance"] not in before]
            if new:
                time.sleep(0.5)
                return {"HYPRLAND_INSTANCE_SIGNATURE": new[0]["instance"], "WAYLAND_DISPLAY": new[0]["wl_socket"]}
        raise RuntimeError(f"the nested Hyprland did not start (see {self.dir})")

    def stop(self):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        shutil.rmtree(self.dir, ignore_errors=True)


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
            SNIP_NO_PORTALS="1",      # no portal dialog may open on the user's screen
            SNIP_NO_TRAY="1",
            # KMS capture can't see the nested Hyprland: ffmpeg's test pattern stands in for gsr
            SNIP_RECORDER="fake",         # no second Snip icon in the user's bar (D-Bus is shared)
            SNIP_CAMERA="fake",           # there's no camera here: a test card stands in for one
            QT_FORCE_STDERR_LOGGING="1",
        )
        (self.tmp / "cfg/snip").mkdir(parents=True)
        self.write_config("")
        (self.tmp / "data/snip").mkdir(parents=True)
        (self.tmp / "data/snip/welcomed").touch()       # the first-run Welcome stays shut (welcome_first_run opens it)
        self.proc = None
        self.log = self.tmp / "snipd.err"

    def write_config(self, extra):
        # objects cast no shadow here, so pixel checks next to them stay exact;
        # editor_object_shadows turns them on
        (self.tmp / "cfg/snip/config.toml").write_text(
            f'[general]\nexport_dir = "{self.export}"\nsounds = false\n{extra}\n[annotate]\nobject_shadows = false\n'
            f'[recording]\nexport_dir = "{self.export}"\ncountdown = 1\n'
        )

    def start(self):
        self.proc = subprocess.Popen(
            [str(self.build / "snipd")], env=self.env, stdout=subprocess.DEVNULL, stderr=open(self.log, "a")
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
        if args and args[0] in ("area", "window", "text", "area-pin", "scroll") and "--monitor" not in args:
            args = (*args, "--monitor", OUT)
        return subprocess.Popen(
            [str(self.build / "snip"), "--json", *args], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
        )

    def debug(self, *args):
        self.cli("debug", *map(str, args))

    def move(self, x, y, out=None):
        self.debug("move", out or OUT, int(x), int(y))
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


def png_size_bytes(b):
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


def where(s, window, name):
    """A window's probe (probe.js): where its control `name` is, {"x", "y"} in logical px
    of the window (a layer's: of its output); -1, -1 when it isn't shown."""
    n = len(s.traces(window + "_where"))
    s.debug("ui", window, "where", name)
    assert wait_for(lambda: len(s.traces(window + "_where")) > n, 2), f"no {window}_where for {name}"
    return s.traces(window + "_where")[-1]


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
    def __init__(self, own_output=True):
        self.own_output = own_output     # created here, so removed at cleanup
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
        # reserved: none (a case may have reserved space, as a bar does)
        hypr_eval(f'hl.monitor({{ output = "{OUT}", mode = "1920x1080@60", position = "auto", scale = {scale}, transform = {transform}, reserved = 0 }})')
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
        if self.own_output:
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
    # nested, the output draws at 15 fps while unfocused: a frame can add 67 ms
    # (28-104 ms there; the cold path took ~145 ms even on a real output)
    assert ms < (120 if OUT == Nested.OUTPUT else 100), ms


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
    # nested, the output draws at 15 fps while unfocused: a frame can add 67 ms
    # to the ~9 ms it takes (the cold path took ~90 ms even on a real output)
    assert ms < (80 if OUT == Nested.OUTPUT else 60), ms


@case
def size_readout(s, env):
    """SEL-03: the size pill shows physical px (at 1.25: 250 × 100 for 200 × 80 logical), or
    logical px with capture.logical_size."""
    def readout():
        p = s.bg("area"); time.sleep(0.5)
        s.move(100, 100); s.debug("down"); s.move(200, 140); s.move(300, 180); time.sleep(0.2)
        n = len(s.traces("overlay_state"))
        s.debug("ui", "state")
        assert wait_for(lambda: len(s.traces("overlay_state")) > n, 2)
        size = s.traces("overlay_state")[-1]["size"]
        s.debug("up")
        finish(p)
        return size

    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    try:
        assert readout() == "250 × 100", "physical px"
        cfg.write_text(before.replace("[capture]\n", "[capture]\nlogical_size = true\n", 1) if "[capture]\n" in before
                       else before + "\n[capture]\nlogical_size = true\n")
        time.sleep(0.8)
        assert readout() == "200 × 80", "logical px"
    finally:
        cfg.write_text(before)
        time.sleep(0.8)
        s.cli("close-all")


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
    """CleanShot's keys (SEL-07, D32): Alt = square, Ctrl = from centre, Space held = move."""
    r, _ = area(s, 100, 100, 300, 200, during=lambda: s.key("Alt", "keydown"))
    s.key("Alt", "keyup")
    assert (r["result"]["width"], r["result"]["height"]) == (250, 250), r
    r, _ = area(s, 400, 400, 450, 430, during=lambda: s.key("Control", "keydown"))
    s.key("Control", "keyup")
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
def hotkey_again_while_opening_cancels(s, env):
    """SEL-09: the capture hotkey again cancels the selection, also while it's still
    opening (the cards stepping aside first, D18): nothing opens, and the cards are back."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "opening.png"
    noise_png(png, 400, 300, seed=11)
    rec = open_card(s, png); time.sleep(0.8)
    c = where(s, "qa", "card:" + rec["id"])
    box = tuple(round(v * 1.25) for v in (c["x"] - 40, c["y"] - 30, c["x"] + 40, c["y"] + 30))

    def card_pixels():
        f = s.tmp / "opening-screen.png"
        sh("grim", "-o", OUT, str(f))
        return Image.open(f).convert("RGB").crop(box).tobytes()

    before = card_pixels()
    try:
        p = s.bg("area")
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:area"))')       # again, at once
        try:
            r, code = finish(p, timeout=5)
        except subprocess.TimeoutExpired:
            s.key("Escape"); finish(p, timeout=5)
            raise AssertionError("the second press didn't cancel it: the selection opened")
        assert code == 1 and r.get("error") == "cancelled", r
        time.sleep(1.0)
        assert s.status()["overlay"] is None, "nothing opened"
        assert wait_for(lambda: card_pixels() == before, 3), "the cards are back"
    finally:
        s.cli("close-all")


@case
def layers_appear_without_a_fade(s, env):
    """Snip's layers show at once, whatever the layer animations are (Omarchy fades layers
    in over 0.2-0.4 s): its rule must match their whole namespace (Hyprland 0.56). With a
    3 s fade on, the window-mode overlay is fully dimmed 0.3 s after it shows."""
    from PIL import Image

    def lum():
        f = s.tmp / "fade.png"
        sh("grim", "-o", OUT, str(f))
        b = Image.open(f).convert("L").crop((700, 300, 1100, 600)).tobytes()
        return sum(b) / len(b)

    try:
        hypr_eval('hl.config({ animations = { enabled = true } })')
        hypr_eval('hl.animation({ leaf = "layersIn", enabled = true, speed = 30, bezier = "linear", style = "fade" })')
        hypr_eval('hl.animation({ leaf = "fadeLayersIn", enabled = true, speed = 30, bezier = "linear" })')
        s.move(1500, 20); time.sleep(0.4)
        base = lum()
        n = len(s.traces("overlay_presented"))
        p = s.bg("window")
        assert wait_for(lambda: len(s.traces("overlay_presented")) > n, 4)
        time.sleep(0.3)
        shown = lum()
        s.key("Escape"); finish(p, timeout=5)
        assert shown < base * 0.7, ("dimmed at once, not fading in", base, shown)
    finally:
        hypr_eval('hl.config({ animations = { enabled = false } })')


@case
def overlay_takes_the_pointer_at_once(s, env):
    """SEL-01: the overlay takes the pointer as soon as it shows. Hyprland hands a new layer
    pointer focus only on motion, so Snip nudges the pointer; else a press made without
    moving first goes to the app underneath (a drag selects its text, not an area). In a
    fresh snipd, one that has never moved a pointer itself."""
    s.stop(); s.start(); time.sleep(1.5)
    n = len(s.traces("pointer_nudged"))
    p = s.bg("area")
    try:
        assert wait_for(lambda: len(s.traces("pointer_nudged")) > n, 3), "nudged"
        assert s.traces("pointer_nudged")[-1]["moved"], "the nudge moved the pointer"
    finally:
        s.key("Escape"); finish(p, timeout=5)


@case
def area_alt_square_at_an_edge(s, env):
    """SEL-07 (D32): Alt makes a square, also where the screen's edge stops the drag: as
    big a square as fits, not one cut into a rectangle."""
    r, _ = area(s, 100, 800, 300, 840, during=lambda: s.key("Alt", "keydown"))
    s.key("Alt", "keyup")
    assert r["ok"] and r["result"]["width"] == r["result"]["height"], (r["result"]["width"], r["result"]["height"])


@case
def window_mode_enter_takes_the_monitor(s, env):
    """SEL-06: Enter captures the picked window, or the monitor when none is picked, in
    window mode (snip window) too."""
    mon = env.monitor()
    lx = env.win["at"][0] - mon["x"]
    p = s.bg("window"); time.sleep(0.5)
    s.move(lx // 2 if lx > 4 else 3, 400); time.sleep(0.2)      # beside the window: none under it
    s.key("Return")
    try:
        r, code = finish(p, timeout=6)
    except subprocess.TimeoutExpired:
        s.key("Escape"); finish(p, timeout=5)
        raise AssertionError("Enter did nothing")
    assert code == 0 and r["result"]["kind"] == "fullscreen", r
    assert (r["result"]["width"], r["result"]["height"]) == (mon["width"], mon["height"]), r


@case
def window_pick_on_scratchpad(s, env):
    """CAP-05: a window on a shown special workspace (Omarchy's scratchpad) can be picked;
    it's drawn over the others, and the one behind it isn't what's captured."""
    cmd = "foot --title=snip-e2e-scratch --app-id=snip-e2e-scratch sh -c 'printf scratch; sleep 3600'"
    hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "special:scratchpad silent" }}))')
    try:
        assert wait_for(lambda: any(c["class"] == "snip-e2e-scratch" for c in hypr_json("clients")), 5), "it opens"
        hypr_eval('hl.dispatch(hl.dsp.workspace.toggle_special("scratchpad"))')
        assert wait_for(lambda: env.monitor()["specialWorkspace"]["id"] != 0, 3), "the scratchpad shows"
        time.sleep(0.4)
        c = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-scratch")
        mon = env.monitor()
        p = s.bg("area"); time.sleep(0.5)
        s.move(c["at"][0] - mon["x"] + c["size"][0] / 2, c["at"][1] - mon["y"] + c["size"][1] / 2)
        s.key("space", "keydown"); s.click()
        r, code = finish(p, timeout=8)
        s.key("space", "keyup")
        assert code == 0 and r["result"].get("app") == "snip-e2e-scratch", (r["result"].get("kind"), r["result"].get("app"))
    finally:
        if env.monitor()["specialWorkspace"]["id"] != 0:
            hypr_eval('hl.dispatch(hl.dsp.workspace.toggle_special("scratchpad"))')
        subprocess.run(["pkill", "-f", "app-id=snip-e2e-scratch"], check=False)
        time.sleep(0.4)


@case
def area_enter_mid_drag_takes_the_area(s, env):
    """SEL-09: Enter while dragging takes the area as dragged, not the whole monitor."""
    p = s.bg("area"); time.sleep(0.5)
    s.move(100, 100); s.debug("down"); s.move(200, 150); s.move(300, 200)
    time.sleep(0.2)
    s.key("Return"); s.debug("up")
    try:
        r, code = finish(p, timeout=6)
    except subprocess.TimeoutExpired:
        s.key("Escape"); finish(p, timeout=5)
        raise AssertionError("Enter took nothing")
    assert code == 0 and (r["result"]["width"], r["result"]["height"]) == (250, 125), (r["result"]["width"], r["result"]["height"])


@case
def crosshair_points_where_the_area_starts(s, env):
    """SEL-04/05: the crosshair, its readout and the magnifier show the pixel a selection
    starting there starts at (at 1.25, (102, 102) logical is 127.5: pixel 128)."""
    p = s.bg("area"); time.sleep(0.5)
    s.move(102, 102); time.sleep(0.2)
    n = len(s.traces("overlay_state")); s.debug("ui", "state")
    assert wait_for(lambda: len(s.traces("overlay_state")) > n, 2)
    cursor = s.traces("overlay_state")[-1]["cursor"]
    s.drag(102, 102, 302, 302)
    r, code = finish(p, timeout=6)
    start = s.traces("finish_area")[-1]["rect"][:2]
    assert cursor == start, ("the crosshair's pixel", cursor, "the area's corner", start)


@case
def area_space_move_into_an_edge_keeps_its_size(s, env):
    """SEL-07: Space moves the selection; pushed into the screen's edge it stops there, and
    after Space the drag goes on from where the corner is: the size doesn't collapse."""
    p = s.bg("area"); time.sleep(0.5)
    s.move(500, 500); s.debug("down"); s.move(550, 550); s.move(600, 600)
    s.key("space", "keydown"); s.move(300, 600); s.move(0, 600); s.key("space", "keyup")
    s.move(1, 600); s.move(2, 600); s.debug("up")
    r, code = finish(p, timeout=6)
    w, h = r["result"]["width"], r["result"]["height"]
    assert w >= 120 and h == 125, ("about the 100 x 100 moved, not a strip", w, h)


@case
def window_mode_skips_windows_behind_a_fullscreen_one(s, env):
    """CAP-05: window mode picks what's seen: a fullscreen window hides the rest of its
    workspace, so a click over a hidden window captures the fullscreen one."""
    cmd = "foot --title=snip-e2e-cover --app-id=snip-e2e-cover sh -c 'printf cover; sleep 3600'"
    ws = env.monitor()["activeWorkspace"]["id"]
    hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "{ws} silent" }}))')
    try:
        assert wait_for(lambda: any(c["class"] == "snip-e2e-cover" for c in hypr_json("clients")), 5), "it opens"
        c = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-cover")
        hypr_eval(f'hl.dispatch(hl.dsp.window.fullscreen({{ mode = "fullscreen", window = "address:{c["address"]}" }}))')
        assert wait_for(lambda: next(x for x in hypr_json("clients") if x["class"] == "snip-e2e-cover")["fullscreen"] == 2, 3), "fullscreen"
        time.sleep(0.4)
        w = env.win
        mon = env.monitor()
        p = s.bg("window"); time.sleep(0.5)
        s.move(w["at"][0] - mon["x"] + 40, w["at"][1] - mon["y"] + 40); time.sleep(0.2); s.click()
        r, code = finish(p, timeout=8)
        assert code == 0 and r["result"].get("app") == "snip-e2e-cover", (r["result"].get("kind"), r["result"].get("app"))
    finally:
        subprocess.run(["pkill", "-f", "app-id=snip-e2e-cover"], check=False)
        time.sleep(0.5)


@case
def area_live_leaves_the_cards_out(s, env):
    """CAP-10, D18: with the screen not frozen (capture.freeze = false) the area is taken
    live, after the overlay; Snip's cards step aside for it too."""
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    try:
        assert "[capture]" not in before, before
        cfg.write_text(before + "\n[capture]\nfreeze = false\n")
        time.sleep(0.8)
        s.cli("close-all"); time.sleep(0.4)
        r0, _ = area(s, 0, 560, 600, 860)              # where cards sit, none there
        png = s.tmp / "live.png"
        noise_png(png, 400, 300, seed=13)
        open_card(s, png); time.sleep(0.8)
        r1, _ = area(s, 0, 560, 600, 860)
        from PIL import Image
        a = Image.open(r0["result"]["source"]).convert("RGB").tobytes()
        b = Image.open(r1["result"]["source"]).convert("RGB").tobytes()
        assert a == b, "the card was captured"
    finally:
        cfg.write_text(before); time.sleep(0.8)
        s.cli("close-all")


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
def window_captures_give_their_buffers_back(s, env):
    """Every window captured kept its capture buffers (4 MB here, ~10 MB on a big screen)
    until snipd quit, even once the window was gone; only the last window's are kept, for
    a scrolling capture to reuse."""
    def shmem():
        status = dict(l.split(":", 1) for l in open(f"/proc/{s.proc.pid}/status") if ":" in l)
        return int(status["RssShmem"].split()[0]) / 1024
    mon, sizes = env.monitor(), []
    for i in range(4):
        app = f"snip-e2e-buf{i}"
        env.spawn_window(app, f"window {i}")
        try:
            c = next(x for x in hypr_json("clients") if x["class"] == app)    # tiled beside the target
            p = s.bg("window"); time.sleep(0.5)
            s.move(c["at"][0] - mon["x"] + c["size"][0] / 2, c["at"][1] - mon["y"] + c["size"][1] / 2); time.sleep(0.2); s.click()
            r, code = finish(p, timeout=8)
            assert code == 0 and r["result"].get("app") == app, (code, r)
        finally:
            subprocess.run(["pkill", "-f", f"app-id={app}"], check=False)
            time.sleep(0.5)
        sizes.append(round(shmem(), 1))
    s.cli("close-all")
    print(f"      shared memory after each window: {sizes} MB")
    assert sizes[3] - sizes[0] < 6, ("MB after each window", sizes)


@case
def window_with_desktop(s, env):
    """Capture Window "With the desktop" (product owner, 2026-10-08): the window as it is on
    screen with 48 px of the desktop around it (wallpaper, other windows, the bar), opaque,
    instead of the clean window on transparency. Shift when clicking gives the clean one."""
    from PIL import Image
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before + "\n[capture]\nwindow_desktop = true\n")
    try:
        time.sleep(0.8)                                                  # the config watcher reloads it
        w, mon = env.win, env.monitor()
        lx, ly = w["at"][0] - mon["x"], w["at"][1] - mon["y"]
        sc, m = mon["scale"], round(48 * mon["scale"])
        x0, y0 = round(lx * sc), round(ly * sc)
        x1, y1 = x0 + round(w["size"][0] * sc), y0 + round(w["size"][1] * sc)
        want = (min(mon["width"], x1 + m) - max(0, x0 - m), min(mon["height"], y1 + m) - max(0, y0 - m))

        p = s.bg("window"); time.sleep(0.5)
        s.move(lx + 50, ly + 50); s.click()
        r, code = finish(p)
        assert code == 0 and r["result"]["kind"] == "window", r
        img = Image.open(r["result"]["source"])
        assert abs(img.size[0] - want[0]) <= 3 and abs(img.size[1] - want[1]) <= 3, ("the window and 48 px around it", img.size, want)
        assert img.mode == "RGB", ("as on screen: opaque", img.mode)

        p = s.bg("window"); time.sleep(0.5)                              # Shift: the clean window
        s.move(lx + 50, ly + 50); s.key("Shift", "keydown"); s.click()
        r, code = finish(p)
        s.key("Shift", "keyup")
        assert code == 0 and Image.open(r["result"]["source"]).mode == "RGBA", "Shift gives the clean window"
    finally:
        cfg.write_text(before)
        time.sleep(0.8)


@case
def capture_variants(s, env):
    """CAP-02, API-01: Capture Area & Copy / & Save / & Annotate (`snip area --action …`, and
    bindable actions) skip the After Capture rules: & Copy is the clipboard only, no card.
    The After Capture rule "Pin to the screen" pins."""
    def capture(*extra):
        p = s.bg("area", *extra); time.sleep(0.5)
        s.drag(200, 200, 400, 320)
        r, code = finish(p)
        assert code == 0, r
        return r["result"]

    s.cli("close-all"); time.sleep(0.3)
    sh("wl-copy", "--clear", check=False)
    rec = capture("--action", "copy")
    assert wait_for(lambda: "image/png" in sh("wl-paste", "-l", check=False).stdout.split(), 3), "& Copy: on the clipboard"
    time.sleep(0.4)
    assert rec["id"] not in s.status()["cards"], "and no card"
    rec = capture("--action", "save")
    assert wait_for(lambda: any(s.export.glob("*.png")), 3), "& Save: in the export folder"
    for f in s.export.glob("*.png"):                               # later cases expect it empty
        f.unlink()
    rec = capture("--action", "annotate")
    assert wait_for(lambda: editor_window(), 5), "& Annotate: the editor"
    close_editor(s)
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before + "\n[after_capture]\nscreenshot = [\"pin\"]\n")
    try:
        time.sleep(0.8)
        capture()
        assert wait_for(lambda: pin_windows(), 3), "After Capture: Pin to the screen"
    finally:
        s.debug("ui", "close-pins")
        cfg.write_text(before)
        time.sleep(0.8)


@case
def capture_notification(s, env):
    """OMA-05: with no card (& Copy, & Save) a notification says what happened, with the
    image; its click runs `snip restore ID`, which brings the card back. (Tests trace the
    notification instead of sending it.)"""
    s.cli("close-all"); time.sleep(0.3)

    def capture(action):
        n = len(s.traces("notified"))
        rec = s.json("area", "--x", "100", "--y", "80", "--w", "200", "--h", "120", "--monitor", OUT, "--action", action)["result"]
        assert wait_for(lambda: len(s.traces("notified")) > n, 3), (action, "a notification")
        return rec, s.traces("notified")[-1]

    rec, note = capture("copy")
    assert (note["id"], note["title"], note["body"]) == (rec["id"], "Screenshot copied", f"{rec['width']} × {rec['height']}"), note
    assert rec["id"] not in s.status()["cards"], "no card"
    s.json("restore", rec["id"])
    assert wait_for(lambda: rec["id"] in s.status()["cards"], 3), "its click brings the card back"
    s.cli("close-all")
    there = set(s.export.iterdir()) if s.export.exists() else set()
    rec, note = capture("save")
    saved = set(s.export.iterdir()) - there
    assert note["title"] == "Screenshot saved" and [f.name for f in saved] == [note["body"]], (note, saved)
    for f in saved:                                                # later cases expect it as it was
        f.unlink()


@case
def capture_by_command(s, env):
    """API-01, CAP-04, OCR-06: `snip area --x --y --w --h` captures that area (logical px)
    with no overlay, and it becomes the previous area; `snip fullscreen --all` puts every
    monitor in one image; `snip text --file / --clipboard` reads a PNG's text."""
    sc = env.monitor()["scale"]
    shown = len(s.traces("overlay_presented"))
    rec = s.json("area", "--x", "100", "--y", "80", "--w", "200", "--h", "120", "--monitor", OUT)["result"]
    assert (rec["kind"], rec["width"], rec["height"]) == ("area", round(200 * sc), round(120 * sc)), rec
    assert len(s.traces("overlay_presented")) == shown, "no overlay"
    again = s.json("previous", "--monitor", OUT)["result"]
    assert (again["width"], again["height"]) == (rec["width"], rec["height"]), "it's the previous area now"
    allmon = s.json("fullscreen", "--all")["result"]
    mon = env.monitor()
    assert (allmon["width"], allmon["height"]) == (mon["width"], mon["height"]), ("every monitor (one here)", allmon)
    png = s.tmp / "words.png"
    make_text_png(png, ["walrus reading test"])
    text = s.json("text", "--file", str(png))["result"]
    assert "walrus" in text, text
    assert wait_for(lambda: "walrus" in sh("wl-paste", "-n", check=False).stdout, 3), "onto the clipboard"
    with open(png, "rb") as f:                                     # wl-copy stays to serve it: no pipes to wait on
        subprocess.run(["wl-copy", "--type", "image/png"], stdin=f, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.3)
    assert "walrus" in s.json("text", "--clipboard")["result"], "from the clipboard's image"


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
def text_lines_and_links(s, env):
    """OCR-02: Capture Text keeps line breaks, or joins them (ocr.keep_line_breaks = false);
    the line-breaks hotkey keeps them whatever the setting says.
    OCR-03/05: a link in the text asks "Open link?"; Open opens it, Never is remembered,
    and with Always it opens with no prompt."""
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    lines, link = s.tmp / "lines.png", s.tmp / "link.png"
    make_text_png(lines, ["walrus reading", "test today"])
    make_text_png(link, ["docs at https://example.com/walrus"])

    def prompt():
        n = len(s.traces("link_prompt"))
        s.debug("ui", "link", "state")
        assert wait_for(lambda: len(s.traces("link_prompt")) > n, 2)
        return s.traces("link_prompt")[-1]

    def click(name):
        b = where(s, "link", name)
        assert b["x"] >= 0, (name, "isn't shown")
        s.move(b["x"], b["y"]); s.click()

    def opened():
        return [t["url"] for t in s.traces("link_open")]

    try:
        assert "walrus reading\ntest today" in s.json("text", "--file", str(lines))["result"], "line breaks kept"
        assert not prompt()["open"], "no link, no prompt"
        cfg.write_text(before + "\n[ocr]\nkeep_line_breaks = false\n")
        time.sleep(0.8)
        assert "walrus reading test today" in s.json("text", "--file", str(lines))["result"], "one line"
        rec = s.json("open", str(lines))["result"]                 # on screen, as a pin
        s.json("pin", rec["id"])
        assert wait_for(lambda: pin_windows(), 3)
        w, mon = pin_windows()[0], env.monitor()
        x, y = w["at"][0] - mon["x"], w["at"][1] - mon["y"]
        p = s.bg("text", "--line-breaks"); time.sleep(0.5)
        s.drag(x + 20, y + 20, x + w["size"][0] - 20, y + w["size"][1] - 20)
        r, code = finish(p, timeout=15)
        assert code == 0 and "walrus reading\ntest today" in r["result"], ("the line-breaks hotkey keeps them", r)
        s.debug("ui", "close-pins")

        s.move(400, 300)
        s.cli("text", "--file", str(link))
        assert wait_for(lambda: prompt()["open"], 3), "Open link?"
        assert prompt()["url"] == "https://example.com/walrus", prompt()
        click("link-open")
        assert wait_for(lambda: "https://example.com/walrus" in opened(), 3), "Open opens it"
        assert wait_for(lambda: not prompt()["open"], 2)
        s.cli("text", "--file", str(link))
        assert wait_for(lambda: prompt()["open"], 3)
        click("link-never")
        assert wait_for(lambda: 'open_links = "never"' in cfg.read_text(), 3), "Never is remembered"
        assert len(opened()) == 1, "and opens nothing"
        time.sleep(0.8)
        s.cli("text", "--file", str(link))
        time.sleep(0.8)
        assert not prompt()["open"], "Never: no prompt"
        cfg.write_text(before + "\n[ocr]\nopen_links = \"always\"\n")
        time.sleep(0.8)
        s.cli("text", "--file", str(link))
        assert wait_for(lambda: len(opened()) == 2, 3), "Always opens it"
        assert not prompt()["open"], "with no prompt"
    finally:
        s.debug("ui", "close-pins")
        s.cli("close-all")
        cfg.write_text(before)
        time.sleep(0.8)


@case
def file_names(s, env):
    """SET-04: with the UTC option the time in a saved file's name is UTC, and the @2x
    option adds the capture's scale (1.25 here), which history keeps."""
    import datetime
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before.replace("[general]\n", "[general]\nfile_name = \"Shot %H\"\nutc_time = true\nscale_suffix = true\n", 1))
    try:
        time.sleep(0.8)
        rec = s.json("fullscreen", "--monitor", OUT)["result"]
        assert rec["scale"] == env.monitor()["scale"], rec
        hours = {datetime.datetime.now(datetime.timezone.utc).hour}
        saved = Path(s.json("save", rec["id"])["result"])
        hours.add(datetime.datetime.now(datetime.timezone.utc).hour)
        assert saved.name in {f"Shot {h:02}@1.25x.png" for h in hours}, (saved.name, hours)
        saved.unlink()                                                # later cases expect the folder empty
    finally:
        cfg.write_text(before)
        time.sleep(0.8)


@case
def lifecycle(s, env):
    """Capture -> history only; Copy -> clipboard; Save -> folder + clipboard; restore."""
    s.cli("close-all"); time.sleep(0.3)
    rec = s.json("fullscreen", "--monitor", OUT)["result"]
    assert not list(s.export.glob("*.png")), "capture must not write to the export folder"
    s.cli("copy", rec["id"])
    time.sleep(0.2)
    types = sh("wl-paste", "-l", check=False).stdout.split()
    # D38: the image only, so web fields get the screenshot (Zen lists text first)
    assert types == ["image/png"], ("Copy offers the image only", types)
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before + "\n[clipboard]\ntext_path = true\n")
    try:
        time.sleep(0.8)
        s.cli("copy", rec["id"])
        time.sleep(0.2)
        types = sh("wl-paste", "-l", check=False).stdout.split()
        assert "image/png" in types and "text/plain;charset=utf-8" in types, types
        path = sh("wl-paste", "-n", "-t", "text/plain;charset=utf-8", check=False).stdout
        assert path.endswith(".png") and png_size(path) == (1920, 1080), path   # with text_path, text targets get the file
    finally:
        cfg.write_text(before)
        time.sleep(0.8)
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
    recs = []
    for _ in range(3):
        recs.append(s.json("fullscreen", "--monitor", OUT)["result"])
        time.sleep(0.4)
    assert len(s.status()["cards"]) == 3
    # The first capture ran with no card on screen; the last with two. The cards'
    # corner must look the same in both (a terminal's blinking cursor would not be there).
    first, last = (Image.open(r["source"]).convert("RGB").crop((0, 500, 700, 1080)) for r in (recs[0], recs[-1]))
    assert first.tobytes() == last.tobytes(), "a card was captured"


@case
def latency(s, env):
    """Hotkey -> every overlay presented: p95 < 35 ms warm (SEL-01)."""
    times = []
    s.move(400, 300)   # Escape goes to the overlay under the pointer
    n = len(s.traces())   # this case's presses only: earlier cases' may have opened nothing
    for i in range(12):
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:area"))')
        time.sleep(0.35)
        s.key("Escape")
        time.sleep(0.25)
    ev = s.traces()[n:]
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
def cards_fit_the_screen(s, env):
    """QAO-06: the stack shows as many cards as the screen has room for, newest at the
    bottom; the rest wait under "+k more". Five cards ran off the top of an 864 px tall
    screen."""
    s.cli("close-all"); time.sleep(0.3)
    ids = []
    for i in range(5):
        png = s.tmp / f"fit-{i}.png"
        make_text_png(png, [f"card {i}"])
        ids.append(open_card(s, png)["id"]); time.sleep(0.25)
    time.sleep(0.8)
    try:
        shown = [p for p in (where(s, "qa", "card:" + i) for i in ids) if p["x"] >= 0]
        half = round(280 * 0.705) / 2
        assert shown and all(p["y"] - half >= 0 for p in shown), ("every card shown is on screen", shown)
        assert where(s, "qa", "card:" + ids[-1])["x"] >= 0, "the newest shows"
        assert len(shown) < 5 and where(s, "qa", "qa-more")["x"] >= 0, ("the rest under +k more", len(shown))
    finally:
        s.cli("close-all"); time.sleep(0.6)


@case
def card_menu_stays_on_screen(s, env):
    """QAO-09: a card's menu stays on screen. It was placed by the height of a menu with no
    card, so a HiDPI capture's (one row more: Scale to 1x) ran off the bottom."""
    s.cli("close-all"); time.sleep(0.6)
    rec = s.json("area", "--x", "100", "--y", "80", "--w", "200", "--h", "120", "--monitor", OUT)["result"]
    assert rec["scale"] == 1.25, rec
    time.sleep(0.6)
    try:
        card = where(s, "qa", "card:" + rec["id"])
        y = card["y"] + 80                                          # near the card's bottom
        s.move(card["x"] + 1, y); s.move(card["x"], y); time.sleep(0.3); s.click(right=True)
        assert wait_for(lambda: layer_mapped("snip-dismiss"), 2), "the menu opens"
        last, mon = where(s, "qa", "menu:hide"), env.monitor()
        assert 0 <= last["y"] <= mon["height"] / mon["scale"] - 16, ("its last row is on screen", last)
        s.move(last["x"], last["y"]); time.sleep(0.15); s.click()     # Hide: the menu goes
    finally:
        s.cli("close-all"); time.sleep(0.6)


@case
def card_save_says_what_happened(s, env):
    """QAO-03: a card says Saved once the file is written, and Couldn't save when it isn't (it
    said Saved either way); a pin's Save says so too (it said nothing)."""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "savefail.png"
    make_text_png(png, ["save me"])
    rec = open_card(s, png); time.sleep(0.5)
    mon, ch = env.monitor(), round(280 * 0.705)
    s.export.mkdir(parents=True, exist_ok=True)
    mode, there = s.export.stat().st_mode, set(s.export.iterdir())
    flashes = lambda: [e["status"] for e in s.traces("card_flash") if e["id"] == rec["id"]]
    try:
        os.chmod(s.export, 0o500)                                   # the folder can't be written
        s.move(24 + 140, mon["height"] / mon["scale"] - 24 - ch / 2); time.sleep(0.6)
        s.debug("ui", "qa-keys", "{ctrl+s}")
        assert wait_for(lambda: "save-failed" in flashes(), 3), ("Couldn't save", flashes())
        assert "saved" not in flashes(), ("not Saved", flashes())
        os.chmod(s.export, mode)
        s.debug("ui", "qa-keys", "{ctrl+s}")
        assert wait_for(lambda: "saved" in flashes(), 3), ("Saved once written", flashes())
        n = len(s.traces("toast"))
        s.json("pin", rec["id"])
        assert wait_for(lambda: pin_windows(), 3), "pinned"
        s.debug("ui", "pin", "act", "save")
        assert wait_for(lambda: [t for t in s.traces("toast")[n:] if t["text"] == "Saved"], 3), "the pin says Saved"
    finally:
        os.chmod(s.export, mode)
        for f in set(s.export.iterdir()) - there:                  # later cases expect it as it was
            f.unlink()
        s.debug("ui", "close-pins")
        s.cli("close-all"); time.sleep(0.6)


@case
def alt_save_asks_where(s, env):
    """QAO-03: on a hovered card Ctrl+S saves; with Alt (Ctrl+Alt+S, or Alt held on Save)
    it asks where instead. (Alt with a click needs a real keyboard: checked by hand.)"""
    s.cli("close-all"); time.sleep(0.3)
    png = s.tmp / "altsave.png"
    make_text_png(png, ["alt save"])
    open_card(s, png); time.sleep(0.5)
    mon = env.monitor()
    ch = round(280 * 0.705)
    files = lambda: set(s.export.iterdir()) if s.export.exists() else set()
    there = files()
    try:
        s.move(24 + 140, mon["height"] / mon["scale"] - 24 - ch / 2); time.sleep(0.6)   # resting on it: the keyboard comes
        n = len(s.traces("save_as_dialog"))
        s.debug("ui", "qa-keys", "{ctrl+alt+s}")
        assert wait_for(lambda: len(s.traces("save_as_dialog")) > n, 2), "Ctrl+Alt+S asks where"
        assert files() == there, "and doesn't save to the folder"
        s.debug("ui", "qa-keys", "{ctrl+s}")
        assert wait_for(lambda: files() - there, 2), "Ctrl+S saves"
        assert len(s.traces("save_as_dialog")) == n + 1, "without asking"
    finally:
        for f in files() - there:                                   # later cases expect it as it was
            f.unlink()
        s.cli("close-all"); time.sleep(0.6)


@case
def card_scale_resize_print(s, env):
    """QAO-09: a card's menu scales a HiDPI capture to 1× (then offers it no more) and
    resizes it, proportions locked; Print… makes a one-page PDF of it, as the editor's
    Print does (ANN-15). The print dialog itself is checked by hand."""
    s.cli("close-all"); time.sleep(0.6)
    rec = s.json("area", "--x", "100", "--y", "80", "--w", "200", "--h", "120", "--monitor", OUT)["result"]
    assert (rec["width"], rec["height"], rec["scale"]) == (250, 150, 1.25), rec
    time.sleep(0.6)                                                # the card slides in

    def qa_at(name):
        return where(s, "qa", name)

    def click(name):
        b = qa_at(name)
        assert b["x"] >= 0, (name, "isn't shown")
        s.move(b["x"], b["y"]); time.sleep(0.15); s.click()

    def menu():
        card = qa_at("card:" + rec["id"])
        s.move(card["x"] + 1, card["y"]); s.move(card["x"], card["y"]); time.sleep(0.3); s.click(right=True)
        assert wait_for(lambda: layer_mapped("snip-dismiss"), 2), "the menu opens"

    def record():
        return next(r for r in s.json("list", "-n", "5")["result"] if r["id"] == rec["id"])

    menu(); click("menu:scale-1x")
    assert wait_for(lambda: (record()["width"], record()["height"], record()["scale"]) == (200, 120, 1.0), 3), record()
    menu()
    assert qa_at("menu:scale-1x")["x"] < 0, "at 1× there's nothing to scale"
    click("menu:resize")
    click("resize-w")
    s.debug("ui", "qa-keys", "{ctrl+a}100")
    click("resize-apply")
    assert wait_for(lambda: (record()["width"], record()["height"]) == (100, 60), 3), ("proportions locked", record())
    assert record()["scale"] == 0.5, record()

    def printed(start):
        n = len(s.traces("print"))
        start()
        assert wait_for(lambda: len(s.traces("print")) > n, 5), "Print…"
        pdf = Path(s.traces("print")[-1]["pdf"])
        data = pdf.read_bytes()
        pdf.unlink()
        assert data.startswith(b"%PDF") and data.count(b"/Type /Page\n") + data.count(b"/Type /Page\r") + data.count(b"/Type /Page ") >= 1, data[:200]
        assert b"/MediaBox [0 0 595" in data, "an A4 page"

    printed(lambda: (menu(), click("menu:print")))
    png = s.tmp / "print.png"
    make_text_png(png, ["print me"])
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    printed(lambda: s.debug("ui", "editor", "print"))
    close_editor(s)
    s.cli("close-all")


@case
def pin_corner_resize(s, env):
    """PIN-04: dragging a pin's corner resizes it, its proportions kept; the opposite
    corner stays where it was."""
    png = s.tmp / "corner.png"
    noise_png(png, 500, 300, seed=8)
    rec = s.json("open", str(png))["result"]
    s.cli("close-all")
    s.json("pin", rec["id"])
    assert wait_for(lambda: pin_windows(), 3), "a pin window opens"
    time.sleep(0.4)
    mon = env.monitor()

    def corner(name):
        b, w = where(s, "pin", name), pin_windows()[0]
        return w["at"][0] - mon["x"] + b["x"], w["at"][1] - mon["y"] + b["y"]

    def box():
        w = pin_windows()[0]
        return w["at"][0], w["at"][1], w["at"][0] + w["size"][0], w["at"][1] + w["size"][1]

    def settled(z0):
        st = pin_state(s)
        w = pin_windows()[0]
        return st["zoom"] != z0 and tuple(w["size"]) == (st["w"], st["h"])

    z0 = pin_state(s)["zoom"]
    x0, y0, x1, y1 = box()
    x, y = corner("corner-bottom-right")
    s.drag(x, y, x + 100, y + 60, steps=10)
    assert wait_for(lambda: settled(z0), 3), (pin_state(s), pin_windows()[0]["size"])
    st = pin_state(s)
    assert st["zoom"] > z0 * 1.1, st
    assert abs((st["w"] - 32) / (st["h"] - 32) - 500 / 300) < 0.02, ("proportions kept", st)
    assert box()[:2] == (x0, y0), ("the top left stays", box(), (x0, y0))

    z1 = st["zoom"]
    x0, y0, x1, y1 = box()
    x, y = corner("corner-top-left")
    s.drag(x, y, x + 80, y + 48, steps=10)
    assert wait_for(lambda: settled(z1) and pin_state(s)["zoom"] < z1, 3), pin_state(s)
    bx = box()
    assert abs(bx[2] - x1) <= 4 and abs(bx[3] - y1) <= 4, ("the bottom right stays", bx, (x1, y1))
    s.debug("ui", "close-pins")


@case
def hide_desktop_clutter(s, env):
    """CAP-12: with Hide desktop clutter, a screenshot shows the wallpaper where a bar
    reserves space (scaled to cover the output, as omarchy-shell draws it) and leaves
    pins out. The bar here is space the monitor reserves; the wallpaper is the user's."""
    from PIL import Image, ImageChops, ImageStat
    wall = Path.home() / ".local/state/omarchy/current/background"
    if not wall.exists():
        raise Skip("no Omarchy wallpaper to cover with")
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    mon = env.monitor()
    sc = mon["scale"]
    hypr_eval(f'hl.monitor({{ output = "{OUT}", mode = "1920x1080@60", position = "auto", scale = {sc}, transform = 0, reserved = {{ top = 40 }} }})')
    png = s.tmp / "magenta.png"
    Image.new("RGB", (300, 200), (255, 0, 255)).save(png)
    rec = s.json("open", str(png))["result"]
    s.cli("close-all")
    s.json("pin", rec["id"])
    try:
        assert wait_for(lambda: env.monitor()["reserved"][1] == 40, 3), env.monitor()["reserved"]
        assert wait_for(lambda: pin_windows(), 3), "a pin"
        time.sleep(0.5)
        w = pin_windows()[0]
        centre = (round((w["at"][0] - mon["x"] + w["size"][0] / 2) * sc), round((w["at"][1] - mon["y"] + w["size"][1] / 2) * sc))

        def shot():
            r = s.json("fullscreen", "--monitor", OUT)["result"]
            s.cli("close-all")
            return Image.open(r["source"]).convert("RGB")

        assert shot().getpixel(centre) == (255, 0, 255), "a plain screenshot has the pin"
        cfg.write_text(before.replace("[capture]\n", "[capture]\nhide_desktop = true\n", 1) if "[capture]\n" in before
                       else before + "\n[capture]\nhide_desktop = true\n")
        time.sleep(0.8)
        hidden = shot()
        assert hidden.getpixel(centre) != (255, 0, 255), "no pin"
        W, H = hidden.size
        band = round(40 * sc)
        im = Image.open(wall).convert("RGB")
        k = max(W / im.width, H / im.height)
        cover = im.resize((round(im.width * k), round(im.height * k)), Image.LANCZOS)
        ox, oy = (cover.width - W) // 2, (cover.height - H) // 2
        diff = ImageStat.Stat(ImageChops.difference(hidden.crop((0, 0, W, band)), cover.crop((ox, oy, ox + W, oy + band)))).mean
        assert max(diff) < 8, ("the bar's band is the wallpaper", diff)
        assert wait_for(lambda: not pin_state(s)["capturing"], 2), "the pin is back after the capture"
    finally:
        cfg.write_text(before)
        s.debug("ui", "close-pins")
        env.configure(sc)


@case
def desktop_toggle(s, env):
    """CAP-12, the live toggle: `snip desktop hide` covers the bar with the wallpaper on
    screen (so a plain screenshot has no bar), takes pins and cards off screen, and
    `show` brings them back; a fullscreen window takes the cover away. (DND is the
    user's: tests never touch it.)"""
    from PIL import Image, ImageChops, ImageStat
    wall = Path.home() / ".local/state/omarchy/current/background"
    if not wall.exists():
        raise Skip("no Omarchy wallpaper to cover with")
    mon = env.monitor()
    sc = mon["scale"]
    hypr_eval(f'hl.monitor({{ output = "{OUT}", mode = "1920x1080@60", position = "auto", scale = {sc}, transform = 0, reserved = {{ top = 40 }} }})')
    png = s.tmp / "deskpin.png"
    Image.new("RGB", (300, 200), (0, 200, 0)).save(png)
    rec = s.json("open", str(png))["result"]
    s.json("pin", rec["id"])

    def state():
        n = len(s.traces("desktop_state"))
        s.debug("ui", "desktop")
        assert wait_for(lambda: len(s.traces("desktop_state")) > n, 2)
        return s.traces("desktop_state")[-1]

    try:
        assert wait_for(lambda: env.monitor()["reserved"][1] == 40, 3) and wait_for(lambda: pin_windows(), 3)
        assert s.json("desktop", "hide")["result"] == {"hidden": True}
        assert wait_for(lambda: layer_mapped("snip-cover"), 3), "the cover is on screen"
        assert wait_for(lambda: not pin_windows(), 3), "pins are off screen"
        st = state()
        assert st["hidden"] and st["pinsHidden"] == 1, st
        time.sleep(0.5)                                            # the cover's fade-in
        shot = Image.open(s.json("fullscreen", "--monitor", OUT)["result"]["source"]).convert("RGB")
        W, H = shot.size
        band = round(40 * sc)
        im = Image.open(wall).convert("RGB")
        k = max(W / im.width, H / im.height)
        cover = im.resize((round(im.width * k), round(im.height * k)), Image.LANCZOS)
        ox, oy = (cover.width - W) // 2, (cover.height - H) // 2
        diff = ImageStat.Stat(ImageChops.difference(shot.crop((0, 0, W, band)), cover.crop((ox, oy, ox + W, oy + band)))).mean
        assert max(diff) < 10, ("a plain screenshot shows the wallpaper over the bar", diff)

        win = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-target")
        hypr_eval(f'hl.dispatch(hl.dsp.window.fullscreen({{ window = "address:{win["address"]}" }}))')
        assert wait_for(lambda: next(c for c in hypr_json("clients") if c["address"] == win["address"])["fullscreen"], 3), "fullscreen"
        assert wait_for(lambda: not layer_mapped("snip-cover"), 3), "no cover over a fullscreen window"
        hypr_eval(f'hl.dispatch(hl.dsp.window.fullscreen({{ window = "address:{win["address"]}" }}))')
        assert wait_for(lambda: layer_mapped("snip-cover"), 3), "the cover is back"

        assert s.json("desktop")["result"] == {"hidden": False}, "toggle"
        assert wait_for(lambda: not layer_mapped("snip-cover"), 3), "the cover goes"
        assert wait_for(lambda: pin_windows(), 3), "the pin is back"
        assert not state()["hidden"]
    finally:
        s.cli("desktop", "show")
        s.debug("ui", "close-pins")
        s.cli("close-all")
        env.configure(sc)


@case
def cursor_toggle(s, env):
    """--cursor paints the pointer into a capture, --no-cursor leaves it out (CAP-11).
    Hyprland draws no pointer on headless outputs (grim -c shows none either), so this
    case captures the first real monitor with its own instance: no card, no clipboard,
    files deleted afterwards."""
    from PIL import Image, ImageChops
    # nested, the test output is the only one, and a real (non-headless) one
    real = next((m for m in hypr_json("monitors") if m["name"] != OUT), None) or env.monitor()
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


def pin_windows():
    return [c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"].startswith("Snip Pin ")]


def pin_state(s):
    n = len(s.traces("pin_state"))
    s.debug("ui", "pin", "state")
    assert wait_for(lambda: len(s.traces("pin_state")) > n, 2), "no pin state"
    return s.traces("pin_state")[-1]


@case
def pins(s, env):
    """PIN-01…07: a pin floats on every workspace at the capture's own size; scroll fades
    it, Ctrl+= zooms it (the window follows), a drag or the arrows move it, `snip pins
    toggle` hides and brings back every pin in place, middle-click closes it; Close All."""
    from PIL import Image
    png = s.tmp / "pin.png"
    noise_png(png, 500, 300, seed=4)
    rec = s.json("open", str(png))["result"]
    assert s.json("pin", rec["id"])["ok"]
    assert wait_for(lambda: pin_windows(), 3), "a pin window opens"
    w, st = pin_windows()[0], pin_state(s)
    assert w["floating"] and w["pinned"], w
    scale = env.monitor()["scale"]
    assert abs(st["baseW"] - 500 / scale) < 0.01 and st["zoom"] == 1, st
    assert tuple(w["size"]) == (st["w"], st["h"]) == (round(500 / scale) + 32, round(300 / scale) + 32), (w["size"], st)
    mon = env.monitor()
    cx, cy = w["at"][0] - mon["x"] + w["size"][0] / 2, w["at"][1] - mon["y"] + w["size"][1] / 2
    s.move(cx, cy); time.sleep(0.2)
    s.debug("scroll", "30")                                      # wheel down: fade
    assert wait_for(lambda: pin_state(s)["fade"] < 1, 2), pin_state(s)
    s.debug("down"); s.debug("up"); time.sleep(0.3)             # a click gives it the keyboard
    s.debug("ui", "pin", "keys", "{ctrl+=}")                     # zoom in
    st = pin_state(s)
    assert abs(st["zoom"] - 1.1) < 0.01, st
    assert wait_for(lambda: tuple(pin_windows()[0]["size"]) == (st["w"], st["h"]), 2), (pin_windows()[0]["size"], st)
    at = pin_windows()[0]["at"]
    s.drag(cx, cy, cx - 120, cy - 60, steps=10)                  # a drag moves it (Hyprland picks it up a step late)
    assert wait_for(lambda: pin_windows()[0]["at"][0] < at[0] - 40 and pin_windows()[0]["at"][1] < at[1] - 15, 2), (at, pin_windows()[0]["at"])
    at = pin_windows()[0]["at"]
    s.debug("ui", "pin", "keys", "{Right}{shift+Down}")           # arrows nudge it (Shift: 10 px)
    assert wait_for(lambda: pin_windows()[0]["at"] == [at[0] + 1, at[1] + 10], 2), (at, pin_windows()[0]["at"])
    at = pin_windows()[0]["at"]
    assert s.json("pins", "toggle")["ok"]                        # hide every pin…
    assert wait_for(lambda: not pin_windows(), 3), "toggle hides the pins"
    s.json("pins", "toggle")                                     # …and bring them back where they were
    assert wait_for(lambda: pin_windows() and pin_windows()[0]["at"] == at, 3), (at, pin_windows())
    s.move(cx - 121, cy - 61); s.move(cx - 120, cy - 60); time.sleep(0.2)   # a real motion: the pin has the pointer again
    s.debug("mdown"); s.debug("mup")                             # middle-click closes
    assert wait_for(lambda: not pin_windows(), 3), "middle-click closes the pin"
    for _ in range(2):
        s.json("pin", rec["id"])
    assert wait_for(lambda: len(pin_windows()) == 2, 3)
    s.debug("ui", "pin", "act", "close-all")
    assert wait_for(lambda: not pin_windows(), 3), "Close All Pins"
    s.json("pin", rec["id"])
    assert wait_for(lambda: pin_windows(), 3)
    s.json("pins", "close")
    assert wait_for(lambda: not pin_windows(), 3), "snip pins close"


@case
def capture_area_and_pin(s, env):
    """PIN-01: Capture Area & Pin pins the area right where it was, at its own size, with
    no card; `snip pin FILE` pins an image file."""
    from PIL import Image
    cards = len(s.json("status")["result"]["cards"])
    p = s.bg("area-pin")
    time.sleep(0.5)
    s.drag(100, 100, 300, 250)
    r, code = finish(p)
    assert code == 0 and r["ok"], r
    assert wait_for(lambda: pin_windows(), 3), "a pin opens"
    w, mon, st = pin_windows()[0], env.monitor(), pin_state(s)
    assert wait_for(lambda: pin_windows()[0]["at"] == [mon["x"] + 100 - 16, mon["y"] + 100 - 16], 2), (pin_windows()[0]["at"], mon)
    assert st["zoom"] == 1 and abs(st["baseW"] - 200) < 1, st
    assert len(s.json("status")["result"]["cards"]) == cards, "no card"
    s.json("pins", "close")
    png = s.tmp / "pinfile.png"
    noise_png(png, 120, 80, seed=2)
    assert s.json("pin", str(png))["ok"]
    assert wait_for(lambda: pin_windows(), 3), "a file pins too"
    s.json("pins", "close")


@case
def pin_copy_text(s, env):
    """PIN-06: a pin's menu copies the text in it (OCR), as the card's does."""
    png = s.tmp / "pintext.png"
    make_text_png(png, ["pinned words here"])
    rec = s.json("open", str(png))["result"]
    s.cli("close-all")
    s.json("pin", rec["id"])
    assert wait_for(lambda: pin_windows(), 3), "the pin opens"
    s.debug("ui", "pin", "act", "text")
    assert wait_for(lambda: [e for e in s.traces("card_status") if e["id"] == rec["id"] and e["status"] == "text-copied"], 10), \
        [e for e in s.traces("card_status") if e["id"] == rec["id"]]
    assert wait_for(lambda: "pinned" in (sh("wl-paste", "-n", check=False).stdout or "").lower(), 3), "the text is on the clipboard"
    s.json("pins", "close")


@case
def pin_look(s, env):
    """PIN-08: [pins] sets the corners, the shadow and the border; square corners show the
    image's own corner pixel."""
    from PIL import Image
    png = s.tmp / "look.png"
    Image.new("RGB", (300, 200), (230, 40, 40)).save(png)
    rec = s.json("open", str(png))["result"]
    s.cli("close-all")

    def corner():
        s.json("pin", rec["id"])
        assert wait_for(lambda: pin_windows(), 3)
        time.sleep(0.4)
        w, mon, scale = pin_windows()[0], env.monitor(), env.monitor()["scale"]
        shot = s.tmp / "pin-look.png"
        sh("grim", "-o", OUT, str(shot))
        x, y = round((w["at"][0] - mon["x"] + 16) * scale), round((w["at"][1] - mon["y"] + 16) * scale)
        px, st = Image.open(shot).convert("RGB").getpixel((x + 1, y + 1)), pin_state(s)
        s.json("pins", "close")
        assert wait_for(lambda: not pin_windows(), 3)
        return px, st

    px, st = corner()
    assert st["radius"] == 10 and st["shadow"] and px[0] < 200, ("rounded by default: the corner is not the image", px)
    cfg = s.tmp / "cfg/snip/config.toml"
    cfg.write_text(cfg.read_text() + "\n[pins]\ncorner_radius = 0\nshadow = false\nborder = false\n")
    try:
        time.sleep(0.8)                                          # the config watcher reloads it
        px, st = corner()
        assert st["radius"] == 0 and not st["shadow"], st
        assert abs(px[0] - 230) < 6 and px[1] < 50, ("square: the image's own corner", px)
    finally:
        cfg.write_text(cfg.read_text().replace("\n[pins]\ncorner_radius = 0\nshadow = false\nborder = false\n", ""))
        time.sleep(0.8)


@case
def pin_lock(s, env):
    """PIN-05: a locked pin lets clicks through to what is under it (an editor draws through
    it), fades under the pointer, and `snip pins unlock` brings it back where it was."""
    from PIL import Image
    under, pinned = s.tmp / "under.png", s.tmp / "pinned.png"
    Image.new("RGB", (600, 400), "white").save(under)
    noise_png(pinned, 300, 200, seed=6)
    s.json("edit", str(under))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "tool", "rect")
    rec = s.json("open", str(pinned))["result"]
    s.cli("close-all")
    s.json("pin", rec["id"])
    assert wait_for(lambda: pin_windows(), 3), "the pin opens"
    at = pin_windows()[0]["at"]
    s.debug("ui", "pin", "act", "lock")
    assert wait_for(lambda: not pin_windows(), 3), "locked: the window gives way to a layer"
    layers = [l for m in hypr_json("layers").values() for lv in m["levels"].values() for l in lv if l["namespace"] == "snip-pin"]
    assert len(layers) == 1 and [layers[0]["x"], layers[0]["y"]] == at, (layers, at)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    cx, cy = at[0] - mon["x"] + layers[0]["w"] / 2, at[1] - mon["y"] + layers[0]["h"] / 2
    s.drag(cx - 40, cy - 30, cx + 40, cy + 30, steps=5)          # through the pin, onto the editor
    assert wait_for(lambda: [i for i in editor_state(s)["items"] if i["kind"] == "rect"], 2), "the editor under it got the drag"
    assert wait_for(lambda: pin_state(s)["lockHover"], 2), "it fades under the pointer"
    s.move(5, 5)
    assert wait_for(lambda: not pin_state(s)["lockHover"], 2), "it comes back when the pointer leaves"
    assert s.json("pins", "unlock")["ok"], "unlock"
    assert wait_for(lambda: pin_windows() and pin_windows()[0]["at"] == at, 3), (at, pin_windows())
    assert wait_for(lambda: not [l for m in hypr_json("layers").values() for lv in m["levels"].values() for l in lv if l["namespace"] == "snip-pin"], 2), \
        "the layer goes"                                            # (Hyprland fades it out first)
    close_editor(s)


@case
def editor_pins(s, env):
    """The editor's pin button pins the image, edited or not: the edits are written back
    first. With nothing edited it pinned nothing."""
    from PIL import Image
    png = s.tmp / "edpin.png"
    Image.new("RGB", (300, 200), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "flush", "pin")                              # nothing edited
    assert wait_for(lambda: pin_windows(), 5), "the editor pins the image as it is"
    s.debug("ui", "close-pins")
    assert wait_for(lambda: not pin_windows(), 3)
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":20,"shadow":0,"radius":0,"inset":0}')
    s.debug("ui", "editor", "flush", "pin")
    assert wait_for(lambda: pin_windows(), 5), "the editor pins its result"
    st = pin_state(s)
    assert abs(st["baseW"] * env.monitor()["scale"] - 340) < 0.5, st     # the image with its background
    s.debug("ui", "close-pins")
    close_editor(s)
    assert wait_for(lambda: not pin_windows(), 3)


@case
def locked_pins_unlock_from_the_bar(s, env):
    """PIN-06: a locked pin lets clicks through, so it can't be unlocked by clicking it.
    Snip's menu in the bar offers Unlock Pins while one is locked (only `snip pins unlock`
    could), and locking says where to find it."""
    from PIL import Image
    png = s.tmp / "lockme.png"
    Image.new("RGB", (300, 200), (40, 120, 200)).save(png)
    rec = s.json("open", str(png))["result"]
    try:
        s.json("pin", rec["id"])
        assert wait_for(lambda: pin_windows(), 3), "pinned"
        s.move(300, 5); s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2)
        assert "pins-unlock" not in dict(tray_state(s)["items"]), "nothing to unlock yet"
        s.debug("ui", "tray", "keys", "{Escape}")
        n = len(s.traces("toast"))
        s.debug("ui", "pin", "act", "lock")
        assert wait_for(lambda: not pin_windows(), 3), "locked: a layer now"
        assert wait_for(lambda: [t for t in s.traces("toast")[n:] if "menu in the bar" in t["text"]], 2), "it says how to unlock"
        s.move(300, 5); s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2)
        b = where(s, "tray", "menu:pins-unlock")
        assert b["x"] >= 0, ("Unlock Pins", tray_state(s)["items"])
        s.move(b["x"], b["y"]); time.sleep(0.1); s.click()
        assert wait_for(lambda: pin_windows(), 3), "Unlock Pins brings it back"
    finally:
        s.debug("ui", "tray", "keys", "{Escape}")
        s.cli("pins", "unlock", check=False)
        s.debug("ui", "close-pins")
        s.cli("close-all")


@case
def editor_floats(s, env):
    """The editor floats, centred, above the tiled windows: Snip's window rule matches its
    title (rules match the whole title, "Snip Editor — <name>"). The rule tags it."""
    from PIL import Image
    png = s.tmp / "floats.png"
    Image.new("RGB", (300, 200), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5), "the editor opens"
    w = editor_window()
    assert any(t.rstrip("*") == "snip-editor" for t in w["tags"]) and w["floating"], (w["title"], w["tags"], w["floating"])
    close_editor(s)


def history_state(s):
    n = len(s.traces("history_state"))
    s.debug("ui", "history", "state")
    assert wait_for(lambda: len(s.traces("history_state")) > n, 2), "no history state"
    return s.traces("history_state")[-1]


def settings_client():
    return next((c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"] == "Snip Settings"), None)


def settings_state(s, key=""):
    n = len(s.traces("settings_state"))
    s.debug("ui", "settings", "state", key)
    assert wait_for(lambda: len(s.traces("settings_state")) > n, 2), "no settings state"
    return s.traces("settings_state")[-1]


def settings_click(s, env, name):
    """Click a control of the settings window with the pointer, as a hand does."""
    p = where(s, "settings", name)
    assert p["x"] >= 0, (name, "isn't shown")
    w, mon = settings_client(), env.monitor()
    s.move(w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.click()


@case
def history_selection_ring(s, env):
    """HIS-01: the ring around the selected capture shows on every side (its top was cut
    off by the list's edge)."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    s.json("fullscreen", "--monitor", OUT)
    s.cli("close-all")
    s.cli("history")
    assert wait_for(lambda: history_state(s)["open"], 3)
    try:
        b = where(s, "history", "thumb:0")
        s.move(b["x"], b["y"]); time.sleep(0.1); s.click()
        assert wait_for(lambda: history_state(s)["current"] == 0, 2), "selected"
        s.move(b["x"] + 400, b["y"] + 400); time.sleep(0.4)        # the pointer off it
        shot = s.tmp / "ring.png"
        sh("grim", "-o", OUT, str(shot))
        im = Image.open(shot).convert("RGB")
        sc = env.monitor()["scale"]
        x = round(b["x"] * sc)
        top, bottom = round((b["y"] - 84 - 2) * sc), round((b["y"] + 84 + 2) * sc)   # the ring's 2 px edges, 3 px out
        assert im.getpixel((x, top)) == im.getpixel((x, bottom)), ("the ring's top edge shows as its bottom does", im.getpixel((x, top)), im.getpixel((x, bottom)))
    finally:
        s.debug("ui", "history", "keys", "{Escape}")
        wait_for(lambda: not history_state(s)["open"], 2)


@case
def history_restore_then_copy(s, env):
    """D39 / QAO-01: a capture restored from History and copied from its card pastes as
    an image, as any card's Copy does."""
    s.cli("close-all"); time.sleep(0.3)
    rec = s.json("fullscreen", "--monitor", OUT)["result"]
    s.cli("close-all"); time.sleep(0.4)
    sh("wl-copy", "--clear", check=False)
    s.cli("history")
    assert wait_for(lambda: history_state(s)["open"], 3)

    def hist_click(name):
        b = where(s, "history", name)
        assert b["x"] >= 0, (name, "isn't shown")
        s.move(b["x"], b["y"]); time.sleep(0.15); s.click()

    hist_click("thumb:0")
    assert wait_for(lambda: history_state(s)["current"] == 0, 2)
    hist_click("restore:0")
    assert wait_for(lambda: rec["id"] in s.status()["cards"], 3), "restored to Quick Access"
    s.debug("ui", "history", "keys", "{Escape}")
    assert wait_for(lambda: not history_state(s)["open"], 2)
    time.sleep(0.6)                                                # the card slides in
    c = where(s, "qa", "card:" + rec["id"])
    s.move(c["x"] + 1, c["y"]); s.move(c["x"], c["y"]); time.sleep(0.4)
    copied = len(s.traces("copied"))
    s.move(c["x"], c["y"] - 20); time.sleep(0.2); s.click()        # the card's Copy
    assert wait_for(lambda: len(s.traces("copied")) > copied, 3), "Copy"
    types = sh("wl-paste", "-l", check=False).stdout.split()
    assert types == ["image/png"], ("the image is on the clipboard", types)
    data = subprocess.run(["wl-paste", "-t", "image/png"], capture_output=True, timeout=5).stdout
    assert data[:8] == b"\x89PNG\r\n\x1a\n" and png_size_bytes(data) == (rec["width"], rec["height"]), (len(data), data[:16])


def make_clip(path, seconds=2, size="320x200", audio=True):
    """A synthetic recording: ffmpeg's test pattern (and a tone)."""
    args = ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"testsrc2=size={size}:rate=30:duration={seconds}"]
    if audio:
        args += ["-f", "lavfi", "-i", f"sine=duration={seconds}", "-c:a", "aac", "-shortest"]
    if str(path).endswith(".gif"):
        args += ["-vf", "fps=10"]
    else:
        args += ["-c:v", "libx264", "-pix_fmt", "yuv420p"]
    subprocess.run(args + [str(path)], check=True)


@case
def recordings_in_history(s, env):
    """REC-12/13, HIS-01: a video or GIF opened in Snip is a capture like a screenshot: its
    card shows its first frame and length, Copy offers the file, Save keeps it as it is,
    History files it under Videos or GIFs, and Space plays it in Quick Look (D51)."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    clip, gif = s.tmp / "clip.mp4", s.tmp / "loop.gif"
    make_clip(clip)
    make_clip(gif, seconds=1, size="160x100", audio=False)
    vid = s.json("open", str(clip))["result"]
    assert (vid["media"], vid["width"], vid["height"]) == ("video", 320, 200), vid
    assert 1900 <= vid["duration"] <= 2100 and vid["path"].endswith("source.mp4"), vid
    assert Image.open(vid["thumb"]).size == (320, 200), "its first frame is the poster"
    g = s.json("open", str(gif))["result"]
    assert (g["media"], g["width"], g["height"]) == ("gif", 160, 100) and g["path"].endswith("source.gif"), g
    assert clip.exists() and gif.exists(), "the user's files stay where they were"

    s.cli("copy", vid["id"])
    assert wait_for(lambda: "text/uri-list" in sh("wl-paste", "-l", check=False).stdout.split(), 3), "Copy offers the video as a file"
    types = sh("wl-paste", "-l", check=False).stdout.split()
    assert "image/png" not in types, types
    path = sh("wl-paste", "-n", "-t", "text/plain;charset=utf-8", check=False).stdout
    assert path.endswith(".mp4") and Path(path).read_bytes() == clip.read_bytes(), path
    s.cli("copy", g["id"])
    assert wait_for(lambda: "image/gif" in sh("wl-paste", "-l", check=False).stdout.split(), 3), "a GIF also as image/gif"

    there = set(s.export.iterdir()) if s.export.exists() else set()
    saved = Path(s.json("save", vid["id"])["result"])
    try:
        assert saved.suffix == ".mp4" and saved.read_bytes() == clip.read_bytes(), "saved as it is"
    finally:
        for f in (set(s.export.iterdir()) if s.export.exists() else set()) - there:
            f.unlink()

    s.cli("close-all"); time.sleep(0.4)
    s.cli("history")
    assert wait_for(lambda: history_state(s)["open"], 3)
    try:
        def pick(name):
            b = where(s, "history", name)
            s.move(b["x"], b["y"]); time.sleep(0.1); s.click()
        shown = lambda: history_state(s)["items"]
        pick("filter-videos")
        assert wait_for(lambda: shown() == [vid["id"]], 2), ("Videos", shown())
        pick("filter-gifs")
        assert wait_for(lambda: shown() == [g["id"]], 2), ("GIFs", shown())
        pick("filter-screenshots")
        assert wait_for(lambda: vid["id"] not in shown() and g["id"] not in shown(), 2), ("Screenshots", shown())
    finally:
        s.debug("ui", "history", "keys", "{Escape}")
        wait_for(lambda: not history_state(s)["open"], 2)

    s.json("restore", vid["id"])
    assert wait_for(lambda: vid["id"] in s.status()["cards"], 3)
    time.sleep(0.6)
    c = where(s, "qa", "card:" + vid["id"])
    s.move(c["x"] + 1, c["y"]); s.move(c["x"], c["y"]); time.sleep(0.5)
    s.debug("ui", "qa-keys", " ")                                  # Space: Quick Look plays it
    assert wait_for(lambda: quick_look_window(), 3), "Quick Look opens"
    assert not layer_mapped("snip-preview"), "not the big preview: that's for screenshots"
    time.sleep(1.0)
    w, mon = quick_look_window(), env.monitor()
    shot = s.tmp / "playing.png"
    sh("grim", "-o", OUT, str(shot))
    im = Image.open(shot).convert("RGB")
    cx, cy = round((w["at"][0] - mon["x"] + w["size"][0] / 2) * mon["scale"]), round((w["at"][1] - mon["y"] + w["size"][1] / 3) * mon["scale"])
    colours = {im.getpixel((cx + dx, cy + dy)) for dx in range(-90, 91, 30) for dy in range(-20, 21, 20)}
    assert len(colours) > 3, ("the video draws in Quick Look (the test pattern is colourful)", colours)
    s.debug("ui", "qa-keys", "{Escape}")                         # the pointer still on the card
    assert wait_for(lambda: not quick_look_window(), 3), "Esc on the cards closes it"
    s.cli("close-all")


def quick_look_window():
    return next((c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"] == "Snip Quick Look"), None)


def quick_look_state(s):
    n = len(s.traces("quicklook_state"))
    s.debug("ui", "quicklook", "state")
    assert wait_for(lambda: len(s.traces("quicklook_state")) > n, 2), "no quicklook_state"
    return s.traces("quicklook_state")[-1]


@case
def quick_look_and_open_with(s, env):
    """D51: a recording's card has CleanShot's Quick Look in its top-right corner: a small
    window that plays it, floating and centred, at its size (at least 480 px wide); a click
    or Space pauses and plays it, Esc closes it, and a GIF plays there too (Space on its
    card closes it again, as on macOS). The bottom-right corner, where CleanShot has its
    cloud, is Open With…."""
    s.cli("close-all"); time.sleep(0.3)
    clip, gif = s.tmp / "ql.mp4", s.tmp / "ql.gif"
    make_clip(clip, seconds=3, size="640x360")
    make_clip(gif, seconds=1, size="320x180", audio=False)

    def qa_at(name):
        return where(s, "qa", name)

    def click(card, name):
        c = qa_at("card:" + card)
        s.move(c["x"] + 1, c["y"]); s.move(c["x"], c["y"]); time.sleep(0.5)    # its corners show
        b = qa_at(name)
        assert b["x"] >= 0, (name, "isn't shown")
        s.move(b["x"], b["y"]); time.sleep(0.15); s.click()

    vid = s.json("open", str(clip))["result"]
    time.sleep(0.6)
    click(vid["id"], "card-quicklook")
    assert wait_for(lambda: quick_look_window(), 3), "the eye opens Quick Look"
    w, m = quick_look_window(), env.monitor()
    assert any(t.rstrip("*") == "snip-quicklook" for t in w["tags"]) and w["floating"], (w["tags"], w["floating"])
    sw, sh_ = m["width"] / m["scale"], m["height"] / m["scale"]
    iw, ih = vid["width"] / (vid.get("scale") or 1), vid["height"] / (vid.get("scale") or 1)
    k = min(sw * 0.6 / iw, sh_ * 0.6 / ih, max(1, 480 / iw))
    assert all(abs(a - round(b * k)) <= 1 for a, b in zip(w["size"], (iw, ih))), ("its size, fitted", w["size"], iw, ih, k)
    assert wait_for(lambda: quick_look_state(s)["playing"], 3), quick_look_state(s)
    # a click on it pauses it, and gives it the keyboard (the cards had it)
    cx, cy = w["at"][0] - m["x"] + w["size"][0] / 2, w["at"][1] - m["y"] + w["size"][1] / 3
    s.move(cx + 1, cy); s.move(cx, cy); time.sleep(0.1); s.click()
    assert wait_for(lambda: not quick_look_state(s)["playing"], 2), "a click pauses it"
    assert wait_for(lambda: hypr_json("activewindow").get("title") == "Snip Quick Look", 2), hypr_json("activewindow").get("title")
    s.debug("ui", "quicklook", "keys", "{Space}")
    assert wait_for(lambda: quick_look_state(s)["playing"], 2), "Space plays it again"
    s.debug("ui", "quicklook", "keys", "{Escape}")
    assert wait_for(lambda: not quick_look_window(), 3), "Esc closes it"

    g = s.json("open", str(gif))["result"]
    time.sleep(0.6)
    click(g["id"], "card-quicklook")
    assert wait_for(lambda: quick_look_window() and quick_look_state(s)["gif"], 3), "a GIF in Quick Look"
    f0 = quick_look_state(s)["position"]
    assert wait_for(lambda: quick_look_state(s)["position"] != f0, 2), "it plays"
    s.debug("ui", "qa-keys", " ")                                # Space on its card again: closed
    assert wait_for(lambda: not quick_look_window(), 3), "Space toggles it, as on macOS"

    n = len(s.traces("open_with"))
    click(vid["id"], "card-open-with")
    assert wait_for(lambda: [e for e in s.traces("open_with")[n:] if e["id"] == vid["id"]], 2), "Open With… in the other corner"
    s.cli("close-all")


def record_state(s):
    return s.json("record", "status").get("result")


@case
def record_by_command(s, env):
    """REC-01, REC-05, REC-12, REC-13, API-01: `snip record area` records at once; pause,
    resume and stop come by command, the paused time isn't in the video, and the card is
    the recording. A GIF is made after; discard throws a take away; restart starts over.
    (gsr can't record the nested Hyprland: ffmpeg's test pattern stands in, so this checks
    Snip's side: sizes, timing, the flow.)"""
    s.cli("close-all"); time.sleep(0.3)
    sc = env.monitor()["scale"]

    def start(*extra):
        p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", OUT, *extra)
        assert wait_for(lambda: (record_state(s) or {}).get("state") == "recording", 5), record_state(s)
        return p

    p = start()
    assert record_state(s)["area"] == [100, 80, 400, 300], record_state(s)
    # kept awake: no input for over 0.6 s, and the session doesn't go idle
    idles = len(s.traces("idle"))
    s.debug("idle-watch", "600")
    time.sleep(1.2)
    assert len(s.traces("idle")) == idles, ("the screen is kept awake", s.traces("idle")[idles:])
    assert s.json("record", "pause")["result"]["state"] == "paused"
    time.sleep(0.8)
    s.json("record", "resume")
    time.sleep(0.8)
    s.json("record", "stop")
    r, code = finish(p, timeout=20)
    assert code == 0, r
    rec = r["result"]
    assert (rec["media"], rec["width"], rec["height"]) == ("video", round(400 * sc), round(300 * sc) & ~1), rec
    assert 1700 <= rec["duration"] <= 2600, ("the paused time is left out", rec["duration"])
    assert wait_for(lambda: rec["id"] in s.status()["cards"], 3), "its card"
    assert record_state(s) is None, "nothing left open"
    s.debug("idle-watch", "600")
    assert wait_for(lambda: any(e["idle"] for e in s.traces("idle")[idles:]), 3), "and may idle again after"
    s.debug("idle-watch", "0")

    p = start("--gif")
    time.sleep(1.0)
    s.json("record", "stop")
    r, code = finish(p, timeout=30)
    assert code == 0 and r["result"]["media"] == "gif", r
    assert r["result"]["width"] == round(400 * sc), "narrower than 800: kept"

    p = start()
    time.sleep(0.5)
    s.json("record", "discard")
    r, code = finish(p, timeout=10)
    assert code != 0 or not r.get("ok"), ("a discarded take is no capture", r)
    assert record_state(s) is None

    p = start()
    time.sleep(1.5)
    s.json("record", "restart")
    assert wait_for(lambda: (record_state(s) or {}).get("state") == "recording", 5)
    time.sleep(0.6)
    s.json("record", "stop")
    r, code = finish(p, timeout=20)
    assert code == 0 and r["result"]["duration"] < 1300, ("restart starts over", r["result"]["duration"])
    s.cli("close-all")


def record_hud(s):
    n = len(s.traces("record_hud"))
    s.debug("ui", "record", "state")
    assert wait_for(lambda: len(s.traces("record_hud")) > n, 2), "no record_hud"
    return s.traces("record_hud")[-1]


def record_click(s, name):
    b = where(s, "record", name)
    assert b["x"] >= 0, (name, "isn't shown")
    s.move(b["x"], b["y"]); time.sleep(0.1); s.click()


def record_panel(s):
    """Record Screen: the panel, at once (D49)."""
    p = s.bg("record")
    assert wait_for(lambda: record_hud(s)["phase"] == "ready", 3), record_hud(s)
    time.sleep(0.3)                                  # the layer takes input a frame after it shows
    return p


@case
def record_with_the_hud(s, env):
    """REC-01…05, D49: Record Screen shows CleanShot's panel at once, centred on the screen;
    a drag picks an area instead, and the panel moves into its middle. Record Video counts
    down, the panel gone first; then the controls (outside the area) pause, resume and stop
    it, and the card is the recording. Esc on the panel records nothing."""
    s.cli("close-all"); time.sleep(0.3)
    m = env.monitor()
    W, H = round(m["width"] / m["scale"]), round(m["height"] / m["scale"])
    p = record_panel(s)
    h = record_hud(s)
    assert h["whole"] and h["area"] == [0, 0, W, H], h
    x, y, w, hh = h["panel"]
    assert abs(x + w / 2 - W / 2) <= 1 and abs(y + hh / 2 - H / 2) <= 1, ("centred on the screen", h["panel"])
    s.drag(300, 200, 700, 500)
    assert wait_for(lambda: record_hud(s)["area"] == [300, 200, 400, 300], 3), record_hud(s)
    h = record_hud(s)
    ax, ay, aw, ah = h["area"]
    x, y, w, hh = h["panel"]
    assert not h["whole"] and (x + w / 2, y + hh / 2) == (500, 350), ("in the area's middle", h["panel"])

    def outside(r):
        x, y, w, hh = r
        return x + w <= ax - 2 or x >= ax + aw + 2 or y + hh <= ay - 2 or y >= ay + ah + 2

    record_click(s, "record-video")
    assert wait_for(lambda: record_hud(s)["phase"] == "countdown", 2), "a countdown first"
    assert record_hud(s)["panel"] is None, "the panel is gone before anything is recorded"
    assert wait_for(lambda: record_hud(s)["phase"] == "recording", 4), record_hud(s)
    assert record_state(s)["area"] == [300, 200, 400, 300], ("the area chosen on the panel", record_state(s))
    h = record_hud(s)
    assert h["controls"] and outside(h["controls"]), ("the controls are never in the recording", h["controls"])
    time.sleep(1.2)
    record_click(s, "record-pause")
    assert wait_for(lambda: record_hud(s)["phase"] == "paused", 2)
    t = record_hud(s)["time"]
    time.sleep(1.2)
    assert record_hud(s)["time"] == t, "paused: the time stands still"
    record_click(s, "record-pause")
    assert wait_for(lambda: record_hud(s)["phase"] == "recording", 2)
    time.sleep(0.6)
    record_click(s, "record-stop")
    r, code = finish(p, timeout=20)
    assert code == 0 and r["result"]["media"] == "video", r
    assert wait_for(lambda: not record_hud(s)["open"], 2), "the HUD goes"
    s.cli("close-all")

    # Esc on the panel records nothing
    p = record_panel(s)
    s.debug("ui", "record", "keys", "{Escape}")
    r, code = finish(p, timeout=5)
    assert code != 0 or not r.get("ok"), r
    assert wait_for(lambda: not record_hud(s)["open"], 2)


@case
def record_gif_by_alt_enter(s, env):
    """REC-02, D49: on the panel, Alt+Enter records a GIF (CleanShot's ⌥↩). The panel has
    the keyboard while it's up, so Snip adds no binds to Hyprland for it."""
    import re
    s.cli("close-all"); time.sleep(0.3)
    alt_enter = lambda: re.search(r"modmask: 8\n\tsubmap: [^\n]*\n\tkey: Return\n", sh("hyprctl", "binds").stdout)
    p = record_panel(s)
    s.drag(300, 200, 700, 500)
    assert wait_for(lambda: record_hud(s)["area"] == [300, 200, 400, 300], 3), record_hud(s)
    assert not record_hud(s)["gif"] and not alt_enter(), "a video by default, and no bind of Snip's"
    s.debug("ui", "record", "keys", "{alt+Return}")
    assert wait_for(lambda: (record_state(s) or {}).get("gif") is True, 3), record_state(s)
    assert wait_for(lambda: record_hud(s)["phase"] == "recording", 5), record_hud(s)
    time.sleep(0.8)
    s.json("record", "stop")
    r, code = finish(p, timeout=30)
    assert code == 0 and r["result"]["media"] == "gif", r


@case
def record_panel_chooses_the_area(s, env):
    """D49: on the panel, the area is the whole screen until a drag draws one; its handles
    resize it, a drag inside moves it, W × H sets its size around its middle, ⤢ toggles the
    whole screen, Select area draws a new one (inside the old one too), and Space then a
    click picks a window. A small area has the panel beside it, not over it. The Record key
    again closes the panel (SEL-09)."""
    s.cli("close-all"); time.sleep(0.3)
    m = env.monitor()
    W, H = round(m["width"] / m["scale"]), round(m["height"] / m["scale"])
    area = lambda: record_hud(s)["area"]
    p = record_panel(s)
    assert record_hud(s)["hint"].startswith("Drag to record an area"), record_hud(s)
    record_click(s, "record-full")                   # ⤢: back to an area (the middle 60 %)
    assert wait_for(lambda: area() == [round(W * 0.2), round(H * 0.2), round(W * 0.6), round(H * 0.6)], 2), area()
    record_click(s, "record-full")
    assert wait_for(lambda: record_hud(s)["whole"], 2), area()
    s.drag(200, 150, 800, 550)
    assert wait_for(lambda: area() == [200, 150, 600, 400], 2), area()
    time.sleep(0.2)
    b = where(s, "record", "record-handle-br")
    assert (b["x"], b["y"]) == (803, 553), ("on the frame, just outside the corner", b)
    s.drag(803, 553, 903, 603)
    assert wait_for(lambda: area() == [200, 150, 700, 450], 2), ("the corner's handle resizes it", area())
    s.drag(250, 200, 300, 230)
    assert wait_for(lambda: area() == [250, 180, 700, 450], 2), ("a drag inside moves it", area())
    record_click(s, "record-w")
    time.sleep(0.2)
    s.debug("ui", "record", "keys", "{ctrl+a}480{Return}")
    assert wait_for(lambda: area() == [360, 180, 480, 450], 2), ("480 wide, around its middle", area())
    record_click(s, "record-crop")
    assert wait_for(lambda: record_hud(s)["drawing"], 2)
    s.drag(400, 220, 520, 300)
    assert wait_for(lambda: area() == [400, 220, 120, 80], 2), ("drawn anew inside the old one", area())
    h = record_hud(s)
    assert not h["drawing"] and h["panel"][1] >= 220 + 80 + 2, ("a small area: the panel under it", h["panel"])
    # Space, then a click: the window under the pointer
    s.debug("ui", "record", "keys", "{Space}")
    assert wait_for(lambda: record_hud(s)["picking"], 2)
    h = record_hud(s)
    assert h["panel"] is None and h["hint"].startswith("Click a window") and h["windows"] >= 1, h
    c = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-target")
    lx, ly = c["at"][0] - m["x"], c["at"][1] - m["y"]
    s.move(lx + c["size"][0] / 2 + 1, ly + c["size"][1] / 2); s.move(lx + c["size"][0] / 2, ly + c["size"][1] / 2)
    time.sleep(0.1); s.click()
    assert wait_for(lambda: area() == [lx, ly, c["size"][0], c["size"][1]], 2), ("the window's", area(), c["at"], c["size"])
    assert not record_hud(s)["picking"]
    s.cli("record", check=False)                     # SEL-09: the Record key again closes it
    r, code = finish(p, timeout=5)
    assert code != 0 or not r.get("ok"), r
    assert wait_for(lambda: not record_hud(s)["open"], 2), "the panel is gone"


@case
def record_panel_gap_is_part_of_the_panel(s, env):
    """D49: the panel is one thing: a press in the gap between its two cards doesn't move or
    redraw the area behind it."""
    s.cli("close-all"); time.sleep(0.3)
    p = record_panel(s)
    try:
        s.drag(200, 150, 1000, 650)
        assert wait_for(lambda: (h := record_hud(s))["area"] == [200, 150, 800, 500] and h["panel"] is not None, 2), record_hud(s)
        px, py, pw, ph = record_hud(s)["panel"]
        s.drag(px + pw / 2, py + 93, px + pw / 2 + 60, py + 133)        # between the cards
        time.sleep(0.3)
        assert wait_for(lambda: record_hud(s)["area"] == [200, 150, 800, 500], 2), ("the area stays", record_hud(s)["area"])
    finally:
        s.debug("ui", "record", "keys", "{Escape}")
        finish(p, timeout=5)


@case
def record_panel_edges_and_typed_sizes(s, env):
    """D49: a jittery click doesn't become the area Full screen goes back to; handles stay
    on screen at its edges; a typed width counts when the field is left, not only on Enter."""
    s.cli("close-all"); time.sleep(0.3)
    p = record_panel(s)
    area = lambda: record_hud(s)["area"]
    try:
        assert record_hud(s)["whole"], "it opens on the whole screen"
        s.move(400, 300); s.debug("down"); s.move(402, 301); s.debug("up"); time.sleep(0.3)
        record_click(s, "record-full")
        assert wait_for(lambda: not record_hud(s)["whole"], 2)
        a = area()
        assert a[2] >= 16 and a[3] >= 16, ("an area, not the click", a)
        s.drag(0, 200, 400, 500)
        assert wait_for(lambda: area() == [0, 200, 400, 300], 2), area()
        b = where(s, "record", "record-handle-l")
        assert b["x"] >= 0, ("the left handle is on screen", b)
        record_click(s, "record-w"); time.sleep(0.2)
        s.debug("ui", "record", "keys", "{ctrl+a}320")
        record_click(s, "record-h"); time.sleep(0.2)
        assert wait_for(lambda: area()[2] == 320, 2), ("the typed width counts", area())
    finally:
        s.debug("ui", "record", "keys", "{Escape}")
        s.debug("ui", "record", "keys", "{Escape}")
        finish(p, timeout=5)


@case
def record_panel_shows_what_is_on(s, env):
    """D49: on the panel, a toggle that's on has a soft accent square behind its icon (the
    icon never moves); a source that's off is crossed out: no microphone, no computer
    audio, no camera. Clicks and keys go grey. (A microphone is only chosen here: nothing
    records.)"""
    s.cli("close-all"); time.sleep(0.3)
    s.write_config("")                               # all off, as by default
    time.sleep(0.6)
    p = record_panel(s)
    icons = lambda: record_hud(s)["icons"]
    assert icons() == ["mic-off", "volume-x", "webcam-off", "click", "command"], icons()
    record_click(s, "record-mic")
    assert wait_for(lambda: record_hud(s)["micsOpen"], 2), "its menu opens"
    time.sleep(0.3)                                  # the menu takes input a frame after it shows
    record_click(s, "record-mic:1")
    record_click(s, "record-system")
    record_click(s, "record-camera-button")
    assert wait_for(lambda: record_hud(s)["camerasOpen"], 2)
    time.sleep(0.3)
    record_click(s, "record-camera:1")
    record_click(s, "record-clicks")
    assert wait_for(lambda: icons() == ["+mic", "+volume", "+webcam", "+click", "command"], 2), icons()
    s.debug("ui", "record", "keys", "{Escape}")
    r, code = finish(p, timeout=5)
    assert code != 0 or not r.get("ok"), r
    s.write_config("")


@case
def recording_keeps_out_cards_and_clutter(s, env):
    """Principle 8, CAP-12: Snip's cards are never in a recording (they step aside while it
    records), and with Hide Desktop Clutter on, a recording covers the bars with the
    wallpaper and hides pins meanwhile, then puts everything back."""
    s.cli("close-all"); time.sleep(0.3)

    def desktop():
        n = len(s.traces("desktop_state"))
        s.debug("ui", "desktop")
        assert wait_for(lambda: len(s.traces("desktop_state")) > n, 2)
        return s.traces("desktop_state")[-1]

    def take():
        p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", OUT)
        assert wait_for(lambda: (record_hud(s) or {}).get("phase") == "recording", 5), record_hud(s)
        return p

    shot = s.json("fullscreen", "--monitor", OUT)["result"]
    assert wait_for(lambda: shot["id"] in s.status()["cards"], 3), "a card on screen"
    p = take()
    assert record_hud(s)["cardsAway"] and not desktop()["hidden"], "the cards step aside; clutter stays (the setting is off)"
    s.json("record", "stop")
    finish(p, timeout=20)
    assert wait_for(lambda: not record_hud(s)["cardsAway"], 2), "the cards come back"
    s.write_config("[capture]\nhide_desktop = true")
    time.sleep(0.6)
    p = take()
    assert wait_for(lambda: desktop()["hidden"], 2), "the bars show the wallpaper while it records"
    s.json("record", "stop")
    finish(p, timeout=20)
    assert wait_for(lambda: not desktop()["hidden"], 2), "back as it was"
    s.write_config("")
    s.cli("close-all")


@case
def record_stops_from_the_bar(s, env):
    """D52: a whole-screen take has no controls on screen, so its countdown says to stop it
    from the bar, and a click on Snip in the bar stops it (during the countdown: before it
    starts, and nothing counts on after). A right-click opens the menu, which also pauses
    and resumes it."""
    s.cli("close-all"); time.sleep(0.3)
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before.replace("countdown = 1", "countdown = 3"))   # time to click during it
    time.sleep(0.8)

    def menu_click(name):
        b = where(s, "tray", name)
        assert b["x"] >= 0, (name, "isn't in the menu")
        s.move(b["x"], b["y"]); time.sleep(0.1); s.click()

    try:
        p = record_panel(s)
        s.debug("ui", "record", "keys", "{Return}")                  # the whole screen
        assert wait_for(lambda: record_hud(s)["phase"] == "countdown", 2), record_hud(s)
        assert record_hud(s)["stopHint"], "the countdown says how to stop it"
        s.debug("ui", "tray-click", "left")
        r, code = finish(p, timeout=5)
        assert code != 0 or not r.get("ok"), ("stopped before it started: nothing kept", r)
        time.sleep(3.2)                                              # past where the countdown would end
        assert record_hud(s)["phase"] == "closed" and record_state(s) is None, (record_hud(s)["phase"], record_state(s))

        p = s.bg("record", "fullscreen", "--monitor", OUT)
        assert wait_for(lambda: (record_state(s) or {}).get("state") == "recording", 5), record_state(s)
        assert record_hud(s)["controls"] is None, "no controls on screen"
        s.move(300, 5)                                               # on the bar, as the icon's click
        s.debug("ui", "tray-click", "right")
        assert wait_for(lambda: tray_state(s)["open"], 2), "a right-click opens the menu"
        labels = tray_state(s)["labels"]
        assert "Stop Recording" in labels and "Pause Recording" in labels, labels
        menu_click("menu:record-pause")
        assert wait_for(lambda: record_state(s)["state"] == "paused", 3), record_state(s)
        s.debug("ui", "tray-click", "right")
        assert wait_for(lambda: tray_state(s)["open"] and "Resume Recording" in tray_state(s)["labels"], 2), tray_state(s)
        menu_click("menu:record-pause")
        assert wait_for(lambda: record_state(s)["state"] == "recording", 3), record_state(s)
        time.sleep(0.5)
        s.debug("ui", "tray-click", "left")                           # a click stops it
        r, code = finish(p, timeout=20)
        assert code == 0 and r["result"]["media"] == "video", r
        assert not tray_state(s)["open"], "and no menu"
    finally:
        cfg.write_text(before)
        s.debug("ui", "tray", "keys", "{Escape}")


@case
def record_without_countdown(s, env):
    """REC-03, D49: with no countdown, Record still waits until the panel is off screen,
    then records."""
    s.cli("close-all"); time.sleep(0.3)
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text(before.replace("countdown = 1", "countdown = 0"))
    time.sleep(0.8)
    try:
        n = len(s.traces("record_status"))
        p = record_panel(s)
        s.drag(300, 200, 700, 500)
        assert wait_for(lambda: record_hud(s)["area"] == [300, 200, 400, 300], 3), record_hud(s)
        s.debug("ui", "record", "keys", "{Return}")
        assert wait_for(lambda: record_hud(s)["phase"] == "recording", 5), record_hud(s)
        states = [e["state"] for e in s.traces("record_status")[n:]]
        assert states[0] == "countdown" and "recording" in states, ("the panel's frame first, then recording", states)
        assert record_hud(s)["panel"] is None, "the panel is gone"
        time.sleep(0.5)
        s.json("record", "stop")
        r, code = finish(p, timeout=20)
        assert code == 0 and r["result"]["media"] == "video", r
    finally:
        cfg.write_text(before)


@case
def record_puts_dnd_back(s, env):
    """REC-11: a take puts Do Not Disturb back as it found it, even when the setting
    changes while it records."""
    s.cli("close-all"); time.sleep(0.3)
    holds = lambda: (s.traces("dnd") or [{"holds": 0}])[-1]["holds"]
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", OUT)
    try:
        assert wait_for(lambda: (record_state(s) or {}).get("state") == "recording", 5), record_state(s)
        assert holds() == 1, "DND on while it records"
        cfg.write_text(before.replace("[recording]\n", "[recording]\ndnd = false\n"))
        time.sleep(0.8)                                              # the watcher reloads it
        s.json("record", "stop")
        r, code = finish(p, timeout=20)
        assert code == 0, r
        assert wait_for(lambda: holds() == 0, 3), ("DND put back", holds())
    finally:
        cfg.write_text(before)


@case
def record_controls_stay_outside_the_area(s, env):
    """REC-05: the controls never sit in the area (they'd be in the video): below or above
    it, else beside it (a full-height area, such as a tiled window); with no room
    anywhere, none, as for a whole-screen take (D52: the bar stops it)."""
    def controls(x, y, w, h):
        p = s.bg("record", "area", "--x", str(x), "--y", str(y), "--w", str(w), "--h", str(h), "--monitor", OUT)
        try:
            assert wait_for(lambda: record_hud(s)["phase"] == "recording", 6), record_hud(s)
            time.sleep(0.3)
            return where(s, "record", "record-controls")
        finally:
            s.cli("record", "discard", check=False)
            finish(p, timeout=8)
            time.sleep(0.4)

    for area in ([300, 0, 900, 864], [20, 30, 700, 800], [300, 200, 600, 400]):
        c = controls(*area)
        x, y, w, h = area
        assert c["x"] >= 0, ("shown", area)
        assert not (x <= c["x"] <= x + w and y <= c["y"] <= y + h), ("outside the area", area, (c["x"], c["y"]))
    assert controls(0, 0, 1536, 820)["x"] < 0, "no room anywhere: none"


@case
def record_controls_tips_stay_outside_the_area(s, env):
    """REC-05: the controls' tips point away from the area, so they're never recorded: with the
    controls above the area, a tip under them reached ~8 px into it."""
    x, y, w, h = 300, 400, 600, 440                                 # no room under it: the controls go above
    p = s.bg("record", "area", "--x", str(x), "--y", str(y), "--w", str(w), "--h", str(h), "--monitor", OUT)
    try:
        assert wait_for(lambda: record_hud(s)["phase"] == "recording", 6), record_hud(s)
        time.sleep(0.3)
        c = where(s, "record", "record-pause")
        assert 0 <= c["y"] < y, ("the controls are above the area", c)
        s.move(c["x"] + 1, c["y"]); s.move(c["x"], c["y"])
        tip = lambda: where(s, "record", "record-pause-tip")
        assert wait_for(lambda: tip()["x"] >= 0, 2), "its tip shows"
        assert tip()["y"] < c["y"], ("it points away from the area: up", tip(), c)
    finally:
        s.move(5, 5)
        s.cli("record", "discard", check=False)
        finish(p, timeout=8)
        time.sleep(0.4)


@case
def record_panel_space_closes_its_menus(s, env):
    """REC-02: Space picks a window, and the panel hides meanwhile; an open microphone or
    camera menu stayed on screen, orphaned. It closes with the panel."""
    p = record_panel(s)
    try:
        record_click(s, "record-mic")
        assert wait_for(lambda: record_hud(s)["menus"], 2), "the microphone menu opens"
        s.debug("ui", "record", "keys", "{Space}")
        assert wait_for(lambda: record_hud(s)["picking"], 2), "Space picks a window"
        assert not record_hud(s)["menus"], "and the menu closes"
    finally:
        for _ in range(4):
            if not record_hud(s)["open"]:
                break
            s.debug("ui", "record", "keys", "{Escape}"); time.sleep(0.3)
        finish(p, timeout=5)
        s.write_config("")


@case
def record_controls_ask_first(s, env):
    """REC-05: Restart and Delete on the controls ask first, as CleanShot's do, and the
    recording goes on meanwhile; Cancel keeps it, Delete throws it away (no card)."""
    s.cli("close-all"); time.sleep(0.3)
    p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", OUT)
    assert wait_for(lambda: (record_hud(s) or {}).get("phase") == "recording", 5), record_hud(s)
    record_click(s, "record-restart")
    assert wait_for(lambda: record_hud(s)["asking"] == "restart", 2), record_hud(s)
    time.sleep(0.3)                                  # the pill grew: its new part takes input a frame later
    record_click(s, "record-confirm-cancel")
    assert wait_for(lambda: record_hud(s)["asking"] == "", 2) and record_state(s)["state"] == "recording", "kept, still recording"
    record_click(s, "record-discard")
    assert wait_for(lambda: record_hud(s)["asking"] == "discard", 2), record_hud(s)
    assert record_state(s)["state"] == "recording", "it goes on while asking"
    time.sleep(0.3)
    record_click(s, "record-confirm")
    r, code = finish(p, timeout=10)
    assert record_state(s) is None and not s.status()["cards"], ("thrown away", r)


@case
def record_camera(s, env):
    """REC-08: the camera, as CleanShot shows it: chosen on the panel, a bubble inside the
    area (so it is recorded) at the bottom right; Small to Huge and four shapes from its menu;
    dragged anywhere in the area; a click fills the area with it and another brings it back;
    it stays while recording. (A test card stands in for a camera: there is none here.)"""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    sc = env.monitor()["scale"]

    def shot():
        f = s.tmp / "camera.png"
        sh("grim", "-o", OUT, str(f))
        im = Image.open(f).convert("RGB")
        return lambda x, y: im.getpixel((round(x * sc), round(y * sc)))

    def card(px):                                    # the test card: blue to green, no red
        r, g, b = px
        return r < 90 and g > 90 and b > 120

    p = record_panel(s)
    s.drag(300, 200, 1100, 650)
    assert wait_for(lambda: record_hud(s)["area"] == [300, 200, 800, 450], 3), record_hud(s)
    h = record_hud(s)
    assert "Test camera" in h["cameras"] and h["camera"] is None, h
    record_click(s, "record-camera-button")
    assert wait_for(lambda: record_hud(s)["camerasOpen"], 2), "its menu opens"
    time.sleep(0.3)                                  # the menu takes input a frame after it shows
    record_click(s, "record-camera:1")               # 0 is "No camera"
    assert wait_for(lambda: record_hud(s)["camera"] is not None and record_hud(s)["cameraReady"], 3), record_hud(s)
    h = record_hud(s)
    ax, ay, aw, ah = h["area"]
    x, y, w, hh = h["camera"]
    assert abs(w - ah * 0.25) <= 1 and w == hh, ("Medium: a quarter of the area's height", h["camera"])
    assert (x + w, y + hh) == (ax + aw - 40, ay + ah - 40), ("40 px in from the bottom right", h["camera"], h["area"])
    assert 'camera = "Test camera"' in (s.tmp / "cfg/snip/config.toml").read_text(), "chosen for next time"
    time.sleep(0.3)
    px = shot()
    assert card(px(x + w / 2, y + hh * 0.15)) and not card(px(x + 3, y + 3)), "a circle: the card inside, not in its corner"
    # Large, and Vertical (9:16)
    record_click(s, "record-camera-button")
    assert wait_for(lambda: record_hud(s)["camerasOpen"], 2)
    time.sleep(0.3)
    record_click(s, "record-camera-size=large")
    assert wait_for(lambda: record_hud(s)["cameraSize"] == "large", 2)
    record_click(s, "record-camera-shape=vertical")
    assert wait_for(lambda: record_hud(s)["cameraShape"] == "vertical", 2)
    record_click(s, "record-camera-button")          # the menu closes
    assert wait_for(lambda: not record_hud(s)["camerasOpen"], 2)
    h = record_hud(s)
    x, y, w, hh = h["camera"]
    assert (h["cameraSize"], h["cameraShape"]) == ("large", "vertical") and abs(hh - ah * 0.3375 * 1.4) <= 1, h
    assert abs(w / hh - 9 / 16) < 0.02, h["camera"]
    # dragged to the top left, kept inside the area
    s.drag(x + w / 2, y + hh / 2, ax - 200, ay - 200, steps=8)
    assert wait_for(lambda: record_hud(s)["camera"][:2] == [ax, ay], 2), ("stopped at the area's corner", record_hud(s)["camera"], h["area"])
    # a click fills the area with it; another brings it back where it was
    record_click(s, "record-camera")
    assert wait_for(lambda: record_hud(s)["camera"] == [ax, ay, aw, ah], 2), record_hud(s)["camera"]
    time.sleep(0.3)
    px = shot()
    assert card(px(ax + 6, ay + 6)) and card(px(ax + aw - 6, ay + ah - 6)), "the card fills the area, corners too"
    s.move(ax + 30, ay + 30); time.sleep(0.1); s.click()   # (the panel is over its middle)
    assert wait_for(lambda: record_hud(s)["camera"][:2] == [ax, ay] and not record_hud(s)["cameraFull"], 2), record_hud(s)
    # it stays while recording, inside the area (the controls stay outside); Enter records
    s.debug("ui", "record", "keys", "{Return}")
    assert wait_for(lambda: record_hud(s)["phase"] == "recording", 5), record_hud(s)
    assert record_hud(s)["camera"] is not None, "the camera is recorded"
    s.json("record", "stop")
    r, code = finish(p, timeout=20)
    assert code == 0 and r["result"]["media"] == "video", r
    assert wait_for(lambda: not record_hud(s)["open"], 3)
    s.cli("close-all")
    s.write_config("")                               # no camera for the cases after


@case
def record_clicks_and_keys(s, env):
    """REC-09/10: Highlight Clicks and Show Keystrokes, from the panel. While it records,
    Hyprland's own hooks report each click (a ring where it is, in the area) and the command
    keys (Ctrl+C, pressed again: ×2); nothing of them is left when the recording ends. (A
    real keyboard can't type into the nested Hyprland: the keys come as the hook sends them,
    the hook itself is checked against a mock of Hyprland's Lua.)"""
    s.cli("close-all"); time.sleep(0.3)
    binds = lambda: sh("hyprctl", "binds").stdout
    p = record_panel(s)
    s.drag(300, 200, 1100, 650)
    assert wait_for(lambda: record_hud(s)["area"] == [300, 200, 800, 450], 3), record_hud(s)
    record_click(s, "record-clicks")
    assert wait_for(lambda: record_hud(s)["clicks"], 2), "Highlight clicks is on"
    record_click(s, "record-keys")
    assert wait_for(lambda: record_hud(s)["keys"], 2), "Show keystrokes is on"
    n = len(s.traces("record_hooks"))
    record_click(s, "record-video")
    assert wait_for(lambda: record_hud(s)["phase"] == "recording", 5), record_hud(s)
    assert wait_for(lambda: len(s.traces("record_hooks")) > n, 3), "the hooks are in"
    sid = s.traces("record_hooks")[-1]["sid"]
    assert "Snip: recording clicks" in binds()
    # a real click in the area: a ring there; one outside it isn't recorded, so none
    rings = record_hud(s)["rings"]
    s.move(700, 420); time.sleep(0.1); s.click()
    assert wait_for(lambda: record_hud(s)["rings"] == rings + 1, 2), ("a ring where it clicked", record_hud(s))
    s.move(1300, 760); time.sleep(0.1); s.click()
    time.sleep(0.4)
    assert record_hud(s)["rings"] == rings + 1, "not for a click outside the area"
    # Ctrl+C, twice, as the key hook sends it
    for code, st in ((37, 1), (54, 1), (54, 0), (54, 1), (54, 0), (37, 0)):
        hypr_eval(f'hl.dispatch(hl.dsp.event("snip:{sid}:k:{code}:{st}"))')
    assert wait_for(lambda: record_hud(s)["keystrokes"] == "Ctrl + C  ×2", 2), record_hud(s)
    assert wait_for(lambda: record_hud(s)["keystrokes"] == "", 3), "it fades"
    hypr_eval('hl.dispatch(hl.dsp.event("snip:other:k:9:1"))')
    time.sleep(0.3)
    assert record_hud(s)["keystrokes"] == "", "another recording's keys aren't shown"
    # a config reload starts Hyprland's Lua over: the hooks go back in
    m = len(s.traces("hypr_reinstalled"))
    sh("hyprctl", "reload")
    env.configure(1.25)                                              # the test output as it was
    assert wait_for(lambda: len(s.traces("hypr_reinstalled")) > m, 3), "snipd saw the reload"
    assert wait_for(lambda: "Snip: recording clicks" in binds(), 3), "the hooks are back after a reload"
    s.json("record", "stop")
    r, code = finish(p, timeout=20)
    assert code == 0, r
    assert wait_for(lambda: "Snip: recording clicks" not in binds(), 3), "the hooks are gone"
    s.write_config("")


@case
def recording_safety(s, env):
    """REC-14: takes left behind when Snip stopped mid-recording are recovered at its next
    start (a finished one as it is, one cut short from its fragments), each with its card;
    a recording stops by itself, and is kept, when the disk is almost full."""
    s.cli("close-all"); time.sleep(0.3)
    before = {r["id"] for r in s.json("list", "-n", "200")["result"]}
    n = len(s.traces("record_recovered"))
    s.cli("quit")
    assert wait_for(lambda: s.cli("ping", check=False).returncode != 0, 5), "Snip has quit"
    stage = s.tmp / "data/snip/recording"
    stage.mkdir(parents=True, exist_ok=True)
    make_clip(stage / "20261009-010000000.mp4", seconds=2, size="320x240")
    # a recorder killed while it wrote fragments: three quarters of a file
    whole = s.tmp / "whole.mp4"
    make_clip(whole, seconds=2, size="320x240")
    cut = stage / "20261009-010100000.mp4"
    sh("ffmpeg", "-v", "error", "-y", "-i", str(whole), "-c", "copy", "-movflags", "+empty_moov+default_base_moof", "-frag_duration", "250000", str(cut))
    cut.write_bytes(cut.read_bytes()[: cut.stat().st_size * 3 // 4])
    s.env["SNIP_RECORD_MIN_FREE"] = str(1 << 60)                    # every disk is "almost full"
    s.start()
    try:
        assert wait_for(lambda: len(s.traces("record_recovered")) >= n + 2, 10), "both takes are recovered"
        new = [r for r in s.json("list", "-n", "200")["result"] if r["id"] not in before]
        lengths = sorted(r["duration_ms"] for r in new)
        assert len(new) == 2 and 500 <= lengths[0] < 1900 and lengths[1] >= 1900, ("most of the cut one, all of the other", lengths)
        assert not list(stage.iterdir()), list(stage.iterdir())
        assert wait_for(lambda: all(r["id"] in s.status()["cards"] for r in new), 3), "each with its card"
        s.cli("close-all"); time.sleep(0.3)
        p = s.bg("record", "area", "--x", "100", "--y", "80", "--w", "400", "--h", "300", "--monitor", OUT)
        r, code = finish(p, timeout=15)
        assert code == 0 and r["result"]["media"] == "video", ("it stops by itself, and is kept", r)
        assert s.traces("record_disk_low") and record_state(s) is None
    finally:
        s.cli("quit", check=False)
        wait_for(lambda: s.cli("ping", check=False).returncode != 0, 5)
        del s.env["SNIP_RECORD_MIN_FREE"]
        s.start()


def video_editor_window():
    return next((c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"].startswith("Snip Video Editor")), None)


def video_state(s):
    n = len(s.traces("video_state"))
    s.debug("ui", "video", "state")
    assert wait_for(lambda: len(s.traces("video_state")) > n, 3), "no video_state"
    return s.traces("video_state")[-1]


def video_point(s, env, name):
    p = where(s, "video", name)
    assert p["x"] >= 0, (name, "isn't shown")
    w, mon = video_editor_window(), env.monitor()
    return w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"]


def video_click(s, env, name):
    s.move(*video_point(s, env, name)); time.sleep(0.1); s.click()


@case
def quick_look_closes_with_its_button(s, env):
    """D51: Quick Look has a close button, top left, with its controls: with no title bar,
    only its card's eye, Esc (with the keyboard) or Super+W closed it."""
    s.cli("close-all"); time.sleep(0.3)
    clip = s.tmp / "qlclose.mp4"
    make_clip(clip, seconds=3, size="640x360")
    vid = s.json("open", str(clip))["result"]
    time.sleep(0.6)
    try:
        c = where(s, "qa", "card:" + vid["id"])
        s.move(c["x"] + 1, c["y"]); s.move(c["x"], c["y"]); time.sleep(0.5)
        b = where(s, "qa", "card-quicklook")
        s.move(b["x"], b["y"]); time.sleep(0.15); s.click()
        assert wait_for(lambda: quick_look_window(), 3), "Quick Look opens"
        assert wait_for(lambda: quick_look_state(s)["playing"], 3), "it plays"
        w, m = quick_look_window(), env.monitor()
        cx, cy = w["at"][0] - m["x"] + w["size"][0] / 2, w["at"][1] - m["y"] + w["size"][1] / 3
        s.move(cx + 1, cy); s.move(cx, cy); time.sleep(0.1); s.click()   # a click pauses it: the controls show
        assert wait_for(lambda: not quick_look_state(s)["playing"], 2), "paused"
        x = lambda: where(s, "quicklook", "quicklook-close")
        assert wait_for(lambda: x()["x"] >= 0, 2), "the close button shows with the controls"
        s.move(w["at"][0] - m["x"] + x()["x"], w["at"][1] - m["y"] + x()["y"]); time.sleep(0.15); s.click()
        assert wait_for(lambda: not quick_look_window(), 3), "a click on it closes Quick Look"
    finally:
        s.debug("ui", "quicklook", "keys", "{Escape}")
        s.cli("close-all")


@case
def video_editor(s, env):
    """VED-01…03, D50: `snip edit` opens a video in the video editor, laid out as CleanShot
    5's (frames along a timeline across the bottom); Split and Cut (by hand), and a trim,
    keep only the parts wanted; ⏮ ⏭ go from part to part; a track mutes; the timeline
    zooms; Save as New gives a new capture of just those parts; Replace (as a GIF, from the
    Export popover) turns the recording into it."""
    s.cli("close-all"); time.sleep(0.3)
    clip = s.tmp / "edit-me.mp4"
    make_clip(clip, seconds=4, size="640x360")
    s.json("edit", str(clip))
    assert wait_for(lambda: video_editor_window(), 6), "the video editor opens"
    time.sleep(0.8)                                                # centred by its rule
    w = video_editor_window()
    from PIL import Image
    shot = s.tmp / "editor.png"
    sh("grim", "-o", OUT, str(shot))
    im, mon = Image.open(shot).convert("RGB"), env.monitor()
    sc = mon["scale"]
    px, py = video_point(s, env, "video-preview")
    cx, cy = round(px * sc), round(py * sc)
    colours = {im.getpixel((cx + dx, cy + dy)) for dx in range(-240, 241, 60) for dy in range(-90, 91, 45)}
    assert len(colours) > 3, ("the first frame shows before playing", colours)
    assert any(t.rstrip("*") == "snip-video-editor" for t in w["tags"]) and w["floating"], (w["tags"], w["floating"])
    st = video_state(s)
    assert st["tracks"] == 1 and 3900 <= st["duration"] <= 4100, st
    assert wait_for(lambda: video_state(s)["thumbs"] == 12, 5), "its frames come along the timeline"
    orig = st["id"]
    # split at 1 s and 2.5 s, by the button at the playhead
    for ms in (1000, 2500):
        s.debug("ui", "video", "seek", str(ms))
        assert wait_for(lambda: abs(video_state(s)["position"] - ms) < 40, 3), video_state(s)
        video_click(s, env, "video-split")
        assert wait_for(lambda: any(abs(c - ms) < 40 for c in video_state(s)["cuts"]), 2), video_state(s)["cuts"]
    # ⏮ ⏭: to the edges of the parts
    s.debug("ui", "video", "seek", "1800")
    assert wait_for(lambda: abs(video_state(s)["position"] - 1800) < 40, 3), video_state(s)
    video_click(s, env, "video-next")
    assert wait_for(lambda: video_state(s)["position"] == 2500, 2), video_state(s)
    video_click(s, env, "video-prev")
    assert wait_for(lambda: video_state(s)["position"] == 1000, 2), video_state(s)
    # the track mutes, and comes back at 100 %
    video_click(s, env, "video-mute:0")
    assert wait_for(lambda: video_state(s)["volumes"] == [0], 2), video_state(s)
    video_click(s, env, "video-mute:0")
    assert wait_for(lambda: video_state(s)["volumes"] == [1], 2), video_state(s)
    # a drag along the timeline: the playhead ends where the pointer does (seeks
    # don't pile up behind it)
    (x0, y0), (x1, y1) = video_point(s, env, "video-part:0"), video_point(s, env, "video-part:2")
    s.drag(x0, y0, x1, y1, steps=12)
    assert wait_for(lambda: abs(video_state(s)["position"] - 3250) < 60, 3), video_state(s)
    # frame steps go from the frame shown to the middle of the next: at 30 fps, 1000 ms
    # is frame 30's first instant, where Qt still shows frame 29
    s.debug("ui", "video", "seek", "1000")
    for key, ms in (("{Right}", 1017), ("{Right}", 1050), ("{Left}", 1017)):
        s.debug("ui", "video", "keys", key)
        assert wait_for(lambda: video_state(s)["position"] == ms, 2), (key, ms, video_state(s))
    # the middle part: select it on the timeline, then Cut
    video_click(s, env, "video-part:1")
    assert wait_for(lambda: video_state(s)["selected"] == 1, 2), video_state(s)
    video_click(s, env, "video-cut")
    assert wait_for(lambda: len(video_state(s)["cut"]) == 1, 2), video_state(s)
    s.debug("ui", "video", "trim", "200", "3800")
    st = video_state(s)
    assert 2000 <= st["keptMs"] <= 2200, ("0.2–1.0 s and 2.5–3.8 s are kept", st["kept"])
    # the timeline zooms (the slider's middle: ×2.8), the playhead kept in view
    video_click(s, env, "video-zoom")
    assert wait_for(lambda: 2.6 < video_state(s)["zoom"] < 3.1, 2), video_state(s)
    n = len(s.traces("video_exported_ui"))
    s.debug("ui", "video", "export", "new")
    assert wait_for(lambda: len(s.traces("video_exported_ui")) > n, 30), "the export ends"
    done = s.traces("video_exported_ui")[-1]
    assert done["ok"], done
    assert wait_for(lambda: not video_editor_window(), 3), "the editor closes after it"
    new = next(r for r in s.json("list", "-n", "5")["result"] if r["id"] == done["id"])
    assert new["id"] != orig and 1950 <= new["duration_ms"] <= 2250, ("just the parts kept", new["duration_ms"])
    assert new["width"] == 640 and done["id"] in s.status()["cards"], "a new capture, with its card"
    # Replace, as a GIF: the recording itself becomes a GIF
    s.cli("close-all"); time.sleep(0.3)                             # that card would cover the editor's corner
    s.json("edit", str(clip))                                       # (a new import of the clip, to replace)
    assert wait_for(lambda: video_editor_window(), 6)
    time.sleep(0.8)
    target = video_state(s)["id"]
    s.debug("ui", "video", "trim", "0", "1000")
    video_click(s, env, "video-export")
    assert wait_for(lambda: video_state(s)["exportOpen"], 2), "the Export popover opens"
    time.sleep(0.2)
    video_click(s, env, "video-format=true")
    assert wait_for(lambda: video_state(s)["gif"], 2)
    n = len(s.traces("video_exported_ui"))
    video_click(s, env, "video-replace")
    assert wait_for(lambda: len(s.traces("video_exported_ui")) > n, 30)
    assert s.traces("video_exported_ui")[-1]["ok"], s.traces("video_exported_ui")[-1]
    rec = next(r for r in s.json("list", "-n", "5")["result"] if r["id"] == target)
    assert rec["source"].endswith("source.gif") and rec["duration_ms"] <= 1200, rec
    s.cli("close-all")


@case
def video_editor_gives_memory_back(s, env):
    """Closing the video editor gives its memory back: opened and closed four times,
    snipd doesn't keep growing (glibc kept what the window's threads freed, until
    snipd trims it)."""
    clip = s.tmp / "mem.mp4"
    make_clip(clip, seconds=6, size="1920x1080")
    vid = s.json("open", str(clip))["result"]
    mon = env.monitor()

    def rss():
        status = dict(l.split(":", 1) for l in open(f"/proc/{s.proc.pid}/status") if ":" in l)
        return int(status["VmRSS"].split()[0]) // 1024

    sizes = []
    for _ in range(4):
        s.debug("annotate", vid["id"])
        assert wait_for(lambda: video_editor_window(), 8), "the editor opens"
        time.sleep(2.0)
        w = video_editor_window()
        s.move(w["at"][0] - mon["x"] + w["size"][0] / 2, w["at"][1] - mon["y"] + 20); s.click(); time.sleep(0.3)
        s.debug("ui", "video", "keys", "{Escape}")
        assert wait_for(lambda: not video_editor_window(), 5), "Escape closes it"
        time.sleep(3.0)
        sizes.append(rss())
    s.cli("close-all")
    # the second close dips below the rest (319-331 MB, then 343-356, on every
    # build): growth is from the higher of the first two
    assert sizes[3] - max(sizes[:2]) <= 25, ("MB after each close", sizes)


@case
def video_editor_keys_come_back_after_a_volume(s, env):
    """VED-02: a volume typed in its box and left with Esc, or with a click on the timeline,
    gives the keys back. Space, B, the arrows and Ctrl+S had gone to the hidden field."""
    s.cli("close-all"); time.sleep(0.3)
    clip = s.tmp / "volkeys.mp4"
    make_clip(clip, seconds=3, size="640x360")
    s.json("edit", str(clip))
    assert wait_for(lambda: video_editor_window(), 6), "the video editor opens"
    time.sleep(0.8)
    try:
        def type_into_volume():
            s.move(*video_point(s, env, "video-volume:0")); time.sleep(0.3)   # its value shows on hover
            video_click(s, env, "video-volume:0-value"); time.sleep(0.2)
        type_into_volume()
        s.debug("ui", "video", "keys", "{Escape}")
        s.debug("ui", "video", "keys", "{Space}")
        assert wait_for(lambda: video_state(s)["playing"], 2), "after Esc, Space plays"
        s.debug("ui", "video", "keys", "{Space}")
        assert wait_for(lambda: not video_state(s)["playing"], 2)
        type_into_volume()
        video_click(s, env, "video-timeline")
        s.debug("ui", "video", "keys", "{Space}")
        assert wait_for(lambda: video_state(s)["playing"], 2), "after a click on the timeline, Space plays"
    finally:
        for _ in range(3):
            if not video_editor_window():
                break
            s.debug("ui", "video", "keys", "{Escape}"); time.sleep(0.4)
        s.cli("close-all")


@case
def video_editor_fits_a_small_screen(s, env):
    """VED-01: the video editor fits the screen it opens on. It was a fixed 1180 x 780, so on
    1280 x 720 logical px (a 1080p screen at 1.5) the timeline and Export were off screen."""
    s.cli("close-all"); time.sleep(0.3)
    clip = s.tmp / "small.mp4"
    make_clip(clip, seconds=2, size="640x360")
    env.configure(1.5)
    try:
        mon = env.monitor()
        W, H = mon["width"] / mon["scale"], mon["height"] / mon["scale"]
        s.json("edit", str(clip))
        assert wait_for(lambda: video_editor_window(), 6), "the video editor opens"
        time.sleep(0.5)
        size = video_editor_window()["size"]
        assert size[0] <= W and size[1] <= H, ("on screen", size, (W, H))
    finally:
        # (never clicked, the editor hasn't the keyboard: Escape wouldn't reach it)
        s.cli("debug", "ui", "close-editors")
        env.configure(1.25)
        s.cli("close-all")


@case
def video_editor_background(s, env):
    """D50: the video editor's Background section puts a background around the recording,
    as the Background tool does around a screenshot: in the preview at once, and in the
    export, which is the background's size with the recording in it, its corners rounded."""
    from PIL import Image
    s.cli("close-all"); time.sleep(0.3)
    clip = s.tmp / "bg-me.mp4"
    make_clip(clip, seconds=2, size="640x360")
    s.json("edit", str(clip))
    assert wait_for(lambda: video_editor_window(), 6), "the video editor opens"
    time.sleep(0.8)
    video_click(s, env, "video-section=background")
    assert wait_for(lambda: video_state(s)["section"] == "background", 2), video_state(s)
    time.sleep(0.3)
    video_click(s, env, "bg-color:#ff3b30")
    assert wait_for(lambda: (video_state(s)["bg"] or {}).get("value") == "#ff3b30", 2), video_state(s)
    st = video_state(s)
    bg, outer = st["bg"], st["outer"]
    pad = bg["padding"] + bg["inset"]
    assert (outer["w"], outer["h"]) == (640 + 2 * pad, 360 + 2 * pad), (bg, outer)
    red = lambda p: p[0] > 200 and p[1] < 90 and p[2] < 90
    # the preview: red around the recording
    time.sleep(0.3)
    shot = s.tmp / "bg-preview.png"
    sh("grim", "-o", OUT, str(shot))
    im, mon, w = Image.open(shot).convert("RGB"), env.monitor(), video_editor_window()
    x, y, rw, rh = st["result"]
    at = lambda dx, dy: im.getpixel((round((w["at"][0] - mon["x"] + x + dx) * mon["scale"]), round((w["at"][1] - mon["y"] + y + dy) * mon["scale"])))
    assert red(at(4, rh / 2)) and not red(at(rw / 2, rh / 2)), ("red around it in the preview", at(4, rh / 2), at(rw / 2, rh / 2))
    # exported: the background's size, red around the recording, the recording inside
    n = len(s.traces("video_exported_ui"))
    s.debug("ui", "video", "export", "new")
    assert wait_for(lambda: len(s.traces("video_exported_ui")) > n, 30), "the export ends"
    done = s.traces("video_exported_ui")[-1]
    assert done["ok"], done
    new = next(r for r in s.json("list", "-n", "5")["result"] if r["id"] == done["id"])
    W, H = (640 + 2 * pad) & ~1, (360 + 2 * pad) & ~1
    assert (new["width"], new["height"]) == (W, H), ("the background's size", new["width"], new["height"], W, H)
    frame = s.tmp / "bg-frame.png"
    sh("ffmpeg", "-v", "error", "-y", "-i", new["source"], "-frames:v", "1", str(frame))
    f = Image.open(frame).convert("RGB")
    assert red(f.getpixel((4, H // 2))), ("red around it in the export", f.getpixel((4, H // 2)))
    r, g, b = f.getpixel((pad + 1, pad + 1))
    assert r > 2 * g and r > 2 * b, ("its corner rounded off: the background (under the shadow) shows", (r, g, b))
    assert not red(f.getpixel((W // 2, H // 2))), "the recording inside"
    s.cli("close-all")


@case
def history_filters(s, env):
    """HIS-01: History's filters: All, Screenshots (plain captures) and Projects (objects
    still editable)."""
    s.cli("close-all"); time.sleep(0.3)
    plain = s.json("fullscreen", "--monitor", OUT)["result"]
    png = s.tmp / "project.png"
    make_text_png(png, ["a project"])
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "rect", "20", "20", "80", "40")
    editor_flush(s)
    project = editor_state(s)["id"]
    close_editor(s)
    s.cli("close-all")
    s.cli("history")
    assert wait_for(lambda: history_state(s)["open"], 3)

    def pick(name):
        b = where(s, "history", name)
        assert b["x"] >= 0, (name, "isn't shown")
        s.move(b["x"], b["y"]); time.sleep(0.1); s.click()

    shown = lambda: history_state(s)["items"]
    assert plain["id"] in shown() and project in shown(), "All"
    pick("filter-projects")
    assert wait_for(lambda: project in shown() and plain["id"] not in shown(), 2), ("Projects", shown())
    pick("filter-screenshots")
    assert wait_for(lambda: plain["id"] in shown() and project not in shown(), 2), ("Screenshots", shown())
    pick("filter-all")
    assert wait_for(lambda: plain["id"] in shown() and project in shown(), 2), ("All again", shown())
    s.debug("ui", "history", "keys", "{Escape}")
    assert wait_for(lambda: not history_state(s)["open"], 2)


@case
def launcher_starts_snip(s, env):
    """OMA-09: Snip in the launcher runs `snip settings`, which works even when Snip isn't
    running (quit from its menu, say): the CLI starts it, then Settings opens."""
    s.cli("quit")
    assert wait_for(lambda: s.cli("ping", check=False).returncode != 0, 5), "Snip has quit"
    try:
        r = s.cli("settings", check=False, timeout=15)
        assert r.returncode == 0, r.stderr
        assert wait_for(lambda: settings_client(), 8), "Snip started and Settings opened"
    finally:
        s.cli("quit", check=False)                                 # the CLI's own Snip goes; the suite's comes back
        wait_for(lambda: s.cli("ping", check=False).returncode != 0, 5)
        s.start()


def welcome_client():
    return next((c for c in hypr_json("clients") if c["class"] == "snip-e2e" and c["title"] == "Welcome to Snip"), None)


def welcome_state(s):
    n = len(s.traces("welcome_state"))
    s.debug("ui", "welcome", "state")
    assert wait_for(lambda: len(s.traces("welcome_state")) > n, 2), "no welcome_state"
    return s.traces("welcome_state")[-1]


def welcome_click(s, env, name):
    p = where(s, "welcome", name)
    assert p["x"] >= 0, (name, "isn't shown")
    w, mon = welcome_client(), env.monitor()
    s.move(w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    time.sleep(0.15)
    s.click()


@case
def welcome_first_run(s, env):
    """OMA-02: on Snip's first start the Welcome opens by itself, floating. Use these keys
    adds one line to hyprland.lua (once) and the keys work at once; Get Started closes it
    for good; `snip welcome` shows it again."""
    hypr = s.tmp / "cfg/hypr"
    hypr.mkdir(parents=True, exist_ok=True)
    (hypr / "hyprland.lua").write_text("-- my config\n")
    marker = s.tmp / "data/snip/welcomed"
    s.cli("quit")
    assert wait_for(lambda: s.cli("ping", check=False).returncode != 0, 5)
    marker.unlink()
    s.start()
    try:
        assert wait_for(lambda: welcome_client(), 8), "the Welcome opens on the first start"
        w = welcome_client()
        assert any(t.rstrip("*") == "snip-welcome" for t in w["tags"]) and w["floating"], (w["tags"], w["floating"])
        assert wait_for(lambda: welcome_state(s)["st"].get("keysInConfig") is False, 2), welcome_state(s)
        welcome_click(s, env, "use-keys")
        assert wait_for(lambda: "snip.lua" in (hypr / "hyprland.lua").read_text(), 3), "the line is added"
        text = (hypr / "hyprland.lua").read_text()
        assert text.startswith("-- my config\n") and text.count("snip.lua") == 1, text
        assert wait_for(lambda: binds_for("Snip: Capture area"), 3), "the keys work at once"
        assert wait_for(lambda: welcome_state(s)["st"]["keysInConfig"] is True, 2), "and it says so"
        time.sleep(0.3)                               # the page is laid out anew: input lands a frame later
        welcome_click(s, env, "welcome-done")
        assert wait_for(lambda: not welcome_client(), 3) and marker.exists(), "Get Started closes it for good"
        s.cli("welcome")
        assert wait_for(lambda: welcome_client(), 5), "snip welcome shows it again"
    finally:
        s.debug("ui", "welcome", "close")
        marker.touch()
        for code in (12, 13, 14, 15):                                  # snip.lua's keys go again
            hypr_eval(f'pcall(hl.unbind, "SUPER + SHIFT + code:{code}")')


@case
def config_typo_keeps_the_rest(s, env):
    """SET-01: a typo in config.toml costs only that key: the rest of the file still
    counts, and `snip reload` names the key it left out."""
    themes = lambda: [t["name"] for t in s.traces("theme")]
    n = len(themes())
    try:
        s.write_config('appearance = "light"\nsoundz = false')
        assert wait_for(lambda: themes()[n:] and themes()[-1] == "snip-light", 3), ("the rest still counts", themes()[n:])
        r = s.json("reload")
        assert not r["ok"] and "general.soundz" in r["error"], r
    finally:
        s.write_config("")
        wait_for(lambda: themes()[-1] != "snip-light", 3)


@case
def appearance_own_or_omarchy(s, env):
    """D48: Settings › General › Appearance: Snip takes the Omarchy theme's colours (the
    default), or its own CleanShot-like look in light or dark, at once."""
    from PIL import Image
    s.cli("settings", "general")
    assert wait_for(lambda: settings_client(), 6), "Settings opens"
    time.sleep(0.6)
    sc = env.monitor()["scale"]

    def page_luma():
        w, mon = settings_client(), env.monitor()
        f = s.tmp / "appearance.png"
        sh("grim", "-o", OUT, str(f))
        x, y = w["at"][0] - mon["x"] + w["size"][0] - 30, w["at"][1] - mon["y"] + w["size"][1] - 30
        r, g, b = Image.open(f).convert("RGB").getpixel((round(x * sc), round(y * sc)))
        return 0.2126 * r + 0.7152 * g + 0.0722 * b

    themes = lambda: [t["name"] for t in s.traces("theme")]
    first = themes()[-1]
    for look, light in (("light", True), ("dark", False)):
        n = len(themes())
        s.write_config(f'appearance = "{look}"')
        assert wait_for(lambda: themes()[n:] and themes()[-1] == f"snip-{look}", 3), themes()[n:]
        time.sleep(0.5)
        luma = page_luma()
        assert (luma > 200) if light else (luma < 60), (look, luma)
    n = len(themes())
    s.write_config("")
    assert wait_for(lambda: themes()[n:] and themes()[-1] == first, 3), ("back to the Omarchy theme", first, themes()[n:])
    s.cli("close-all")


@case
def settings_window(s, env):
    """SET-02: `snip settings TAB` opens Settings on that tab, floating (its rule tags it).
    A click writes config.toml, keeping the file's comments, and the window follows the
    file; a hand edit shows in it at once. Escape closes it."""
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    cfg.write_text("# my settings\n" + before)
    try:
        s.json("settings", "annotate")
        assert wait_for(lambda: settings_client(), 5), "the window opens"
        w = settings_client()
        assert any(t.rstrip("*") == "snip-settings" for t in w["tags"]) and w["floating"], (w["tags"], w["floating"])
        st = settings_state(s, "annotate.object_shadows")
        assert st["tab"] == "annotate" and st["value"] is False and st["version"], st
        settings_click(s, env, "annotate.object_shadows")
        assert wait_for(lambda: "object_shadows = true" in cfg.read_text(), 2), cfg.read_text()
        assert cfg.read_text().startswith("# my settings\n"), "the file's comments stay"
        assert wait_for(lambda: settings_state(s, "annotate.object_shadows")["value"] is True, 2), "the window follows"
        settings_click(s, env, "tab:quick-access")
        assert wait_for(lambda: settings_state(s)["tab"] == "quick-access", 2), "the tab bar switches"
        settings_click(s, env, "quick_access.position=right")
        assert wait_for(lambda: 'position = "right"' in cfg.read_text(), 2), cfg.read_text()
        cfg.write_text(cfg.read_text().replace('position = "right"', 'position = "left"'))      # by hand
        assert wait_for(lambda: settings_state(s, "quick_access.position")["value"] == "left", 3), "a hand edit shows"
        s.debug("ui", "settings", "keys", "{Escape}")
        assert wait_for(lambda: not settings_client(), 2), "Escape closes it"
    finally:
        s.debug("ui", "settings", "close")
        cfg.write_text(before)
        time.sleep(0.8)


QT_SHIFT, QT_CTRL, QT_ALT, QT_META = 0x02000000, 0x04000000, 0x08000000, 0x10000000


def binds_for(description):
    """The key specs Hyprland binds with this description (`hyprctl binds`)."""
    specs = []
    for block in sh("hyprctl", "binds").stdout.split("\n\n"):
        f = {k.strip(): v.strip() for k, v in (line.split(":", 1) for line in block.splitlines() if ":" in line)}
        if f.get("description") == description and not f.get("submap"):
            specs.append(f["key"])
    return specs


@case
def shortcut_recorder(s, env):
    """SET-08: Settings › Shortcuts records a key for an action while an empty submap holds
    Hyprland's binds off, binds it and writes it to config.toml. A key that already runs
    something asks first; × clears a key; Restore Defaults brings snip.lua's keys back."""
    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    # stand-ins for snip.lua's default keys and an Omarchy bind, in the nested Hyprland
    hypr_eval('function snip_bind_defaults() pcall(hl.unbind, "SUPER + SHIFT + code:13") '
              'hl.bind("SUPER + SHIFT + code:13", hl.dsp.global("snip-e2e:area"), { description = "Snip: Capture area" }) end '
              'snip_bind_defaults()')
    hypr_eval('hl.bind("SUPER + SHIFT + code:12", hl.dsp.no_op(), { description = "Move window to workspace 3" })')
    try:
        s.json("settings", "shortcuts")
        assert wait_for(lambda: settings_client(), 5), "the window opens"
        area = next(x for x in settings_state(s)["shortcuts"] if x["id"] == "area")
        assert area["keys"] == ["Super+Shift+4"], area

        settings_click(s, env, "keys:history")
        assert wait_for(lambda: settings_state(s)["recording"] == "history", 2), "a click starts recording"
        assert sh("hyprctl", "submap").stdout.strip() == "snip-record", ("Hyprland's binds are held off", sh("hyprctl", "submap").stdout)
        s.debug("ui", "settings", "press", QT_META | QT_ALT, 32)              # Super+Alt+O
        assert wait_for(lambda: binds_for("Snip: Open capture history") == ["SUPER + ALT + code:32"], 2), binds_for("Snip: Open capture history")
        assert sh("hyprctl", "submap").stdout.strip() == "default", "the binds are back"
        assert 'history = "SUPER + ALT + code:32"' in cfg.read_text(), cfg.read_text()
        assert next(x for x in settings_state(s)["shortcuts"] if x["id"] == "history")["keys"] == ["Super+Alt+O"]

        s.debug("ui", "settings", "record", "window")
        s.debug("ui", "settings", "keys", "{shift+a}")                         # a capital A, for every app
        assert settings_state(s)["recording"] == "window", "Shift alone isn't enough: it keeps listening"
        s.debug("ui", "settings", "keys", "{Escape}")
        st = settings_state(s)
        assert st["recording"] == "" and st["open"], ("Escape cancels recording, the window stays", st["recording"], st["open"])
        assert sh("hyprctl", "submap").stdout.strip() == "default", "and the binds are back"

        s.debug("ui", "settings", "record", "fullscreen")
        s.debug("ui", "settings", "press", QT_META | QT_SHIFT, 12)             # Super+Shift+3: Omarchy's
        p = settings_state(s)["pending"]
        assert p and p["takes"] == "Move window to workspace 3" and p["label"] == "Super+Shift+3", p
        assert binds_for("Snip: Capture fullscreen") == [], "nothing changes before the answer"
        settings_click(s, env, "use-anyway")
        assert wait_for(lambda: binds_for("Snip: Capture fullscreen") == ["SUPER + SHIFT + code:12"], 2)
        assert binds_for("Move window to workspace 3") == [], "taken over"

        settings_click(s, env, "clear:history")
        assert wait_for(lambda: binds_for("Snip: Open capture history") == [], 2), "× clears it"
        assert 'history = ""' in cfg.read_text(), cfg.read_text()

        settings_click(s, env, "restore-defaults")
        assert wait_for(lambda: "shortcuts" not in cfg.read_text(), 2), cfg.read_text()
        assert wait_for(lambda: binds_for("Snip: Capture fullscreen") == [], 2), binds_for("Snip: Capture fullscreen")
        assert binds_for("Snip: Capture area") == ["SUPER + SHIFT + code:13"], "snip.lua's default is back"
    finally:
        s.debug("ui", "settings", "close")
        cfg.write_text(before)
        hypr_eval('pcall(hl.unbind, "SUPER + SHIFT + code:13") pcall(hl.unbind, "SUPER + SHIFT + code:12") '
                  'pcall(hl.unbind, "SUPER + ALT + code:32") snip_bind_defaults = nil')
        time.sleep(0.8)


LINES_PY = """import sys
for i in range(int(sys.argv[1])):
    cells = "".join(f"\\x1b[48;2;{20 * b};{0 if (i >> b) & 1 else 255};{255 if (i >> b) & 1 else 0}m  " for b in range(12))
    sys.stdout.write(cells + f"\\x1b[0m line {i}\\n")
"""


def numbered_lines(png):
    """The numbers of the lines in an image of `LINES_PY`'s output, top to bottom, one per
    line: each line's cells carry its bits as colours, (20b, 0, 255) set and (20b, 255, 0)
    clear, so no font metrics are needed."""
    from PIL import Image
    img = Image.open(png).convert("RGB")
    w, h = img.size
    px = img.load()
    out = []
    for y in range(h):
        cols = {px[x, y] for x in range(min(w, 400))}
        known = [b for b in range(12) if (20 * b, 0, 255) in cols or (20 * b, 255, 0) in cols]
        n = sum(1 << b for b in range(12) if (20 * b, 0, 255) in cols) if len(known) == 12 else None
        if n is not None and (not out or out[-1][1] != y - 1 or out[-1][0] != n):
            out.append((n, y))
        elif n is not None:
            out[-1] = (n, y)
    return [n for n, _ in out]


def scroll_page(s, env, lines):
    """A foot window showing `lines` numbered lines (LINES_PY), scrolled to the first."""
    (s.tmp / "lines.py").write_text(LINES_PY)
    cmd = f"foot --title=scrollpage --app-id=snip-e2e-scroll sh -c 'python3 {s.tmp / 'lines.py'} {lines}; sleep 3600'"
    hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]]))')
    assert wait_for(lambda: [c for c in hypr_json("clients") if c["class"] == "snip-e2e-scroll"], 5), "the page opens"
    time.sleep(0.8)
    c = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-scroll")
    mon = env.monitor()
    s.move(c["at"][0] - mon["x"] + c["size"][0] // 2, c["at"][1] - mon["y"] + c["size"][1] // 2)
    s.debug("scroll", -20000)
    time.sleep(0.5)
    return c


def scroll_hud(s):
    n = len(s.traces("scroll_hud"))
    s.debug("ui", "scroll", "state")
    assert wait_for(lambda: len(s.traces("scroll_hud")) > n, 2), "no scroll_hud"
    return s.traces("scroll_hud")[-1]


@case
def scrolling_capture_auto(s, env):
    """SCR-02: Auto-Scroll turns the wheel itself, pauses while the pointer is out of the
    frame, and ends at the bottom of the page on its own: every line from the frame's first
    down, once. natural_scroll is on: a virtual pointer's wheel bypasses libinput, so it
    must not be flipped."""
    hypr_eval('hl.config({ input = { natural_scroll = true } })')
    c = scroll_page(s, env, 120)
    try:
        mon = env.monitor()
        lx, ly, lw, lh = c["at"][0] - mon["x"], c["at"][1] - mon["y"], c["size"][0], c["size"][1]
        p = s.bg("scroll"); time.sleep(0.5)
        s.drag(lx + 30, ly + 60, lx + lw - 30, ly + lh - 120)
        assert wait_for(lambda: scroll_hud(s)["open"], 3), "the frame and panel show"

        def click(name):
            b = where(s, "scroll", name)
            assert b["x"] >= 0, (name, "isn't shown")
            s.move(b["x"], b["y"]); s.click()

        assert scroll_hud(s)["tips"], "SCR-05: tips the first time"
        click("scroll-tips-ok")
        assert wait_for(lambda: not scroll_hud(s)["tips"], 2), "Got it"
        click("scroll-auto")                                           # Auto-Scroll: Snip moves the pointer in
        assert wait_for(lambda: scroll_hud(s)["auto"], 3), "auto-scrolling"
        s.move(lx + lw + 300, ly + lh - 40)                             # out of the frame: it waits
        assert wait_for(lambda: scroll_hud(s)["paused"], 3), "paused"
        h = scroll_hud(s)["height"]
        time.sleep(0.5)
        assert scroll_hud(s)["height"] == h, "nothing scrolls while paused"
        s.move(lx + lw // 2, ly + lh // 2)                              # back: on it goes, to the end
        r, code = finish(p)
        assert code == 0 and r["result"]["kind"] == "scroll", r
        lines = numbered_lines(r["result"]["source"])
        assert lines == list(range(lines[0], lines[0] + len(lines))), ("every line once, in order", lines)
        assert lines[0] <= 3 and lines[-1] >= 110, (lines[0], lines[-1])
    finally:
        os.kill(c["pid"], 15)
        hypr_eval('hl.config({ input = { natural_scroll = false } })')


PAGE_HTML_ROW = '<div class="r">{cells}<em>row {i}</em></div>'


def browser_page(path, rows):
    """A long page for a real browser: rows numbered as LINES_PY's (cell colours), a sticky
    header (#202833) and a fixed footer (#7a2a3a), as spike S06 had."""
    body = "".join(PAGE_HTML_ROW.format(i=i, cells="".join(
        f'<span style="background:rgb({20 * b},{0 if (i >> b) & 1 else 255},{255 if (i >> b) & 1 else 0})"></span>' for b in range(12)))
        for i in range(rows))
    path.write_text(
        "<!doctype html><meta charset=utf-8><style>body{margin:0;background:#fafafa;font:16px sans-serif}"
        "header{position:sticky;top:0;height:60px;background:#202833}footer{position:fixed;bottom:0;left:0;right:0;height:44px;background:#7a2a3a}"
        ".r{height:34px;display:flex;align-items:center;border-bottom:1px solid #ddd;padding-left:16px}"
        ".r span{display:inline-block;width:18px;height:22px}.r em{margin-left:16px}main{padding-bottom:44px}</style>"
        f"<header></header><main>{body}</main><footer></footer>")


def colour_runs(png, rgb):
    """Runs of rows whose middle pixel is `rgb`: where a header or footer sits."""
    from PIL import Image
    img = Image.open(png).convert("RGB")
    w, h = img.size
    px = img.load()
    runs = []
    for y in range(h):
        if all(abs(px[w // 2, y][k] - rgb[k]) <= 6 for k in range(3)):
            if runs and runs[-1][1] == y - 1:
                runs[-1][1] = y
            else:
                runs.append([y, y])
    return runs


@case
def scrolling_capture_browser(s, env):
    """SCR-03 on a real browser (Chromium's smooth scrolling and its own scale): every row
    once, in order; the sticky header once at the top and the fixed footer once at the
    bottom (a smooth scroll's first small step once left footer rows in the middle)."""
    if not shutil.which("chromium"):
        raise Skip("needs chromium")
    browser_page(s.tmp / "page.html", 300)
    cmd = (f"chromium --user-data-dir={s.tmp / 'chromium'} --ozone-platform=wayland --no-first-run "
           f"--no-default-browser-check --app=file://{s.tmp / 'page.html'}")
    before = {c["address"] for c in hypr_json("clients")}
    hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]]))')
    new = lambda: [c for c in hypr_json("clients") if c["address"] not in before]
    assert wait_for(lambda: new(), 20), "the browser opens"
    time.sleep(2.5)                                                      # the page has rendered
    c = new()[0]
    try:
        mon = env.monitor()
        lx, ly, lw, lh = c["at"][0] - mon["x"], c["at"][1] - mon["y"], c["size"][0], c["size"][1]
        p = s.bg("scroll"); time.sleep(0.5)
        s.drag(lx + 4, ly + 4, lx + lw - 4, ly + lh - 4)
        assert wait_for(lambda: scroll_hud(s)["open"], 3), "the frame and panel show"
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-go"))')
        assert wait_for(lambda: scroll_hud(s)["phase"] == "capturing", 3)
        s.move(lx + lw // 2, ly + lh // 2)
        for _ in range(60):                                            # a notch at a time, as a hand turns the wheel
            s.debug("wheel", 1)
            time.sleep(0.18)
        time.sleep(0.6)
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-go"))')
        r, code = finish(p)
        assert code == 0, r
        src = r["result"]["source"]
        lines = numbered_lines(src)
        assert lines == list(range(lines[0], lines[0] + len(lines))) and lines[0] == 0 and len(lines) > 150, ("every row once, in order", lines)
        head, foot = colour_runs(src, (0x20, 0x28, 0x33)), colour_runs(src, (0x7a, 0x2a, 0x3a))
        h = r["result"]["height"]
        assert len(head) == 1 and head[0][0] == 0, ("the header once, at the top", head)
        assert len(foot) == 1 and foot[0][1] == h - 1, ("the footer once, at the bottom", foot)
    finally:
        os.kill(c["pid"], 15)


@case
def scroll_panel_stays_outside_the_area(s, env):
    """SCR-01: the scrolling capture's panel is never in the area (it would be stitched into
    the capture): beside it, else under or over it."""
    p = s.bg("scroll"); time.sleep(0.5)
    try:
        s.drag(0, 100, 1536, 500)                                 # the full width: no room beside
        hud = lambda: (s.debug("ui", "scroll", "state"), wait_for(lambda: s.traces("scroll_hud"), 2), s.traces("scroll_hud")[-1])[2]
        assert wait_for(lambda: hud()["open"] and hud()["phase"] == "ready", 3), "the frame and panel show"
        c = where(s, "scroll", "scroll-panel")
        assert c["x"] >= 0 and not (100 <= c["y"] <= 500), ("outside the area", c)
    finally:
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-cancel"))')
        finish(p, timeout=6)


@case
def scrolling_capture(s, env):
    """SCR-01/03: choose an area inside a window, Start (Enter), scroll by hand, Done (Enter):
    one tall capture with every line once, in order, from the window's own buffer. Enter
    and Esc are bound only while it runs."""
    (s.tmp / "lines.py").write_text(LINES_PY)
    cmd = f"foot --title=scrollpage --app-id=snip-e2e-scroll sh -c 'python3 {s.tmp / 'lines.py'} 200; sleep 3600'"
    hypr_eval(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]]))')
    assert wait_for(lambda: [c for c in hypr_json("clients") if c["class"] == "snip-e2e-scroll"], 5), "the page opens"
    time.sleep(0.8)
    c = next(c for c in hypr_json("clients") if c["class"] == "snip-e2e-scroll")
    try:
        mon = env.monitor()
        lx, ly, lw, lh = c["at"][0] - mon["x"], c["at"][1] - mon["y"], c["size"][0], c["size"][1]
        s.move(lx + lw // 2, ly + lh // 2)
        s.debug("scroll", -20000)                                      # to the first line
        time.sleep(0.5)

        p = s.bg("scroll"); time.sleep(0.5)
        s.drag(lx + 30, ly + 60, lx + lw - 30, ly + lh - 120)
        hud = lambda: (s.debug("ui", "scroll", "state"), wait_for(lambda: s.traces("scroll_hud"), 2), s.traces("scroll_hud")[-1])[2]
        assert wait_for(lambda: hud()["open"] and hud()["phase"] == "ready", 3), "the frame and panel show"
        assert "Return" in sh("hyprctl", "binds").stdout, "Enter is bound for the session"
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-go"))')    # Enter: Start Capture
        assert wait_for(lambda: hud()["phase"] == "capturing", 3), "capturing"

        s.move(lx + lw // 2, ly + lh // 2)
        for _ in range(24):                                            # ~9 lines a step, as a hand scrolls
            s.debug("scroll", 15)
            time.sleep(0.25)
        time.sleep(0.4)
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-go"))')    # Enter: Done
        r, code = finish(p)
        assert code == 0 and r["result"]["kind"] == "scroll", r
        lines = numbered_lines(r["result"]["source"])
        assert lines == list(range(lines[0], lines[0] + len(lines))), ("every line once, in order", lines)
        assert lines[0] <= 3 and len(lines) > 150, (lines[0], len(lines))
        assert "Return" not in sh("hyprctl", "binds").stdout, "Enter is the page's again"
        assert wait_for(lambda: not hud()["open"], 2), "the frame and panel are gone"

        p = s.bg("scroll"); time.sleep(0.5)                            # Esc cancels
        s.drag(lx + 30, ly + 60, lx + lw - 30, ly + lh - 120)
        assert wait_for(lambda: hud()["open"], 3)
        # a config reload (Omarchy's theme switch does one) starts Hyprland's Lua over:
        # Snip binds the session's keys again
        n = len(s.traces("hypr_reinstalled"))
        sh("hyprctl", "reload")
        env.configure(1.25)                                            # the test output as it was
        assert wait_for(lambda: len(s.traces("hypr_reinstalled")) > n, 3), "snipd saw the reload"
        assert wait_for(lambda: "Return" in sh("hyprctl", "binds").stdout, 3), "Enter is bound again after a reload"
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:scroll-cancel"))')
        r, code = finish(p)
        assert code == 1 and "Return" not in sh("hyprctl", "binds").stdout, (r, code)
    finally:
        os.kill(c["pid"], 15)


def tray_state(s):
    n = len(s.traces("tray_state"))
    s.debug("ui", "tray", "state")
    assert wait_for(lambda: len(s.traces("tray_state")) > n, 2), "no tray_state"
    return s.traces("tray_state")[-1]


@case
def tray_menu(s, env):
    """OMA-08: Snip's menu in the bar, CleanShot's in its order: the captures with their
    keys, Open…, Open from Clipboard, Pin to the Screen…, History, Settings…, About, Quit. It
    opens under the bar where it was clicked; an entry runs as its key does (Settings…
    opens Settings, About opens its tab); Esc or a click elsewhere closes it."""
    hypr_eval('hl.bind("SUPER + SHIFT + code:13", hl.dsp.global("snip-e2e:area"), { description = "Snip: Capture area" })')
    mon = env.monitor()
    try:
        s.move(300, 5)                                                  # on the bar, as the icon's click
        s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2), "the menu opens"
        st = tray_state(s)
        items = dict(st["items"])
        assert list(items) == ["all-in-one", "area", "previous", "fullscreen", "window", "timer-area", "scroll", "text", "record",
                               "open", "edit-clipboard", "pin", "history", "restore", "desktop-toggle", "settings", "about", "quit"], st["items"]
        assert items["area"] == "Super+Shift+4" and items["fullscreen"] == "", items
        x, y, w, h = st["menu"]
        assert abs(x - (300 - 18)) <= 1 and y >= mon["reserved"][1], ("under the click, below the bar", st["menu"])

        s.debug("ui", "tray", "keys", "{Escape}")
        assert wait_for(lambda: not tray_state(s)["open"], 2), "Esc closes it"
        s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2)
        s.move(900, 700); s.click()                                     # elsewhere
        assert wait_for(lambda: not tray_state(s)["open"], 2), "a click elsewhere closes it"

        s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2)
        p = where(s, "tray", "menu:settings")
        s.move(p["x"], p["y"]); s.click()
        assert wait_for(lambda: settings_client(), 5), "Settings… opens Settings"
        assert not tray_state(s)["open"], "and the menu is gone"
        s.debug("ui", "settings", "close")
        assert wait_for(lambda: not settings_client(), 2)
        s.debug("ui", "tray", "open")
        assert wait_for(lambda: tray_state(s)["open"], 2)
        p = where(s, "tray", "menu:about")
        s.move(p["x"], p["y"]); s.click()
        assert wait_for(lambda: settings_client() and settings_state(s)["tab"] == "about", 5), "About Snip opens its tab"
    finally:
        s.debug("ui", "settings", "close")
        s.debug("ui", "tray", "keys", "{Escape}")
        hypr_eval('pcall(hl.unbind, "SUPER + SHIFT + code:13")')


@case
def history_header_fits(s, env):
    """HIS-01: History's header fits its window: the title, filters, search and count on the
    left never run into the keys and Clear History on the right, nor into the question
    before deleting. The search box grew with what was typed, into them."""
    png = s.tmp / "fits.png"
    make_text_png(png, ["header"])
    open_card(s, png); time.sleep(0.3)
    s.json("history")
    try:
        assert wait_for(lambda: history_state(s)["open"], 3), "History opens"
        def clear(st):
            hx, hy, hw, hh = st["header"]
            rx, ry, rw, rh = st["right"]
            return rx >= hx + hw + 8 or ry >= hy + hh, (st["header"], st["right"])
        ok, why = clear(history_state(s))
        assert ok, ("the keys and Clear History", why)
        long = "where did i see that long error message about the build cache and the linker on the arm runner it said something about a missing symbol"
        time.sleep(0.5)                                             # laid out (a frame after a change)
        w0 = history_state(s)["header"][2]
        s.debug("ui", "history", "keys", long)
        assert wait_for(lambda: history_state(s)["query"] == long, 2), "typing searches"
        assert wait_for(lambda: history_state(s)["header"][2] >= w0 + 60, 2), "the search box grows"
        time.sleep(0.4)                                             # the results, and so the count, 120 ms later
        ok, why = clear(history_state(s))
        assert ok, ("a long search grew into them", why)
        s.debug("ui", "history", "keys", "{Escape}")                # clears the search
        assert wait_for(lambda: (lambda st: st["query"] == "" and st["items"])(history_state(s)), 2), "the list is back"
        s.debug("ui", "history", "keys", "{Right}{Delete}")         # the question before deleting one
        assert wait_for(lambda: history_state(s)["asking"] not in ("", "all"), 2), "it asks"
        ok, why = clear(history_state(s))
        assert ok, ("the question", why)
        s.debug("ui", "history", "keys", "{Escape}")                # no
    finally:
        for _ in range(3):
            if not history_state(s)["open"]:
                break
            s.debug("ui", "history", "keys", "{Escape}"); time.sleep(0.3)
        s.cli("close-all")


@case
def history_window(s, env):
    """HIS-01, 05: `snip history` shows past captures newest first; ←/→ move, Enter restores
    one to Quick Access (the band stays for more), typing searches their text, Ctrl+C copies,
    Esc clears the search then closes; Annotate opens the editor."""
    recs = []
    for words in (["alpha history"], ["zebra history"], ["gamma history"]):
        png = s.tmp / f"hist-{words[0].split()[0]}.png"
        make_text_png(png, words)
        recs.append(open_card(s, png))
        time.sleep(0.2)
    assert wait_for(lambda: s.json("search", "zebra")["result"], 10), "the text is indexed"
    s.cli("close-all")
    assert s.json("history")["ok"]
    assert wait_for(lambda: layer_mapped("snip-history"), 3), "the history band shows"
    st = history_state(s)
    assert st["open"] and st["items"][:3] == [r["id"] for r in reversed(recs)], (st["items"][:3], [r["id"] for r in recs])
    s.debug("ui", "history", "keys", "{Right}{Right}{Return}")  # nothing chosen at first; the second newest: zebra
    assert wait_for(lambda: recs[1]["id"] in s.json("status")["result"]["cards"], 3), "Enter restores it to Quick Access"
    st = history_state(s)
    assert st["open"] and st["restored"] == [recs[1]["id"]], st
    s.debug("ui", "history", "keys", "zebra")
    assert wait_for(lambda: history_state(s)["items"] == [recs[1]["id"]], 3), history_state(s)
    s.debug("ui", "history", "keys", "{ctrl+c}")
    assert wait_for(lambda: "image/png" in sh("wl-paste", "-l", check=False).stdout.split(), 3), "Ctrl+C copies the capture"
    s.debug("ui", "history", "keys", "{Escape}")                # clears the search (the list follows 120 ms later)
    assert wait_for(lambda: len(history_state(s)["items"]) >= 3, 2), history_state(s)
    assert history_state(s)["open"] and history_state(s)["query"] == "", "Esc cleared the search, the band stays"
    s.debug("ui", "history", "keys", "{Escape}")                # closes
    assert wait_for(lambda: not layer_mapped("snip-history"), 3), "Esc closes it"
    s.json("history")
    assert wait_for(lambda: history_state(s)["open"], 3)
    s.debug("ui", "history", "keys", "{Right}")                 # choose the newest
    s.debug("ui", "history", "annotate")
    assert wait_for(lambda: editor_window(), 5), "Annotate opens the editor"
    assert not history_state(s)["open"], "and the band goes"
    close_editor(s)


def history_click(s, name):
    """Click a control of the history band with the pointer."""
    p = where(s, "history", name)
    assert p["x"] >= 0, (name, "isn't shown")
    s.move(p["x"], p["y"]); s.click()


@case
def history_click_selects(s, env):
    """A click on a capture in history selects it and shows its Restore button; a second
    click deselects it. Only Restore brings its card back (the product owner: "only on
    restore button click it need to show floating window")."""
    recs = []
    for k in range(2):
        png = s.tmp / f"sel-{k}.png"
        noise_png(png, 160, 100, seed=40 + k)
        recs.append(open_card(s, png))
        time.sleep(0.2)
    s.cli("close-all")
    s.json("history")
    assert wait_for(lambda: history_state(s)["open"], 3), "the history opens"
    assert history_state(s)["current"] == -1, "nothing chosen at first"
    history_click(s, "thumb:1")
    assert wait_for(lambda: history_state(s)["current"] == 1, 2), "a click selects"
    time.sleep(0.4)
    assert s.json("status")["result"]["cards"] == [] and history_state(s)["restored"] == [], "and restores nothing"
    history_click(s, "thumb:1")
    assert wait_for(lambda: history_state(s)["current"] == -1, 2), "a second click deselects"
    time.sleep(0.6)                                                 # past the double-click time (that annotates)
    history_click(s, "thumb:1")
    assert wait_for(lambda: history_state(s)["current"] == 1, 2)
    history_click(s, "restore:1")
    assert wait_for(lambda: recs[0]["id"] in s.json("status")["result"]["cards"], 3), "Restore brings its card back"
    assert history_state(s)["open"], "the band stays for more"
    s.debug("ui", "history", "keys", "{Escape}")


@case
def history_delete(s, env):
    """HIS-03: Backspace deletes the chosen capture from history after asking (Esc keeps
    it); its card goes too; the menu pins; Clear History empties it, asked first."""
    recs = []
    for k in range(3):
        png = s.tmp / f"del-{k}.png"
        noise_png(png, 120, 80, seed=20 + k)
        recs.append(open_card(s, png))
        time.sleep(0.2)
    s.json("history")
    assert wait_for(lambda: history_state(s)["open"], 3), "the history opens"
    newest = recs[-1]["id"]
    s.debug("ui", "history", "keys", "{Right}{Backspace}")      # choose the newest, then delete
    assert history_state(s)["asking"] == newest, "it asks first"
    s.debug("ui", "history", "keys", "{Escape}")
    st = history_state(s)
    assert st["asking"] == "" and newest in st["items"] and st["open"], "Esc keeps it"
    s.debug("ui", "history", "keys", "{Backspace}{Return}")
    assert wait_for(lambda: newest not in history_state(s)["items"], 3), "deleted"
    assert newest not in [r["id"] for r in s.json("list", "-n", "50")["result"]], "gone from history"
    assert newest not in s.json("status")["result"]["cards"], "its card went too"
    s.debug("ui", "history", "act", "pin")                     # the menu's Pin, on the new first one
    assert wait_for(lambda: pin_windows(), 3), "Pin to Screen from history"
    s.json("pins", "close")
    s.debug("ui", "history", "clear")
    s.debug("ui", "history", "keys", "{Return}")
    assert wait_for(lambda: history_state(s)["items"] == [], 3), history_state(s)["items"]
    assert s.json("list", "-n", "50")["result"] == [], "Clear History empties it"
    s.debug("ui", "history", "keys", "{Escape}")


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
    """Drawing an arrow, moving it and bending a curved one with the pointer; Copy writes back,
    copies and closes."""
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
    s.debug("ui", "editor", "arrow-style", "curved")   # only Curved arrows bend (CleanShot)
    b = editor_state(s)["items"][0]
    mx, my = (b["x1"] + 2 * b["cx"] + b["x2"]) / 4, (b["y1"] + 2 * b["cy"] + b["y2"]) / 4
    s.drag(*at(mx, my), *at(mx, my + 50), steps=4)     # its middle is the bend handle
    time.sleep(0.2)                                     # the release reaches the editor after the drag call returns
    c = editor_state(s)["items"][0]
    assert abs(c["y1"] - b["y1"]) < 1 and c["cy"] - b["cy"] > 80, (b, c)
    n = len(s.traces("copied"))
    s.debug("ui", "editor", "flush", "copy")
    assert wait_for(lambda: len(s.traces("copied")) > n and editor_window() is None, 5), "Copy did not copy and close"
    px = Image.open(rec["source"]).convert("RGB").getpixel((round(c["x2"]), round(c["y2"])))  # the arrow's tip
    assert px[0] > 200 and px[1] < 90, px


@case
def card_after_done(s, env):
    """After Done, the edited capture is in Quick Access even when its card had gone
    (closed, or timed out while annotating): there to drag, save or copy. One card only."""
    from PIL import Image
    png = s.tmp / "after-done.png"
    Image.new("RGB", (300, 200), "white").save(png)
    rec = open_card(s, png)
    s.debug("annotate", rec["id"])
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.cli("close-all")                                          # the card goes while the editor is open
    assert wait_for(lambda: rec["id"] not in s.json("status")["result"]["cards"], 3), "the card closed"
    s.debug("ui", "editor", "frect", 20, 20, 60, 40)
    close_editor(s)                                             # Done
    assert wait_for(lambda: rec["id"] in s.json("status")["result"]["cards"], 3), "the card is back"
    s.debug("annotate", rec["id"])                              # with its card still there: no second one
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens again"
    close_editor(s)
    time.sleep(0.3)
    assert s.json("status")["result"]["cards"].count(rec["id"]) == 1, s.json("status")["result"]["cards"]


@case
def editor_done_button(s, env):
    """CleanShot's top-right Done: the edits go back to the capture and the editor closes."""
    from PIL import Image
    png = s.tmp / "done.png"
    Image.new("RGB", (400, 240), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    for tool in ("text", "arrow", "redact", "counter"):     # the widest option sets
        s.debug("ui", "editor", "tool", tool)
        st = editor_state(s)
        assert st["toolbarNeeds"] <= st["winW"], f"{tool}: the toolbar needs {st['toolbarNeeds']} of {st['winW']}"
    s.debug("ui", "editor", "rect", 50, 50, 120, 80)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    n = len(s.traces("edited"))
    s.move(w["at"][0] - mon["x"] + st["winW"] - 12 - 30, w["at"][1] - mon["y"] + 26)   # Done, the last button on the bar
    time.sleep(0.2); s.click()
    assert wait_for(lambda: editor_window() is None, 5), "Done did not close the editor"
    assert len(s.traces("edited")) > n, "Done must write the edits back"
    assert Image.open(rec["source"]).convert("RGB").getpixel((50, 90))[1] < 120


@case
def editor_shapes(s, env):
    """ANN-03: Filled rectangle (F), Ellipse (E) and Line (L), drawn with the pointer.
    (Shift for squares and 45° lines needs a held key, which the virtual pointer can't send.)"""
    from PIL import Image
    png = s.tmp / "shapes.png"
    Image.new("RGB", (600, 360), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.move(*at(300, 340)); s.click(); time.sleep(0.2)           # focus
    for key, a, b in (("f", (30, 30), (150, 120)), ("e", (200, 30), (380, 150)), ("l", (420, 40), (560, 160))):
        s.debug("ui", "editor", "keys", key)
        s.drag(*at(*a), *at(*b), steps=5); time.sleep(0.15)
    items = editor_state(s)["items"]
    assert [(i["kind"], i.get("filled", False)) for i in items] == [("rect", True), ("ellipse", False), ("line", False)], items
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    red = lambda p: p[0] > 200 and p[1] < 90
    assert red(out.getpixel((90, 75))), "the filled rectangle is filled"
    e = items[1]
    assert red(out.getpixel((round(e["x"] + e["w"] / 2), round(e["y"] + e["size"] / 2)))), "the ellipse's top edge"
    assert not red(out.getpixel((round(e["x"] + e["w"] / 2), round(e["y"] + e["h"] / 2)))), "an ellipse is not filled"
    assert red(out.getpixel((490, 100))), "the line's middle"
    close_editor(s)


@case
def editor_arrow_styles(s, env):
    """ANN-04: Standard, Fancy, Curved and Double arrows, and changing a selected arrow's style."""
    from PIL import Image
    png = s.tmp / "arrows.png"
    Image.new("RGB", (560, 340), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    for args in ((40, 50, 260, 50, "standard"), (40, 150, 260, 150, "double"), (40, 250, 260, 250, "fancy"), (300, 250, 520, 250, "curved")):
        s.debug("ui", "editor", "arrow", *args)
    s.debug("ui", "editor", "arrow", 300, 60, 520, 60, "standard")
    s.debug("ui", "editor", "arrow-style", "curved")             # applies to the selected (last) arrow
    items = editor_state(s)["items"]
    assert [i["style"] for i in items] == ["standard", "double", "fancy", "curved", "curved"], items
    assert abs(items[4]["cy"] - 60) > 20, "a curved arrow starts bent"
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    red = lambda x, y: (lambda p: p[0] > 200 and p[1] < 90)(out.getpixel((round(x), round(y))))
    size = items[0]["size"]; head = size * 3.2 + 4
    assert red(40 + head * 0.6, 150 - head * 0.25) and not red(40 + head * 0.6, 50 - head * 0.25), "Double has a head at its start"
    assert red(200, 250 - size * 0.75) and not red(200, 50 - size * 0.75), "Fancy is wider than Standard near the head"
    assert not red(410, 250) and not red(410, 60), "Curved arrows bend away from their chord"
    close_editor(s)


@case
def editor_text_styles(s, env):
    """ANN-05: the box styles draw a tint of the text colour behind it; the right handle sets
    a wrap width and the corner square scales the text (pointer)."""
    from PIL import Image
    png = s.tmp / "texts.png"
    Image.new("RGB", (640, 360), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    for st, y in (("standard", 40), ("box", 120)):
        s.debug("ui", "editor", "tool", "text"); s.debug("ui", "editor", "text-style", st)
        s.debug("ui", "editor", "text", 60, y, "Hello styles")
    st = editor_state(s)
    assert [i["style"] for i in st["items"]] == ["standard", "box"], st["items"]
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    size = st["items"][0]["size"]
    tint = lambda p: p != (255, 255, 255) and p[0] > 200       # light pink, not white
    assert out.getpixel((60 - 3, 40 + size // 2)) == (255, 255, 255), "Standard has no box"
    assert tint(out.getpixel((60 - 3, 120 + size // 2))), "Box draws the tint around the text"

    w, mon = editor_window(), env.monitor()
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    b = st["selectionBox"]                                      # the box text, still selected
    s.drag(*at(b["x"] + b["w"], b["y"] + b["h"] / 2), *at(b["x"] + b["w"] * 0.55, b["y"] + b["h"] / 2), steps=5)
    time.sleep(0.2)                                             # the release reaches the editor after the drag call returns
    t = editor_state(s)["items"][1]
    assert 0 < t["w"] < b["w"] * 0.6, ("the right handle sets a wrap width", t)
    b = editor_state(s)["selectionBox"]
    assert b["h"] > st["selectionBox"]["h"] * 1.5, "wrapped onto two lines"
    s.drag(*at(b["x"] + b["w"], b["y"] + b["h"]), *at(b["x"] + b["w"], b["y"] + b["h"] * 1.5), steps=5)
    time.sleep(0.2)
    assert editor_state(s)["items"][1]["size"] >= size * 1.3, "the corner square scales the text"
    close_editor(s)


@case
def editor_spotlight(s, env):
    """ANN-07: Spotlight dims everything outside its shapes; overlapping spotlights merge;
    an ellipse dims its corners; arrows drawn after it stay bright."""
    from PIL import Image
    png = s.tmp / "spot.png"
    Image.new("RGB", (600, 360), (240, 240, 240)).save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "spotlight", 40, 40, 200, 140, "rect", 0.5)
    s.debug("ui", "editor", "spotlight", 180, 100, 160, 120, "rect", 0.5)       # overlaps the first
    s.debug("ui", "editor", "spotlight", 380, 60, 180, 120, "ellipse", 0.5)
    s.debug("ui", "editor", "arrow", 100, 300, 300, 300)
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    lit = lambda x, y: out.getpixel((x, y))[0] > 225
    assert lit(100, 100) and lit(210, 150) and lit(300, 200), "inside the spotlights (and their overlap) is not dimmed"
    assert not lit(20, 20) and not lit(500, 300), "outside is dimmed"
    assert lit(470, 120) and not lit(384, 64), "an ellipse: its middle is lit, its corner is not"
    r, g, b = out.getpixel((200, 300))
    assert r > 200 and g < 90, ("an arrow drawn on top stays bright", (r, g, b))
    close_editor(s)


@case
def editor_counters(s, env):
    """ANN-08: counter styles and start number, renumbering on delete, counters on top."""
    from PIL import Image
    png = s.tmp / "counters.png"
    Image.new("RGB", (500, 300), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    for x in (60, 160, 260):
        s.debug("ui", "editor", "counter", x, 80, "roman")
    s.debug("ui", "editor", "counter-start", 3)
    assert editor_state(s)["counterLabels"] == ["III", "IV", "V"]
    s.debug("ui", "editor", "delete", 1)
    assert editor_state(s)["counterLabels"] == ["III", "IV"], "renumbered after a delete"
    s.debug("ui", "editor", "counter", 360, 80, "alpha")
    s.debug("ui", "editor", "counter-start", 1)
    assert editor_state(s)["counterLabels"] == ["I", "II", "A"], "each style counts on its own"
    s.debug("ui", "editor", "color", "#0a7cff")                 # recolours the selected (last) counter; then a blue box over the first
    s.debug("ui", "editor", "frect", 30, 50, 80, 80)
    editor_flush(s)
    r, g, b = Image.open(rec["source"]).convert("RGB").getpixel((60, 80 - 13))
    assert r > 200 and b < 150, ("the first counter (red) stays on top of the blue box drawn later", (r, g, b))
    close_editor(s)


@case
def editor_pencil(s, env):
    """ANN-09: the Pencil (D) draws a smoothed free-hand line with the pointer."""
    import math
    from PIL import Image
    png = s.tmp / "pencil.png"
    Image.new("RGB", (600, 300), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.move(*at(300, 280)); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "d")
    wave = [(50 + i * 10, 150 + 60 * math.sin(i / 6)) for i in range(51)]
    s.move(*at(*wave[0])); s.debug("down")
    for p in wave[1:]:
        s.move(*at(*p))
    s.debug("up"); time.sleep(0.2)
    items = editor_state(s)["items"]
    assert len(items) == 1 and items[0]["kind"] == "pencil", items
    assert 4 <= len(items[0]["pts"]) < 50, ("simplified, still a curve", len(items[0]["pts"]))
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    red = lambda p: p[0] > 200 and p[1] < 100
    hits = sum(red(out.getpixel((round(x), round(y)))) for x, y in wave[2:-2])
    assert hits >= len(wave) * 0.8, ("the line follows the hand", hits)
    close_editor(s)


@case
def editor_highlighter(s, env):
    """ANN-10: the Highlighter (M) snaps to text: pressed on a line, the band takes the
    line's height and runs from word edge to word edge; elsewhere it is a free band."""
    from PIL import Image, ImageDraw, ImageFont
    try:
        font = ImageFont.truetype("/usr/share/fonts/noto/NotoSans-Regular.ttf", 28)
    except OSError:
        font = ImageFont.load_default(28)
    im = Image.new("RGB", (640, 260), "white")
    d = ImageDraw.Draw(im)
    d.text((40, 40), "alpha bravo charlie", fill="black", font=font)
    png = s.tmp / "hl.png"
    im.save(png)
    bravo = d.textbbox((40, 40), "alpha bravo charlie", font=font)
    x_b0 = 40 + d.textlength("alpha ", font=font); x_b1 = 40 + d.textlength("alpha bravo", font=font)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.move(*at(320, 230)); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "m")
    assert wait_for(lambda: editor_state(s)["words"] >= 3, 8), "no OCR words for the highlighter"
    mid = (bravo[1] + bravo[3]) / 2
    s.drag(*at(x_b0 + 8, mid), *at(x_b1 - 12, mid), steps=5); time.sleep(0.2)   # partly across "bravo"
    h = editor_state(s)["items"][-1]
    assert h["kind"] == "highlight", h
    assert abs(h["x"] - x_b0) < 8 and abs(h["x"] + h["w"] - x_b1) < 8, ("snapped to the word's edges", h, x_b0, x_b1)
    assert h["y"] <= bravo[1] and h["y"] + h["h"] >= bravo[3] - 2, ("the line's height", h, bravo)
    s.drag(*at(100, 180), *at(300, 185), steps=4); time.sleep(0.2)            # empty space: a free band
    f = editor_state(s)["items"][-1]
    assert f["kind"] == "highlight" and abs(f["w"] - 200) < 4, f
    editor_flush(s)
    px = Image.open(rec["source"]).convert("RGB").getpixel((200, 180))
    assert px[2] < 220 and px[0] > 230, ("translucent yellow", px)
    close_editor(s)


@case
def editor_select_commands(s, env):
    """ANN-02: Ctrl+D duplicates; Ctrl+C copies the selected object (not the image) and
    Ctrl+V pastes it, also into another image."""
    from PIL import Image
    for name in ("one.png", "two.png"):
        Image.new("RGB", (400, 260), "white").save(s.tmp / name)
    s.json("edit", str(s.tmp / "one.png"))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.move(*at(390, 250)); s.click(); time.sleep(0.2)            # focus; nothing under the pointer
    s.debug("ui", "editor", "rect", 40, 40, 100, 60)
    n = len(s.traces("copied"))
    s.debug("ui", "editor", "keys", "{ctrl+d}")
    items = editor_state(s)["items"]
    assert len(items) == 2 and items[1]["x"] > items[0]["x"], items
    s.debug("ui", "editor", "keys", "{ctrl+c}")
    time.sleep(0.3)
    assert len(s.traces("copied")) == n and editor_window() is not None, "Ctrl+C with an object selected copies the object, not the image"
    close_editor(s)
    s.json("edit", str(s.tmp / "two.png"))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    w, st = editor_window(), editor_state(s)
    s.move(*at(390, 250)); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "{ctrl+v}")
    items = editor_state(s)["items"]
    assert len(items) == 1 and items[0]["kind"] == "rect" and items[0]["w"] == 100, items
    close_editor(s)


@case
def editor_colors(s, env):
    """ANN-13: the eyedropper picks a colour anywhere on screen (the frozen overlay with
    the magnifier), this canvas included; My Colors are kept across editors."""
    from PIL import Image
    im = Image.new("RGB", (400, 260), "white")
    im.paste((18, 160, 90), (100, 80, 220, 180))
    png = s.tmp / "colors.png"
    im.save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    sessions = len(s.traces("session"))
    s.debug("ui", "editor", "eyedropper")
    assert wait_for(lambda: len(s.traces("session")) > sessions, 3), "the eyedropper's overlay opens"
    assert s.traces("session")[-1]["json"]["magnifier"], "with the magnifier"
    time.sleep(0.3)
    s.move(*at(160, 130)); time.sleep(0.2); s.click()
    assert wait_for(lambda: not editor_state(s)["picking"], 3)
    st = editor_state(s)
    assert st["ink"].lower() == "#12a05a", st["ink"]
    assert st["items"] == [], "picking a colour draws nothing"
    s.debug("ui", "editor", "save-color", "#12a05a")
    close_editor(s)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5)
    assert "#12a05a" in editor_state(s)["savedColors"], "My Colors survive into the next editor"
    s.debug("ui", "editor", "save-color", "#112233")
    s.debug("ui", "editor", "save-color", "#445566")             # newest first: 445566, 112233, 12a05a
    s.debug("ui", "editor", "move-color", 0, 2)
    assert editor_state(s)["savedColors"][:3] == ["#112233", "#12a05a", "#445566"], editor_state(s)["savedColors"]
    s.debug("ui", "editor", "remove-color", 1)
    assert "#12a05a" not in editor_state(s)["savedColors"], "removed"
    close_editor(s)


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
    s.debug("ui", "editor", "keys", "Hello", "snip")
    s.move(*at(61, 100)); s.click()                              # a click in the text moves the caret there
    assert wait_for(lambda: editor_state(s)["caret"] == 0, 2), editor_state(s)["caret"]
    assert editor_state(s)["editingText"], "clicking inside the text keeps typing"
    s.debug("ui", "editor", "keys", "Oh, {Return}")
    st = editor_state(s)
    assert [i.get("text") for i in st["items"]] == ["Oh, Hello snip"] and not st["editingText"], st
    s.debug("ui", "editor", "keys", "{ctrl+z}")
    assert editor_state(s)["items"] == []
    s.debug("ui", "editor", "keys", "{ctrl+shift+z}")
    assert len(editor_state(s)["items"]) == 1
    s.debug("ui", "editor", "keys", "r")
    assert editor_state(s)["tool"] == "rect"
    editor_flush(s)
    px = Image.open(rec["source"]).convert("RGB")
    assert any(px.getpixel((x, y))[1] < 100 for x in range(60, 200) for y in range(85, 115)), "the text should be drawn"
    s.move(w["at"][0] - mon["x"] + 42, w["at"][1] - mon["y"] + st["winH"] - 25)   # the zoom capsule, bottom left
    s.click(); time.sleep(0.2)
    assert editor_state(s)["menuOpen"], "the zoom menu should open"
    s.debug("ui", "editor", "keys", "{Escape}")     # close the menu
    assert not editor_state(s)["menuOpen"] and editor_window(), "Escape closes the menu first, not the editor"
    s.debug("ui", "editor", "keys", "{Escape}")     # deselect
    s.debug("ui", "editor", "keys", "{Escape}")     # close
    assert wait_for(lambda: editor_window() is None, 5), "Escape with nothing selected closes the editor"


@case
def editor_double_click_changes_a_text(s, env):
    """ANN-05: double-clicking a text changes it. With any tool but Text, Enter left the text
    as it was and added the changed one over it: two texts."""
    from PIL import Image
    png = s.tmp / "retext.png"
    Image.new("RGB", (500, 260), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.debug("ui", "editor", "text", 60, 120, "Hello")
    s.debug("ui", "editor", "tool", "select")
    s.move(*at(75, 132)); s.click(); time.sleep(0.05); s.click()
    assert wait_for(lambda: editor_state(s)["editingText"], 2), "a double-click changes the text"
    s.debug("ui", "editor", "keys", " there{Return}")
    items = editor_state(s)["items"]
    assert [i["text"] for i in items] == ["Hello there"], items
    close_editor(s)


@case
def editor_text_takes_colour_and_size_while_typing(s, env):
    """ANN-05: a colour or size chosen while typing goes to that text, a new one or one being
    changed. A new text kept the colour it started with, and a changed one got its old
    colour back once more was typed."""
    from PIL import Image
    png = s.tmp / "inktext.png"
    Image.new("RGB", (500, 260), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.debug("ui", "editor", "tool", "text")
    s.move(*at(60, 120)); s.click(); time.sleep(0.2)
    assert editor_state(s)["editingText"], "typing"
    s.debug("ui", "editor", "keys", "Hello")
    s.debug("ui", "editor", "color", "#e5484d")
    s.debug("ui", "editor", "size", 3)
    s.debug("ui", "editor", "keys", "{Return}")
    t = editor_state(s)["items"][0]
    assert (t["text"], t["color"], t["size"]) == ("Hello", "#e5484d", round(40 * st["dpr"])), t
    s.debug("ui", "editor", "tool", "select")
    s.move(*at(75, 125)); s.click(); time.sleep(0.05); s.click()      # a double-click changes it
    assert wait_for(lambda: editor_state(s)["editingText"], 2), "changing the text"
    s.debug("ui", "editor", "color", "#30a46c")
    s.debug("ui", "editor", "keys", " there{Return}")
    items = editor_state(s)["items"]
    assert [(i["text"], i["color"]) for i in items] == [("Hello there", "#30a46c")], items
    close_editor(s)


@case
def editor_text_emoji(s, env):
    """ANN-05: the blue smiley above the text being typed opens an emoji picker (Omarchy's
    list); searching and Return insert the emoji at the caret, and typing goes on."""
    from PIL import Image
    if not Path(os.environ.get("OMARCHY_PATH", "/usr/share/omarchy"), "shell/plugins/emojis/emojis.json").exists():
        raise Skip("no Omarchy emoji list here")
    png = s.tmp / "emoji.png"
    Image.new("RGB", (500, 260), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    win = lambda x, y: (w["at"][0] - mon["x"] + x, w["at"][1] - mon["y"] + y)
    at = lambda ix, iy: win(st["originX"] + ix * st["k"], st["originY"] + iy * st["k"])
    s.debug("ui", "editor", "tool", "text")
    s.move(*at(60, 120)); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "Launch ")
    b = editor_state(s)["emojiButton"]
    assert b, "the smiley shows while typing"
    s.move(*win(b["x"], b["y"])); s.click(); time.sleep(0.3)
    assert editor_state(s)["menuOpen"], "the smiley opens the picker"
    s.debug("ui", "editor", "keys", "rocket{Return}")
    st2 = editor_state(s)
    assert not st2["menuOpen"] and st2["editingText"] and st2["typed"] == "Launch \U0001F680", st2["typed"]
    s.debug("ui", "editor", "keys", " day{Return}")
    assert [i.get("text") for i in editor_state(s)["items"]] == ["Launch \U0001F680 day"]
    editor_flush(s)
    px = Image.open(rec["source"]).convert("RGB")
    colourful = [p for p in px.crop((60, 100, 360, 150)).get_flattened_data() if max(p) - min(p) > 80]
    assert len(colourful) > 30, "the emoji is drawn in colour"
    close_editor(s)


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
def editing_reads_the_text_again_once(s, env):
    """HIS-05: each write-back of an edit drops the capture's text from the search, and the
    indexer read it again after every one (a full OCR each time); it reads it once the
    editing pauses, and the capture is found again."""
    from PIL import Image, ImageDraw, ImageFont
    try:
        font = ImageFont.truetype("/usr/share/fonts/noto/NotoSans-Regular.ttf", 32)
    except OSError:
        font = ImageFont.load_default(32)
    im = Image.new("RGB", (520, 200), "white")
    ImageDraw.Draw(im).text((20, 70), "quokka lantern", fill="black", font=font)
    png = s.tmp / "reread.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    reads = lambda: [e for e in s.traces("indexed") if e["id"] == rec["id"]]
    # searchable, and the indexer's round over (one going on reads what an edit drops)
    assert wait_for(lambda: reads() and s.json("search", "quokka")["result"] and not s.status()["index_pending"], 10), "it's searchable first"
    n = len(reads())
    for i in range(4):
        s.debug("ui", "editor", "rect", 400 + 25 * i, 20, 15, 15)
        editor_flush(s)
        time.sleep(0.8)
    assert wait_for(lambda: s.json("search", "quokka")["result"], 10), "found again once the editing pauses"
    time.sleep(0.5)
    assert len(reads()) - n == 1, ("read again once", reads()[n:])
    close_editor(s)


@case
def editor_redact_intensity(s, env):
    """ANN-06: intensity sets the Pixelate blocks' size; Natural blur (not secure) really
    blurs what is under it."""
    from PIL import Image
    im = Image.new("RGB", (400, 240), (200, 210, 220))
    for x in range(40, 360):        # fine black and white stripes under the blur
        for y in range(140, 200):
            im.putpixel((x, y), (255, 255, 255) if (x // 2) % 2 else (0, 0, 0))
    for x in range(0, 400):         # a colourful band above and below the pixelated rows
        for y in list(range(0, 30)) + list(range(110, 130)):
            im.putpixel((x, y), ((x * 3) % 256, 90, 255 - (x * 3) % 256))
    png = s.tmp / "intensity.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "redact", 20, 40, 360, 30, "pixelate", 7, 0.6)
    s.debug("ui", "editor", "redact", 20, 75, 360, 30, "pixelate", 7, 2.4)
    s.debug("ui", "editor", "redact", 40, 140, 320, 60, "natural", 1, 1)
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")

    def runs(y):
        row = [out.getpixel((x, y)) for x in range(22, 378)]
        return 1 + sum(1 for a, b in zip(row, row[1:]) if a != b)
    low, high = runs(55), runs(90)
    assert low > high * 2, f"Maximum gives far bigger blocks: {low} vs {high} colour runs"
    lum = [sum(out.getpixel((x, 170))) / 3 for x in range(60, 340)]
    assert max(lum) - min(lum) < 30 and abs(sum(lum) / len(lum) - 128) < 20, "the stripes blur into grey"
    close_editor(s)


@case
def editor_text_shape(s, env):
    """D30: Text shape shows word-shaped ink blocks on the paper colour, and the hidden
    words can't be read (they leave the history search)."""
    from PIL import Image, ImageDraw, ImageFont
    try:
        font = ImageFont.truetype("/usr/share/fonts/noto/NotoSans-Regular.ttf", 26)
    except OSError:
        font = ImageFont.load_default(26)
    im = Image.new("RGB", (520, 160), (250, 250, 252))
    ImageDraw.Draw(im).text((30, 50), "token hunter2 swordfish", fill=(20, 20, 30), font=font)
    png = s.tmp / "shape.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    assert wait_for(lambda: s.json("search", "swordfish")["result"], 8), "the words should be searchable first"
    s.debug("ui", "editor", "redact", 20, 40, 400, 50, "text", 5)
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB").crop((20, 40, 420, 90))
    lum = [sum(p) / 3 for p in out.get_flattened_data()]
    assert min(lum) < 120 and max(lum) > 220, "ink blocks and paper both show"
    assert out.tobytes() != im.crop((20, 40, 420, 90)).tobytes()
    assert wait_for(lambda: not s.json("search", "swordfish")["result"], 8), "hidden words are still searchable"
    close_editor(s)


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
    # cropping shows room around the image: the view starts at (shownX, shownY)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + (ix - st["shownX"]) * st["k"], w["at"][1] - mon["y"] + st["originY"] + (iy - st["shownY"]) * st["k"])
    s.drag(*at(301, 203), *at(201, 153), steps=5)
    time.sleep(0.2)
    c = editor_state(s)["crop"]
    assert c and c["x"] == 0 and c["y"] == 0 and abs(c["w"] - 201) <= 2 and abs(c["h"] - 153) <= 2, c
    close_editor(s)


@case
def editor_crop_outward_and_ratios(s, env):
    """ANN-11: cropping outward grows the canvas in the image's edge colour; a ratio
    squares the crop; a handle dragged near the image's edge snaps onto it."""
    from PIL import Image
    im = Image.new("RGB", (300, 200), (40, 120, 200))
    im.paste((250, 250, 250), (100, 60, 200, 140))
    png = s.tmp / "outward.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "crop", -40, -30, 380, 260)
    assert editor_state(s)["fill"].lower() == "#2878c8", editor_state(s)["fill"]
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (380, 260), out.size
    assert out.getpixel((5, 5)) == (40, 120, 200) and out.getpixel((40 + 150, 30 + 100)) == (250, 250, 250), "grown canvas, image in place"
    assert out.crop((40, 30, 340, 230)).tobytes() == im.tobytes(), "the image is exact inside the grown canvas"

    s.debug("ui", "editor", "crop", "reset")
    s.debug("ui", "editor", "crop-ratio", 1)
    c = editor_state(s)["crop"]
    assert c and c["w"] == c["h"] == 200, c                     # the largest square in 300 x 200

    s.debug("ui", "editor", "crop-ratio", 0)
    s.debug("ui", "editor", "crop", 20, 20, 200, 120)
    s.debug("ui", "editor", "tool", "crop"); time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + (ix - st["shownX"]) * st["k"], w["at"][1] - mon["y"] + st["originY"] + (iy - st["shownY"]) * st["k"])
    s.drag(*at(220, 140), *at(296, 197), steps=5); time.sleep(0.2)   # near the corner (300, 200)
    c = editor_state(s)["crop"]
    assert (c["x"] + c["w"], c["y"] + c["h"]) == (300, 200), ("snapped to the image's corner", c)
    close_editor(s)


@case
def editor_menu_gives_the_keys_back(s, env):
    """A field in a menu (Resize's width) kept the keys once its menu had closed: Ctrl+Z and
    the tool keys went to the hidden field. The canvas gets them back."""
    from PIL import Image
    png = s.tmp / "menukeys.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.debug("ui", "editor", "rect", 50, 50, 100, 80)
    s.debug("ui", "editor", "tool", "crop")
    s.move(*win(where(s, "editor", "resize-button"))); s.click()
    assert wait_for(lambda: editor_state(s)["menuOpen"], 2), "the Resize menu opens"
    s.move(*win(where(s, "editor", "resize-w"))); s.click(); time.sleep(0.2)       # the width field
    s.move(*win({"x": st["originX"] + 15 * st["k"], "y": st["originY"] + 285 * st["k"]})); s.click()
    assert wait_for(lambda: not editor_state(s)["menuOpen"], 2), "a click beside it closes the menu"
    s.debug("ui", "editor", "keys", "{ctrl+z}")
    assert wait_for(lambda: editor_state(s)["items"] == [], 2), "Ctrl+Z undoes the rect"
    close_editor(s)


@case
def editor_colour_menu_fits_the_window(s, env):
    """ANN-13: the colour menu stays inside the editor's window. It was one column, ~34 px a
    colour, so in a small capture's window the rainbow and the saved colours fell off the
    bottom, out of reach (found when an earlier case had saved colours)."""
    from PIL import Image
    png = s.tmp / "short.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    added = [c for c in ("#123456", "#654321", "#abcdef", "#fedcba") if c not in editor_state(s)["savedColors"]]
    try:
        for c in added:
            s.debug("ui", "editor", "save-color", c)
        w, mon, st = editor_window(), env.monitor(), editor_state(s)
        win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
        s.move(*win(where(s, "editor", "color-swatch"))); s.click()
        assert wait_for(lambda: editor_state(s)["menuOpen"], 2), "the colour menu opens"
        time.sleep(0.3)                                             # laid out
        rb = where(s, "editor", "color-custom")
        assert 0 <= rb["y"] <= st["winH"] - 15, ("the rainbow is in the window", rb["y"], st["winH"])
        s.move(*win(rb)); s.click()
        assert wait_for(lambda: where(s, "editor", "color-square")["x"] >= 0, 2), "and it opens the custom panel"
    finally:
        s.debug("ui", "editor", "keys", "{Escape}")
        for c in added:                                             # later cases expect them as they were
            if c in editor_state(s)["savedColors"]:
                s.debug("ui", "editor", "remove-color", editor_state(s)["savedColors"].index(c))
        close_editor(s)


@case
def editor_colour_drag_is_one_undo_step(s, env):
    """ANN-13: a drag in the custom colour panel is one undo step. Every shade it passed
    through was one, so Ctrl+Z stepped back through dozens of them."""
    from PIL import Image
    png = s.tmp / "colourdrag.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon = editor_window(), env.monitor()
    win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.debug("ui", "editor", "rect", 50, 50, 100, 80)
    before = editor_state(s)
    s.move(*win(where(s, "editor", "color-swatch"))); s.click()
    assert wait_for(lambda: editor_state(s)["menuOpen"], 2), "the colour menu opens"
    time.sleep(0.3)                                                 # laid out
    s.move(*win(where(s, "editor", "color-custom"))); s.click()
    assert wait_for(lambda: where(s, "editor", "color-square")["x"] >= 0, 2), "the custom panel shows"
    sq = where(s, "editor", "color-square")
    x, y = win(sq)
    s.drag(x - 40, y - 30, x + 40, y + 30, steps=8)
    st = editor_state(s)
    assert st["items"][0]["color"] != before["items"][0]["color"], "the rect took the colour"
    assert st["undo"] == before["undo"] + 1, ("one undo step", before["undo"], st["undo"])
    s.debug("ui", "editor", "keys", "{Escape}")                     # the menu
    s.debug("ui", "editor", "keys", "{ctrl+z}")
    assert wait_for(lambda: editor_state(s)["items"][0]["color"] == before["items"][0]["color"], 2), "Ctrl+Z: its colour as before"
    close_editor(s)


@case
def editor_checkerboard_at_400(s, env):
    """ANN-01: transparency shows as a checkerboard, zoomed in too. The canvas that painted
    it took 0.5 s and up to half a gigabyte at 400 %; a 16 px tile, repeated, takes neither."""
    from PIL import Image
    png = s.tmp / "clear.png"
    Image.new("RGBA", (1800, 1100), (0, 0, 0, 0)).save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.8)

    def rss():
        status = dict(l.split(":", 1) for l in open(f"/proc/{s.proc.pid}/status") if ":" in l)
        return int(status["VmRSS"].split()[0]) // 1024

    before = rss()
    s.debug("ui", "editor", "zoom", 4)
    time.sleep(1.5)
    grown = rss() - before
    print(f"      400 %: {grown} MB more")
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    shot = s.tmp / "checker.png"
    sh("grim", "-o", OUT, str(shot))
    im, sc = Image.open(shot).convert("RGB"), mon["scale"]
    cx, cy = round((w["at"][0] - mon["x"] + st["winW"] / 2) * sc), round((w["at"][1] - mon["y"] + st["winH"] / 2) * sc)
    greys = {im.getpixel((cx + dx, cy + dy))[0] for dx in range(0, 40, 2) for dy in range(0, 40, 2)}
    assert any(abs(g - 216) <= 2 for g in greys) and any(abs(g - 244) <= 2 for g in greys), ("the checkerboard shows", greys)
    assert grown < 40, ("MB for the checkerboard at 400 %", grown)
    close_editor(s)


@case
def editor_crop_cancel_undoes_its_turns(s, env):
    """ANN-11: Cancel in the crop bar goes back to the image as it was when cropping started,
    a rotation, flip or resize made meanwhile too: only the crop and the fill came back."""
    from PIL import Image
    png = s.tmp / "turns.png"
    Image.new("RGB", (400, 300), (90, 140, 220)).save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "tool", "crop")
    s.debug("ui", "editor", "transform", "rotate-left")
    assert wait_for(lambda: (editor_state(s)["imgW"], editor_state(s)["imgH"]) == (300, 400), 3), "turned"
    s.debug("ui", "editor", "keys", "{Escape}")                    # Cancel
    st = editor_state(s)
    assert st["tool"] != "crop" and (st["imgW"], st["imgH"]) == (400, 300), ("as it was", st["tool"], st["imgW"], st["imgH"])
    close_editor(s)


@case
def editor_crop_typed_size_applies(s, env):
    """ANN-11: a width typed in the crop bar applies when you click Crop (or anywhere else):
    it applied only on Enter or Tab, and was dropped otherwise."""
    from PIL import Image
    png = s.tmp / "cropw.png"
    Image.new("RGB", (400, 300), (90, 140, 220)).save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon = editor_window(), env.monitor()
    win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.debug("ui", "editor", "tool", "crop")
    time.sleep(0.3)
    s.move(*win(where(s, "editor", "crop-w"))); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "{ctrl+a}200")
    s.move(*win(where(s, "editor", "crop-done"))); s.click()
    assert wait_for(lambda: editor_state(s)["tool"] != "crop", 2), "Crop ends cropping"
    crop = editor_state(s)["crop"]
    assert crop and crop["w"] == 200, ("the typed width", crop)
    close_editor(s)


@case
def editor_crop_menu_takes_esc_first(s, env):
    """ANN-11: with the crop bar's ratio menu open, Esc closes the menu. It cancelled the whole
    crop instead, and left the menu on screen."""
    from PIL import Image
    png = s.tmp / "cropmenu.png"
    Image.new("RGB", (400, 300), (90, 140, 220)).save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon = editor_window(), env.monitor()
    win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.debug("ui", "editor", "tool", "crop")
    time.sleep(0.3)
    s.move(*win(where(s, "editor", "crop-ratio"))); s.click()
    assert wait_for(lambda: editor_state(s)["menuOpen"], 2), "the ratio menu opens"
    s.debug("ui", "editor", "keys", "{Escape}")
    st = editor_state(s)
    assert not st["menuOpen"] and st["tool"] == "crop", ("the menu closes, cropping goes on", st["menuOpen"], st["tool"])
    s.debug("ui", "editor", "keys", "{Escape}")                    # now Cancel
    close_editor(s)


@case
def editor_esc_mid_drag_drops_the_object(s, env):
    """ANN-01: Esc while drawing drops the half-drawn object. With nothing selected, it closed
    the editor in the middle of the drag."""
    from PIL import Image
    png = s.tmp / "middrag.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.debug("ui", "editor", "tool", "rect")
    s.move(*at(50, 50)); s.debug("down"); s.move(*at(120, 100)); s.move(*at(150, 120))
    s.debug("ui", "editor", "keys", "{Escape}")
    time.sleep(0.3)
    open_ = editor_window() is not None
    s.move(*at(160, 130)); s.debug("up")
    assert open_, "the editor stays open"
    assert wait_for(lambda: editor_state(s)["items"] == [], 2), ("nothing drawn", editor_state(s)["items"])
    close_editor(s)


@case
def editor_alt_copy_keeps_the_editor_open(s, env):
    """ANN-15: Copy (Ctrl+Shift+C) puts the image on the clipboard and closes the editor; with
    Alt it stays open. It closed either way."""
    from PIL import Image
    png = s.tmp / "altcopy.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "rect", 40, 40, 80, 60)                 # an edit, written back first
    n = len(s.traces("toast"))
    s.debug("ui", "editor", "keys", "{ctrl+alt+shift+c}")
    assert wait_for(lambda: [t for t in s.traces("toast")[n:] if t["text"] == "Copied"], 3), "copied"
    time.sleep(0.6)
    assert editor_window(), "with Alt the editor stays open"
    s.debug("ui", "editor", "keys", "{ctrl+shift+c}")
    assert wait_for(lambda: editor_window() is None, 3), "without Alt it closes"


@case
def editor_background_closes_to_the_last_tool(s, env):
    """ANN-07: closing the Background panel with its button goes back to the tool you had,
    as Esc does; it went to Select."""
    from PIL import Image
    png = s.tmp / "bgtool.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    time.sleep(0.3)
    w, mon = editor_window(), env.monitor()
    win = lambda p: (w["at"][0] - mon["x"] + p["x"], w["at"][1] - mon["y"] + p["y"])
    s.debug("ui", "editor", "tool", "rect")
    s.move(*win(where(s, "editor", "tool-background"))); s.click()
    assert wait_for(lambda: editor_state(s)["tool"] == "background", 2), "the panel opens"
    s.move(*win(where(s, "editor", "tool-background"))); s.click()
    assert wait_for(lambda: editor_state(s)["tool"] != "background", 2), "and closes"
    assert editor_state(s)["tool"] == "rect", ("back to the rectangle", editor_state(s)["tool"])
    close_editor(s)


@case
def editor_crop_bar(s, env):
    """CleanShot's crop bar: the canvas beyond the image can be transparent, white, black or
    any colour; Cancel (or Escape) goes back to the crop it started with."""
    from PIL import Image
    png = s.tmp / "cropbar.png"
    Image.new("RGB", (300, 200), (90, 140, 220)).save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "crop", -20, -20, 340, 240)        # outward: the edge colour by default
    assert editor_state(s)["fill"] == "#5a8cdc", editor_state(s)["fill"]
    s.debug("ui", "editor", "fill", "transparent")
    editor_flush(s)
    out = Image.open(rec["source"])
    assert out.mode == "RGBA" and out.getpixel((5, 5))[3] == 0 and out.getpixel((150, 100))[3] == 255, out.mode
    s.debug("ui", "editor", "fill", "#000000")
    editor_flush(s)
    assert Image.open(rec["source"]).convert("RGB").getpixel((5, 5)) == (0, 0, 0)
    s.debug("ui", "editor", "tool", "crop")                    # Cancel: back to where cropping started
    s.debug("ui", "editor", "crop", 10, 10, 100, 80)
    s.debug("ui", "editor", "fill", "#ffffff")
    s.debug("ui", "editor", "keys", "{Escape}")
    st = editor_state(s)
    assert st["crop"] == {"x": -20, "y": -20, "w": 340, "h": 240} and st["fill"] == "#000000" and st["tool"] != "crop", st
    s.debug("ui", "editor", "tool", "crop")                    # the bar's buttons take real clicks
    s.debug("ui", "editor", "crop", 30, 30, 120, 90)
    w, mon, at = editor_window(), env.monitor(), editor_state(s)["cropCancelAt"]
    s.move(w["at"][0] - mon["x"] + at["x"], w["at"][1] - mon["y"] + at["y"])
    assert wait_for(lambda: editor_state(s)["cropCancelHovered"], 2), "the pointer is on Cancel"
    s.click()
    # the release comes back through Hyprland, so it can trail the state request
    assert wait_for(lambda: editor_state(s)["tool"] != "crop", 2), "clicking Cancel leaves cropping"
    assert editor_state(s)["crop"] == {"x": -20, "y": -20, "w": 340, "h": 240}, editor_state(s)["crop"]
    close_editor(s)


@case
def editor_rotate_flip_resize(s, env):
    """ANN-11: rotate, flip and resize the image with its objects; undo puts it back."""
    from PIL import Image
    im = Image.new("RGB", (300, 200), "white")
    im.paste((230, 20, 20), (0, 0, 30, 30))                      # a red marker, top-left
    png = s.tmp / "turn.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "color", "#0a7cff")
    s.debug("ui", "editor", "rect", 200, 20, 60, 40)
    s.debug("ui", "editor", "transform", "rotate-right")
    st = editor_state(s)
    assert (st["imgW"], st["imgH"]) == (200, 300) and st["base"], st
    r = st["items"][0]
    assert (r["x"], r["y"], r["w"], r["h"]) == (140, 200, 40, 60), ("the rectangle turns with the image", r)
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (200, 300) and out.getpixel((195, 5))[0] > 200, "the marker is now top-right"
    s.debug("ui", "editor", "undo")
    assert (editor_state(s)["imgW"], editor_state(s)["base"]) == (300, ""), "undo brings the image back"
    s.debug("ui", "editor", "transform", "flip-vertical")
    assert editor_state(s)["items"][0]["y"] == 200 - 60, editor_state(s)["items"][0]
    s.debug("ui", "editor", "undo")
    s.debug("ui", "editor", "transform", "resize", 150, 100)
    st = editor_state(s)
    r = st["items"][0]
    assert (st["imgW"], st["imgH"]) == (150, 100) and (r["x"], r["w"]) == (100, 30), (st["imgW"], r)
    editor_flush(s)
    assert Image.open(rec["source"]).size == (150, 100)
    close_editor(s)


@case
def editor_background(s, env):
    """BG-01…06: the Background tool wraps the image: padding grows the export, the image
    stays exact inside, corners round it, a ratio squares it, None takes it away."""
    from PIL import Image
    png = s.tmp / "bg.png"
    noise_png(png, 200, 120, seed=5)
    orig = Image.open(png).convert("RGB")
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":40,"inset":0,"radius":0,"shadow":0,"ratio":0,"align":"c"}')
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (280, 200), out.size
    assert out.getpixel((3, 3)) == (16, 160, 80), out.getpixel((3, 3))
    assert out.crop((40, 40, 240, 160)).tobytes() == orig.tobytes(), "the image is exact inside its background"
    s.debug("ui", "editor", "bg", '{"radius":24}')
    editor_flush(s)
    assert Image.open(rec["source"]).convert("RGB").getpixel((41, 41)) == (16, 160, 80), "rounded corners show the background"
    s.debug("ui", "editor", "bg", '{"ratio":1,"radius":0}')
    editor_flush(s)
    assert Image.open(rec["source"]).size == (280, 280)
    s.debug("ui", "editor", "bg", "none")
    editor_flush(s)
    assert Image.open(rec["source"]).convert("RGB").tobytes() == orig.tobytes(), "no background: the image again"
    close_editor(s)


@case
def editor_background_presets(s, env):
    """BG-03: a background saved as a named preset comes back in any editor; "Save Changes"
    updates it, "Apply Previous Settings" brings the last background, presets can go."""
    from PIL import Image
    a, b = s.tmp / "pa.png", s.tmp / "pb.png"
    noise_png(a, 200, 120, seed=1)
    noise_png(b, 160, 100, seed=2)
    s.json("edit", str(a))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "bg", '{"kind":"gradient","value":3,"padding":40,"radius":12,"shadow":0}')
    s.debug("ui", "editor", "bg-preset-save", "Blog", "post")
    st = editor_state(s)
    assert st["bgPresets"] == ["Blog post"] and st["bgPreset"] == "Blog post" and not st["bgPresetChanged"], st["bgPresets"]
    s.debug("ui", "editor", "bg", '{"padding":20}')
    assert editor_state(s)["bgPresetChanged"], "an edited preset shows it"
    s.debug("ui", "editor", "bg-preset-save", "Blog", "post")    # Save Changes
    assert not editor_state(s)["bgPresetChanged"] and editor_state(s)["bgPresets"] == ["Blog post"]
    s.debug("ui", "editor", "bg", "none")
    s.debug("ui", "editor", "bg-preset", "Blog", "post")
    bg = editor_state(s)["bg"]
    assert (bg["kind"], bg["value"], bg["padding"], bg["radius"]) == ("gradient", 3, 20, 12), bg
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":30}')
    close_editor(s)

    rec = s.json("edit", str(b))["result"]                       # another editor, another image
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    st = editor_state(s)
    assert st["bg"] is None and st["bgPresets"] == ["Blog post"], "presets are kept, nothing is applied by itself"
    s.debug("ui", "editor", "bg-previous")
    bg = editor_state(s)["bg"]
    assert (bg["kind"], bg["value"], bg["padding"]) == ("color", "#10a050", 30), bg
    s.debug("ui", "editor", "bg-preset", "Blog", "post")
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (200, 140), out.size
    s.debug("ui", "editor", "bg-preset-delete", "Blog", "post")
    st = editor_state(s)
    assert st["bgPresets"] == [] and st["bgPreset"] == "", st
    close_editor(s)


@case
def background_auto_apply(s, env):
    """BG-03 "Apply to New Screenshots": a preset set to apply automatically wraps every
    new screenshot before its card, as an edit (the background stays editable)."""
    from PIL import Image
    png = s.tmp / "auto-a.png"
    noise_png(png, 200, 120, seed=1)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":40,"inset":0,"radius":0,"shadow":0,"ratio":0,"align":"c"}')
    s.debug("ui", "editor", "bg-preset-save", "Green")
    s.debug("ui", "editor", "bg-auto", "Green")
    assert editor_state(s)["bgAuto"] == "Green", "set to apply automatically"
    close_editor(s)
    try:
        r, code = area(s, 100, 100, 300, 220)                     # 200x120 logical
        assert code == 0 and r["ok"], r
        rec = r["result"]
        assert wait_for(lambda: [e for e in s.traces("edited") if e["id"] == rec["id"]], 5), "the background is applied"
        out = Image.open(rec["source"]).convert("RGB")
        pad = 40                                                  # saved in points, back in this output's pixels
        assert out.size == (rec["width"] + 2 * pad, rec["height"] + 2 * pad), (out.size, rec["width"], rec["height"], pad)
        assert out.getpixel((3, 3)) == (16, 160, 80), out.getpixel((3, 3))
        original = Image.open(Path(rec["source"]).parent / "original.png").convert("RGB")
        assert out.crop((pad, pad, pad + original.width, pad + original.height)).tobytes() == original.tobytes(), "the shot is exact inside"
        assert wait_for(lambda: not [l for m in hypr_json("layers").values() for lv in m["levels"].values() for l in lv
                                     if l["namespace"] == "snip-render"], 2), "the render layer goes"
        assert wait_for(lambda: rec["id"] in s.json("status")["result"]["cards"], 3), "then the card"
        s.debug("annotate", rec["id"])                             # and it stays editable
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
        bg = editor_state(s)["bg"]
        assert bg and bg["kind"] == "color" and bg["padding"] == pad, bg
        close_editor(s)
    finally:
        s.json("edit", str(png))
        if wait_for(lambda: editor_window(), 5):
            s.debug("ui", "editor", "bg-auto", "Green")          # off again for the cases after
            s.debug("ui", "editor", "bg-preset-delete", "Green")
            close_editor(s)


@case
def capture_keeps_its_own_action(s, env):
    """CAP-02: a capture's own action (& Copy, --action) stays its own. A second capture
    started while the first one's auto-applied background is still being drawn (BG-03)
    doesn't take it, nor give it to the second."""
    png = s.tmp / "own-action.png"
    noise_png(png, 200, 120, seed=4)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":40,"inset":0,"radius":0,"shadow":0,"ratio":0,"align":"c"}')
    s.debug("ui", "editor", "bg-preset-save", "Green")
    s.debug("ui", "editor", "bg-auto", "Green")
    close_editor(s)
    s.cli("close-all"); time.sleep(0.3)
    try:
        rect = ("--x", "100", "--y", "80", "--w", "200", "--h", "120", "--monitor", OUT)
        first = s.json("area", *rect, "--action", "copy")["result"]
        second = s.json("area", *rect)["result"]                  # while the first's background is drawn
        for rec in (first, second):
            assert wait_for(lambda: [e for e in s.traces("edited") if e["id"] == rec["id"]], 5), "the background is applied"
        copied = lambda id: [e for e in s.traces("copied") if e["id"] == id]
        assert wait_for(lambda: copied(first["id"]), 3), "the first is copied, as it asked"
        assert wait_for(lambda: second["id"] in s.status()["cards"], 3), "the second gets its card"
        assert not copied(second["id"]), "and isn't copied"
    finally:
        s.cli("close-all")
        s.json("edit", str(png))
        if wait_for(lambda: editor_window(), 5):
            s.debug("ui", "editor", "bg-auto", "Green")          # off again for the cases after
            s.debug("ui", "editor", "bg-preset-delete", "Green")
            close_editor(s)


@case
def shift_skips_preset(s, env):
    """BG-03, D32: Shift held as the selection ends skips the auto-applied preset (and
    doesn't square the selection any more); the next capture gets it again."""
    from PIL import Image
    png = s.tmp / "skip.png"
    noise_png(png, 200, 120, seed=1)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":40,"inset":0,"radius":0,"shadow":0,"ratio":0,"align":"c"}')
    s.debug("ui", "editor", "bg-preset-save", "Skip")
    s.debug("ui", "editor", "bg-auto", "Skip")
    close_editor(s)
    try:
        r, code = area(s, 100, 100, 300, 160, during=lambda: s.key("Shift", "keydown"))
        s.key("Shift", "keyup")
        assert code == 0 and r["ok"], r
        rec = r["result"]
        assert (rec["width"], rec["height"]) == (250, 75), ("Shift no longer squares", rec)
        assert wait_for(lambda: rec["id"] in s.json("status")["result"]["cards"], 3), "its card"
        assert not [e for e in s.traces("auto_background") if e["id"] == rec["id"]], "Shift skipped the preset"
        assert Image.open(rec["source"]).size == (250, 75), "the plain shot"
        r, _ = area(s, 100, 100, 300, 160)
        assert wait_for(lambda: [e for e in s.traces("edited") if e["id"] == r["result"]["id"]], 5), "without Shift: the preset again"
        # Shift on a capture that gets no preset (Capture Text) is for that capture only:
        # an exact area from `snip area` after it still gets the preset
        p = s.bg("text"); time.sleep(0.5)
        s.drag(100, 100, 300, 160, during=lambda: s.key("Shift", "keydown"))
        s.key("Shift", "keyup")
        finish(p, timeout=20)
        r = s.json("area", "--x", "100", "--y", "100", "--w", "200", "--h", "120", "--monitor", OUT)
        assert r["ok"], r
        assert wait_for(lambda: [e for e in s.traces("edited") if e["id"] == r["result"]["id"]], 5), "the preset, after a Shift-released text capture"
    finally:
        s.json("edit", str(png))
        if wait_for(lambda: editor_window(), 5):
            s.debug("ui", "editor", "bg-auto", "Skip")
            s.debug("ui", "editor", "bg-preset-delete", "Skip")
            close_editor(s)


@case
def editor_own_wallpaper(s, env):
    """BG-01: an image of your own becomes a wallpaper (copied into Snip's data, so it
    stays when the file goes), shows behind the capture, and can be removed again."""
    from PIL import Image
    src = s.tmp / "mywall.png"
    Image.new("RGB", (64, 64), (200, 30, 160)).save(src)
    png = s.tmp / "w.png"
    noise_png(png, 120, 80, seed=3)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "add-wallpaper", str(src))
    st = editor_state(s)
    walls = st["ownWalls"]
    assert len(walls) == 1 and walls[0].startswith(str(s.tmp / "data")) and Path(walls[0]).exists(), walls
    assert st["bg"]["kind"] == "wallpaper" and st["bg"]["value"] == walls[0], st["bg"]
    src.unlink()                                                # the copy is what counts
    s.debug("ui", "editor", "bg", '{"padding":30,"shadow":0,"radius":0,"inset":0}')
    editor_flush(s)
    corner = Image.open(rec["source"]).convert("RGB").getpixel((4, 4))
    assert abs(corner[0] - 200) < 12 and corner[1] < 50 and abs(corner[2] - 160) < 12, corner
    s.debug("ui", "editor", "add-wallpaper", str(s.tmp / "missing.txt"))
    assert len(editor_state(s)["ownWalls"]) == 1, "only images become wallpapers"
    s.debug("ui", "editor", "remove-wallpaper", walls[0])
    st = editor_state(s)
    assert st["ownWalls"] == [] and not Path(walls[0]).exists() and st["bg"] is None, st["bg"]
    close_editor(s)


@case
def editor_auto_balance(s, env):
    """BG-02 Auto-balance: the plain border around the content is evened out to its
    widest side, in the border's colour, so the content sits centred."""
    from PIL import Image
    im = Image.new("RGB", (200, 120), "white")
    for x in range(20, 150):                    # content: 20 px from the left, 50 from the right,
        for y in range(10, 100):                # 10 from the top, 20 from the bottom
            im.putpixel((x, y), (30, 60, 200) if (x + y) % 7 else (200, 40, 40))
    png = s.tmp / "balance.png"
    im.save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":20,"inset":0,"radius":0,"shadow":0,"ratio":0,"align":"c","balance":true}')
    st = editor_state(s)
    assert st["balance"] == {"l": 20, "t": 10, "r": 50, "b": 20, "color": "#ffffff"}, st["balance"]
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (20 + 50 + 130 + 50 + 20, 20 + 50 + 90 + 50 + 20), out.size     # 50 px of white on every side
    assert out.getpixel((5, 5)) == (16, 160, 80), "the background around it"
    assert out.getpixel((25, 135)) == (255, 255, 255) and out.getpixel((140, 25)) == (255, 255, 255), "the border, evened out"
    assert out.crop((50, 60, 250, 180)).tobytes() == im.tobytes(), "the image is exact inside"
    s.debug("ui", "editor", "bg", '{"balance":false}')
    editor_flush(s)
    assert Image.open(rec["source"]).size == (240, 160), "off again: just the padding"
    close_editor(s)


@case
def editor_typed_values(s, env):
    """BG-05: hovering a Background slider shows its value; click it to type one, ↑/↓
    step it, Enter or Tab commits (one undo step each)."""
    from PIL import Image
    png = s.tmp / "typed.png"
    noise_png(png, 300, 1000, seed=8)                          # a tall window, so the sliders are in view
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "tool", "background")
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":40}')
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    unit, undo = st["dpr"], st["undo"]
    box = st["paddingValue"]
    assert box["y"] < st["winH"] - 60, (box, st["winH"])
    s.move(w["at"][0] - mon["x"] + box["x"], w["at"][1] - mon["y"] + box["y"])
    time.sleep(0.2); s.click(); time.sleep(0.2)
    s.debug("ui", "editor", "keys", "48{Tab}")
    st = editor_state(s)
    assert st["bg"]["padding"] == round(48 * unit) and st["undo"] == undo + 1, (st["bg"], unit)
    s.click(); time.sleep(0.2)                                  # the value box again (the pointer is still there)
    s.debug("ui", "editor", "keys", "{Up}{shift+Up}{Return}")
    assert editor_state(s)["bg"]["padding"] == round(59 * unit), editor_state(s)["bg"]
    s.debug("ui", "editor", "keys", "{ctrl+z}")                 # the editor has its keys back
    assert editor_state(s)["bg"]["padding"] == round(48 * unit)
    close_editor(s)


@case
def editor_take_screenshot(s, env):
    """ANN-12 Add image > Take Screenshot: the editor steps aside, an area is selected,
    and the shot lands in the canvas as an image; no capture goes to history."""
    from PIL import Image
    png = s.tmp / "shot-into.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    ingested, sessions = len(s.traces("ingested")), len(s.traces("session"))
    s.debug("ui", "editor", "take-screenshot")
    assert wait_for(lambda: editor_window() is None, 3), "the editor steps aside"
    assert wait_for(lambda: len(s.traces("session")) > sessions, 3), "an area overlay opens"
    time.sleep(0.3)
    s.drag(100, 100, 300, 260)
    assert wait_for(lambda: editor_window(), 5), "the editor comes back"
    st = editor_state(s)
    img = [i for i in st["items"] if i["kind"] == "image"]
    assert len(img) == 1 and abs(img[0]["w"] / img[0]["h"] - 250 / 200) < 0.02, st["items"]   # 200x160 at 1.25
    time.sleep(0.5)
    assert len(s.traces("ingested")) == ingested, "not a capture in history"
    s.debug("ui", "editor", "take-screenshot")                 # Escape cancels: nothing added
    assert wait_for(lambda: len(s.traces("session")) > sessions + 1, 3)
    time.sleep(0.3)
    s.key("Escape")
    assert wait_for(lambda: editor_window(), 5)
    assert len([i for i in editor_state(s)["items"] if i["kind"] == "image"]) == 1
    close_editor(s)


@case
def editor_snapping(s, env):
    """ANN-12 snapping guides: a moved object snaps its edges or middle to the canvas's
    (and other objects') edges and middle when it comes near them."""
    from PIL import Image
    png = s.tmp / "snap.png"
    Image.new("RGB", (400, 300), "white").save(png)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "rect", 100, 100, 80, 60)            # its middle: (140, 130)
    s.debug("ui", "editor", "tool", "select")
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + (ix - st["shownX"]) * st["k"], w["at"][1] - mon["y"] + st["originY"] + (iy - st["shownY"]) * st["k"])
    s.drag(*at(102, 130), *at(159, 148), steps=6)               # middle to (197, 148): near the canvas's (200, 150)
    assert wait_for(lambda: editor_state(s)["items"][0]["x"] != 100, 2)
    r = editor_state(s)["items"][0]
    assert (round(r["x"]), round(r["y"])) == (160, 120), ("snapped to the canvas's middle", r)
    assert editor_state(s)["guides"] is None, "the guides go when the drag ends"
    close_editor(s)


@case
def editor_drop_zones(s, env):
    """ANN-12 combine images: a file dropped near an edge joins the canvas on that side,
    scaled to its width (or height), and the canvas grows (one undo step); dropped in the
    middle, it lands on top."""
    from PIL import Image
    base, other = s.tmp / "base.png", s.tmp / "other.png"
    noise_png(base, 400, 300, seed=1)
    Image.new("RGB", (200, 100), (200, 30, 160)).save(other)
    rec = s.json("edit", str(base))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    h0 = editor_window()["size"][1]
    s.debug("ui", "editor", "drop", str(other), 150, 280)       # near the bottom edge
    assert wait_for(lambda: editor_window()["size"][1] > h0 + 100, 3), ("the window grows with the canvas", h0, editor_window()["size"])
    st = editor_state(s)
    img = st["items"][-1]
    assert (img["kind"], img["x"], img["y"], img["w"], img["h"]) == ("image", 0, 300, 400, 200), img
    assert st["crop"] == {"x": 0, "y": 0, "w": 400, "h": 500}, st["crop"]
    editor_flush(s)
    out = Image.open(rec["source"]).convert("RGB")
    assert out.size == (400, 500) and out.crop((0, 0, 400, 300)).tobytes() == Image.open(base).convert("RGB").tobytes()
    assert out.getpixel((200, 400)) == (200, 30, 160), "the dropped image fills the new part"
    s.debug("ui", "editor", "drop", str(other), 395, 150)       # the right edge: as tall as the canvas (500)
    img = editor_state(s)["items"][-1]
    assert (img["x"], img["y"], img["w"], img["h"]) == (400, 0, 1000, 500), img
    assert editor_state(s)["crop"] == {"x": 0, "y": 0, "w": 1400, "h": 500}
    s.debug("ui", "editor", "undo")
    assert editor_state(s)["crop"] == {"x": 0, "y": 0, "w": 400, "h": 500}, "one undo takes back the image and the growth"
    s.debug("ui", "editor", "drop", str(other), 200, 250)       # the middle: on top, the canvas stays
    st = editor_state(s)
    assert st["crop"] == {"x": 0, "y": 0, "w": 400, "h": 500} and st["items"][-1]["w"] <= 200, st["items"][-1]
    close_editor(s)


@case
def background_tiles_have_no_frame(s, env):
    """The Background panel's thumbnails run to their rounded edges: no white frame around
    a gradient (one showed before, 2 px wide)."""
    from PIL import Image
    png = s.tmp / "tiles.png"
    noise_png(png, 300, 1000, seed=8)
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "tool", "background")
    assert wait_for(lambda: editor_state(s)["firstTile"], 2), "the panel shows"
    time.sleep(0.3)
    w, mon, t, scale = editor_window(), env.monitor(), editor_state(s)["firstTile"], env.monitor()["scale"]
    shot = s.tmp / "tiles-shot.png"
    sh("grim", "-o", OUT, str(shot))
    x, y = round((w["at"][0] - mon["x"] + t["x"]) * scale), round((w["at"][1] - mon["y"] + t["y"] + 12) * scale)
    px = Image.open(shot).convert("RGB").getpixel((x + 1, y))
    assert min(px) < 230, ("a white frame at the tile's edge", px)
    close_editor(s)


@case
def slider_drag_stays_put(s, env):
    """Dragging a Background slider moves the slider only: a drag that wanders up or down
    never scrolls the panel."""
    png = s.tmp / "sliders.png"
    noise_png(png, 300, 1000, seed=8)                           # a tall window: the sliders in view
    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "tool", "background")
    s.debug("ui", "editor", "bg", '{"kind":"color","value":"#10a050","padding":10}')
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    t, width, before = st["paddingTrack"], st["paddingWidth"], st["bg"]["padding"]
    x0, y0 = w["at"][0] - mon["x"] + t["x"], w["at"][1] - mon["y"] + t["y"]
    s.move(x0 + width * 0.1, y0); s.debug("down")
    for k, dy in enumerate([-15, -30, -45, -60, -75, -90], 1):   # left to right, drifting up as a hand does
        s.move(x0 + width * (0.1 + 0.12 * k), y0 + dy)
    s.debug("up")
    assert wait_for(lambda: editor_state(s)["bg"]["padding"] > before + 50, 2), editor_state(s)["bg"]
    assert editor_state(s)["panelScrolled"] == 0, ("the panel scrolled", editor_state(s)["panelScrolled"])
    s.debug("scroll", "60")                                      # the wheel still scrolls it
    assert wait_for(lambda: editor_state(s)["panelScrolled"] > 0, 2), "the wheel scrolls the panel"
    close_editor(s)


@case
def editor_add_image(s, env):
    """ANN-12: add an image from a file and from the clipboard; it keeps its shape when
    resized from a corner, and is drawn into the export."""
    from PIL import Image
    Image.new("RGB", (400, 300), "white").save(s.tmp / "canvas.png")
    Image.new("RGB", (200, 100), (200, 30, 160)).save(s.tmp / "sticker.png")
    rec = s.json("edit", str(s.tmp / "canvas.png"))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    s.debug("ui", "editor", "add-image", str(s.tmp / "sticker.png"))
    im = editor_state(s)["items"][0]
    assert im["kind"] == "image" and (im["w"], im["h"]) == (200, 100), im
    w, mon, st = editor_window(), env.monitor(), editor_state(s)
    at = lambda ix, iy: (w["at"][0] - mon["x"] + st["originX"] + ix * st["k"], w["at"][1] - mon["y"] + st["originY"] + iy * st["k"])
    s.drag(*at(im["x"] + im["w"], im["y"] + im["h"]), *at(im["x"] + im["w"] - 100, im["y"] + im["h"] - 10), steps=5)
    time.sleep(0.2)
    r = editor_state(s)["items"][0]
    assert abs(r["w"] / r["h"] - 2) < 0.02 and abs(r["w"] - 100) < 3, ("resized from the corner, still 2:1", r)
    subprocess.run(["wl-copy", "--type", "image/png"], stdin=open(s.tmp / "sticker.png", "rb"), check=True)
    time.sleep(0.3)
    try:
        s.debug("ui", "editor", "paste-image")
        assert [i["kind"] for i in editor_state(s)["items"]] == ["image", "image"]
    finally:
        subprocess.run(["wl-copy", "--clear"], check=False)
    editor_flush(s)
    c = editor_state(s)["items"][1]
    px = Image.open(rec["source"]).convert("RGB").getpixel((round(c["x"] + c["w"] / 2), round(c["y"] + c["h"] / 2)))
    assert px == (200, 30, 160), px
    close_editor(s)


@case
def editor_auto_expand(s, env):
    """ANN-17 "Auto-expand canvas": an object drawn past the image grows the canvas to
    hold it, filled with the edge colour, in one undo step; inside it nothing changes."""
    from PIL import Image
    cfg = s.tmp / "cfg/snip/config.toml"
    cfg.write_text(cfg.read_text().replace("object_shadows = false", "object_shadows = false\nauto_expand = true"))
    try:
        time.sleep(0.8)                                         # the config watcher reloads it
        png = s.tmp / "expand.png"
        noise_png(png, 300, 200, seed=5)
        Image.open(png).convert("RGB").save(png)
        im = Image.open(png).convert("RGB")
        for x in range(300):                                    # a plain grey frame, the edge colour
            for y in list(range(0, 6)) + list(range(194, 200)):
                im.putpixel((x, y), (90, 90, 90))
        for y in range(200):
            for x in list(range(0, 6)) + list(range(294, 300)):
                im.putpixel((x, y), (90, 90, 90))
        im.save(png)
        rec = s.json("edit", str(png))["result"]
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
        s.debug("ui", "editor", "rect", 100, 60, 80, 50)        # inside: no change
        assert editor_state(s)["crop"] is None
        s.debug("ui", "editor", "frect", 250, 150, 100, 80)     # past the right and bottom edges
        st = editor_state(s)
        c = st["crop"]
        assert c and c["x"] == 0 and c["y"] == 0 and c["w"] >= 350 and c["h"] >= 230, c
        assert st["fill"].lower() == "#5a5a5a", st["fill"]
        editor_flush(s)
        out = Image.open(rec["source"]).convert("RGB")
        assert out.size == (c["w"], c["h"]), out.size
        assert out.getpixel((10, 10)) == im.getpixel((10, 10)), "the image stays where it was"
        assert out.getpixel((c["w"] - 2, 20)) == (90, 90, 90), "the new canvas takes the edge colour"
        assert out.getpixel((320, 200))[1] < 80, "the box is drawn in the new canvas"
        s.debug("ui", "editor", "undo")
        st = editor_state(s)
        assert st["crop"] is None and len(st["items"]) == 1, "one undo takes back the box and the growth"
        s.debug("ui", "editor", "text", 230, 40, "Expanding canvas")   # text measures itself
        c = editor_state(s)["crop"]
        assert c and c["w"] > 330 and c["y"] == 0 and c["h"] == 200, c
        editor_flush(s)
        out = Image.open(rec["source"]).convert("RGB")
        assert any(out.getpixel((x, y))[1] < 120 and out.getpixel((x, y))[0] > 150 for x in range(300, c["w"]) for y in range(30, 80)), \
            "the text past the old edge is drawn"
        close_editor(s)
    finally:
        cfg.write_text(cfg.read_text().replace("\nauto_expand = true", ""))
        time.sleep(0.8)


@case
def editor_remembers_tool(s, env):
    """The next editor starts with the tool the last one used (CleanShot does): text, the
    counter, the arrow... Crop and the Background tool are modes, not remembered."""
    from PIL import Image
    png = s.tmp / "tool.png"
    Image.new("RGB", (300, 200), "white").save(png)

    def editor(steps=()):
        s.json("edit", str(png))
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
        tool = editor_state(s)["tool"]
        for t in steps:
            s.debug("ui", "editor", "tool", t)
        close_editor(s)
        return tool

    editor(["counter"])
    assert editor(["text", "crop"]) == "counter", "the last one's tool"
    assert editor() == "text", "crop isn't remembered: the tool before it is"
    editor(["arrow"])


@case
def editor_window_settings(s, env):
    """ANN-18: "Always on top" pins the editor (Hyprland); "Remember if the Background
    tool was opened" opens the next editor with it."""
    from PIL import Image
    cfg = s.tmp / "cfg/snip/config.toml"
    on = "object_shadows = false\nalways_on_top = true\nremember_background_tool = true"
    cfg.write_text(cfg.read_text().replace("object_shadows = false", on))
    try:
        time.sleep(0.8)                                             # the config watcher reloads it
        png = s.tmp / "win.png"
        Image.new("RGB", (300, 200), "white").save(png)
        s.json("edit", str(png))
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
        assert wait_for(lambda: editor_window()["pinned"], 3), "always on top: pinned"
        s.debug("ui", "editor", "tool", "background")
        close_editor(s)
        s.json("edit", str(png))
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens again"
        assert editor_state(s)["tool"] == "background", "the Background tool is open again"
        s.debug("ui", "editor", "tool", "arrow")
        close_editor(s)
        s.json("edit", str(png))
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens a third time"
        assert editor_state(s)["tool"] != "background", "closed without it: opens without it"
        close_editor(s)
    finally:
        cfg.write_text(cfg.read_text().replace(on, "object_shadows = false"))
        time.sleep(0.8)


@case
def editor_object_shadows(s, env):
    """CleanShot's "Draw shadow on objects" (config [annotate] object_shadows, on by default):
    a soft shadow under objects; the objects themselves stay crisp."""
    from PIL import Image
    cfg = s.tmp / "cfg/snip/config.toml"
    cfg.write_text(cfg.read_text().replace("object_shadows = false", "object_shadows = true"))
    try:
        time.sleep(0.8)                                         # the config watcher reloads it
        png = s.tmp / "shadow.png"
        Image.new("RGB", (300, 200), "white").save(png)
        rec = s.json("edit", str(png))["result"]
        assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
        s.debug("ui", "editor", "frect", 100, 60, 80, 50)
        editor_flush(s)
        out = Image.open(rec["source"]).convert("RGB")
        below = out.getpixel((140, 60 + 50 + 3))
        assert below[0] < 250 and abs(below[0] - below[2]) < 12, ("a grey shadow under the box", below)
        inside, edge = out.getpixel((140, 85)), out.getpixel((100, 85))
        assert inside[0] > 230 and inside[1] < 60 and edge[0] > 220 and edge[1] < 70, ("crisp box", inside, edge)
        close_editor(s)
    finally:
        cfg.write_text(cfg.read_text().replace("object_shadows = true", "object_shadows = false"))
        time.sleep(0.8)


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
        hypr_eval('hl.dispatch(hl.dsp.global("snip-e2e:edit-clipboard"))')   # SUPER+SHIFT+6 (D29)
        assert wait_for(lambda: editor_window(), 5), "the edit-clipboard shortcut opened no editor"
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
def project_file(s, env):
    """ANN-15/16: Save As a .snip project keeps the original, the objects and their files;
    opening it gives back an editor whose objects are still editable."""
    import zipfile
    from PIL import Image
    base, other = s.tmp / "proj.png", s.tmp / "proj-add.png"
    noise_png(base, 300, 200, seed=3)
    Image.new("RGB", (100, 60), (200, 30, 160)).save(other)
    rec = s.json("edit", str(base))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "rect", 20, 20, 80, 50)
    s.debug("ui", "editor", "drop", str(other), 150, 190)        # an added image, below
    editor_flush(s)
    close_editor(s)
    flat = Image.open(rec["source"]).convert("RGB")
    snip = s.tmp / "work.snip"
    out = s.json("debug", "save-as", rec["id"], str(snip))
    assert out["ok"] and out["result"].endswith("work.snip"), out
    names = zipfile.ZipFile(snip).namelist()
    assert {"manifest.json", "source.png", "document.json", "preview.png", "thumb.webp"} <= set(names), names
    assert any(n.startswith("assets/") for n in names), ("the added image travels with it", names)
    with zipfile.ZipFile(snip) as z:
        assert all(i.compress_type == zipfile.ZIP_STORED for i in z.infolist()), "nothing compressed"
    rec2 = s.json("edit", str(snip))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the project opens"
    st = editor_state(s)
    assert [i["kind"] for i in st["items"]] == ["rect", "image"], st["items"]
    assert Image.open(rec2["source"]).convert("RGB").tobytes() == flat.tobytes(), "the same picture"
    s.debug("ui", "editor", "delete", 0)                          # the objects are objects again
    editor_flush(s)
    assert Image.open(rec2["source"]).convert("RGB").getpixel((40, 40)) == Image.open(base).convert("RGB").getpixel((40, 40)), "the rectangle came off"
    close_editor(s)


@case
def open_with(s, env):
    """CleanShot's share is "Open With…" here: the editor writes its edits back first,
    then asks the desktop's app chooser (the portal; not opened in tests) for that file."""
    from PIL import Image
    png = s.tmp / "share.png"
    Image.new("RGB", (300, 200), "white").save(png)
    rec = s.json("edit", str(png))["result"]
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3), "the editor opens"
    s.debug("ui", "editor", "frect", 20, 20, 60, 40)
    s.debug("ui", "editor", "share")
    assert wait_for(lambda: [e for e in s.traces("open_with") if e["id"] == rec["id"]], 5), "Open With asked"
    path = [e for e in s.traces("open_with") if e["id"] == rec["id"]][-1]["path"]
    assert Path(path).exists() and Image.open(path).convert("RGB").getpixel((40, 40))[1] < 100, ("the edited image", path)
    assert editor_window(), "the editor stays open"
    close_editor(s)


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
def file_dialogs_answer_who_asked(s, env):
    """Each desktop file dialog answers the request that opened it: Save As on a card and
    in the editor, Add Image, Add Wallpaper, Open… and Pin to the Screen… in the bar's
    menu, and Settings › Export location. (The test portal answers a path given ahead.)"""
    from PIL import Image
    png = s.tmp / "chosen.png"
    Image.new("RGB", (240, 160), (40, 160, 90)).save(png)
    answer = lambda p: s.debug("ui", "answer-dialog", str(p))
    s.cli("close-all"); time.sleep(0.3)
    rec = open_card(s, png); time.sleep(0.3)
    answer(s.tmp / "card-as.png")
    s.debug("ui", "save-as", rec["id"])
    assert wait_for(lambda: (s.tmp / "card-as.png").exists(), 3), "a card's Save As writes where the dialog said"
    s.cli("close-all")

    s.json("edit", str(png))
    assert wait_for(lambda: editor_window(), 5) and wait_for(lambda: editor_state(s)["ready"], 3)
    answer(png)
    s.debug("ui", "editor", "choose-image")
    assert wait_for(lambda: [i["kind"] for i in editor_state(s)["items"]] == ["image"], 3), "Add Image adds the file"
    answer(png)
    s.debug("ui", "editor", "choose-wallpaper")
    assert wait_for(lambda: len(editor_state(s)["ownWalls"]) == 1, 3), "Add Wallpaper adds it"
    s.debug("ui", "editor", "remove-wallpaper", editor_state(s)["ownWalls"][0])
    answer(s.tmp / "editor-as.png")
    s.debug("ui", "editor", "save-as")
    assert wait_for(lambda: (s.tmp / "editor-as.png").exists(), 5), "the editor's Save As writes there"
    assert wait_for(lambda: editor_window() is None, 5), "and closes"

    try:
        for item, opened in (("open", lambda: editor_window()), ("pin", lambda: pin_windows())):
            answer(png)
            s.move(300, 5)                                              # on the bar, as the icon's click
            s.debug("ui", "tray", "open")
            assert wait_for(lambda: tray_state(s)["open"], 2)
            p = where(s, "tray", "menu:" + item)
            s.move(p["x"], p["y"]); s.click()
            assert wait_for(opened, 5), item + "… opens the chosen file"
    finally:
        s.debug("ui", "close-editors")
        s.debug("ui", "close-pins")

    cfg = s.tmp / "cfg/snip/config.toml"
    before = cfg.read_text()
    folder = s.tmp / "exports"
    folder.mkdir(exist_ok=True)
    try:
        s.json("settings", "general")
        assert wait_for(lambda: settings_client(), 5)
        answer(folder)
        settings_click(s, env, "choose:general.export_dir")
        assert wait_for(lambda: settings_state(s, "general.export_dir")["value"] == str(folder), 3), "Export location takes the folder"
    finally:
        s.debug("ui", "settings", "close")
        cfg.write_text(before)
        time.sleep(0.8)


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
    ap.add_argument("--live", action="store_true", help="test in the user's own session (moves their pointer)")
    ap.add_argument("--force", action="store_true", help="--live even if the pointer is moving")
    a = ap.parse_args()
    nested = None
    if a.live:
        if not a.force and not pointer_idle():
            print("the pointer is in use: run --live while the machine is idle (or --force)")
            return 3
    else:
        nested = Nested()
        os.environ.update(nested.start())   # every hyprctl, grim, wl-copy and snipd below talks to it
        global OUT
        OUT = Nested.OUTPUT

    env = Env(own_output=nested is None)
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
            # the test output as every case expects it (a case, or the session around
            # the nested Hyprland, may have changed it: say so, and put it back)
            m = env.monitor()
            if (m["scale"], m["transform"], m["width"], m["height"], m["reserved"]) != (1.25, 0, 1920, 1080, [0, 0, 0, 0]):
                print(f"      the test output changed after {fn.__name__}: scale {m['scale']}, transform {m['transform']}, "
                      f"{m['width']}x{m['height']}, reserved {m['reserved']}; restored")
                env.configure(1.25)
            s.cli("close-all", check=False)
            s.cli("record", "discard", check=False)              # nor a recording
            s.cli("debug", "ui", "close-editors", check=False)   # a failed case must not leave its editors to the next
            s.cli("debug", "ui", "close-pins", check=False)
            time.sleep(0.2)
            (s.tmp / "data/snip/editor.json").unlink(missing_ok=True)   # each case's editor starts with the arrow
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
        if nested:
            nested.stop()
        if a.keep or failed:
            print(f"  logs: {s.tmp}")
        else:
            shutil.rmtree(s.tmp, ignore_errors=True)
    print((f"{len(failed)} failed" if failed else "all passed") + (f" ({len(skipped)} skipped: {', '.join(skipped)})" if skipped else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
