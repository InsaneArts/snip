//! Hyprland IPC. One request per connection: Hyprland serves `.socket.sock`
//! synchronously, so a client that keeps it open can stall the compositor.
//! JSON is parsed tolerantly (`serde_json::Value`): hyprctl output changes
//! between versions and must never crash Snip.

pub mod keys;

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn socket_dir() -> Option<PathBuf> {
    let rt = std::env::var_os("XDG_RUNTIME_DIR")?;
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    Some(PathBuf::from(rt).join("hypr").join(sig))
}

pub fn available() -> bool {
    socket_dir().is_some_and(|d| d.join(".socket.sock").exists())
}

/// Raw request, e.g. "j/clients", "eval <lua>".
pub fn request(cmd: &str) -> std::io::Result<String> {
    let dir = socket_dir().ok_or_else(|| std::io::Error::other("not running under Hyprland"))?;
    let mut s = UnixStream::connect(dir.join(".socket.sock"))?;
    s.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    s.write_all(cmd.as_bytes())?;
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
pub fn resize_window(class: &str, title: &str, w: i32, h: i32) -> std::io::Result<String> {
    let c = window(class, title)?;
    eval(&format!(r#"hl.dispatch(hl.dsp.window.resize({{ x = {w}, y = {h}, window = "address:0x{:x}" }}))"#, c.address))
}

/// Pin one of our windows: above the others, on every workspace.
pub fn pin_window(class: &str, title: &str) -> std::io::Result<String> {
    let c = window(class, title)?;
    eval(&format!(r#"hl.dispatch(hl.dsp.window.pin({{ action = "enable", window = "address:0x{:x}" }}))"#, c.address))
}

/// Move one of our windows to (x, y) in layout px, or by (x, y) when
/// `relative`: a client can't place itself.
pub fn move_window(class: &str, title: &str, x: i32, y: i32, relative: bool) -> std::io::Result<String> {
    let c = window(class, title)?;
    eval(&format!(r#"hl.dispatch(hl.dsp.window.move({{ x = {x}, y = {y}, relative = {relative}, window = "address:0x{:x}" }}))"#, c.address))
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

/// Run Lua in Hyprland (0.56+ config API). Returns the reply ("ok" on success).
pub fn eval(lua: &str) -> std::io::Result<String> {
    request(&format!("eval {lua}"))
}

/// Runtime rules Snip needs whatever the user's config says: overlays must
/// appear instantly (no layer fade), which also keeps fading overlays out of
/// the first frames of a recording; the editor floats, centred; a pin floats
/// on every workspace, above fullscreen windows, with its own look. Re-applied
/// on every `configreloaded`.
pub fn install_runtime_rules() -> std::io::Result<String> {
    eval(
        r#"SNIP_LAYER_RULE = hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })
SNIP_EDITOR_RULE = hl.window_rule({ match = { title = "^Snip Editor.*" }, float = true, center = true, opacity = "1 1", tag = "+snip-editor" })
SNIP_VIDEO_EDITOR_RULE = hl.window_rule({ match = { title = "^Snip Video Editor.*" }, float = true, center = true, opacity = "1 1", tag = "+snip-video-editor" })
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
    fn address_parsing() {
        assert_eq!(super::parse_address("0x5a73050505e0"), Some(0x5a73050505e0));
        assert_eq!(super::parse_address("zz"), None);
    }
}
