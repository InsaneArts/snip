//! Window captures (CAP-05): the toplevel buffer has square corners and no
//! shadow, because Hyprland draws both at composite time. We draw them, so a
//! window shot looks like the window on screen (CleanShot does the same).

/// Corner shape of Hyprland's `decoration:rounding` / `rounding_power`
/// (a superellipse; power 2 is a circle). `radius` is in buffer pixels.
#[derive(Clone, Copy, Debug)]
pub struct Corners {
    pub radius: f64,
    pub power: f64,
}

/// Soft drop shadow, in buffer pixels (design doc §3 `shadow.float`).
#[derive(Clone, Copy, Debug)]
pub struct Shadow {
    pub blur: f64,
    pub offset_y: f64,
    pub opacity: f64,
}

impl Shadow {
    /// The design token, scaled to the capture's pixel density.
    pub fn float(scale: f64) -> Self {
        Self { blur: 40.0 * scale, offset_y: 12.0 * scale, opacity: 0.45 }
    }
}

/// Coverage (0..1) of the window shape at pixel centre (x, y).
fn coverage(x: f64, y: f64, w: f64, h: f64, c: Corners) -> f64 {
    let r = c.radius.min(w / 2.0).min(h / 2.0);
    if r < 0.5 {
        return 1.0;
    }
    let cx = if x < r { r } else if x > w - r { w - r } else { return 1.0 };
    let cy = if y < r { r } else if y > h - r { h - r } else { return 1.0 };
    let (dx, dy) = ((x - cx).abs(), (y - cy).abs());
    let p = c.power.max(1.0);
    let d = (dx.powf(p) + dy.powf(p)).powf(1.0 / p);
    (r - d + 0.5).clamp(0.0, 1.0)
}

/// Apply the corner mask to tightly packed RGBA in place.
pub fn round_corners(rgba: &mut [u8], w: u32, h: u32, c: Corners) {
    let r = c.radius.ceil() as u32 + 1;
    if c.radius < 0.5 {
        return;
    }
    let (fw, fh) = (w as f64, h as f64);
    let mut apply = |x: u32, y: u32| {
        let a = coverage(x as f64 + 0.5, y as f64 + 0.5, fw, fh, c);
        if a < 1.0 {
            let i = ((y * w + x) * 4 + 3) as usize;
            rgba[i] = (rgba[i] as f64 * a).round() as u8;
        }
    };
    for y in 0..h.min(r) {
        for x in (0..w.min(r)).chain(w.saturating_sub(r).max(r.min(w))..w) {
            apply(x, y);
        }
    }
    for y in h.saturating_sub(r).max(r.min(h))..h {
        for x in (0..w.min(r)).chain(w.saturating_sub(r).max(r.min(w))..w) {
            apply(x, y);
        }
    }
}

/// One horizontal + vertical box-blur pass on an f32 plane (running sums).
fn box_blur(src: &mut Vec<f32>, tmp: &mut Vec<f32>, w: usize, h: usize, r: usize) {
    if r == 0 {
        return;
    }
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut tmp[y * w..(y + 1) * w];
        let mut acc: f32 = row[..r.min(w)].iter().sum();
        for x in 0..w {
            if x + r < w {
                acc += row[x + r];
            }
            out[x] = acc * norm;
            if x >= r {
                acc -= row[x - r];
            }
        }
    }
    for x in 0..w {
        let mut acc: f32 = (0..r.min(h)).map(|y| tmp[y * w + x]).sum();
        for y in 0..h {
            if y + r < h {
                acc += tmp[(y + r) * w + x];
            }
            src[y * w + x] = acc * norm;
            if y >= r {
                acc -= tmp[(y - r) * w + x];
            }
        }
    }
}

/// Window capture -> rounded corners and an optional soft shadow on a
/// transparent canvas. Returns (rgba, width, height).
pub fn decorate(rgba: Vec<u8>, w: u32, h: u32, corners: Option<Corners>, shadow: Option<Shadow>) -> (Vec<u8>, u32, u32) {
    let mut rgba = rgba;
    let c = corners.unwrap_or(Corners { radius: 0.0, power: 2.0 });
    round_corners(&mut rgba, w, h, c);
    let Some(s) = shadow else { return (rgba, w, h) };

    // Three box passes approximate a Gaussian with sigma = blur / 2.
    let sigma = (s.blur / 2.0).max(0.5);
    let box_r = (((12.0 * sigma * sigma / 3.0 + 1.0).sqrt() - 1.0) / 2.0).round().max(1.0) as usize;
    let spread = 3 * box_r;
    let pad = spread as u32 + 1;
    let off = s.offset_y.round() as u32;
    let (ow, oh) = (w + 2 * pad, h + 2 * pad + off);

    // The shadow is cast by the window *shape*, not its (possibly translucent) pixels.
    let (fw, fh) = (w as f64, h as f64);
    let mut plane = vec![0f32; (ow * oh) as usize];
    for y in 0..h {
        let row = ((y + pad + off) * ow + pad) as usize;
        for x in 0..w {
            plane[row + x as usize] = coverage(x as f64 + 0.5, y as f64 + 0.5, fw, fh, c) as f32;
        }
    }
    let mut tmp = vec![0f32; plane.len()];
    for _ in 0..3 {
        box_blur(&mut plane, &mut tmp, ow as usize, oh as usize, box_r);
    }

    // Composite the window over the shadow (straight alpha).
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    let op = s.opacity as f32;
    for (i, a) in plane.iter().enumerate() {
        out[i * 4 + 3] = (a * op * 255.0).round().min(255.0) as u8;
    }
    for y in 0..h {
        for x in 0..w {
            let si = ((y * w + x) * 4) as usize;
            let di = (((y + pad) * ow + x + pad) * 4) as usize;
            let sa = rgba[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            let da = out[di + 3] as f32 / 255.0;
            let oa = sa + da * (1.0 - sa);
            for k in 0..3 {
                // shadow colour is black, so only the source contributes colour
                out[di + k] = ((rgba[si + k] as f32 * sa) / oa).round().min(255.0) as u8;
            }
            out[di + 3] = (oa * 255.0).round() as u8;
        }
    }
    (out, ow, oh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_are_cut_and_centre_kept() {
        let (w, h) = (40u32, 30u32);
        let mut px = vec![255u8; (w * h * 4) as usize];
        round_corners(&mut px, w, h, Corners { radius: 10.0, power: 2.0 });
        let a = |x: u32, y: u32| px[((y * w + x) * 4 + 3) as usize];
        assert_eq!(a(0, 0), 0);
        assert_eq!(a(w - 1, h - 1), 0);
        assert_eq!(a(20, 15), 255);
        assert_eq!(a(20, 0), 255); // straight edge untouched
        assert_eq!(a(0, 15), 255);
    }

    #[test]
    fn superellipse_cuts_less_than_circle() {
        let circle = coverage(2.5, 2.5, 100.0, 100.0, Corners { radius: 10.0, power: 2.0 });
        let squircle = coverage(2.5, 2.5, 100.0, 100.0, Corners { radius: 10.0, power: 4.0 });
        assert!(squircle > circle);
    }

    #[test]
    fn shadow_pads_canvas_and_keeps_window_opaque() {
        let (w, h) = (64u32, 48u32);
        let px = vec![200u8; (w * h * 4) as usize].chunks(4).flat_map(|_| [200u8, 100, 50, 255]).collect();
        let (out, ow, oh) = decorate(px, w, h, Some(Corners { radius: 8.0, power: 2.0 }), Some(Shadow { blur: 12.0, offset_y: 4.0, opacity: 0.5 }));
        assert!(ow > w && oh > h + 4);
        let pad = (ow - w) / 2;
        let at = |x: u32, y: u32| &out[((y * ow + x) * 4) as usize..((y * ow + x) * 4 + 4) as usize];
        assert_eq!(at(pad + w / 2, pad + h / 2), &[200, 100, 50, 255]);
        assert_eq!(at(0, 0)[3], 0); // far corner fully transparent
        let below = at(pad + w / 2, pad + h + 4)[3];
        let above = at(pad + w / 2, pad.saturating_sub(4))[3];
        assert!(below > above, "shadow is offset downward ({below} vs {above})");
    }
}
