//! Scrolling capture's stitcher (SCR-01, SCR-03): Snip's own (D27). Frames of
//! one region, taken while its content scrolls, become one tall image.
//!
//! A 1-D alignment, not a panorama: each frame is matched against the last
//! one by per-row signatures (a cost for every shift), checked on pixels, and
//! the rows it reveals are appended. Rows are kept by their position in the
//! content: the first frame gives rows `[0, H − footer)`, each later frame the
//! rows past the end, and the last frame its footer. A sticky header is
//! therefore only ever taken from the first frame, and a footer only from the
//! last; bands are found as rows that stay put while the rest moves.

/// What a frame did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// the first frame
    First,
    /// the content moved by this many rows (up is positive); `added` rows are new
    Moved { dy: i32, added: u32 },
    /// nothing changed
    Still,
    /// several shifts fit (repeated or blank rows): wait for a clearer frame
    Unsure,
    /// no shift fits: scrolled too far between frames, or the page changed
    Lost,
}

pub struct Stitcher {
    w: usize,
    h: usize,
    prev: Option<Frame>,
    top: usize,
    bottom: usize,
    /// content position of the last matched frame's row 0
    pos: i64,
    /// content rows in `canvas`
    end: i64,
    canvas: Vec<u8>,
    last_dy: i32,
}

struct Frame {
    rgba: Vec<u8>,
    gray: Vec<u8>,
    sig: Vec<[u8; BINS]>,
    textured: Vec<bool>,
}

const BINS: usize = 32;
/// at least this many textured rows must overlap for a match
const MIN_ROWS: usize = 8;
/// mean difference per signature bin (grey levels) a match may have
const MAX_COST: f32 = 6.0;
/// the second-best shift is as good as the best: ambiguous
const AMBIGUOUS: f32 = 0.7;
/// rows this close (mean grey difference) are the same row: noise and
/// resampling stay under it
const STILL: f32 = 3.0;

impl Stitcher {
    pub fn new(width: u32, height: u32) -> Self {
        Self { w: width as usize, h: height as usize, prev: None, top: 0, bottom: 0, pos: 0, end: 0, canvas: vec![], last_dy: 0 }
    }

    /// The stitched image's height so far.
    pub fn height(&self) -> u32 {
        (self.end as usize + if self.prev.is_some() { self.bottom } else { 0 }) as u32
    }

    /// Add a frame (RGBA, `width × height`).
    pub fn push(&mut self, rgba: &[u8]) -> Step {
        assert_eq!(rgba.len(), self.w * self.h * 4, "frame size");
        let cur = self.frame(rgba);
        let Some(prev) = self.prev.take() else {
            self.canvas = rgba.to_vec();
            self.end = self.h as i64;
            self.prev = Some(cur);
            return Step::First;
        };
        let (h, cap) = (self.h, self.h / 3);
        let still: Vec<bool> = (0..h).map(|y| self.row_diff(&prev, y, &cur, y) <= STILL).collect();
        if still.iter().all(|&s| s) {
            self.prev = Some(prev);
            return Step::Still;
        }
        let dy = match self.shift(&prev, &cur, &still) {
            Ok(dy) => dy,
            Err(step) => {
                self.prev = Some(prev);
                return step;
            }
        };
        if dy != 0 {
            // A band row stayed put while the content moved, and isn't just
            // content that looks the same shifted (a blank stretch).
            let band = |y: &usize| {
                let from = *y as i32 + dy;
                !(0..h as i32).contains(&from) || self.row_diff(&prev, from as usize, &cur, *y) > STILL
            };
            let top = (0..h).take_while(|&y| still[y]).filter(band).last().map_or(0, |y| y + 1).min(cap);
            let bottom = (0..h).rev().take_while(|&y| still[y]).filter(band).last().map_or(0, |y| h - y).min(cap);
            self.top = self.top.max(top);
            if bottom > self.bottom {
                // rows past the content area of the last frame were its footer
                self.end = self.end.min(self.pos + (h - bottom) as i64);
                self.canvas.truncate(self.end as usize * self.w * 4);
                self.bottom = bottom;
            }
        }
        self.pos += dy as i64;
        let reach = self.pos + (h - self.bottom) as i64;
        let added = (reach - self.end).max(0);
        for p in self.end..reach {
            let y = (p - self.pos) as usize;
            self.canvas.extend_from_slice(&rgba[y * self.w * 4..(y + 1) * self.w * 4]);
        }
        self.end = self.end.max(reach);
        self.last_dy = dy;
        self.prev = Some(cur);
        Step::Moved { dy, added: added as u32 }
    }

    /// The stitched image: the content so far and the last frame's footer.
    pub fn image(&self) -> (u32, u32, Vec<u8>) {
        let mut out = self.canvas.clone();
        if let Some(p) = &self.prev {
            out.extend_from_slice(&p.rgba[(self.h - self.bottom) * self.w * 4..]);
        }
        (self.w as u32, self.height(), out)
    }

    fn frame(&self, rgba: &[u8]) -> Frame {
        let (w, h) = (self.w, self.h);
        let gray: Vec<u8> = rgba.chunks_exact(4).map(|p| ((p[0] as u32 * 77 + p[1] as u32 * 150 + p[2] as u32 * 29) >> 8) as u8).collect();
        let cols = self.cols();
        let mut sig = vec![[0u8; BINS]; h];
        let mut textured = vec![false; h];
        for y in 0..h {
            let row = &gray[y * w..y * w + cols];
            for (k, s) in sig[y].iter_mut().enumerate() {
                let bin = &row[k * cols / BINS..((k + 1) * cols / BINS).max(k * cols / BINS + 1)];
                *s = (bin.iter().map(|&v| v as u32).sum::<u32>() / bin.len() as u32) as u8;
            }
            let (lo, hi) = row.iter().fold((255u8, 0u8), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            textured[y] = hi - lo > 24;
        }
        Frame { rgba: rgba.to_vec(), gray, sig, textured }
    }

    /// Mean grey difference of two rows (scrollbar aside).
    fn row_diff(&self, a: &Frame, ya: usize, b: &Frame, yb: usize) -> f32 {
        let cols = self.cols();
        let (ra, rb) = (&a.gray[ya * self.w..ya * self.w + cols], &b.gray[yb * self.w..yb * self.w + cols]);
        ra.iter().zip(rb).map(|(&p, &q)| (p as i32 - q as i32).unsigned_abs()).sum::<u32>() as f32 / cols as f32
    }

    /// Columns that are matched: a scrollbar's thumb moves with the content,
    /// so the right edge is left out.
    fn cols(&self) -> usize {
        if self.w > 200 { self.w - 24 } else { self.w }
    }

    /// How far the content moved from `prev` to `cur` (cur's row y shows what
    /// prev's row y + dy did), between the known bands, on the rows that moved.
    fn shift(&self, prev: &Frame, cur: &Frame, still: &[bool]) -> Result<i32, Step> {
        let (lo, hi) = (self.top as i32, (self.h - self.bottom) as i32);
        let span = hi - lo;
        if span < MIN_ROWS as i32 * 2 {
            return Err(Step::Lost);
        }
        let reach = span - MIN_ROWS as i32;
        let cost = |dy: i32| -> Option<f32> {
            let (mut sum, mut n) = (0u32, 0usize);
            for y in lo.max(lo - dy)..hi.min(hi - dy) {
                let (a, b) = ((y + dy) as usize, y as usize);
                if still[a] || still[b] || !(prev.textured[a] || cur.textured[b]) {
                    continue;
                }
                sum += prev.sig[a].iter().zip(&cur.sig[b]).map(|(&p, &c)| (p as i32 - c as i32).unsigned_abs()).sum::<u32>();
                n += 1;
            }
            (n >= MIN_ROWS).then(|| sum as f32 / (n * BINS) as f32)
        };
        let costs: Vec<(i32, f32)> = (-reach..=reach).filter_map(|d| cost(d).map(|c| (d, c))).collect();
        let Some(&(best, c1)) = costs.iter().min_by(|a, b| a.1.total_cmp(&b.1)) else {
            return Err(Step::Unsure);
        };
        if c1 > MAX_COST {
            return Err(Step::Lost);
        }
        // the runner-up: the best shift clearly apart from the winner
        let c2 = costs.iter().filter(|(d, _)| (d - best).abs() > 3).map(|c| c.1).fold(f32::INFINITY, f32::min);
        let dy = if c1 > AMBIGUOUS * c2 || c2 < 0.5 {
            // several fit: take the one nearest the last scroll if it's among them
            let near: Vec<i32> = costs.iter().filter(|(_, c)| *c <= c1.max(0.5) / AMBIGUOUS).map(|c| c.0).collect();
            match near.iter().min_by_key(|d| (*d - self.last_dy).abs()) {
                Some(&d) if self.last_dy != 0 && (d - self.last_dy).abs() <= 3 => d,
                _ => return Err(Step::Unsure),
            }
        } else {
            best
        };
        if self.pixel_error(prev, cur, still, dy, lo, hi) > 8.0 {
            return Err(Step::Lost);
        }
        Ok(dy)
    }

    /// Mean grey difference over the overlap's moving textured rows, on every 4th pixel.
    fn pixel_error(&self, prev: &Frame, cur: &Frame, still: &[bool], dy: i32, lo: i32, hi: i32) -> f32 {
        let w = self.w;
        let (mut sum, mut n) = (0u64, 0u64);
        for y in lo.max(lo - dy)..hi.min(hi - dy) {
            let (a, b) = ((y + dy) as usize, y as usize);
            if still[a] || still[b] || !cur.textured[b] {
                continue;
            }
            for x in (0..self.cols()).step_by(4) {
                sum += (prev.gray[a * w + x] as i32 - cur.gray[b * w + x] as i32).unsigned_abs() as u64;
                n += 1;
            }
        }
        if n == 0 { 0.0 } else { sum as f32 / n as f32 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 320;
    const LINE: usize = 24; // one numbered line of the page
    const HEADER: usize = 40;
    const FOOTER: usize = 30;
    const VIEW: usize = 400;

    /// A long page of numbered lines: a 12-bit barcode of the line's index
    /// on the left, text-like noise on the right, white gaps between.
    fn page(lines: usize) -> Vec<u8> {
        let mut px = vec![255u8; W * lines * LINE * 4];
        let mut seed = 7u32;
        for i in 0..lines {
            for y in i * LINE + 3..i * LINE + LINE - 5 {
                for x in 0..W {
                    let v = if x < 12 * 12 {
                        if (i >> (x / 12)) & 1 == 1 { 20 } else { 235 }
                    } else if x < W - 40 {
                        seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                        if (seed >> 16) % 5 == 0 { 40 } else { 250 }
                    } else {
                        255
                    };
                    px[(y * W + x) * 4..(y * W + x) * 4 + 3].copy_from_slice(&[v, v, v]);
                }
            }
        }
        px
    }

    /// The window: a sticky header and footer over the page scrolled to
    /// `pos`, with a scrollbar thumb that follows it, plus `noise`.
    fn frame(page: &[u8], pos: usize, noise: u32) -> Vec<u8> {
        let rows = page.len() / (W * 4);
        let mut f = vec![0u8; W * VIEW * 4];
        let mut seed = pos as u32 * 31 + 1;
        for y in 0..VIEW {
            for x in 0..W {
                let o = (y * W + x) * 4;
                let mut v = if y < HEADER {
                    if (x / 9 + y / 7) % 3 == 0 { 30 } else { 90 }
                } else if y >= VIEW - FOOTER {
                    if (x / 13) % 2 == 0 { 200 } else { 60 }
                } else if x >= W - 10 {
                    let thumb = HEADER + pos * (VIEW - HEADER - FOOTER - 40) / rows;
                    if (thumb..thumb + 40).contains(&y) { 120 } else { 230 }
                } else {
                    page[((pos + y - HEADER) * W + x) * 4]
                };
                if noise > 0 {
                    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
                    v = (v as i32 + ((seed >> 16) % (2 * noise + 1)) as i32 - noise as i32).clamp(0, 255) as u8;
                }
                f[o..o + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        f
    }

    /// The line numbers of the stitched image, top to bottom, read from the
    /// barcodes (one per line), and whether the header and footer are there.
    fn read(img: &[u8], height: usize) -> (Vec<usize>, usize, usize) {
        let row = |y: usize| &img[y * W * 4..(y + 1) * W * 4];
        let near = |v: u8, want: u8| (v as i32 - want as i32).abs() <= 4;
        let header = |y: usize| near(row(y)[0], 30) && near(row(y)[9 * 4], 90);
        let footer = |y: usize| near(row(y)[0], 200) && near(row(y)[13 * 4], 60);
        let headers = (0..height).filter(|&y| y % 7 == 0 && header(y)).count();
        let footers = (0..height).filter(|&y| footer(y)).count();
        let mut lines = vec![];
        let mut y = 0;
        while y < height {
            // a barcode row: the first bit cell is black or near-white, and the row isn't blank
            let r = row(y);
            let dark = (0..W - 40).filter(|x| r[x * 4] < 128).count();
            if y >= HEADER && dark > 0 && !header(y) && !footer(y) {
                let idx = (0..12).fold(0, |acc, b| acc | (((r[(b * 12 + 6) * 4] < 128) as usize) << b));
                lines.push(idx);
                y += LINE - 8; // past this line's bars
                continue;
            }
            y += 1;
        }
        (lines, headers, footers)
    }

    fn stitch(page: &[u8], positions: &[usize], noise: u32) -> (Stitcher, Vec<Step>) {
        let mut s = Stitcher::new(W as u32, VIEW as u32);
        let steps = positions.iter().map(|&p| s.push(&frame(page, p, noise))).collect();
        (s, steps)
    }

    fn check(s: &Stitcher, lines: usize) {
        let (_, h, img) = s.image();
        let (seen, headers, footers) = read(&img, h as usize);
        assert_eq!(seen, (0..lines).collect::<Vec<_>>(), "every line once, in order");
        assert!(headers > 0 && headers <= HEADER / 7 + 1, "the header once ({headers} rows)");
        assert_eq!(footers, FOOTER, "the footer once");
        assert_eq!(h as usize, HEADER + lines * LINE + FOOTER, "no row missing or twice");
    }

    /// Without noise the content is the page itself, pixel for pixel (the
    /// scrollbar aside).
    fn exact(s: &Stitcher, p: &[u8]) {
        let (_, _, img) = s.image();
        let rows = p.len() / (W * 4);
        for y in 0..rows {
            let a = &img[((HEADER + y) * W) * 4..((HEADER + y) * W + W - 10) * 4];
            let b = &p[(y * W) * 4..(y * W + W - 10) * 4];
            assert!(a.chunks(4).zip(b.chunks(4)).all(|(a, b)| a[..3] == b[..3]), "page row {y} differs");
        }
    }

    /// Scroll positions from the top to the end of a page of `total_rows`.
    fn walk(total_rows: usize, steps: &[usize]) -> Vec<usize> {
        let last = total_rows - (VIEW - HEADER - FOOTER);
        let mut pos = vec![0];
        let mut i = 0;
        while *pos.last().unwrap() < last {
            pos.push((pos.last().unwrap() + steps[i % steps.len()]).min(last));
            i += 1;
        }
        pos
    }

    #[test]
    fn a_long_page_stitches_every_line_once() {
        let lines = 120;
        let p = page(lines);
        let (s, steps) = stitch(&p, &walk(lines * LINE, &[71, 72]), 0);
        assert!(steps.iter().skip(1).all(|st| matches!(st, Step::Moved { .. })), "{steps:?}");
        check(&s, lines);
        exact(&s, &p);
    }

    #[test]
    fn resampling_noise_is_tolerated() {
        let lines = 80;
        let p = page(lines);
        let (s, steps) = stitch(&p, &walk(lines * LINE, &[57, 90, 33]), 2);
        assert!(steps.iter().skip(1).all(|st| matches!(st, Step::Moved { .. })), "{steps:?}");
        check(&s, lines);
    }

    #[test]
    fn scrolling_back_and_forth_adds_nothing_twice() {
        let lines = 60;
        let p = page(lines);
        let last = lines * LINE - (VIEW - HEADER - FOOTER);
        let pos = [0, 100, 60, 180, 120, 250, 400, 330, 500, 600, 560, 700, 820, 940, last - 100, last, last - 40, last];
        let (s, steps) = stitch(&p, &pos, 0);
        assert!(matches!(steps[2], Step::Moved { dy: -40, added: 0 }), "{:?}", steps[2]);
        check(&s, lines);
        exact(&s, &p);
    }

    #[test]
    fn a_jump_past_the_view_is_lost_until_scrolled_back() {
        let lines = 40;
        let p = page(lines);
        let last = lines * LINE - (VIEW - HEADER - FOOTER);
        let (s, steps) = stitch(&p, &[0, 100, 600, 190, 280, 370, 460, 550, 620, last], 0);
        assert_eq!(steps[2], Step::Lost, "no overlap with the last frame");
        assert!(steps[3..].iter().all(|st| matches!(st, Step::Moved { .. })), "{steps:?}");
        check(&s, lines);
    }

    #[test]
    fn a_frame_that_did_not_move_is_still() {
        let p = page(30);
        let (s, steps) = stitch(&p, &[0, 0, 80, 80], 0);
        assert_eq!(steps, vec![Step::First, Step::Still, Step::Moved { dy: 80, added: 80 }, Step::Still]);
        assert_eq!(s.height() as usize, VIEW + 80);
    }

    #[test]
    fn repeated_lines_follow_the_last_scroll() {
        let lines = 60;
        let mut p = page(lines);
        // lines 20..40 all look like line 20
        let line = p[20 * LINE * W * 4..21 * LINE * W * 4].to_vec();
        for i in 21..40 {
            p[i * LINE * W * 4..(i + 1) * LINE * W * 4].copy_from_slice(&line);
        }
        let (s, steps) = stitch(&p, &walk(lines * LINE, &[50]), 0);
        assert!(steps.iter().skip(1).all(|st| matches!(st, Step::Moved { .. })), "{steps:?}");
        exact(&s, &p);
    }

    #[test]
    fn a_footer_with_blank_padding_is_kept_whole() {
        // the footer's top rows are as white as the page: they must not be taken for content
        let lines = 50;
        let p = page(lines);
        let mut s = Stitcher::new(W as u32, VIEW as u32);
        for pos in walk(lines * LINE, &[66]) {
            let mut f = frame(&p, pos, 0);
            for y in VIEW - FOOTER - 12..VIEW - FOOTER {
                for x in 0..W {
                    f[(y * W + x) * 4..(y * W + x) * 4 + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
            s.push(&f);
        }
        let (_, h, img) = s.image();
        let (seen, _, _) = read(&img, h as usize);
        assert_eq!(seen, (0..lines).collect::<Vec<_>>());
        assert_eq!(h as usize, HEADER + lines * LINE + FOOTER, "the padding once, with the footer");
    }

    #[test]
    #[ignore = "timing: cargo test --release -p snip-core stitch_speed -- --ignored --nocapture"]
    fn stitch_speed() {
        let (w, h) = (1500usize, 1200usize);
        let mut page = vec![255u8; w * 6000 * 4];
        let mut seed = 3u32;
        for v in page.chunks_mut(4) {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let g = if (seed >> 16) % 7 == 0 { 30 } else { 240 };
            v.copy_from_slice(&[g, g, g, 255]);
        }
        let mut s = Stitcher::new(w as u32, h as u32);
        let t = std::time::Instant::now();
        let mut n = 0;
        for pos in (0..6000 - h).step_by(150) {
            s.push(&page[pos * w * 4..(pos + h) * w * 4]);
            n += 1;
        }
        eprintln!("{:.1} ms per frame ({n} frames)", t.elapsed().as_secs_f64() * 1000.0 / n as f64);
    }

    #[test]
    fn a_blank_gap_shorter_than_the_view_is_crossed() {
        let lines = 50;
        let mut p = page(lines);
        // lines 20..28 blank: 192 rows of white, less than the 330-row view
        for v in &mut p[20 * LINE * W * 4..28 * LINE * W * 4] {
            *v = 255;
        }
        let (s, steps) = stitch(&p, &walk(lines * LINE, &[60]), 0);
        assert!(steps.iter().skip(1).all(|st| matches!(st, Step::Moved { .. })), "{steps:?}");
        exact(&s, &p);
    }
}
