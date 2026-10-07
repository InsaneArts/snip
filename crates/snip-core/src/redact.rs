//! The editor's Pixelate / Blur tool, secure by construction (ANN-06, D26).
//!
//! Pixelation and blur of the hidden content can be reversed (Hill et al.
//! 2016, Depix, Unredacter: processing doc §4.1). So the look is built only
//! from pixels *outside* the region, blended across it, plus noise from a
//! seed. Nothing inside the region is ever read: two images that differ only
//! there give identical tiles.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Large tiles, drawn without smoothing
    Pixelate,
    /// Small tiles with gentle noise, drawn smoothed
    Blur,
}

/// How far outside the region the surroundings are sampled (px).
const RING: i64 = 6;

/// Tiles for `rect` (x, y, w, h in image px) of an image whose pixels are
/// `bpp` bytes each (3 or 4). Returns opaque RGBA, `cols` × `rows`, to be
/// stretched over the region.
pub fn tiles(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64), style: Style, seed: u32) -> (Vec<u8>, u32, u32) {
    let (x, y, w, h) = rect;
    let (w, h) = (w.max(1), h.max(1));
    let block = match style {
        Style::Pixelate => (w.min(h) / 4).clamp(8, 20),
        Style::Blur => (w.min(h) / 10).clamp(3, 8),
    };
    let cols = ((w + block - 1) / block).max(1) as usize;
    let rows = ((h + block - 1) / block).max(1) as usize;

    let px = |sx: i64, sy: i64| -> Option<[f32; 3]> {
        if sx < 0 || sy < 0 || sx >= width as i64 || sy >= height as i64 {
            return None;
        }
        // never inside the region
        if sx >= x && sx < x + w && sy >= y && sy < y + h {
            return None;
        }
        let i = (sy as usize * width as usize + sx as usize) * bpp;
        Some([data[i] as f32, data[i + 1] as f32, data[i + 2] as f32])
    };
    // Mean colour of the band just outside one side, over a span along it.
    let side = |pick: &dyn Fn(i64, i64) -> (i64, i64), along: (i64, i64)| -> Option<[f32; 3]> {
        let (mut sum, mut n) = ([0f32; 3], 0f32);
        for a in along.0..along.1 {
            for d in 1..=RING {
                let (sx, sy) = pick(a, d);
                if let Some(c) = px(sx, sy) {
                    for k in 0..3 {
                        sum[k] += c[k];
                    }
                    n += 1.0;
                }
            }
        }
        (n > 0.0).then(|| sum.map(|s| s / n))
    };

    // Each side's colour profile, one value per tile column (top/bottom) or
    // row (left/right), each averaged over that tile's span.
    let span = |i: usize, n: usize, len: i64| (i as i64 * len / n as i64, ((i + 1) as i64 * len / n as i64).max(i as i64 * len / n as i64 + 1));
    let top: Vec<_> = (0..cols).map(|i| side(&|a, d| (x + a, y - d), span(i, cols, w))).collect();
    let bottom: Vec<_> = (0..cols).map(|i| side(&|a, d| (x + a, y + h - 1 + d), span(i, cols, w))).collect();
    let left: Vec<_> = (0..rows).map(|i| side(&|a, d| (x - d, y + a), span(i, rows, h))).collect();
    let right: Vec<_> = (0..rows).map(|i| side(&|a, d| (x + w - 1 + d, y + a), span(i, rows, h))).collect();

    // Overall surroundings: the fallback for a side outside the image, and
    // the spread that sets how strong the noise is.
    let all: Vec<[f32; 3]> = top.iter().chain(&bottom).chain(&left).chain(&right).flatten().copied().collect();
    let mean = if all.is_empty() {
        [128.0; 3]
    } else {
        let mut m = [0f32; 3];
        for c in &all {
            for k in 0..3 {
                m[k] += c[k] / all.len() as f32;
            }
        }
        m
    };
    let luma = |c: &[f32; 3]| 0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2];
    let spread = if all.len() > 1 {
        let ml = luma(&mean);
        (all.iter().map(|c| (luma(c) - ml).powi(2)).sum::<f32>() / all.len() as f32).sqrt()
    } else {
        0.0
    };
    let amp = match style {
        Style::Pixelate => (spread * 0.6).clamp(12.0, 28.0),
        Style::Blur => (spread * 0.25).clamp(2.0, 8.0),
    };

    let mut rng = seed.wrapping_mul(2654435761).wrapping_add(0x9e3779b9) | 1;
    let mut next = move || {
        // xorshift32: noise from the seed only
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    let mut out = Vec::with_capacity(cols * rows * 4);
    for r in 0..rows {
        let v = (r as f32 + 0.5) / rows as f32;
        for c in 0..cols {
            let u = (c as f32 + 0.5) / cols as f32;
            let t = top[c].unwrap_or(mean);
            let b = bottom[c].unwrap_or(mean);
            let l = left[r].unwrap_or(mean);
            let rt = right[r].unwrap_or(mean);
            let noise = next() * amp;
            for k in 0..3 {
                // blend top↔bottom and left↔right, then add the noise
                let vert = t[k] * (1.0 - v) + b[k] * v;
                let horiz = l[k] * (1.0 - u) + rt[k] * u;
                out.push(((vert + horiz) / 2.0 + noise).round().clamp(0.0, 255.0) as u8);
            }
            out.push(255);
        }
    }
    (out, cols as u32, rows as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                v.extend_from_slice(&f(x, y));
            }
        }
        v
    }

    #[test]
    fn hidden_content_never_reaches_the_tiles() {
        let rect = (40, 30, 80, 40);
        let inside = |x: u32, y: u32| (40..120).contains(&x) && (30..70).contains(&y);
        // the same surroundings, different secrets inside
        let a = image(200, 100, |x, y| if inside(x, y) { [(x * 7) as u8, 0, 0] } else { [(x + y) as u8, 90, 200] });
        let b = image(200, 100, |x, y| if inside(x, y) { [0, (y * 13) as u8, 255] } else { [(x + y) as u8, 90, 200] });
        for style in [Style::Pixelate, Style::Blur] {
            assert_eq!(tiles(&a, 200, 100, 3, rect, style, 42), tiles(&b, 200, 100, 3, rect, style, 42), "{style:?}");
        }
    }

    #[test]
    fn tiles_follow_the_surroundings_and_the_seed() {
        let red = image(100, 100, |_, _| [220, 30, 30]);
        let (t, cols, rows) = tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 1);
        assert_eq!((cols, rows), (6, 4)); // 60x40 in 10 px blocks
        assert!(t.chunks_exact(4).all(|p| p[0] > 180 && p[1] < 70 && p[3] == 255), "red around → reddish tiles");
        assert_ne!(t, tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 2).0, "another seed, another mosaic");
        assert_eq!(t, tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 1).0, "same seed, same mosaic");
    }

    #[test]
    fn the_whole_image_falls_back_to_grey() {
        let img = image(50, 50, |x, _| [x as u8 * 5, 0, 0]);
        let (t, _, _) = tiles(&img, 50, 50, 3, (0, 0, 50, 50), Style::Blur, 3);
        assert!(t.chunks_exact(4).all(|p| (110..=146).contains(&p[1])), "nothing outside: mid grey");
    }
}
