//! Hyprland IPC. One request per connection: Hyprland serves `.socket.sock`
//! synchronously, so a client that keeps it open can stall the compositor.
//! JSON is parsed tolerantly (`serde_json::Value`): hyprctl output changes
//! between versions and must never crash Snip.

pub mod keys;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn socket_dir() -> Option<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(PathBuf::from(rt).join("hypr").join(sig))
}

pub fn available() -> bool {
    socket_dir().is_some_and(|d| d.join(".socket.sock").exists())
}

/// Raw request, e.g. "j/clients", "/eval <lua>".
pub fn request(cmd: &str) -> std::io::Result<String> {
    let dir = socket_dir().ok_or_else(|| std::io::Error::other("not running under Hyprland"))?;
    let mut s = UnixStream::connect(dir.join(".socket.sock"))?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    s.write_all(cmd.as_bytes())?;
    // Hyprland reads 1023 bytes at a time until a short read: a request of
    // exactly n×1023 bytes would block its main thread until we gave up
    s.shutdown(std::net::Shutdown::Write)?;
    let mut out = String::new();
    s.read_to_string(&mut out)?;
    Ok(out)
}

fn json(cmd: &str) -> serde_json::Value {
    request(cmd).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(serde_json::Value::Null)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
    pub focused: bool,
    pub active_workspace: i64,
    /// Space bars keep for themselves (logical px): left, top, right, bottom
    pub reserved: [i32; 4],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Client {
    pub address: u64,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub workspace: i64,
    pub monitor: i64,
    pub class: String,
    pub title: String,
    pub floating: bool,
    pub fullscreen: bool,
    pub hidden: bool,
    pub pinned: bool,
}

fn i(v: &serde_json::Value) -> i32 {
    v.as_i64().unwrap_or(0) as i32
}

pub fn parse_address(s: &str) -> Option<u64> {
    u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()
}

/// Whether the session is locked. Hyprland says so only through its
/// monitors: a lock screen blocks solitary rendering ("LOCK").
pub fn locked() -> bool {
    locked_in(&json("j/monitors"))
}

fn locked_in(monitors: &serde_json::Value) -> bool {
    monitors.as_array().is_some_and(|a| a.iter().any(|m| m["solitaryBlockedBy"].as_array().is_some_and(|b| b.iter().any(|r| r == "LOCK"))))
}

pub fn monitors() -> Vec<Monitor> {
    json("j/monitors")
        .as_array()
        .map(|a| {
            a.iter()
                .map(|m| Monitor {
                    name: m["name"].as_str().unwrap_or_default().into(),
                    x: i(&m["x"]),
                    y: i(&m["y"]),
                    width: i(&m["width"]),
                    height: i(&m["height"]),
                    scale: m["scale"].as_f64().unwrap_or(1.0),
                    focused: m["focused"].as_bool().unwrap_or(false),
                    active_workspace: m["activeWorkspace"]["id"].as_i64().unwrap_or(0),
                    reserved: [0, 1, 2, 3].map(|k| i(&m["reserved"][k])),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn clients() -> Vec<Client> {
    json("j/clients")
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| {
                    Some(Client {
                        address: parse_address(c["address"].as_str()?)?,
                        x: i(&c["at"][0]),
                        y: i(&c["at"][1]),
                        w: i(&c["size"][0]),
                        h: i(&c["size"][1]),
                        workspace: c["workspace"]["id"].as_i64().unwrap_or(0),
                        monitor: c["monitor"].as_i64().unwrap_or(-1),
                        class: c["class"].as_str().unwrap_or_default().into(),
                        title: c["title"].as_str().unwrap_or_default().into(),
                        floating: c["floating"].as_bool().unwrap_or(false),
                        fullscreen: c["fullscreen"].as_i64().unwrap_or(0) != 0,
                        hidden: c["hidden"].as_bool().unwrap_or(false),
                        pinned: c["pinned"].as_bool().unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Windows visible on the focused workspaces of all monitors, front-most
/// first (floating/pinned above tiled), for window-mode hit testing.
pub fn visible_windows() -> Vec<Client> {
    let active: Vec<i64> = monitors().iter().map(|m| m.active_workspace).collect();
    let mut v: Vec<Client> = clients()
        .into_iter()
        .filter(|c| !c.hidden && c.w > 0 && c.h > 0 && (c.pinned || active.contains(&c.workspace)))
        .collect();
    v.sort_by_key(|c| (!(c.pinned || c.floating), !c.fullscreen));
    v
}

/// One of our own windows: the one of class `class` with this exact title.
pub fn window(class: &str, title: &str) -> std::io::Result<Client> {
    clients().into_iter().find(|c| c.class == class && c.title == title).ok_or_else(|| std::io::Error::other("no such window"))
}

/// Resize one of our windows to `w` × `h` (logical px): Hyprland keeps a
/// floating window's size, so a client can't resize itself.
pub fn resize_window(class: &str, title: &str, w: i32, h: i32) -> Result<(), String> {
    let c = window(class, title).map_err(|e| e.to_string())?;
    run(&format!(r#"hl.dispatch(hl.dsp.window.resize({{ x = {w}, y = {h}, window = "address:0x{:x}" }}))"#, c.address))
}

/// Pin one of our windows: above the others, on every workspace.
pub fn pin_window(class: &str, title: &str) -> Result<(), String> {
    let c = window(class, title).map_err(|e| e.to_string())?;
    run(&format!(r#"hl.dispatch(hl.dsp.window.pin({{ action = "enable", window = "address:0x{:x}" }}))"#, c.address))
}

/// Move one of our windows to (x, y) in layout px, or by (x, y) when
/// `relative`: a client can't place itself.
pub fn move_window(class: &str, title: &str, x: i32, y: i32, relative: bool) -> Result<(), String> {
    let c = window(class, title).map_err(|e| e.to_string())?;
    run(&format!(r#"hl.dispatch(hl.dsp.window.move({{ x = {x}, y = {y}, relative = {relative}, window = "address:0x{:x}" }}))"#, c.address))
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let v = json("j/cursorpos");
    Some((v["x"].as_i64()? as i32, v["y"].as_i64()? as i32))
}

pub fn option_int(name: &str) -> Option<i64> {
    json(&format!("j/getoption {name}"))["int"].as_i64()
}

pub fn option_str(name: &str) -> Option<String> {
    json(&format!("j/getoption {name}"))["str"].as_str().map(String::from)
}

pub fn option_float(name: &str) -> Option<f64> {
    let v = json(&format!("j/getoption {name}"));
    v["float"].as_f64().or_else(|| v["int"].as_f64())
}

/// Run Lua in Hyprland (0.56+ config API): Ok when it answers "ok", else
/// what it said. A Lua error comes back as an ordinary reply, never as a
/// failed request, so every caller goes through here.
pub fn run(lua: &str) -> Result<(), String> {
    // the leading "/": no flags (hyprctl's form)
    ok_reply(request(&format!("/eval {lua}")))
}

fn ok_reply(r: std::io::Result<String>) -> Result<(), String> {
    match r {
        Ok(r) if r.trim() == "ok" => Ok(()),
        Ok(r) => Err(r.trim().to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Load a Lua file into Hyprland, as its config would (Snip's module).
pub fn dofile(path: &Path) -> Result<(), String> {
    run(&format!("dofile({:?})", path.to_string_lossy()))
}

/// What Snip has added to Hyprland's Lua that a config reload wipes
/// (Hyprland starts its Lua over): `reinstall` puts it back.
pub(crate) struct Footprint {
    /// the scrolling capture's Enter and Esc, for this app id
    pub(crate) session: Option<String>,
    /// the shortcut recorder's empty submap (SET-08)
    pub(crate) recorder: bool,
    /// a recording's hooks (REC-09/10): its id, keys, clicks; muted while paused
    pub(crate) hooks: Option<(String, bool, bool)>,
    pub(crate) muted: bool,
}

static FOOTPRINT: Mutex<Footprint> = Mutex::new(Footprint { session: None, recorder: false, hooks: None, muted: false });

pub(crate) fn footprint(f: impl FnOnce(&mut Footprint)) {
    f(&mut FOOTPRINT.lock().unwrap_or_else(|e| e.into_inner()));
}

/// After Hyprland reloaded its config (`configreloaded`): Snip's rules, and
/// whatever of its keys and hooks was on. Every part is tried; the first
/// error is returned.
pub fn reinstall() -> Result<(), String> {
    let (session, recorder, hooks, muted) = {
        let f = FOOTPRINT.lock().unwrap_or_else(|e| e.into_inner());
        (f.session.clone(), f.recorder, f.hooks.clone(), f.muted)
    };
    let mut done = install_runtime_rules();
    if let Some(app) = session {
        done = done.and(run(&keys::session_lua(&app, true)));
    }
    if recorder {
        done = done.and(run(keys::record_lua(true)));
    }
    if let Some((sid, keys_on, clicks)) = hooks {
        done = done.and(run(&keys::recording_lua(&sid, keys_on, clicks)));
        if muted {
            done = done.and(run(&keys::recording_mute_lua(true)));
        }
    }
    done
}

/// Snip's rules, its only copy: overlays appear instantly (no layer fade),
/// which also keeps fading overlays out of the first frames of a recording;
/// the editors, Settings, the Welcome and Quick Look float, centred; a pin
/// floats on every workspace, above fullscreen windows, with its own look.
/// Applied at start and on every `configreloaded` (`reinstall`).
///
/// Hyprland 0.56: a title pattern matches the whole title ("Snip Editor —
/// <name>" needs `.*`); a rule matching class and title together never
/// matches, nor does a pattern with non-ASCII characters; and when one rule
/// moves a window, `float` from another rule is dropped. So: one ASCII field
/// per rule, everything a window needs in one rule.
pub fn install_runtime_rules() -> Result<(), String> {
    run(
        r#"SNIP_LAYER_RULE = hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })
SNIP_EDITOR_RULE = hl.window_rule({ match = { title = "^Snip Editor.*" }, float = true, center = true, opacity = "1 1", tag = "+snip-editor" })
SNIP_VIDEO_EDITOR_RULE = hl.window_rule({ match = { title = "^Snip Video Editor.*" }, float = true, center = true, opacity = "1 1", tag = "+snip-video-editor" })
SNIP_QUICKLOOK_RULE = hl.window_rule({ match = { title = "^Snip Quick Look$" }, float = true, center = true, opacity = "1 1", tag = "+snip-quicklook" })
SNIP_SETTINGS_RULE = hl.window_rule({ match = { title = "^Snip Settings$" }, float = true, center = true, opacity = "1 1", tag = "+snip-settings" })
SNIP_WELCOME_RULE = hl.window_rule({ match = { title = "^Welcome to Snip$" }, float = true, center = true, opacity = "1 1", tag = "+snip-welcome" })
SNIP_PIN_RULE = hl.window_rule({ match = { title = "^Snip Pin .*" }, float = true, pin = true, center = true, border_size = 0, no_shadow = true, no_blur = true, no_dim = true, no_anim = true, rounding = 0, opacity = "1 override 1 override", suppress_event = "maximize fullscreen" })"#,
    )
}

/// socket2 events: `name>>data` lines. Runs `f` on a background thread until
/// it returns false or the socket closes.
pub fn subscribe(mut f: impl FnMut(&str, &str) -> bool + Send + 'static) -> std::io::Result<()> {
    let dir = socket_dir().ok_or_else(|| std::io::Error::other("not running under Hyprland"))?;
    let s = UnixStream::connect(dir.join(".socket2.sock"))?;
    std::thread::Builder::new().name("snip-hypr-events".into()).spawn(move || {
        for line in BufReader::new(s).lines().map_while(Result::ok) {
            let (name, data) = line.split_once(">>").unwrap_or((line.as_str(), ""));
            if !f(name, data) {
                break;
            }
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_ok_is_success() {
        assert_eq!(super::ok_reply(Ok("ok".into())), Ok(()));
        assert_eq!(super::ok_reply(Ok("ok\n".into())), Ok(()));
        // a Lua error is an ordinary reply
        let lua = "[string \"eval\"]:1: attempt to call a nil value (field 'nope')";
        assert_eq!(super::ok_reply(Ok(lua.into())), Err(lua.to_string()));
        assert_eq!(super::ok_reply(Err(std::io::Error::other("no socket"))), Err("no socket".into()));
    }

    #[test]
    fn address_parsing() {
        assert_eq!(super::parse_address("0x5a73050505e0"), Some(0x5a73050505e0));
        assert_eq!(super::parse_address("zz"), None);
    }

    #[test]
    fn a_lock_screen_blocks_solitary_rendering() {
        let unlocked = serde_json::json!([{ "name": "DP-2", "solitaryBlockedBy": ["WINDOWED", "CANDIDATE"] }]);
        let locked = serde_json::json!([{ "name": "DP-2", "solitaryBlockedBy": ["LOCK", "WINDOWED"] }]);
        assert!(!super::locked_in(&unlocked) && super::locked_in(&locked) && !super::locked_in(&serde_json::Value::Null));
    }
}
