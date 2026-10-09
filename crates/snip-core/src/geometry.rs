//! Physical/logical geometry and wl_output transforms.
//!
//! Selections are held in **physical** pixels of the *upright* (as-displayed)
//! output image. Captured buffers are in the panel's native orientation with
//! the frame's wl_output transform applied to their contents
//! (ext-image-copy-capture `frame.transform`), so cropping maps the upright
//! rectangle into buffer space and un-rotates the pixels afterwards.

/// wl_output.transform values (0..=7). Rotations are counter-clockwise; the
/// flipped variants flip around the vertical axis first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Transform {
    #[default]
    Normal,
    R90,
    R180,
    R270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

impl Transform {
    pub fn from_wl(v: u32) -> Self {
        match v {
            1 => Self::R90,
            2 => Self::R180,
            3 => Self::R270,
            4 => Self::Flipped,
            5 => Self::Flipped90,
            6 => Self::Flipped180,
            7 => Self::Flipped270,
            _ => Self::Normal,
        }
    }

    pub fn to_wl(self) -> u32 {
        self as u32
    }

    fn rotation(self) -> u32 {
        (self as u32) % 4
    }

    fn flipped(self) -> bool {
        (self as u32) >= 4
    }

    /// Whether width and height swap between buffer and upright image.
    pub fn swaps_axes(self) -> bool {
        self.rotation() % 2 == 1
    }

    /// Map a normalised upright point (u, v) in [0,1]^2 to the normalised
    /// buffer coordinate holding that pixel: buffer = rot_ccw^k(flip?(content)).
    pub fn buffer_coord(self, u: f64, v: f64) -> (f64, f64) {
        let u = if self.flipped() { 1.0 - u } else { u };
        match self.rotation() {
            1 => (v, 1.0 - u),
            2 => (1.0 - u, 1.0 - v),
            3 => (1.0 - v, u),
            _ => (u, v),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }

    pub fn area(&self) -> i64 {
        self.w.max(0) as i64 * self.h.max(0) as i64
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.right() && py < self.bottom()
    }

    /// Intersection, or an empty rect.
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        if x1 <= x0 || y1 <= y0 {
            Rect::default()
        } else {
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    }
}

/// Logical (compositor) rectangle with fractional coordinates, e.g. from
/// pointer positions (wl_fixed = 1/256 px).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Logical -> physical with **edges rounded independently** (like Hyprland's
/// CBox::round), so adjacent selections tile without gaps or overlaps.
pub fn to_physical(r: RectF, scale: f64) -> Rect {
    let x0 = (r.x * scale).round() as i32;
    let y0 = (r.y * scale).round() as i32;
    let x1 = ((r.x + r.w) * scale).round() as i32;
    let y1 = ((r.y + r.h) * scale).round() as i32;
    Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
}

/// A rect of whole logical px on an output -> physical px, edges rounded as
/// `to_physical` does: the overlay, `snip area` and recording all agree on
/// the pixels of the same logical rect. (Rounding x, y, w, h each on its own
/// can be a pixel off: at 1.25, x = 1, w = 1 is 1 px wide, not 2.)
pub fn logical_to_physical(r: Rect, scale: f64) -> Rect {
    to_physical(RectF { x: r.x as f64, y: r.y as f64, w: r.w as f64, h: r.h as f64 }, scale)
}

/// Physical px -> whole logical px, edges rounded: the inverse of
/// `logical_to_physical` for the rects it makes, at any scale of 1 or more.
pub fn physical_to_logical(r: Rect, scale: f64) -> Rect {
    let s = if scale > 0.0 { scale } else { 1.0 };
    let x0 = (r.x as f64 / s).round() as i32;
    let y0 = (r.y as f64 / s).round() as i32;
    let x1 = (r.right() as f64 / s).round() as i32;
    let y1 = (r.bottom() as f64 / s).round() as i32;
    Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
}

/// Size of the upright image for a buffer of `bw` x `bh` with transform `t`.
pub fn upright_size(bw: i32, bh: i32, t: Transform) -> (i32, i32) {
    if t.swaps_axes() {
        (bh, bw)
    } else {
        (bw, bh)
    }
}

/// Map an upright rectangle (physical px) to the buffer-space rectangle that
/// holds the same pixels.
pub fn upright_to_buffer(r: Rect, bw: i32, bh: i32, t: Transform) -> Rect {
    let (uw, uh) = upright_size(bw, bh, t);
    let corners = [(r.x, r.y), (r.right(), r.y), (r.x, r.bottom()), (r.right(), r.bottom())];
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (cx, cy) in corners {
        let (bu, bv) = t.buffer_coord(cx as f64 / uw as f64, cy as f64 / uh as f64);
        let (bx, by) = ((bu * bw as f64).round() as i32, (bv * bh as f64).round() as i32);
        x0 = x0.min(bx);
        y0 = y0.min(by);
        x1 = x1.max(bx);
        y1 = y1.max(by);
    }
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// Un-rotate a tightly packed pixel block (`bpp` bytes per pixel) cut from a
/// transformed buffer, producing the upright image. `w`,`h` are the block's
/// buffer-space size; returns (data, upright_w, upright_h).
pub fn untransform_block(src: &[u8], w: usize, h: usize, bpp: usize, t: Transform) -> (Vec<u8>, usize, usize) {
    if t == Transform::Normal {
        return (src.to_vec(), w, h);
    }
    let (uw, uh) = if t.swaps_axes() { (h, w) } else { (w, h) };
    let mut out = vec![0u8; uw * uh * bpp];
    for uy in 0..uh {
        for ux in 0..uw {
            // centre of the upright pixel, normalised
            let (bu, bv) = t.buffer_coord((ux as f64 + 0.5) / uw as f64, (uy as f64 + 0.5) / uh as f64);
            let bx = ((bu * w as f64).floor() as usize).min(w - 1);
            let by = ((bv * h as f64).floor() as usize).min(h - 1);
            let s = (by * w + bx) * bpp;
            let d = (uy * uw + ux) * bpp;
            out[d..d + bpp].copy_from_slice(&src[s..s + bpp]);
        }
    }
    (out, uw, uh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_and_physical_px_convert_by_edges_both_ways() {
        // by edges, not by value: x = 1, w = 1 at 1.25 covers physical 1..3
        assert_eq!(logical_to_physical(Rect::new(1, 1, 1, 1), 1.25), Rect::new(1, 1, 2, 2));
        // neighbours share an edge: no gap, no overlap
        let (a, b) = (logical_to_physical(Rect::new(0, 0, 3, 1), 1.25), logical_to_physical(Rect::new(3, 0, 5, 1), 1.25));
        assert_eq!(a.right(), b.x);
        // the inverse gives back what went in, at every scale Hyprland offers
        for scale in [1.0, 1.25, 1.5, 1.6, 5.0 / 3.0, 1.75, 2.0, 2.4, 3.0] {
            for (x, y, w, h) in [(0, 0, 1536, 864), (1, 1, 1, 1), (307, 173, 922, 518), (17, 3, 9, 41)] {
                let r = Rect::new(x, y, w, h);
                assert_eq!(physical_to_logical(logical_to_physical(r, scale), scale), r, "{r:?} at {scale}");
            }
        }
    }

    #[test]
    fn rounding_matches_hyprland_edge_rounding() {
        // research table: scale 1.25, y=37 h=77 -> 46.25..142.5 -> 46, h=97 (edge rounding)
        let r = to_physical(RectF { x: 0.0, y: 37.0, w: 10.0, h: 77.0 }, 1.25);
        assert_eq!((r.y, r.h), (46, 97));
        // scale 1.5, x=3 w=5 -> 4.5..12 -> 5 (round half away), w=7
        let r = to_physical(RectF { x: 3.0, y: 0.0, w: 5.0, h: 1.0 }, 1.5);
        assert_eq!((r.x, r.w), (5, 7));
        // scale 5/3, x=2 w=1 -> 3.33..5 -> 3, w=2
        let r = to_physical(RectF { x: 2.0, y: 0.0, w: 1.0, h: 1.0 }, 5.0 / 3.0);
        assert_eq!((r.x, r.w), (3, 2));
    }

    #[test]
    fn adjacent_selections_tile() {
        for scale in [1.0, 1.25, 1.5, 5.0 / 3.0, 2.0] {
            let a = to_physical(RectF { x: 10.3, y: 0.0, w: 7.7, h: 1.0 }, scale);
            let b = to_physical(RectF { x: 18.0, y: 0.0, w: 9.1, h: 1.0 }, scale);
            assert_eq!(a.right(), b.x, "gap/overlap at scale {scale}");
        }
    }

    #[test]
    fn buffer_coord_identity_and_inverse_pairs() {
        assert_eq!(Transform::Normal.buffer_coord(0.25, 0.75), (0.25, 0.75));
        // 90 then 270 are inverses on the corner set
        for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (0.3, 0.6)] {
            let (a, b) = Transform::R90.buffer_coord(u, v);
            let (c, d) = Transform::R270.buffer_coord(a, b);
            assert!((c - u).abs() < 1e-9 && (d - v).abs() < 1e-9);
        }
    }

    /// Build a buffer the way the compositor would for transform `t` from an
    /// upright image, then check crop + untransform returns the upright block.
    fn roundtrip(t: Transform) {
        let (uw, uh) = (7usize, 5usize);
        let upright: Vec<u8> = (0..uw * uh).map(|i| i as u8).collect();
        let (bw, bh) = if t.swaps_axes() { (uh, uw) } else { (uw, uh) };
        let mut buf = vec![0u8; bw * bh];
        for uy in 0..uh {
            for ux in 0..uw {
                let (bu, bv) = t.buffer_coord((ux as f64 + 0.5) / uw as f64, (uy as f64 + 0.5) / uh as f64);
                let (bx, by) = ((bu * bw as f64) as usize, (bv * bh as f64) as usize);
                buf[by * bw + bx] = upright[uy * uw + ux];
            }
        }
        let sel = Rect::new(2, 1, 3, 3);
        let br = upright_to_buffer(sel, bw as i32, bh as i32, t);
        let mut block = Vec::new();
        for y in br.y..br.bottom() {
            for x in br.x..br.right() {
                block.push(buf[y as usize * bw + x as usize]);
            }
        }
        let (out, ow, oh) = untransform_block(&block, br.w as usize, br.h as usize, 1, t);
        assert_eq!((ow, oh), (3, 3), "{t:?}");
        let mut want = Vec::new();
        for y in 1..4 {
            for x in 2..5 {
                want.push(upright[y * uw + x]);
            }
        }
        assert_eq!(out, want, "{t:?}");
    }

    #[test]
    fn crop_untransform_roundtrip_all_transforms() {
        for v in 0..8 {
            roundtrip(Transform::from_wl(v));
        }
    }

    #[test]
    fn rect_ops() {
        let a = Rect::new(0, 0, 10, 10);
        assert_eq!(a.intersect(&Rect::new(5, 5, 10, 10)), Rect::new(5, 5, 5, 5));
        assert!(a.intersect(&Rect::new(20, 20, 1, 1)).is_empty());
        assert!(a.contains(9, 9) && !a.contains(10, 0));
    }
}
