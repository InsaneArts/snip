//! Minimal Hyprland IPC: one request per connection (never keep .socket.sock open).

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

fn request(cmd: &str) -> Option<String> {
    let rt = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
    let mut s = UnixStream::connect(format!("{rt}/hypr/{sig}/.socket.sock")).ok()?;
    s.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    s.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Name of the focused monitor (where the cursor/keyboard focus is).
pub fn focused_monitor() -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(&request("j/monitors")?).ok()?;
    json.as_array()?
        .iter()
        .find(|m| m["focused"].as_bool() == Some(true))
        .and_then(|m| m["name"].as_str().map(str::to_owned))
}
