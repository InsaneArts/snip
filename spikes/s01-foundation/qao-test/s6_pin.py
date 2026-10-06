#!/usr/bin/env python3
"""S6: pinned screenshot window: always on top, over fullscreen, on all
workspaces, and click-through ("lock") via an empty input region."""
import http.server, json, os, subprocess, sys, threading, time
sys.path.insert(0, os.path.dirname(__file__))
import dnd_matrix as M

def app_events(name):
    return [json.loads(l) for l in open(M.SPIKE / "out/pin-app.jsonl") if f'"ev":"{name}"' in l]

def click_at(x, y):
    # Arrive with real motion: Hyprland re-evaluates pointer focus (and input
    # regions) on motion, so warping onto the same spot again keeps stale focus.
    for dx, dy in ((90, 70), (40, 30), (0, 0)):
        M.move(x + dx, y + dy); time.sleep(0.08)
    time.sleep(0.2); M.ctl("down"); time.sleep(0.06); M.ctl("up"); time.sleep(0.8)

def main():
    M.setup_dirs()
    M.MON = M.monitor(); M.EXT = (round(M.MON["width"] / M.MON["scale"]), round(M.MON["height"] / M.MON["scale"]))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", M.PORT), M.H)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    prev = json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
    M.hypr('SNIP_PIN_RULE = hl.window_rule({ match = { title = "^snip-pin$" }, float = true, pin = true, center = true, '
           'border_size = 0, no_shadow = true, no_blur = true, no_anim = true, no_dim = true, opacity = "1 1", tag = "-default-opacity" })')
    app = subprocess.Popen([str(M.SPIKE / "build/snip-spike"), "--test"], stdout=open(M.SPIKE / "out/pin-app.jsonl", "w"),
                           stderr=open(M.SPIKE / "out/pin-app.err", "w"), env=dict(os.environ, QT_FORCE_STDERR_LOGGING="1", WAYLAND_DEBUG=os.environ.get("WL_DEBUG", "")))
    chromium = None
    res = {}
    try:
        time.sleep(1.2)
        ws = "name:snip-pin"
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{ws}" }}))')
        M.hypr(f'hl.dispatch(hl.dsp.exec_cmd([[{M.APPS[0][2]}]], {{ workspace = "{ws}" }}))')
        for _ in range(40):
            time.sleep(0.5); chromium = M.find("SNIP-TEST chromium")
            if chromium: break
        time.sleep(2)
        M.ctl(f"pin {M.TEST_PNG}"); time.sleep(1.2)
        pin = M.find("snip-pin")
        res["pin_window"] = {k: pin.get(k) for k in ("at", "size", "floating", "pinned", "class")} if pin else None
        print("pin:", res["pin_window"])
        px, py = M.center(pin)
        M.take_results("chromium", "click"); M.take_results("chromium", "pointerdown")
        # 0) control: a click on the page away from the pin must reach the page
        cx, cy = chromium["at"][0] + 200, chromium["at"][1] + 300
        click_at(cx, cy)
        res["control_page_click"] = M.take_results("chromium", "click")
        # 1) normal mode: the pin takes the click
        click_at(px, py)
        res["normal"] = {"pin_clicked": len(app_events("pin_clicked")), "page_click": M.take_results("chromium", "click")}
        # 2) lock mode: clicks fall through to the page underneath
        M.ctl("pinlock on"); time.sleep(0.5)
        before = len(app_events("pin_clicked"))
        click_at(px, py)
        res["locked_toplevel_empty_region"] = {"pin_clicked_new": len(app_events("pin_clicked")) - before, "page_click": M.take_results("chromium", "click")}
        # 2b) lock = swap to a layer surface at the same spot with an empty input region
        M.ctl(f"pinlock on {pin['at'][0] - M.MON['x']} {pin['at'][1] - M.MON['y']}"); time.sleep(0.8)
        lay = [l for lv in json.loads(M.sh("hyprctl", "layers", "-j").stdout)["DP-2"]["levels"].values() for l in lv if l["namespace"] == "snip-pin-locked"]
        before = len(app_events("pin_clicked"))
        click_at(px, py)
        res["locked_layer"] = {"layer": bool(lay), "pin_clicked_new": len(app_events("pin_clicked")) - before,
                               "page_click": M.take_results("chromium", "click"),
                               "page_pointerdown": M.take_results("chromium", "pointerdown"),
                               "active_window": json.loads(M.sh("hyprctl", "activewindow", "-j").stdout).get("title")}
        M.ctl("pinlock off"); time.sleep(0.5)
        # 3) over a fullscreen window
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ window = "address:{chromium["address"]}" }}))')
        M.hypr(f'hl.dispatch(hl.dsp.window.fullscreen({{ window = "address:{chromium["address"]}" }}))'); time.sleep(1.2)
        M.sh("grim", "-o", "DP-2", "-s", "0.4", str(M.SPIKE / "out/pin-over-fullscreen.png"))
        res["fullscreen"] = {"chromium_fullscreen": (M.find("SNIP-TEST chromium") or {}).get("fullscreen")}
        M.hypr(f'hl.dispatch(hl.dsp.window.fullscreen({{ window = "address:{chromium["address"]}" }}))'); time.sleep(0.8)
        # 4) another workspace: pinned windows follow
        M.hypr('hl.dispatch(hl.dsp.focus({ workspace = "name:snip-pin-2" }))'); time.sleep(1.0)
        M.sh("grim", "-o", "DP-2", "-s", "0.4", str(M.SPIKE / "out/pin-other-ws.png"))
        p2 = M.find("snip-pin")
        res["other_workspace"] = {"pin_workspace": (p2 or {}).get("workspace"), "active": json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["name"]}
    finally:
        print(json.dumps(res, indent=1))
        app.terminate()
        if chromium:
            M.hypr(f'hl.dispatch(hl.dsp.window.close({{ window = "address:{chromium["address"]}" }}))')
        M.hypr('if SNIP_PIN_RULE then SNIP_PIN_RULE:set_enabled(false) end')
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')
        srv.shutdown()

main()
