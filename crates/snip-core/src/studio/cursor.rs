//! The redrawn pointer's look (D59): the user's own cursor theme's arrow,
//! from an XCursor file, at its largest size so it stays sharp when a zoom
//! grows it.

use std::path::{Path, PathBuf};

/// A cursor image: straight RGBA, its hotspot (px of the image), and the
/// nominal size it was drawn for.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub hot: [u32; 2],
    pub nominal: u32,
    pub rgba: Vec<u8>,
}

/// The arrow of `theme` (or of a theme it inherits), at its largest size.
pub fn arrow(theme: &str) -> Option<Image> {
    let file = find(theme, &mut vec![])?;
    largest(&std::fs::read(file).ok()?)
}

/// Where themes are (the XCursor search path).
fn dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let data_home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
    let data = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    let mut out = vec![PathBuf::from(format!("{data_home}/icons")), PathBuf::from(format!("{home}/.icons"))];
    out.extend(data.split(':').filter(|d| !d.is_empty()).map(|d| Path::new(d).join("icons")));
    out.push("/usr/share/pixmaps".into());
    out
}

/// `theme`'s arrow file, else its inherited themes' (each looked at once).
fn find(theme: &str, seen: &mut Vec<String>) -> Option<PathBuf> {
    if seen.iter().any(|t| t == theme) {
        return None;
    }
    seen.push(theme.to_string());
    for dir in dirs() {
        for name in ["default", "left_ptr", "arrow"] {
            let f = dir.join(theme).join("cursors").join(name);
            if f.is_file() {
                return Some(f);
            }
        }
    }
    let inherits: Vec<String> = dirs()
        .iter()
        .filter_map(|d| std::fs::read_to_string(d.join(theme).join("index.theme")).ok())
        .flat_map(|text| text.lines().filter_map(|l| l.trim().strip_prefix("Inherits=").map(|v| v.split([',', ';']).map(|s| s.trim().to_string()).collect::<Vec<_>>())).collect::<Vec<_>>())
        .flatten()
        .collect();
    inherits.iter().find_map(|t| find(t, seen)).or_else(|| (theme != "Adwaita").then(|| find("Adwaita", seen)).flatten())
}

/// The largest image in an XCursor file (an animated one's first frame).
fn largest(b: &[u8]) -> Option<Image> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    if b.get(0..4)? != b"Xcur" {
        return None;
    }
    let (header, count) = (u32_at(4)? as usize, u32_at(12)? as usize);
    // the table of contents: type, nominal size, where; images are 0xfffd0002
    let (_, nominal, at) = (0..count)
        .filter_map(|i| {
            let e = header + i * 12;
            Some((u32_at(e)?, u32_at(e + 4)?, u32_at(e + 8)? as usize))
        })
        .filter(|&(kind, _, _)| kind == 0xfffd_0002)
        .max_by_key(|&(_, size, _)| size)?;
    let (width, height, hx, hy) = (u32_at(at + 16)?, u32_at(at + 20)?, u32_at(at + 24)?, u32_at(at + 28)?);
    let pixels = b.get(at + 36..at + 36 + (width * height * 4) as usize)?;
    // premultiplied ARGB, little-endian: straight RGBA
    let rgba = pixels
        .chunks_exact(4)
        .flat_map(|p| {
            let a = p[3] as u32;
            let un = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
            [un(p[2]), un(p[1]), un(p[0]), p[3]]
        })
        .collect();
    Some(Image { width, height, hot: [hx, hy], nominal, rgba })
}
