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

/// The Lua files of the Hyprland config, as text.
fn lua_texts(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .map(|d| d.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "lua")).filter_map(|e| std::fs::read_to_string(e.path()).ok()).collect())
        .unwrap_or_default()
}

/// Does a Lua file in the Hyprland config load Snip's keys?
pub fn keys_in_config(dir: &Path) -> bool {
    lua_texts(dir).iter().any(|t| t.contains("snip.lua"))
}

/// Does a Lua file in the Hyprland config give Snip the Print Screen keys
/// (`snip_print_keys = true`, outside a comment)?
pub fn print_keys_in_config(dir: &Path) -> bool {
    lua_texts(dir).iter().any(|t| t.lines().any(|l| l.split("--").next().unwrap_or("").replace(' ', "") == "snip_print_keys=true"))
}

/// Use these keys, in the Hyprland config at `dir`: hyprland.lua loads Snip's
/// `module` and, with `print`, gives it Print and Super+Ctrl+Print too (D66).
/// Only what the config doesn't do already goes in, at the end.
pub fn use_keys_in(dir: &Path, module: &Path, print: bool) -> Result<(), String> {
    let print = print && !print_keys_in_config(dir);
    let lines = match (keys_in_config(dir), print) {
        (false, _) => format!(
            "-- Snip's keys (added by Snip's welcome; delete these lines to undo)\n{}dofile({:?})\n",
            if print { "snip_print_keys = true\n" } else { "" },
            module.to_string_lossy()
        ),
        // the module, loaded already, binds its keys again with Print's
        (true, true) => "-- Print Screen for Snip (added by Snip's welcome; delete these lines to undo)\nsnip_print_keys = true\nsnip_bind_defaults()\n".to_string(),
        (true, false) => return Ok(()),
    };
    let file = dir.join("hyprland.lua");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let sep = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
    std::fs::write(&file, format!("{text}{sep}\n{lines}")).map_err(|e| e.to_string())
}

/// Use these keys: hyprland.lua loads Snip's module from now on (with Print
/// Screen's keys if `print`), and the keys work right away.
pub fn use_keys(print: bool) -> Result<(), String> {
    let module = keys_module().ok_or("Snip's Hyprland module wasn't found")?;
    use_keys_in(&hypr_dir(), &module, print)?;
    if print {
        crate::engine::hypr("print_keys", snip_hypr::run("snip_print_keys = true"));
    }
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
        "printKeys": print_keys_in_config(&hypr_dir()),
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
        let dir = config("once", &[("hyprland.lua", "require(\"hypr.autostart\")")]);
        use_keys_in(&dir, m, false).unwrap();
        let once = std::fs::read_to_string(dir.join("hyprland.lua")).unwrap();
        assert!(once.starts_with("require(\"hypr.autostart\")\n\n-- Snip's keys"), "{once}");
        assert!(once.ends_with("\ndofile(\"/usr/share/snip/hypr/snip.lua\")\n") && !once.contains("snip_print_keys"), "{once}");
        use_keys_in(&dir, m, false).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("hyprland.lua")).unwrap(), once, "never twice");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn config(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("snip-welcome-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (f, text) in files {
            std::fs::write(dir.join(f), text).unwrap();
        }
        dir
    }

    #[test]
    fn print_screen_comes_with_the_keys_when_ticked() {
        // D66: Snip's keys, with Print and Super+Ctrl+Print taken too
        let m = Path::new("/usr/share/snip/hypr/snip.lua");
        let dir = config("print", &[("hyprland.lua", "require(\"hypr.autostart\")\n")]);
        assert!(!print_keys_in_config(&dir));
        use_keys_in(&dir, m, true).unwrap();
        let text = std::fs::read_to_string(dir.join("hyprland.lua")).unwrap();
        assert!(text.starts_with("require(\"hypr.autostart\")\n"), "{text}");
        assert!(text.ends_with("snip_print_keys = true\ndofile(\"/usr/share/snip/hypr/snip.lua\")\n"), "the option before the module reads it: {text}");
        assert!(keys_in_config(&dir) && print_keys_in_config(&dir));
        use_keys_in(&dir, m, true).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("hyprland.lua")).unwrap(), text, "never twice");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_config_with_snips_keys_gains_print_screen_alone() {
        // the keys loaded from another file (as the owner's macos.lua does): only Print is added
        let m = Path::new("/usr/share/snip/hypr/snip.lua");
        let dir = config("loaded", &[("hyprland.lua", "require(\"hypr.macos\")\n"), ("macos.lua", "dofile(HOME .. \"/OpenShotX/hypr/snip.lua\")\n")]);
        use_keys_in(&dir, m, false).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("hyprland.lua")).unwrap(), "require(\"hypr.macos\")\n", "nothing to add");
        use_keys_in(&dir, m, true).unwrap();
        let text = std::fs::read_to_string(dir.join("hyprland.lua")).unwrap();
        assert!(text.ends_with("snip_print_keys = true\nsnip_bind_defaults()\n") && !text.contains("dofile"), "{text}");
        assert!(print_keys_in_config(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn print_screen_isnt_taken_by_a_comment() {
        // snip.lua's own header shows the option, commented out
        let dir = config("comment", &[("snip.lua", "--   snip_print_keys = true     -- also take PRINT\n")]);
        assert!(!print_keys_in_config(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
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
