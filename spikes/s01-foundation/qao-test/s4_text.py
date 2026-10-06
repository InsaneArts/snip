#!/usr/bin/env python3
"""S4: text entry into QML TextEdit on an xdg toplevel and on an exclusive
layer surface: Latin, Unicode via virtual keyboard, Georgian via xkb layout."""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(__file__)); import dnd_matrix as M
SP = M.SPIKE
KB = "all"

def last_text():
    t = [json.loads(l)["text"] for l in open(SP / "out/s4.jsonl") if '"ev":"text"' in l]
    return t[-1] if t else ""

def preedits():
    return [json.loads(l)["text"] for l in open(SP / "out/s4.jsonl") if '"ev":"preedit"' in l]

def run(target):
    res = {}
    prev = json.loads(M.sh("hyprctl", "activeworkspace", "-j").stdout)["id"]
    M.hypr('hl.dispatch(hl.dsp.focus({ workspace = "name:snip-text" }))')
    M.hypr('SNIP_TEXT_RULE = hl.window_rule({ match = { title = "^snip-text$" }, float = true, center = true, no_anim = true })')
    app = subprocess.Popen([str(SP / "build/snip-spike"), "--textinput", target], stdout=open(SP / "out/s4.jsonl", "w"),
                           stderr=open(SP / "out/s4.err", "w"), env=dict(os.environ, QT_FORCE_STDERR_LOGGING="1", **({"QT_IM_MODULE": os.environ["IMMOD"]} if "IMMOD" in os.environ else {})))
    try:
        time.sleep(1.8)
        if target == "toplevel":
            w = M.find("snip-text")
            M.hypr(f'hl.dispatch(hl.dsp.focus({{ window = "address:{w["address"]}" }}))'); time.sleep(0.3)
        # 1) Latin
        M.sh("wtype", "Hello Snip 123"); time.sleep(0.4)
        res["latin"] = last_text()
        # 2) Unicode through the virtual keyboard (Georgian, symbols, emoji)
        M.sh("wtype", " ქართული ✓ é 😀"); time.sleep(0.5)
        res["unicode"] = last_text()
        # 3) real layout switch to Georgian, keys sent by Hyprland (physical-key path)
        before = last_text()
        M.sh("hyprctl", "switchxkblayout", KB, "1"); time.sleep(0.3)
        active = json.loads(M.sh("hyprctl", "devices", "-j").stdout)["keyboards"]
        layout = [k["active_keymap"] for k in active if k.get("main")]
        for key in ("SPACE", "Georgian_an", "Georgian_ban", "Georgian_gan", "Georgian_don"):
            M.hypr(f'hl.dispatch(hl.dsp.send_shortcut({{ mods = "", key = "{key}" }}))'); time.sleep(0.12)
        time.sleep(0.4)
        res["ge_layout"] = {"active_keymap": layout, "typed": last_text()[len(before):]}
        res["preedit_events"] = preedits()[:5]
    finally:
        M.sh("hyprctl", "switchxkblayout", KB, "0")
        app.terminate()
        M.hypr('if SNIP_TEXT_RULE then SNIP_TEXT_RULE:set_enabled(false) end')
        M.hypr(f'hl.dispatch(hl.dsp.focus({{ workspace = "{prev}" }}))')
    return res

out = {t: run(t) for t in ("toplevel", "layer")}
print(json.dumps(out, ensure_ascii=False, indent=1))
