//! First-run setup (OMA-02): Snip's keys in the Hyprland config, and start at
//! login. The Welcome window asks; nothing here runs on its own.

use std::path::{Path, PathBuf};

/// Snip's Hyprland module: the installed one, else this checkout's.
pub fn keys_module() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    [dir.join("../share/snip/hypr/snip.lua"), dir.join("../hypr/snip.lua")]
        .into_iter()
        .find(|p| p.exists())
        .map(|p| p.canonicalize().unwrap_or(p))
}

fn hypr_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(home).join(".config")).join("hypr")
}

/// Does a Lua file in the Hyprland config load Snip's keys?
pub fn keys_in_config(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "lua")).any(|e| std::fs::read_to_string(e.path()).is_ok_and(|t| t.contains("snip.lua"))))
        .unwrap_or(false)
}

/// hyprland.lua, loading Snip's keys at the end (once).
pub fn with_keys(text: &str, module: &Path) -> String {
    if text.contains("snip.lua") {
        return text.to_string();
    }
    let sep = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
    format!("{text}{sep}\n-- Snip's keys (added by Snip's welcome; delete these two lines to undo)\ndofile({:?})\n", module.to_string_lossy())
}

/// Use these keys: hyprland.lua loads Snip's module from now on, and the keys
/// work right away.
pub fn use_keys() -> Result<(), String> {
    let module = keys_module().ok_or("Snip's Hyprland module wasn't found")?;
    let file = hypr_dir().join("hyprland.lua");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::write(&file, with_keys(&text, &module)).map_err(|e| e.to_string())?;
    crate::engine::hypr("dofile", snip_hypr::dofile(&module));
    Ok(())
}

/// The systemd user unit running this Snip (snipd, or snip-dev from source).
pub fn unit() -> Option<String> {
    let cg = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    cg.trim().rsplit('/').next().filter(|u| u.ends_with(".service")).map(String::from)
}

/// Whether Snip starts at login (None: it isn't run by systemd).
pub fn autostart() -> Option<bool> {
    let out = std::process::Command::new("systemctl").args(["--user", "is-enabled", &unit()?]).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim() == "enabled")
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    let unit = unit().ok_or("Snip isn't run by systemd here")?;
    let ok = std::process::Command::new("systemctl").args(["--user", if on { "enable" } else { "disable" }, &unit]).status().is_ok_and(|s| s.success());
    if ok { Ok(()) } else { Err(format!("systemctl couldn't change {unit}")) }
}

/// Marks the welcome as seen; it opens by itself until then.
pub fn marker() -> PathBuf {
    snip_core::paths::data_dir().join("welcomed")
}

pub fn state() -> serde_json::Value {
    serde_json::json!({
        "keysInConfig": keys_in_config(&hypr_dir()),
        "module": keys_module(),
        "autostart": autostart(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_line_goes_in_once() {
        let m = Path::new("/usr/share/snip/hypr/snip.lua");
        let once = with_keys("require(\"hypr.autostart\")", m);
        assert!(once.starts_with("require(\"hypr.autostart\")\n\n-- Snip's keys"), "{once}");
        assert!(once.ends_with("dofile(\"/usr/share/snip/hypr/snip.lua\")\n"), "{once}");
        assert_eq!(with_keys(&once, m), once, "never twice");
        assert!(with_keys("dofile(\"/home/me/OpenShotX/hypr/snip.lua\")\n", m).matches("snip.lua").count() == 1, "a config that loads them already is left alone");
    }

    #[test]
    fn keys_are_found_in_any_hyprland_lua_file() {
        let dir = std::env::temp_dir().join(format!("snip-welcome-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("hyprland.lua"), "require(\"hypr.macos\")\n").unwrap();
        assert!(!keys_in_config(&dir));
        std::fs::write(dir.join("macos.lua"), "dofile(HOME .. \"/OpenShotX/hypr/snip.lua\")\n").unwrap();
        assert!(keys_in_config(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
