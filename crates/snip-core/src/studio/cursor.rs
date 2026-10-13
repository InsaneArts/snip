//! The redrawn pointer's look (D59): the user's own cursor theme's shapes,
//! from its XCursor files, at their largest size so they stay sharp when a
//! zoom grows them. A take logs the pointer's hotspot (spike S6), which
//! tells the shapes apart: the arrow, the hand, the I-beam.

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

/// A shape of the pointer: its name, its largest image (what's drawn), and
/// its hotspot at the take's pointer size (what the take logged).
pub struct Shape {
    pub name: &'static str,
    pub image: Image,
    pub hot_at_size: [u32; 2],
}

/// The shapes told apart, the arrow first (drawn when none matches), each
/// with the other names themes give it.
const SHAPES: [(&str, &[&str]); 3] =
    [("default", &["default", "left_ptr", "arrow"]), ("pointer", &["pointer", "hand2", "hand1", "pointing_hand"]), ("text", &["text", "xterm", "ibeam"])];

/// `theme`'s shapes (or a theme's it inherits), for a pointer `size` logical
/// px: those it has, the arrow first.
pub fn shapes(theme: &str, size: u32) -> Vec<Shape> {
    SHAPES
        .iter()
        .filter_map(|&(name, names)| {
            let bytes = std::fs::read(find(theme, names, &mut vec![])?).ok()?;
            Some(Shape { name, image: largest(&bytes)?, hot_at_size: hot_at(&bytes, size)? })
        })
        .collect()
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

/// The file of a shape called one of `names` in `theme`, else in the themes
/// it inherits (each looked at once).
fn find(theme: &str, names: &[&str], seen: &mut Vec<String>) -> Option<PathBuf> {
    if seen.iter().any(|t| t == theme) {
        return None;
    }
    seen.push(theme.to_string());
    for dir in dirs() {
        for name in names {
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
    inherits.iter().find_map(|t| find(t, names, seen)).or_else(|| (theme != "Adwaita").then(|| find("Adwaita", names, seen)).flatten())
}

/// An XCursor file's images: their nominal size and where they are (an
/// animated one's first frame of each size).
fn images(b: &[u8]) -> Option<Vec<(u32, usize)>> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    if b.get(0..4)? != b"Xcur" {
        return None;
    }
    let (header, count) = (u32_at(4)? as usize, u32_at(12)? as usize);
    // the table of contents: type, nominal size, where; images are 0xfffd0002
    Some(
        (0..count)
            .filter_map(|i| {
                let e = header + i * 12;
                Some((u32_at(e)?, u32_at(e + 4)?, u32_at(e + 8)? as usize))
            })
            .filter(|&(kind, _, _)| kind == 0xfffd_0002)
            .map(|(_, size, at)| (size, at))
            .collect(),
    )
}

/// The hotspot of the image nearest `size` (px of that image).
fn hot_at(b: &[u8], size: u32) -> Option<[u32; 2]> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    let &(_, at) = images(b)?.iter().min_by_key(|&&(nominal, _)| nominal.abs_diff(size))?;
    Some([u32_at(at + 24)?, u32_at(at + 28)?])
}

/// The largest image in an XCursor file.
fn largest(b: &[u8]) -> Option<Image> {
    let u32_at = |at: usize| Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?));
    let (nominal, at) = images(b)?.into_iter().max_by_key(|&(size, _)| size)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    // an XCursor file with an image per (nominal size, hotspot), each nominal px square
    fn xcursor(images: &[(u32, [u32; 2])]) -> Vec<u8> {
        let mut b: Vec<u8> = b"Xcur".to_vec();
        let push = |b: &mut Vec<u8>, v: u32| b.extend(v.to_le_bytes());
        push(&mut b, 16);
        push(&mut b, 0x1_0000);
        push(&mut b, images.len() as u32);
        let mut at = 16 + 12 * images.len() as u32;
        for &(nominal, _) in images {
            push(&mut b, 0xfffd_0002);
            push(&mut b, nominal);
            push(&mut b, at);
            at += 36 + nominal * nominal * 4;
        }
        for &(nominal, hot) in images {
            for v in [36, 0xfffd_0002, nominal, 1, nominal, nominal, hot[0], hot[1], 0] {
                push(&mut b, v);
            }
            b.extend(std::iter::repeat_n(0xffu8, (nominal * nominal * 4) as usize));
        }
        b
    }

    #[test]
    fn a_shape_is_drawn_from_its_largest_image_and_told_by_its_hotspot_at_the_takes_size() {
        let file = xcursor(&[(24, [3, 1]), (48, [6, 2]), (96, [12, 4])]);
        let big = largest(&file).unwrap();
        assert_eq!((big.width, big.nominal, big.hot), (96, 96, [12, 4]), "drawn sharp");
        assert_eq!(hot_at(&file, 24), Some([3, 1]), "the hotspot the take logged at 24");
        assert_eq!(hot_at(&file, 30), Some([3, 1]), "the nearest size the theme has");
        assert_eq!(hot_at(&file, 64), Some([6, 2]));
    }
}
