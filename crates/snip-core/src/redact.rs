//! The editor's Pixelate / Blur tool, secure by construction (ANN-06, D26).
//!
//! Pixelation and blur of the hidden content can be reversed (Hill et al.
//! 2016, Depix, Unredacter: processing doc §4.1). So the look is built only
//! from pixels *outside* the region, blended across it, plus noise from a
//! seed. Pixelate and Blur never read inside the region: two images that
//! differ only there give identical tiles.
//!
//! Text shape (D30) looks like CleanShot's pixelated text. It reads inside the
//! region for exactly four things: the paper colour (the most common one),
//! the mean ink colour, the text's line height, and how much ink each block
//! holds (none, some, much), blocks being just over half the line height. So word lengths
//! and roughly where tall or dropping letters are can show; no pixel value
//! does, and two texts with the same blocks inked give identical tiles.
//! Pixelate, the default, shows nothing of the content.
//!
//! Natural blur is the one exception: a real blur of the region, for
//! softening a background. It is not secure, and the editor says so.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Large tiles, drawn without smoothing
    Pixelate,
    /// Small tiles with gentle noise, drawn smoothed
    Blur,
    /// Blocks of ink where the words are, on the surroundings (D30)
    TextShape,
    /// A real blur of the region: **not secure**
    NaturalBlur,
}

/// How far outside the region the surroundings are sampled (px).
const RING: i64 = 6;

/// Tiles for `rect` (x, y, w, h in image px) of an image whose pixels are
/// `bpp` bytes each (3 or 4). Returns opaque RGBA, `cols` × `rows`, to be
/// stretched over the region. `level` is the intensity: it scales the blocks
/// (1 is the usual look) or, for Natural blur, the blur radius.
#[allow(clippy::too_many_arguments)]
pub fn tiles(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64), style: Style, seed: u32, level: f32) -> (Vec<u8>, u32, u32) {
    let (x, y, w, h) = rect;
    let (w, h) = (w.max(1), h.max(1));
    if style == Style::NaturalBlur {
        return natural_blur(data, width, height, bpp, (x, y, w, h), level);
    }
    let block = match style {
        Style::Pixelate => (w.min(h) / 4).clamp(8, 20),
        Style::Blur => (w.min(h) / 10).clamp(3, 8),
        _ => text_block(data, width, height, bpp, (x, y, w, h)).unwrap_or((w.min(h) / 5).clamp(5, 12)),
    };
    let block = ((block as f32 * level).round() as i64).max(2);
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
        _ => 5.0,
    };

    let mut rng = seed.wrapping_mul(2654435761).wrapping_add(0x9e3779b9) | 1;
    let mut next = move || {
        // xorshift32: noise from the seed only
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    // the surroundings blended across the region: each tile's background
    let background = |r: usize, c: usize| -> [f32; 3] {
        let (u, v) = ((c as f32 + 0.5) / cols as f32, (r as f32 + 0.5) / rows as f32);
        let (t, b, l, rt) = (top[c].unwrap_or(mean), bottom[c].unwrap_or(mean), left[r].unwrap_or(mean), right[r].unwrap_or(mean));
        // blend top↔bottom and left↔right
        std::array::from_fn(|k| (t[k] * (1.0 - v) + b[k] * v + l[k] * (1.0 - u) + rt[k] * u) / 2.0)
    };
    let shape = (style == Style::TextShape).then(|| text_shape(data, width, height, bpp, (x, y, w, h), block, cols, rows));

    let mut out = Vec::with_capacity(cols * rows * 4);
    for r in 0..rows {
        for c in 0..cols {
            let mut tile = background(r, c);
            if let Some((mask, paper, ink)) = &shape {
                // some ink: a light block; much ink: a dark one
                let amount = [0.0, 0.4, 0.75][mask[r * cols + c] as usize];
                tile = std::array::from_fn(|k| paper[k] * (1.0 - amount) + ink[k] * amount);
            }
            let noise = next() * amp;
            for v in tile {
                out.push((v + noise).round().clamp(0.0, 255.0) as u8);
            }
            out.push(255);
        }
    }
    (out, cols as u32, rows as u32)
}

/// Natural blur: three box blurs (close to a Gaussian) of the region and a
/// margin around it, at full size. It reads the hidden pixels.
fn natural_blur(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64), level: f32) -> (Vec<u8>, u32, u32) {
    let (x, y, w, h) = rect;
    let r = ((6.0 * level).round() as i64).max(1);
    // the margin lets edges blur into their surroundings
    let (x0, y0) = ((x - 3 * r).max(0), (y - 3 * r).max(0));
    let (x1, y1) = ((x + w + 3 * r).min(width as i64), (y + h + 3 * r).min(height as i64));
    let (bw, bh) = ((x1 - x0).max(0) as usize, (y1 - y0).max(0) as usize);
    let mut buf: Vec<[f32; 3]> = Vec::with_capacity(bw * bh);
    for sy in y0..y1 {
        for sx in x0..x1 {
            buf.push(pixel(data, width, height, bpp, sx, sy).unwrap_or([0.0; 3]));
        }
    }
    // a running box sum along a line of n samples `step` apart, edges clamped
    let pass = |buf: &mut Vec<[f32; 3]>, n: usize, lines: usize, at: &dyn Fn(usize, usize) -> usize| {
        let mut line = vec![[0f32; 3]; n];
        for l in 0..lines {
            for (i, v) in line.iter_mut().enumerate() {
                *v = buf[at(l, i)];
            }
            let get = |i: i64| line[i.clamp(0, n as i64 - 1) as usize];
            let mut sum = [0f32; 3];
            for i in -r..=r {
                let v = get(i);
                for k in 0..3 {
                    sum[k] += v[k];
                }
            }
            for i in 0..n as i64 {
                buf[at(l, i as usize)] = sum.map(|s| s / (2 * r + 1) as f32);
                let (add, sub) = (get(i + r + 1), get(i - r));
                for k in 0..3 {
                    sum[k] += add[k] - sub[k];
                }
            }
        }
    };
    if bw > 0 && bh > 0 {
        for _ in 0..3 {
            pass(&mut buf, bw, bh, &|l, i| l * bw + i);
            pass(&mut buf, bh, bw, &|l, i| i * bw + l);
        }
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for sy in y..y + h {
        for sx in x..x + w {
            let (bx, by) = (sx - x0, sy - y0);
            let v = if bx >= 0 && by >= 0 && (bx as usize) < bw && (by as usize) < bh { buf[by as usize * bw + bx as usize] } else { [0.0; 3] };
            out.extend(v.map(|c| c.round().clamp(0.0, 255.0) as u8));
            out.push(255);
        }
    }
    (out, w as u32, h as u32)
}

fn luma(c: &[f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

fn pixel(data: &[u8], width: u32, height: u32, bpp: usize, sx: i64, sy: i64) -> Option<[f32; 3]> {
    if sx < 0 || sy < 0 || sx >= width as i64 || sy >= height as i64 {
        return None;
    }
    let i = (sy as usize * width as usize + sx as usize) * bpp;
    Some([data[i] as f32, data[i + 1] as f32, data[i + 2] as f32])
}

/// The paper: the region's most common colour (5 bits per channel), averaged.
fn paper(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64)) -> [f32; 3] {
    let (x, y, w, h) = rect;
    let mut bins: std::collections::HashMap<u16, (u32, [f64; 3])> = Default::default();
    for sy in y..y + h {
        for sx in x..x + w {
            if let Some(p) = pixel(data, width, height, bpp, sx, sy) {
                let key = ((p[0] as u16 >> 3) << 10) | ((p[1] as u16 >> 3) << 5) | (p[2] as u16 >> 3);
                let e = bins.entry(key).or_insert((0, [0.0; 3]));
                e.0 += 1;
                e.1.iter_mut().zip(p).for_each(|(s, v)| *s += v as f64);
            }
        }
    }
    bins.values().max_by_key(|(n, _)| *n).map(|(n, sum)| std::array::from_fn(|k| (sum[k] / *n as f64) as f32)).unwrap_or([128.0; 3])
}

/// Text shape's block: just over half the text's line height (the median run
/// of pixel rows holding ink). A line is two rows of blocks: word shapes
/// show, letters don't (a third of the line made 19 px text readable).
fn text_block(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64)) -> Option<i64> {
    let (x, y, w, h) = rect;
    let bg = luma(&paper(data, width, height, bpp, rect));
    let inked: Vec<bool> = (y..y + h)
        .map(|sy| (x..x + w).any(|sx| pixel(data, width, height, bpp, sx, sy).is_some_and(|p| (luma(&p) - bg).abs() > 48.0)))
        .collect();
    let mut runs: Vec<i64> = inked.split(|i| !i).map(|r| r.len() as i64).filter(|&n| n > 0).collect();
    runs.sort_unstable();
    runs.get(runs.len() / 2).map(|lh| (lh * 11 / 20).clamp(8, 24))
}

/// Text shape's readings of the region: the paper colour, how much ink each
/// block holds (none, some, much), and the mean ink colour. Nothing else
/// reaches the tiles.
#[allow(clippy::too_many_arguments)]
fn text_shape(data: &[u8], width: u32, height: u32, bpp: usize, rect: (i64, i64, i64, i64), block: i64, cols: usize, rows: usize) -> (Vec<u8>, [f32; 3], [f32; 3]) {
    let (x, y, w, h) = rect;
    let at = |sx: i64, sy: i64| pixel(data, width, height, bpp, sx, sy);
    let paper = paper(data, width, height, bpp, rect);
    let bg = luma(&paper);
    let mut mask = vec![0u8; cols * rows];
    let (mut ink, mut inked) = ([0f64; 3], 0f64);
    for r in 0..rows {
        for c in 0..cols {
            let (mut n, mut hits) = (0u32, 0u32);
            for sy in (y + r as i64 * block)..(y + (r as i64 + 1) * block).min(y + h) {
                for sx in (x + c as i64 * block)..(x + (c as i64 + 1) * block).min(x + w) {
                    let Some(p) = at(sx, sy) else { continue };
                    n += 1;
                    if (luma(&p) - bg).abs() > 48.0 {
                        hits += 1;
                        for k in 0..3 {
                            ink[k] += p[k] as f64;
                        }
                        inked += 1.0;
                    }
                }
            }
            let share = if n > 0 { hits as f32 / n as f32 } else { 0.0 };
            mask[r * cols + c] = if share > 0.3 { 2 } else if share > 0.08 { 1 } else { 0 };
        }
    }
    let ink = if inked > 0.0 { std::array::from_fn(|k| (ink[k] / inked) as f32) } else { paper };
    (mask, paper, ink)
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
            assert_eq!(tiles(&a, 200, 100, 3, rect, style, 42, 1.0), tiles(&b, 200, 100, 3, rect, style, 42, 1.0), "{style:?}");
        }
    }

    #[test]
    fn tiles_follow_the_surroundings_and_the_seed() {
        let red = image(100, 100, |_, _| [220, 30, 30]);
        let (t, cols, rows) = tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 1, 1.0);
        assert_eq!((cols, rows), (6, 4)); // 60x40 in 10 px blocks
        assert!(t.chunks_exact(4).all(|p| p[0] > 180 && p[1] < 70 && p[3] == 255), "red around → reddish tiles");
        assert_ne!(t, tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 2, 1.0).0, "another seed, another mosaic");
        assert_eq!(t, tiles(&red, 100, 100, 3, (20, 20, 60, 40), Style::Pixelate, 1, 1.0).0, "same seed, same mosaic");
    }

    /// Draws "words" into a white region (20, 20, 300, 40) with grey
    /// surroundings: runs of 8 px columns, inked on a 15 px band (y 28..43),
    /// so the line height gives 8 px blocks aligned with the words.
    /// `letters` decides which pixels of each word are ink.
    fn words(lens: &[usize], letters: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        image(340, 80, |x, y| {
            let inside = (20..320).contains(&x) && (20..60).contains(&y);
            if !inside {
                return [200, 200, 200];
            }
            let col = (x - 20) / 8;
            let mut start = 0;
            for &len in lens {
                if (28..43).contains(&y) && col >= start && col < start + len as u32 && letters(x, y) {
                    return [30, 30, 30];
                }
                start += len as u32 + 2; // two empty blocks between words
            }
            [255, 255, 255]
        })
    }

    #[test]
    fn text_shape_shows_inked_blocks_never_pixels() {
        let rect = (20, 20, 300, 40);
        // the same blocks inked as much, different "letters" inside them
        let a = words(&[7, 9], |x, y| (x + y) % 2 == 0);
        let b = words(&[7, 9], |x, y| (x * 7 + y * 3) % 5 < 3);
        let (ta, cols, rows) = tiles(&a, 340, 80, 3, rect, Style::TextShape, 9, 1.0);
        let (tb, _, _) = tiles(&b, 340, 80, 3, rect, Style::TextShape, 9, 1.0);
        assert_eq!((cols, rows), (38, 5), "blocks are just over half the 15 px line");
        assert_eq!(ta, tb, "different pixels, same inked blocks: identical tiles");
        let dark = |t: &[u8], r: usize, c: usize| t[(r * cols as usize + c) * 4] < 150;
        assert!(dark(&ta, 2, 0) && dark(&ta, 2, 6) && !dark(&ta, 2, 7) && dark(&ta, 2, 9), "words show as ink, gaps as paper");
        assert!(dark(&ta, 1, 3) && !dark(&ta, 0, 3) && !dark(&ta, 3, 3), "above and below the line is paper");
        assert!(ta[(2 * cols as usize + 7) * 4] > 230, "the paper is the region's white, not the grey around it");
        let (tc, _, _) = tiles(&words(&[3, 9], |x, y| (x + y) % 2 == 0), 340, 80, 3, rect, Style::TextShape, 9, 1.0);
        assert_ne!(ta, tc, "other word lengths look different");
    }

    #[test]
    fn intensity_scales_the_blocks_and_stays_secure() {
        let rect = (40, 30, 80, 40);
        let inside = |x: u32, y: u32| (40..120).contains(&x) && (30..70).contains(&y);
        let a = image(200, 100, |x, y| if inside(x, y) { [(x * 7) as u8, 0, 0] } else { [(x + y) as u8, 90, 200] });
        let b = image(200, 100, |x, y| if inside(x, y) { [0, (y * 13) as u8, 255] } else { [(x + y) as u8, 90, 200] });
        let (_, cols, _) = tiles(&a, 200, 100, 3, rect, Style::Pixelate, 1, 1.0);
        let (_, more, _) = tiles(&a, 200, 100, 3, rect, Style::Pixelate, 1, 2.0);
        assert!(more < cols, "a higher intensity gives bigger blocks: {more} vs {cols} columns");
        for level in [0.6, 2.4] {
            assert_eq!(tiles(&a, 200, 100, 3, rect, Style::Pixelate, 4, level), tiles(&b, 200, 100, 3, rect, Style::Pixelate, 4, level));
        }
    }

    #[test]
    fn natural_blur_softens_the_region_itself() {
        // fine stripes inside, flat grey outside
        let img = image(120, 80, |x, y| if (20..100).contains(&x) && (20..60).contains(&y) { if x % 2 == 0 { [255; 3] } else { [0; 3] } } else { [128; 3] });
        let (t, w, h) = tiles(&img, 120, 80, 3, (20, 20, 80, 40), Style::NaturalBlur, 0, 1.0);
        assert_eq!((w, h), (80, 40), "full size, one pixel per image pixel");
        let mid = |x: u32| t[((20 * w + x) * 4) as usize] as i32;
        assert!((mid(40) - 128).abs() < 6 && (mid(41) - 128).abs() < 6, "stripes blur to their mean: {} {}", mid(40), mid(41));
        // wide stripes: a higher intensity blurs them further
        let wide = image(120, 80, |x, _| if (x / 8) % 2 == 0 { [255; 3] } else { [0; 3] });
        let contrast = |level: f32| {
            let (t, w, _) = tiles(&wide, 120, 80, 3, (20, 20, 80, 40), Style::NaturalBlur, 0, level);
            let row: Vec<i32> = (0..w).map(|x| t[((20 * w + x) * 4) as usize] as i32).collect();
            row.iter().max().unwrap() - row.iter().min().unwrap()
        };
        assert!(contrast(0.3) > contrast(1.0) + 40, "{} vs {}", contrast(0.3), contrast(1.0));
    }

    #[test]
    fn the_whole_image_falls_back_to_grey() {
        let img = image(50, 50, |x, _| [x as u8 * 5, 0, 0]);
        let (t, _, _) = tiles(&img, 50, 50, 3, (0, 0, 50, 50), Style::Blur, 3, 1.0);
        assert!(t.chunks_exact(4).all(|p| (110..=146).contains(&p[1])), "nothing outside: mid grey");
    }
}
