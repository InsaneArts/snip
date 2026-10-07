//! The Background tool's Auto-balance (BG-02): how far the plain border
//! around a screenshot's content reaches on each side, so the background can
//! even the margins out with that border's colour.

/// The plain margins of `rect` (x, y, w, h in px) of an image whose pixels
/// are `bpp` bytes each: [left, top, right, bottom], and the border colour
/// (the most common one along the rect's edge). A pixel more than 16 off that
/// colour in any channel is content. Without content the margins are 0.
pub fn margins(data: &[u8], width: u32, height: u32, bpp: usize, rect: (u32, u32, u32, u32)) -> ([u32; 4], [u8; 3]) {
    let (x0, y0) = (rect.0.min(width), rect.1.min(height));
    let (x1, y1) = ((rect.0 + rect.2).min(width), (rect.1 + rect.3).min(height));
    if x1 <= x0 || y1 <= y0 {
        return ([0; 4], [255; 3]);
    }
    let px = |x: u32, y: u32| {
        let i = (y as usize * width as usize + x as usize) * bpp;
        [data[i], data[i + 1], data[i + 2]]
    };
    let mut bins: std::collections::HashMap<[u8; 3], usize> = Default::default();
    for x in x0..x1 {
        *bins.entry(px(x, y0)).or_default() += 1;
        *bins.entry(px(x, y1 - 1)).or_default() += 1;
    }
    for y in y0..y1 {
        *bins.entry(px(x0, y)).or_default() += 1;
        *bins.entry(px(x1 - 1, y)).or_default() += 1;
    }
    let border = bins.into_iter().max_by_key(|(_, n)| *n).map(|(c, _)| c).unwrap_or([255; 3]);
    let content = |x: u32, y: u32| px(x, y).iter().zip(border).any(|(a, b)| a.abs_diff(b) > 16);
    let col = |x: u32| (y0..y1).any(|y| content(x, y));
    let row = |y: u32| (x0..x1).any(|x| content(x, y));
    let Some(left) = (x0..x1).find(|&x| col(x)) else {
        return ([0; 4], border);
    };
    let right = (x0..x1).rev().find(|&x| col(x)).unwrap_or(left);
    let top = (y0..y1).find(|&y| row(y)).unwrap_or(y0);
    let bottom = (y0..y1).rev().find(|&y| row(y)).unwrap_or(top);
    ([left - x0, top - y0, x1 - 1 - right, y1 - 1 - bottom], border)
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
    fn measures_the_plain_border_on_each_side() {
        // content from (10, 5) to (79, 59) on white
        let img = image(100, 80, |x, y| if (10..80).contains(&x) && (5..60).contains(&y) { [20, 90, 200] } else { [255; 3] });
        assert_eq!(margins(&img, 100, 80, 3, (0, 0, 100, 80)), ([10, 5, 20, 20], [255; 3]));
        // within a crop
        assert_eq!(margins(&img, 100, 80, 3, (4, 0, 90, 70)).0, [6, 5, 14, 10]);
    }

    #[test]
    fn small_differences_are_not_content_and_plain_images_have_none() {
        let img = image(60, 40, |x, _| if x == 30 { [250, 250, 250] } else { [255; 3] });
        assert_eq!(margins(&img, 60, 40, 3, (0, 0, 60, 40)).0, [0; 4]);
    }
}
