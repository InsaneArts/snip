//! Hyprland IPC. One request per connection: Hyprland serves `.socket.sock`
//! synchronously, so a client that keeps it open can stall the compositor.
//! JSON is parsed tolerantly (`serde_json::Value`): hyprctl output changes
//! between versions and must never crash Snip.

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

pub fn cursor_pos() -> Option<(i32, i32)> {
    let v = json("j/cursorpos");
    Some((v["x"].as_i64()? as i32, v["y"].as_i64()? as i32))
}

pub fn option_int(name: &str) -> Option<i64> {
    json(&format!("j/getoption {name}"))["int"].as_i64()
}

/// Run Lua in Hyprland (0.56+ config API). Returns the reply ("ok" on success).
pub fn eval(lua: &str) -> std::io::Result<String> {
    request(&format!("eval {lua}"))
}

/// Runtime rules Snip needs whatever the user's config says: overlays must
/// appear instantly (no layer fade), which also keeps fading overlays out of
/// the first frames of a recording. Re-applied on every `configreloaded`.
pub fn install_runtime_rules() -> std::io::Result<String> {
    eval(r#"SNIP_LAYER_RULE = hl.layer_rule({ match = { namespace = "^snip-" }, no_anim = true, animation = "none" })"#)
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
