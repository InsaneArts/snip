//! Snip's key bindings (SET-08): the actions, the binds Hyprland has for them,
//! names for keys, and the Lua that changes them. A Lua config's binds all
//! dispatch through `__lua`, so `hyprctl binds` can't say where a bind goes:
//! Snip's binds are known by their description, "Snip: <action's name>".

/// Every action a key can trigger (`hl.dsp.global("<app>:<id>")`), with its
/// name: the global shortcut's description, the bind's and the Settings row's.
pub const ACTIONS: &[(&str, &str)] = &[
    ("area", "Capture area"),
    ("fullscreen", "Capture fullscreen"),
    ("window", "Capture window"),
    ("previous", "Capture previous area"),
    ("all-in-one", "All-In-One"),
    ("text", "Capture text (OCR)"),
    ("area-pin", "Capture area and pin it"),
    ("scroll", "Scrolling capture"),
    ("timer-area", "Self-timer: area"),
    ("timer-fullscreen", "Self-timer: fullscreen"),
    ("restore", "Restore the last closed capture"),
    ("close-all", "Close all Quick Access cards"),
    ("history", "Open capture history"),
    ("edit-clipboard", "Edit the image on the clipboard"),
    ("pins-toggle", "Hide or show all pins"),
    ("pins-close", "Close all pins"),
    ("pins-unlock", "Unlock all pins"),
    ("settings", "Open Snip's settings"),
];

/// Shortcuts a session binds for its length (Enter, Esc), not set in Settings.
pub const SESSION_KEYS: &[(&str, &str)] = &[("scroll-go", "Scrolling capture: start or finish"), ("scroll-cancel", "Scrolling capture: cancel")];

/// Lua binding Enter and Esc to a scrolling capture while it runs, or
/// unbinding them. A bind handle is removed exactly once (removing one twice
/// crashes Hyprland 0.56): the table is cleared before its handles go. A
/// timer unbinds them if Snip never does.
pub fn session_lua(app_id: &str, on: bool) -> String {
    const OFF: &str = "if SNIP_SESSION_KEYS then local k = SNIP_SESSION_KEYS; SNIP_SESSION_KEYS = nil; for _, b in ipairs(k) do b:remove() end end";
    if !on {
        return OFF.into();
    }
    format!(
        r#"if not SNIP_SESSION_KEYS then
  SNIP_SESSION_KEYS = {{ hl.bind("Return", hl.dsp.global("{app_id}:scroll-go")), hl.bind("Escape", hl.dsp.global("{app_id}:scroll-cancel")) }}
  hl.timer(function() {OFF} end, {{ timeout = 900000, type = "oneshot" }})
end"#
    )
}

/// The description Snip's binds for `action` carry.
pub fn description(action: &str) -> Option<String> {
    ACTIONS.iter().find(|(id, _)| *id == action).map(|(_, name)| format!("Snip: {name}"))
}

/// One bind, as `hyprctl binds` lists it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bind {
    pub modmask: u32,
    /// a keysym name ("Print") or "code:N"
    pub key: String,
    pub description: String,
    pub submap: String,
}

impl Bind {
    /// What `hl.unbind` takes: "SUPER + SHIFT + code:13".
    pub fn spec(&self) -> String {
        let mut parts: Vec<&str> = MODS.iter().filter(|(bit, _, _)| self.modmask & bit != 0).map(|(_, name, _)| *name).collect();
        parts.push(&self.key);
        parts.join(" + ")
    }
}

/// `hyprctl binds` (text: its JSON leaves out the key of a `code:` bind).
/// A `code:` bind prints its whole spec as the key, a keysym bind just the key.
pub fn parse_binds(text: &str) -> Vec<Bind> {
    let mut out = Vec::new();
    for block in text.split("\n\n") {
        let mut b = Bind::default();
        let mut any = false;
        for line in block.lines() {
            let Some((k, v)) = line.trim().split_once(':') else { continue };
            let v = v.trim();
            match k {
                "modmask" => {
                    b.modmask = v.parse().unwrap_or(0);
                    any = true;
                }
                "key" => b.key = v.rsplit(" + ").next().unwrap_or(v).trim().to_string(),
                "description" => b.description = v.to_string(),
                "submap" => b.submap = v.to_string(),
                _ => {}
            }
        }
        if any {
            out.push(b);
        }
    }
    out
}

pub fn binds() -> Vec<Bind> {
    crate::request("binds").map(|t| parse_binds(&t)).unwrap_or_default()
}

const MODS: &[(u32, &str, &str)] = &[(64, "SUPER", "Super"), (4, "CTRL", "Ctrl"), (8, "ALT", "Alt"), (1, "SHIFT", "Shift")];

/// Keys by xkb keycode (US positions): keysym name, label.
const KEYS: &[(u32, &str, &str)] = &[
    (9, "Escape", "Esc"),
    (10, "1", "1"),
    (11, "2", "2"),
    (12, "3", "3"),
    (13, "4", "4"),
    (14, "5", "5"),
    (15, "6", "6"),
    (16, "7", "7"),
    (17, "8", "8"),
    (18, "9", "9"),
    (19, "0", "0"),
    (20, "minus", "-"),
    (21, "equal", "="),
    (22, "BackSpace", "Backspace"),
    (23, "Tab", "Tab"),
    (24, "q", "Q"),
    (25, "w", "W"),
    (26, "e", "E"),
    (27, "r", "R"),
    (28, "t", "T"),
    (29, "y", "Y"),
    (30, "u", "U"),
    (31, "i", "I"),
    (32, "o", "O"),
    (33, "p", "P"),
    (34, "bracketleft", "["),
    (35, "bracketright", "]"),
    (36, "Return", "Enter"),
    (38, "a", "A"),
    (39, "s", "S"),
    (40, "d", "D"),
    (41, "f", "F"),
    (42, "g", "G"),
    (43, "h", "H"),
    (44, "j", "J"),
    (45, "k", "K"),
    (46, "l", "L"),
    (47, "semicolon", ";"),
    (48, "apostrophe", "'"),
    (49, "grave", "`"),
    (51, "backslash", "\\"),
    (52, "z", "Z"),
    (53, "x", "X"),
    (54, "c", "C"),
    (55, "v", "V"),
    (56, "b", "B"),
    (57, "n", "N"),
    (58, "m", "M"),
    (59, "comma", ","),
    (60, "period", "."),
    (61, "slash", "/"),
    (65, "space", "Space"),
    (67, "F1", "F1"),
    (68, "F2", "F2"),
    (69, "F3", "F3"),
    (70, "F4", "F4"),
    (71, "F5", "F5"),
    (72, "F6", "F6"),
    (73, "F7", "F7"),
    (74, "F8", "F8"),
    (75, "F9", "F9"),
    (76, "F10", "F10"),
    (95, "F11", "F11"),
    (96, "F12", "F12"),
    (107, "Print", "Print"),
    (110, "Home", "Home"),
    (111, "Up", "↑"),
    (112, "Prior", "Page Up"),
    (113, "Left", "←"),
    (114, "Right", "→"),
    (115, "End", "End"),
    (116, "Down", "↓"),
    (117, "Next", "Page Down"),
    (118, "Insert", "Insert"),
    (119, "Delete", "Delete"),
    (127, "Pause", "Pause"),
];

/// "SUPER + SHIFT + code:13" → (modmask, key)
pub fn parse_spec(spec: &str) -> (u32, String) {
    let mut mask = 0;
    let mut key = String::new();
    for part in spec.split('+').map(str::trim).filter(|p| !p.is_empty()) {
        match MODS.iter().find(|(_, name, _)| name.eq_ignore_ascii_case(part)).or_else(|| (part.eq_ignore_ascii_case("CONTROL")).then_some(&MODS[1])) {
            Some((bit, _, _)) => mask |= bit,
            None => key = part.to_string(),
        }
    }
    (mask, key)
}

/// The keycode a key means ("code:13", or a keysym on the US layout).
fn keycode(key: &str) -> Option<u32> {
    match key.strip_prefix("code:") {
        Some(n) => n.parse().ok(),
        None => KEYS.iter().find(|(_, sym, _)| sym.eq_ignore_ascii_case(key)).map(|(c, _, _)| *c),
    }
}

/// How a key looks in Settings: "Super+Shift+4".
pub fn label(modmask: u32, key: &str) -> String {
    let name = match keycode(key).and_then(|c| KEYS.iter().find(|(k, _, _)| *k == c)) {
        Some((_, _, label)) => label.to_string(),
        None => key.to_string(),
    };
    let mut parts: Vec<String> = MODS.iter().filter(|(bit, _, _)| modmask & bit != 0).map(|(_, _, l)| l.to_string()).collect();
    parts.push(name);
    parts.join("+")
}

fn same_key(a: &str, b: &str) -> bool {
    match (keycode(a), keycode(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// Another bind on `spec` (not one of `action`'s own): what recording it
/// would take over.
pub fn conflict<'a>(binds: &'a [Bind], action: &str, spec: &str) -> Option<&'a Bind> {
    let own = description(action);
    let (mask, key) = parse_spec(spec);
    binds.iter().find(|b| b.submap.is_empty() && Some(&b.description) != own.as_ref() && b.modmask == mask && same_key(&b.key, &key))
}

/// Lua giving actions their keys: `(action, its binds now, its key or "")`.
/// Every old bind goes before any new one comes, so two actions can swap keys.
pub fn set_lua(app_id: &str, changes: &[(String, Vec<String>, String)]) -> String {
    let mut lua = String::new();
    for (_, old, new) in changes {
        for s in old.iter().chain(std::iter::once(new).filter(|n| !n.is_empty())) {
            lua += &format!("pcall(hl.unbind, {s:?})\n");
        }
    }
    for (action, _, new) in changes {
        if let (Some(d), false) = (description(action), new.is_empty()) {
            lua += &format!("hl.bind({new:?}, hl.dsp.global({:?}), {{ description = {d:?} }})\n", format!("{app_id}:{action}"));
        }
    }
    lua
}

/// While a key is recorded, Hyprland's binds must let it through to the
/// Settings window: an empty submap (Escape still leaves it, and a timer does
/// if Snip never comes back).
pub fn record_lua(on: bool) -> &'static str {
    if on {
        r#"if not SNIP_RECORD_SUBMAP then
  hl.define_submap("snip-record", function() hl.bind("Escape", hl.dsp.submap("reset"), { non_consuming = true }) end)
  SNIP_RECORD_SUBMAP = true
end
hl.dispatch(hl.dsp.submap("snip-record"))
hl.timer(function() if hl.get_current_submap() == "snip-record" then hl.dispatch(hl.dsp.submap("reset")) end end, { timeout = 20000, type = "oneshot" })"#
    } else {
        r#"if hl.get_current_submap() == "snip-record" then hl.dispatch(hl.dsp.submap("reset")) end"#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BINDS: &str = "bindd\n\tmodmask: 65\n\tsubmap: \n\tkey: SUPER + SHIFT + code:13\n\tkeycode: 0\n\tcatchall: false\n\tdescription: Snip: Capture area\n\tdispatcher: __lua\n\targ: 291\n\n\
bindd\n\tmodmask: 65\n\tsubmap: \n\tkey: SUPER + SHIFT + code:12\n\tkeycode: 0\n\tcatchall: false\n\tdescription: Move window to workspace 3\n\tdispatcher: __lua\n\targ: 85\n\n\
bindd\n\tmodmask: 64\n\tsubmap: \n\tkey: RETURN\n\tkeycode: 0\n\tcatchall: false\n\tdescription: Terminal\n\tdispatcher: __lua\n\targ: 20\n\n\
bind\n\tmodmask: 0\n\tsubmap: snip-record\n\tkey: Escape\n\tkeycode: 0\n\tcatchall: false\n\tdescription: \n\tdispatcher: __lua\n\targ: 400\n";

    #[test]
    fn reads_hyprctl_binds() {
        let b = parse_binds(BINDS);
        assert_eq!(b.len(), 4);
        assert_eq!(b[0], Bind { modmask: 65, key: "code:13".into(), description: "Snip: Capture area".into(), submap: String::new() });
        assert_eq!(b[0].spec(), "SUPER + SHIFT + code:13");
        assert_eq!(b[2].spec(), "SUPER + RETURN");
        assert_eq!(b[3].submap, "snip-record");
    }

    #[test]
    fn labels_keys() {
        assert_eq!(label(65, "code:13"), "Super+Shift+4");
        assert_eq!(label(64, "RETURN"), "Super+Enter");
        assert_eq!(label(0, "Print"), "Print");
        assert_eq!(label(76, "code:107"), "Super+Ctrl+Alt+Print");
        assert_eq!(parse_spec("SUPER + CTRL + PRINT"), (68, "PRINT".into()));
    }

    #[test]
    fn finds_what_a_key_takes_over() {
        let b = parse_binds(BINDS);
        let c = |action, spec| conflict(&b, action, spec).map(|b| b.description.clone());
        assert_eq!(c("history", "SUPER + SHIFT + code:12").as_deref(), Some("Move window to workspace 3"));
        assert_eq!(c("history", "SUPER + code:36").as_deref(), Some("Terminal"), "a keycode meets a keysym");
        assert_eq!(c("area", "SUPER + SHIFT + code:13"), None, "its own key is no conflict");
        assert_eq!(c("history", "SUPER + SHIFT + code:13").as_deref(), Some("Snip: Capture area"));
        assert_eq!(c("history", "Escape"), None, "a submap's binds don't count");
        assert_eq!(c("history", "SUPER + ALT + code:13"), None);
    }

    #[test]
    fn session_keys_are_removed_once() {
        let on = session_lua("snip", true);
        assert!(on.starts_with("if not SNIP_SESSION_KEYS then"), "never bound twice");
        assert!(on.contains(r#"hl.bind("Return", hl.dsp.global("snip:scroll-go"))"#), "{on}");
        let off = session_lua("", false);
        assert!(off.find("SNIP_SESSION_KEYS = nil").unwrap() < off.find("b:remove()").unwrap(), "cleared before the handles go: {off}");
        assert!(on.contains(&off), "the timer unbinds the same way");
    }

    #[test]
    fn swapping_keys_unbinds_before_binding() {
        let lua = set_lua(
            "snip",
            &[
                ("area".into(), vec!["SUPER + SHIFT + code:13".into()], "SUPER + SHIFT + code:12".into()),
                ("fullscreen".into(), vec!["SUPER + SHIFT + code:12".into()], "SUPER + SHIFT + code:13".into()),
            ],
        );
        let first_bind = lua.find("hl.bind").unwrap();
        assert!(lua.rfind("hl.unbind").unwrap() < first_bind, "{lua}");
        assert!(lua.contains(r#"hl.bind("SUPER + SHIFT + code:12", hl.dsp.global("snip:area"), { description = "Snip: Capture area" })"#), "{lua}");
        assert!(!set_lua("snip", &[("history".into(), vec!["F1".into()], String::new())]).contains("hl.bind("), "a cleared key binds nothing");
    }
}
