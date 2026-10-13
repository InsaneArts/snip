//! Omarchy themes -> Snip design tokens (design doc §3; verified in spike S03).
//! Palette comes from Omarchy's own resolver (`omarchy-theme-color --all`) so
//! Snip matches every other themed app; semantics are picked by hue because
//! Omarchy's `red`/`green` are ANSI-slot based (Aetheria: red = #c8e967 lime).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug)]
struct Rgb(f64, f64, f64);

fn parse_hex(s: &str) -> Option<Rgb> {
    let h = s.trim().trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok().map(|v| v as f64 / 255.0);
    Some(Rgb(p(0)?, p(2)?, p(4)?))
}

fn hex(c: Rgb) -> String {
    let f = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", f(c.0), f(c.1), f(c.2))
}

fn lum(c: Rgb) -> f64 {
    let l = |v: f64| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * l(c.0) + 0.7152 * l(c.1) + 0.0722 * l(c.2)
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn hsl(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (c.0, c.1, c.2);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-9 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h * 60.0, s, l)
}

fn mix(a: Rgb, b: Rgb, t: f64) -> Rgb {
    Rgb(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

/// Contrast guard: move `fg` towards black/white until it reaches `min` on `bg`.
fn ensure_contrast(fg: Rgb, bg: Rgb, min: f64) -> (Rgb, bool) {
    if contrast(fg, bg) >= min {
        return (fg, false);
    }
    let target = if lum(bg) > 0.4 { Rgb(0.0, 0.0, 0.0) } else { Rgb(1.0, 1.0, 1.0) };
    for i in 1..=20 {
        let c = mix(fg, target, i as f64 / 20.0);
        if contrast(c, bg) >= min {
            return (c, true);
        }
    }
    (target, true)
}

/// Pick the palette colour whose hue is nearest `hue`, saturated enough and
/// readable on `bg`; fall back to a fixed accessible colour.
fn by_hue(palette: &[Rgb], hue: f64, bg: Rgb, fallback: Rgb) -> (Rgb, bool) {
    let mut best: Option<(f64, Rgb)> = None;
    for &c in palette {
        let (h, s, l) = hsl(c);
        if s < 0.35 || !(0.2..=0.85).contains(&l) || contrast(c, bg) < 3.0 {
            continue;
        }
        let d = { let d = (h - hue).abs() % 360.0; d.min(360.0 - d) };
        if d <= 30.0 && best.map(|(bd, _)| d < bd).unwrap_or(true) {
            best = Some((d, c));
        }
    }
    match best {
        Some((_, c)) => (c, false),
        None => (fallback, true),
    }
}

fn resolve(colors: &Path) -> HashMap<String, String> {
    let out = Command::new("omarchy-theme-color").arg("--file").arg(colors).arg("--all").output();
    let mut m = HashMap::new();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            if let Some((k, v)) = line.split_once('\t') {
                m.insert(k.to_string(), v.to_string());
            }
        }
    }
    m
}

fn gradient(colors: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(colors).unwrap_or_default();
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("hyprland_active_border") {
            let v = v.trim_start_matches([' ', '=']).trim().trim_matches('"');
            return v
                .split_whitespace()
                .filter_map(|tok| {
                    let t = tok.trim_start_matches("rgba(").trim_start_matches("rgb(").trim_end_matches(')');
                    (t.len() >= 6 && t.chars().take(6).all(|c| c.is_ascii_hexdigit())).then(|| format!("#{}", &t[..6]))
                })
                .collect();
        }
    }
    Vec::new()
}

fn first_background(dir: &Path) -> Option<String> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir.join("backgrounds")).ok()?.flatten().map(|e| e.path()).collect();
    v.sort();
    v.into_iter()
        .find(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("jpg" | "jpeg" | "png" | "webp")))
        .map(|p| p.to_string_lossy().into_owned())
}

/// Tokens for one theme directory (must contain colors.toml).
pub fn resolve_dir(name: &str, dir: &Path, rounding: u32) -> serde_json::Value {
    let mut v = tokens(name, dir);
    v["radius"] = serde_json::json!(rounding);
    v["fontUi"] = serde_json::json!(ui_font());
    v["fontMono"] = serde_json::json!("monospace");
    v
}

/// The theme Omarchy currently applies (~/.local/state/omarchy/current/theme).
/// Omarchy 4 copies the theme into `current/theme/` and records its name in
/// `current/theme.name`; the wallpaper in use is the `current/background` link.
pub fn current(rounding: u32) -> serde_json::Value {
    let cur = current_dir();
    let dir = cur.join("theme");
    let name = std::fs::read_to_string(cur.join("theme.name"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::fs::read_link(&dir).ok().and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())))
        .unwrap_or_else(|| "current".into());
    let mut v = resolve_dir(&name, &dir, rounding);
    if let Ok(bg) = std::fs::canonicalize(cur.join("background")) {
        v["wallpaper"] = serde_json::json!(bg.to_string_lossy());
    }
    v
}

/// Wallpapers for the editor's Background tool: the one in use, the current
/// theme's, then the user's own for that theme (at most `limit`).
pub fn wallpapers(limit: usize) -> Vec<String> {
    let cur = current_dir();
    let name = std::fs::read_to_string(cur.join("theme.name")).map(|s| s.trim().to_string()).unwrap_or_default();
    let home = std::env::var("HOME").unwrap_or_default();
    let mut out: Vec<String> = vec![];
    if let Ok(bg) = std::fs::canonicalize(cur.join("background")) {
        out.push(bg.to_string_lossy().into_owned());
    }
    for dir in [cur.join("theme/backgrounds"), PathBuf::from(format!("{home}/.config/omarchy/backgrounds/{name}"))] {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&dir).map(|d| d.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        v.sort();
        for p in v {
            let ok = matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("jpg" | "jpeg" | "png" | "webp"));
            let p = std::fs::canonicalize(&p).unwrap_or(p).to_string_lossy().into_owned();
            if ok && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out.truncate(limit);
    out
}

/// Directory to watch for theme switches.
pub fn current_dir() -> PathBuf {
    PathBuf::from(format!("{}/.local/state/omarchy/current", std::env::var("HOME").unwrap_or_default()))
}

/// CleanShot-style proportional UI font (decision Q-D1): Inter when installed.
pub fn ui_font() -> String {
    let out = Command::new("fc-list").arg(":family=Inter").arg("family").output();
    match out {
        Ok(o) if !o.stdout.is_empty() => "Inter".into(),
        _ => "sans-serif".into(),
    }
}

fn tokens(name: &str, dir: &Path) -> serde_json::Value {
    let colors = dir.join("colors.toml");
    tokens_from(name, &resolve(&colors), gradient(&colors), first_background(dir))
}

/// Snip's own looks (D48), when it doesn't follow Omarchy: CleanShot's on
/// macOS, neutral greys and the system blue.
pub fn builtin(dark: bool, rounding: u32) -> serde_json::Value {
    let colors: &[(&str, &str)] = if dark {
        &[("background", "#1e1e1e"), ("foreground", "#f5f5f7"), ("accent", "#0a84ff"), ("color1", "#ff453a"), ("color2", "#32d74b"), ("color3", "#ffd60a"), ("mode", "dark")]
    } else {
        &[("background", "#f5f5f7"), ("foreground", "#1d1d1f"), ("accent", "#007aff"), ("color1", "#ff3b30"), ("color2", "#28a745"), ("color3", "#e6a700"), ("mode", "light")]
    };
    let p: HashMap<String, String> = colors.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    // the wallpaper is the desktop's, whatever Snip's colours
    let wallpaper = std::fs::canonicalize(current_dir().join("background")).ok().map(|p| p.to_string_lossy().into_owned());
    let mut v = tokens_from(if dark { "snip-dark" } else { "snip-light" }, &p, vec![], wallpaper);
    // white on the system blue, as macOS has it (the contrast rule picks black)
    v["onAccent"] = serde_json::json!("#ffffff");
    v["radius"] = serde_json::json!(rounding);
    v["fontUi"] = serde_json::json!(ui_font());
    v["fontMono"] = serde_json::json!("monospace");
    v
}

fn tokens_from(name: &str, p: &HashMap<String, String>, grad: Vec<String>, wallpaper: Option<String>) -> serde_json::Value {
    let get = |k: &str| p.get(k).and_then(|v| parse_hex(v));
    let bg = get("background").unwrap_or(Rgb(0.08, 0.08, 0.1));
    let fg0 = get("foreground").unwrap_or(Rgb(0.9, 0.9, 0.9));
    let accent0 = get("accent").or_else(|| get("blue")).unwrap_or(Rgb(0.3, 0.5, 1.0));
    let light = p.get("mode").map(|m| m == "light").unwrap_or(lum(bg) > 0.4);
    let palette: Vec<Rgb> = (0..16).filter_map(|i| get(&format!("color{i}"))).chain(get("accent")).collect();
    let (text, text_adj) = ensure_contrast(fg0, bg, 4.5);
    let (accent, accent_adj) = ensure_contrast(accent0, bg, 3.0);
    let on_accent = if contrast(Rgb(1.0, 1.0, 1.0), accent) >= contrast(Rgb(0.0, 0.0, 0.0), accent) { Rgb(1.0, 1.0, 1.0) } else { Rgb(0.07, 0.07, 0.08) };
    let (danger, danger_fb) = by_hue(&palette, 0.0, bg, Rgb(0.88, 0.2, 0.24));
    let (success, success_fb) = by_hue(&palette, 130.0, bg, Rgb(0.2, 0.7, 0.35));
    let (warning, warning_fb) = by_hue(&palette, 42.0, bg, Rgb(0.93, 0.66, 0.15));
    let surface = if light { mix(bg, Rgb(1.0, 1.0, 1.0), 0.35) } else { mix(bg, fg0, 0.06) };
serde_json::json!({
        "name": name,
        "mode": if light { "light" } else { "dark" },
        "background": hex(bg),
        "surface": hex(surface),
        "text": hex(text),
        "textSecondary": hex(mix(text, surface, 0.38)),
        "accent": hex(accent),
        "onAccent": hex(on_accent),
        "danger": hex(danger),
        "success": hex(success),
        "warning": hex(warning),
        "signature": if grad.is_empty() { vec![hex(accent)] } else { grad },
        "scrim": if light { "#59000000" } else { "#73000000" },
        "wallpaper": wallpaper,
        "report": {
            "text_contrast": (contrast(text, bg) * 100.0).round() / 100.0,
            "text_adjusted": text_adj,
            "accent_adjusted": accent_adj,
            "danger_fallback": danger_fb,
            "success_fallback": success_fb,
            "warning_fallback": warning_fb,
            "omarchy_red_is": p.get("red"),
        }
})
}

/// All installed themes (user themes override system themes of the same name),
/// resolved into Snip tokens, as a JSON array.
pub fn themes_json() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs: HashMap<String, PathBuf> = HashMap::new();
    for root in ["/usr/share/omarchy/themes".to_string(), format!("{home}/.config/omarchy/themes")] {
        if let Ok(rd) = std::fs::read_dir(&root) {
            for e in rd.flatten() {
                if e.path().join("colors.toml").exists() {
                    dirs.insert(e.file_name().to_string_lossy().into_owned(), e.path());
                }
            }
        }
    }
    let mut names: Vec<_> = dirs.keys().cloned().collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        out.push(tokens(&name, &dirs[&name]));
    }
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snips_own_looks_are_dark_and_light() {
        let (d, l) = (builtin(true, 8), builtin(false, 8));
        assert_eq!((d["name"].as_str(), d["mode"].as_str(), l["mode"].as_str()), (Some("snip-dark"), Some("dark"), Some("light")));
        assert_eq!((d["accent"].as_str(), l["accent"].as_str(), d["radius"].as_u64()), (Some("#0a84ff"), Some("#007aff"), Some(8)));
        assert_eq!((d["onAccent"].as_str(), l["onAccent"].as_str()), (Some("#ffffff"), Some("#ffffff")));
        for t in [&d, &l] {
            assert!(t["report"]["text_contrast"].as_f64().unwrap() >= 4.5 && t["report"]["accent_adjusted"] == false, "{t}");
        }
    }

    #[test]
    fn a_theme_with_a_broken_colour_is_still_drawn() {
        // a theme's colours are what its files say: a character of two bytes
        // where a digit goes, a colour cut short, words
        let palette = |v: &str| -> HashMap<String, String> {
            ["background", "foreground", "accent", "color1"].iter().map(|k| (k.to_string(), v.to_string())).collect()
        };
        let t = tokens_from("broken", &palette("#aé2345"), vec![], None);
        assert_eq!(t["background"].as_str().map(|c| c.len()), Some(7), "the fallback: {t}");
        crate::fuzz::run(&[b"#1e1e1e", b"rgba(30,30,30,1)", b"ffffff"], 5000, |b| {
            tokens_from("fuzz", &palette(&String::from_utf8_lossy(b)), vec![], None);
        });
    }
}
