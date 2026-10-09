//! XDG locations. Snip never writes into the user's folders except on Save
//! (product §5.0); everything else lives in Snip's own XDG directories.

use std::path::{Path, PathBuf};

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn env_dir(var: &str, fallback: impl FnOnce() -> PathBuf) -> PathBuf {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(fallback)
}

pub fn config_dir() -> PathBuf {
    env_dir("XDG_CONFIG_HOME", || home().join(".config")).join("snip")
}

pub fn data_dir() -> PathBuf {
    env_dir("XDG_DATA_HOME", || home().join(".local/share")).join("snip")
}

pub fn cache_dir() -> PathBuf {
    env_dir("XDG_CACHE_HOME", || home().join(".cache")).join("snip")
}

/// `SNIP_RUNTIME_DIR` lets a test instance run next to the user's daemon
/// (XDG_RUNTIME_DIR itself can't move: the Wayland socket lives there).
pub fn runtime_dir() -> PathBuf {
    env_dir("SNIP_RUNTIME_DIR", || env_dir("XDG_RUNTIME_DIR", std::env::temp_dir).join("snip"))
}

pub fn ctl_socket() -> PathBuf {
    runtime_dir().join("ctl.sock")
}

pub fn history_dir() -> PathBuf {
    data_dir().join("history")
}

pub fn drag_dir() -> PathBuf {
    cache_dir().join("drag")
}

/// Expand a leading `~` and `$HOME`.
pub fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        return home().join(rest);
    }
    if p == "~" {
        return home();
    }
    PathBuf::from(p.replace("$HOME", &home().to_string_lossy()))
}

/// XDG user dir (e.g. "PICTURES") from ~/.config/user-dirs.dirs.
pub fn user_dir(name: &str) -> Option<PathBuf> {
    let file = env_dir("XDG_CONFIG_HOME", || home().join(".config")).join("user-dirs.dirs");
    let text = std::fs::read_to_string(file).ok()?;
    let key = format!("XDG_{name}_DIR=");
    text.lines().find_map(|l| {
        let v = l.trim().strip_prefix(&key)?.trim().trim_matches('"');
        Some(expand(v))
    })
}

pub fn default_screenshot_dir() -> PathBuf {
    user_dir("PICTURES").unwrap_or_else(|| home().join("Pictures")).join("Screenshots")
}

pub fn default_recording_dir() -> PathBuf {
    user_dir("VIDEOS").unwrap_or_else(|| home().join("Videos")).join("Screencasts")
}

/// Create a directory (and parents) with mode 0700 for private Snip state.
pub fn ensure_private_dir(p: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home() {
        let h = home();
        assert_eq!(expand("~/Pictures"), h.join("Pictures"));
        assert_eq!(expand("$HOME/x"), h.join("x"));
        assert_eq!(expand("/abs"), PathBuf::from("/abs"));
    }
}
