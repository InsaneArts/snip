//! S17: resolve Omarchy themes into Snip design tokens (design doc §3).
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
    let p = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
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
        let dir = &dirs[&name];
        let colors = dir.join("colors.toml");
        let p = resolve(&colors);
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
        let grad = gradient(&colors);
        out.push(serde_json::json!({
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
            "wallpaper": first_background(dir),
            "report": {
                "text_contrast": (contrast(text, bg) * 100.0).round() / 100.0,
                "text_adjusted": text_adj,
                "accent_adjusted": accent_adj,
                "danger_fallback": danger_fb,
                "success_fallback": success_fb,
                "warning_fallback": warning_fb,
                "omarchy_red_is": p.get("red"),
            }
        }));
    }
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
}
