#!/usr/bin/env python3
"""S3/S10/S14 matrix: drag the QAO card into real apps (virtual pointer) and
paste (Ctrl+V via Hyprland send_shortcut); record what each app received.

Runs everything on a temporary workspace "name:sniptest", then restores the
previously active workspace and closes the test windows.
"""
import http.server
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPIKE = HERE.parent
RUNTIME = os.environ["XDG_RUNTIME_DIR"]
W = Path(os.environ.get("SNIP_TEST_DIR", "/tmp/snip-dnd-test"))
PORT = 47613
TEST_PNG = HERE / "test.png"
RESULTS = []
OUTPUT = "DP-2"

# ------------------------------------------------------------------ helpers

def sh(*args, **kw):
    return subprocess.run(args, capture_output=True, text=True, **kw)

def hypr(lua):
    r = sh("hyprctl", "eval", lua)
    if r.stdout.strip() not in ("ok", ""):
        print("  hyprctl:", r.stdout.strip(), file=sys.stderr)

def clients():
    return json.loads(sh("hyprctl", "clients", "-j").stdout)

def find(title_part):
    for c in clients():
        if title_part in c.get("title", "") or title_part in c.get("initialTitle", ""):
            return c
    return None

def ctl(*lines):
    s = socket.socket(socket.AF_UNIX)
    s.connect(f"{RUNTIME}/snip-spike.sock")
    s.sendall(("\n".join(lines) + "\n").encode())
    s.settimeout(2)
    buf = b""
    try:
        while buf.count(b"\n") < len(lines):
            buf += s.recv(4096)
    except socket.timeout:
        pass
    s.close()
    return buf.decode().split()

def monitor():
    for m in json.loads(sh("hyprctl", "monitors", "-j").stdout):
        if m["name"] == OUTPUT:
            return m
    raise SystemExit("monitor not found")

MON = None
EXT = (0, 0)

def move(x, y):
    ctl(f"move {OUTPUT} {int(x - MON['x'])} {int(y - MON['y'])} {EXT[0]} {EXT[1]}")

def card_center():
    layers = json.loads(sh("hyprctl", "layers", "-j").stdout)
    for lvl in layers.get(OUTPUT, {}).get("levels", {}).values():
        for l in lvl:
            if l["namespace"] == "snip-qao":
                return l["x"] + l["w"] / 2, l["y"] + l["h"] / 2
    return None

def drag_to(tx, ty, steps=14, shot=None):
    c = card_center()
    if not c:
        return "no-card"
    cx, cy = c
    move(cx, cy); time.sleep(0.25)
    ctl("down"); time.sleep(0.08)
    for i in range(1, steps + 1):
        f = i / steps
        move(cx + (tx - cx) * f, cy + (ty - cy) * f)
        time.sleep(0.035)
    # wiggle inside the target so it gets several motion events, then hover
    for dx in (12, -12, 6, 0):
        move(tx + dx, ty + dx); time.sleep(0.06)
    time.sleep(0.5)
    if shot:
        sh("grim", "-o", OUTPUT, "-s", "0.4", shot)
    ctl("up")
    time.sleep(1.4)
    return "done"

def center(c):
    x, y = c["at"]; w, h = c["size"]
    return x + w / 2, y + h / 2

def focus(c):
    hypr(f'hl.dispatch(hl.dsp.focus({{ window = "address:{c["address"]}" }}))')

def send_paste(c, mods="CTRL"):
    focus(c); time.sleep(0.3)
    hypr(f'hl.dispatch(hl.dsp.send_shortcut({{ mods = "{mods}", key = "V", window = "address:{c["address"]}" }}))')
    time.sleep(1.5)

# ------------------------------------------------------------------ result server

class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        body = (HERE / "page.html").read_bytes()
        self.send_response(200); self.send_header("Content-Type", "text/html"); self.end_headers(); self.wfile.write(body)
    def do_POST(self):
        n = int(self.headers.get("Content-Length", 0))
        RESULTS.append(json.loads(self.rfile.read(n) or b"{}"))
        self.send_response(204); self.end_headers()

def take_results(target, kind):
    got = [r for r in RESULTS if r.get("target") == target and r.get("kind") == kind]
    for r in got:
        RESULTS.remove(r)
    return got[-1] if got else None

# ------------------------------------------------------------------ setup

def setup_dirs():
    if W.exists():
        shutil.rmtree(W)
    for d in ("snip-drop-target", "chromium", "zen", "vscode/User", "vscodews", "src"):
        (W / d).mkdir(parents=True)
    (W / "zen" / "user.js").write_text("\n".join([
        'user_pref("browser.shell.checkDefaultBrowser", false);',
        'user_pref("browser.aboutwelcome.enabled", false);',
        'user_pref("zen.welcome-screen.seen", true);',
        'user_pref("browser.startup.homepage_override.mstone", "ignore");',
        'user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);',
        'user_pref("startup.homepage_welcome_url", "");',
    ]) + "\n")
    (W / "vscode/User/settings.json").write_text(json.dumps({
        "security.workspace.trust.enabled": False,
        "workbench.startupEditor": "none",
        "markdown.editor.filePaste.enabled": "always",
        "markdown.editor.drop.enabled": "always",
        "update.mode": "none",
        "telemetry.telemetryLevel": "off",
        "chat.disableAIFeatures": True,
        "workbench.welcomePage.walkthroughs.openOnInstall": False,
        "workbench.tips.enabled": False,
    }))
    (W / "vscodews/test.md").write_text("# paste target\n\n")
    for t in ("term.txt", "kitty.txt", "foot.txt"):
        (W / t).write_text("")

APPS = [
    ("chromium", "SNIP-TEST chromium",
     f"chromium --user-data-dir={W}/chromium --no-first-run --no-default-browser-check --ozone-platform=wayland --app=http://127.0.0.1:{PORT}/?t=chromium"),
    ("zen", "SNIP-TEST zen",
     f"zen-browser --profile {W}/zen --no-remote --new-instance http://127.0.0.1:{PORT}/?t=zen"),
    ("nautilus", "snip-drop-target", f"nautilus --new-window {W}/snip-drop-target"),
    ("ghostty", "snip-term-target",
     f"ghostty --title=snip-term-target -e sh -c 'printf \"\\033[?2004h\"; stty raw -echo; exec cat > {W}/term.txt'"),
    ("kitty", "snip-kitty-target",
     f"kitty --title snip-kitty-target sh -c 'stty raw -echo; exec cat > {W}/kitty.txt'"),
    ("foot", "snip-foot-target",
     f"foot --title=snip-foot-target sh -c 'stty raw -echo; exec cat > {W}/foot.txt'"),
    ("vscode", "test.md",
     f"code --user-data-dir {W}/vscode --extensions-dir {W}/vscode-ext --new-window {W}/vscodews {W}/vscodews/test.md"),
]

def main():
    global MON, EXT
    only = set(sys.argv[1:])
    setup_dirs()
    MON = monitor()
    EXT = (round(MON["width"] / MON["scale"]), round(MON["height"] / MON["scale"]))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()

    app = subprocess.Popen([str(SPIKE / "build/snip-spike"), "--test"], stdout=open(SPIKE / "out/dnd-app.jsonl", "w"),
                           stderr=open(SPIKE / "out/dnd-app.err", "w"), env=dict(os.environ, QT_FORCE_STDERR_LOGGING="1"))
    time.sleep(1.5)
    prev_ws = json.loads(sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
    opened = []
    report = {}
    VARIANTS = os.environ.get("SNIP_VARIANTS", "uri png portal all").split()
    TERMFILE = {"ghostty": "term.txt", "kitty": "kitty.txt", "foot": "foot.txt"}
    try:
        for name, title, cmd in APPS:
            if only and name not in only:
                continue
            ws = f"name:snip-{name}"
            hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{ws}" }}))')
            hypr(f'hl.dispatch(hl.dsp.exec_cmd([[{cmd}]], {{ workspace = "{ws}" }}))')
            deadline = time.time() + 25
            c = None
            while time.time() < deadline and not c:
                time.sleep(0.5)
                c = find(title)
            if not c:
                print(f"{name}: window did not appear"); continue
            time.sleep(3)
            c = find(title)
            opened.append(c)
            print(f"{name}: at {c['at']} size {c['size']} xwayland={c.get('xwayland')}")
            tx, ty = center(c)
            if name == "vscode":   # aim at the editor area, not the sidebar
                tx = c["at"][0] + c["size"][0] * 0.65; ty = c["at"][1] + c["size"][1] * 0.35
            if name == "vscode":   # dismiss any first-run dialog
                for _ in range(2):
                    hypr(f'hl.dispatch(hl.dsp.send_shortcut({{ mods = "", key = "Escape", window = "address:{c["address"]}" }}))')
                    time.sleep(0.4)
            for v in VARIANTS:
                src = W / "src" / f"snip-{name}-{v}.png"
                shutil.copy(TEST_PNG, src)
                ctl(f"card {src}", f"variant {v}")
                time.sleep(0.7)
                before_dir = set(os.listdir(W / "snip-drop-target"))
                tf = W / TERMFILE.get(name, "term.txt")
                before_term = tf.read_text(errors="replace")
                shot = str(SPIKE / f"out/hover-{name}-{v}.png")
                drag_to(tx, ty, shot=shot)
                res = {"src_still_exists": src.exists()}
                if name in ("chromium", "zen"):
                    res["page"] = take_results(name, "drop")
                elif name == "nautilus":
                    res["new_files"] = sorted(set(os.listdir(W / "snip-drop-target")) - before_dir)
                elif name in TERMFILE:
                    res["typed"] = tf.read_text(errors="replace")[len(before_term):]
                elif name == "vscode":
                    res["title_after"] = (find("Visual Studio Code") or {}).get("title")
                    res["ws_files"] = sorted(os.listdir(W / "vscodews"))
                ok = bool(res.get("page") or res.get("new_files") or res.get("typed") or
                          (name == "vscode" and "snip-vscode" in str(res.get("title_after"))))
                res["ok"] = ok
                if ok:
                    os.remove(shot) if os.path.exists(shot) else None
                report[f"drop:{name}:{v}"] = res
                print(f"  drop {v:7} -> {json.dumps(res)[:260]}")
                if name == "vscode" and ok:
                    hypr(f'hl.dispatch(hl.dsp.send_shortcut({{ mods = "CTRL", key = "W", window = "address:{c["address"]}" }}))')
                    time.sleep(0.5)
            # paste: our clipboard offers image/png only
            ctl(f"copyfile {TEST_PNG}")
            time.sleep(0.4)
            before_dir = set(os.listdir(W / "snip-drop-target"))
            tf = W / TERMFILE.get(name, "term.txt")
            before_term = tf.read_text(errors="replace")
            if name == "vscode":   # click into the editor first
                move(tx, ty); time.sleep(0.2); ctl("down"); time.sleep(0.05); ctl("up"); time.sleep(0.3)
            send_paste(c, "CTRL SHIFT" if name in TERMFILE else "CTRL")
            res = {}
            if name in ("chromium", "zen"):
                res["page"] = take_results(name, "paste")
            elif name == "nautilus":
                res["new_files"] = sorted(set(os.listdir(W / "snip-drop-target")) - before_dir)
            elif name in TERMFILE:
                res["typed"] = tf.read_text(errors="replace")[len(before_term):]
            elif name == "vscode":
                res["ws_files"] = sorted(os.listdir(W / "vscodews"))
                res["md"] = (W / "vscodews/test.md").read_text()[:200]
            report[f"paste:{name}"] = res
            print(f"  paste   -> {json.dumps(res)[:260]}")
            # close this app's window before the next one
            if name in TERMFILE:
                for proc in subprocess.run(["pgrep", "-x", "cat"], capture_output=True, text=True).stdout.split():
                    try:
                        if str(W) in open(f"/proc/{proc}/cmdline").read():
                            os.kill(int(proc), 15)
                    except OSError:
                        pass
            else:
                hypr(f'hl.dispatch(hl.dsp.window.close({{ window = "address:{c["address"]}" }}))')
            time.sleep(1.0)
        (SPIKE / "out/dnd-matrix.json").write_text(json.dumps(report, indent=1))
    finally:
        for c in opened:
            if find(c["title"]):
                hypr(f'hl.dispatch(hl.dsp.window.close({{ window = "address:{c["address"]}" }}))')
        time.sleep(1)
        hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev_ws}" }}))')
        app.terminate()
        srv.shutdown()

if __name__ == "__main__":
    main()
