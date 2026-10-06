#!/usr/bin/env python3
"""R18: input routing with exclusive-keyboard overlays on two monitors.
Drag a selection on the *second* monitor; the stored capture must be the
right size from the right output, and Esc must still close the overlay."""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__)); import dnd_matrix as M
SP = M.SPIKE
NAME = "SNIPR18"

def mon(name):
    return next(m for m in json.loads(M.sh("hyprctl", "monitors", "-j").stdout) if m["name"] == name)

def ctl_move(output, x, y, m):
    M.ctl(f"move {output} {int(x)} {int(y)} {round(m['width'] / m['scale'])} {round(m['height'] / m['scale'])}")

def events(name):
    return [json.loads(l) for l in open(SP / "out/r18.jsonl") if f'"ev":"{name}"' in l]

res = {}
M.sh("hyprctl", "output", "create", "headless", NAME); time.sleep(0.4)
M.hypr(f'hl.monitor({{ output = "{NAME}", mode = "1920x1080@60", position = "auto", scale = 1.5 }})'); time.sleep(0.8)
app = subprocess.Popen([str(SP / "build/snip-spike"), "--test"], stdout=open(SP / "out/r18.jsonl", "w"), stderr=subprocess.DEVNULL,
                       env=dict(os.environ, QT_FORCE_STDERR_LOGGING="1"))
try:
    time.sleep(1.5)
    m2 = mon(NAME)
    res["second_monitor"] = {k: m2[k] for k in ("x", "y", "width", "height", "scale")}
    # put the cursor on the virtual monitor first, then open the overlays (trigger = all outputs)
    ctl_move(NAME, 100, 100, m2); time.sleep(0.2)
    M.ctl("pin /dev/null") if False else None
    subprocess.run(["python3", "-c", "import socket,os;s=socket.socket(socket.AF_UNIX);s.connect(os.environ['XDG_RUNTIME_DIR']+'/snip-spike.sock');s.sendall(b'ping\\n')"])
    # trigger an area capture without the hotkey: the test instance has no shortcut, so use the overlay's own flow
    M.ctl("trigger") if False else None
    # SnipCore.trigger() is not on the control socket yet -> send via a tiny helper command
    r = M.ctl("trigger-area")
    time.sleep(0.8)
    shown = events("qml_show")
    res["overlay_shown"] = bool(shown)
    # drag a 400x200 logical selection on the second monitor (logical coords within that output)
    ctl_move(NAME, 300, 200, m2); time.sleep(0.15)
    M.ctl("down"); time.sleep(0.05)
    for i in range(1, 11):
        ctl_move(NAME, 300 + 40 * i, 200 + 20 * i, m2); time.sleep(0.03)
    time.sleep(0.15); M.ctl("up"); time.sleep(1.2)
    stored = events("area_stored")
    res["area_stored"] = stored[-1] if stored else None
    res["hidden_after_release"] = bool(events("qml_hide"))
    # Esc routing: open again, press Esc via the compositor -> overlay must close
    before = len(events("qml_hide"))
    M.ctl("trigger-area"); time.sleep(0.8)
    M.hypr('hl.dispatch(hl.dsp.send_shortcut({ mods = "", key = "Escape" }))'); time.sleep(0.6)
    res["esc_closed"] = len(events("qml_hide")) > before
finally:
    app.terminate()
    M.sh("hyprctl", "output", "remove", NAME)
print(json.dumps(res, indent=1))
