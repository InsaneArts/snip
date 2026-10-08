//! Hide desktop clutter (CAP-12): the space bars reserve on an output, painted
//! over with the wallpaper as omarchy-shell draws it (scaled to cover the
//! output, centred), so a capture shows the desktop without the bar. Spike
//! S03: visually seamless, not bit-exact.

use std::path::Path;

use crate::frame::Frame;
use crate::geometry::Rect;

/// A wallpaper's pixels: RGB, tightly packed.
pub struct Wallpaper {
    pub data: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

/// Decode a JPEG or PNG wallpaper.
pub fn load(path: &Path) -> Option<Wallpaper> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.starts_with(&[0xff, 0xd8]) {
        use zune_jpeg::zune_core::{colorspace::ColorSpace, options::DecoderOptions};
        let mut d = zune_jpeg::JpegDecoder::new_with_options(&bytes, DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB));
        let data = d.decode().ok()?;
        let (w, h) = d.dimensions()?;
        return Some(Wallpaper { data, w: w as u32, h: h as u32 });
    }
    let (data, w, h, alpha) = crate::encode::decode_png(&bytes).ok()?;
    let data = if alpha { data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect() } else { data };
    Some(Wallpaper { data, w, h })
}

/// The bands bars reserve on an output, in upright physical px. `reserved` is
/// Hyprland's left, top, right, bottom (logical px); a band is rounded out so
/// no row of a bar is left.
pub fn bands(reserved: [i32; 4], uw: i32, uh: i32, scale: f64) -> Vec<Rect> {
    let px = |v: i32| ((v as f64 * scale).ceil() as i32).min(uw.max(uh));
    let [l, t, r, b] = reserved.map(px);
    [Rect::new(0, 0, l, uh), Rect::new(0, 0, uw, t), Rect::new(uw - r, 0, r, uh), Rect::new(0, uh - b, uw, b)]
        .into_iter()
        .filter(|r| r.w > 0 && r.h > 0)
        .collect()
}

/// The wallpaper's pixels for `band` of an output `uw`×`uh` (upright
/// physical px): RGB, `band.w`×`band.h`.
pub fn cover_block(wall: &Wallpaper, uw: i32, uh: i32, band: Rect) -> Vec<u8> {
    // scaled to cover the output, centred (QML's PreserveAspectCrop)
    let s = (uw as f64 / wall.w as f64).max(uh as f64 / wall.h as f64);
    let (ox, oy) = ((wall.w as f64 * s - uw as f64) / 2.0, (wall.h as f64 * s - uh as f64) / 2.0);
    // the band's source rectangle, whole pixels
    let sx0 = (((band.x as f64 + ox) / s).floor() as i64).clamp(0, wall.w as i64 - 1) as u32;
    let sy0 = (((band.y as f64 + oy) / s).floor() as i64).clamp(0, wall.h as i64 - 1) as u32;
    let sx1 = (((band.right() as f64 + ox) / s).ceil() as i64).clamp(sx0 as i64 + 1, wall.w as i64) as u32;
    let sy1 = (((band.bottom() as f64 + oy) / s).ceil() as i64).clamp(sy0 as i64 + 1, wall.h as i64) as u32;
    let (cw, ch) = (sx1 - sx0, sy1 - sy0);
    let mut crop = Vec::with_capacity((cw * ch * 3) as usize);
    for y in sy0..sy1 {
        let row = ((y * wall.w + sx0) * 3) as usize;
        crop.extend_from_slice(&wall.data[row..row + (cw * 3) as usize]);
    }
    crate::encode::resize(&crop, cw, ch, 3, band.w as u32, band.h as u32)
}

/// Paint an upright RGB block over `band` (upright physical px) of a frame,
/// whatever its transform.
pub fn paint(frame: &Frame, band: Rect, rgb: &[u8]) {
    let buf = &frame.buf;
    let (bw, bh) = (buf.width as usize, buf.height as usize);
    let (uw, uh) = frame.upright_size();
    // SAFETY: called before the frame is handed to any reader (overlay,
    // encoder); the mapping is writable and outlives this call.
    let px = unsafe { std::slice::from_raw_parts_mut(buf.ptr() as *mut u8, buf.len()) };
    for y in 0..band.h {
        for x in 0..band.w {
            let (ux, uy) = (band.x + x, band.y + y);
            if ux < 0 || uy < 0 || ux >= uw || uy >= uh {
                continue;
            }
            let (bu, bv) = frame.transform.buffer_coord((ux as f64 + 0.5) / uw as f64, (uy as f64 + 0.5) / uh as f64);
            let (bx, by) = (((bu * bw as f64) as usize).min(bw - 1), ((bv * bh as f64) as usize).min(bh - 1));
            let d = by * buf.stride as usize + bx * 4;
            let s = ((y * band.w + x) * 3) as usize;
            // wl_shm (A|X)RGB8888 is B, G, R, A in memory
            px[d..d + 4].copy_from_slice(&[rgb[s + 2], rgb[s + 1], rgb[s], 255]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{PixelFormat, ShmBuf};
    use crate::geometry::Transform;
    use std::sync::Arc;

    #[test]
    fn bands_are_the_reserved_edges_in_physical_px() {
        // Omarchy's bar: 35 logical px at the top of a 1.25 output
        assert_eq!(bands([0, 35, 0, 0], 3440, 1440, 1.25), vec![Rect::new(0, 0, 3440, 44)]);
        assert_eq!(bands([0, 0, 0, 30], 1920, 1080, 1.0), vec![Rect::new(0, 1050, 1920, 30)]);
        assert!(bands([0, 0, 0, 0], 1920, 1080, 1.0).is_empty());
    }

    /// A wallpaper whose red is its column and green its row.
    fn ramp(w: u32, h: u32) -> Wallpaper {
        let data = (0..h).flat_map(|y| (0..w).flat_map(move |x| [x as u8, y as u8, 0])).collect();
        Wallpaper { data, w, h }
    }

    #[test]
    fn the_cover_is_the_wallpaper_scaled_to_cover_the_output_and_centred() {
        // 200×100 on a 100×100 output: scaled ×1, columns 50..150 show
        let wall = ramp(200, 100);
        let block = cover_block(&wall, 100, 100, Rect::new(0, 0, 100, 10));
        assert_eq!((block[0], block[1]), (50, 0), "the left of the band is the wallpaper's column 50");
        let last = (9 * 100 + 99) * 3;
        assert_eq!((block[last], block[last + 1]), (149, 9));
        // 100×50 on a 200×200 output: scaled ×4, the middle 50×50 shows
        let wall = ramp(100, 50);
        let block = cover_block(&wall, 200, 200, Rect::new(0, 0, 200, 8));
        assert_eq!(block.len(), 200 * 8 * 3);
        assert!((block[0] as i32 - 25).abs() <= 1 && block[1] <= 1, "{:?}", &block[..3]);
    }

    #[test]
    fn jpeg_and_png_wallpapers_load() {
        let dir = std::env::temp_dir().join(format!("snip-desktop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rgb: Vec<u8> = (0..48 * 32).flat_map(|_| [30u8, 120, 220]).collect();
        let jpg = dir.join("wall.jpg");
        jpeg_encoder::Encoder::new_file(&jpg, 95).unwrap().encode(&rgb, 48, 32, jpeg_encoder::ColorType::Rgb).unwrap();
        let png = dir.join("wall.png");
        std::fs::write(&png, crate::encode::encode_png(&rgb, 48, 32, false)).unwrap();
        for p in [jpg, png] {
            let w = load(&p).unwrap();
            assert_eq!((w.w, w.h, w.data.len()), (48, 32, 48 * 32 * 3), "{p:?}");
            assert!(w.data[..3].iter().zip([30, 120, 220]).all(|(a, b)| (*a as i32 - b).abs() <= 3), "{p:?}: {:?}", &w.data[..3]);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn frame(w: u32, h: u32, t: Transform) -> Frame {
        Frame { output: "T".into(), buf: Arc::new(ShmBuf::new(w, h, PixelFormat::Xrgb8888).unwrap()), transform: t, scale: 1.0, logical: Rect::new(0, 0, w as i32, h as i32), epoch: 1 }
    }

    fn upright_bgr(f: &Frame, x: i32, y: i32) -> [u8; 3] {
        let (uw, uh) = f.upright_size();
        let (bu, bv) = f.transform.buffer_coord((x as f64 + 0.5) / uw as f64, (y as f64 + 0.5) / uh as f64);
        let (bx, by) = ((bu * f.buf.width as f64) as usize, (bv * f.buf.height as f64) as usize);
        let d = by * f.buf.stride as usize + bx * 4;
        let b = f.buf.bytes();
        [b[d], b[d + 1], b[d + 2]]
    }

    #[test]
    fn paint_lands_on_the_upright_band_of_a_rotated_frame() {
        for t in [Transform::Normal, Transform::R90, Transform::Flipped270] {
            let f = frame(40, 30, t); // buffer 40×30
            let (uw, _) = f.upright_size();
            let band = Rect::new(0, 0, uw, 2);
            let rgb: Vec<u8> = (0..band.w * band.h).flat_map(|_| [200u8, 100, 50]).collect();
            paint(&f, band, &rgb);
            assert_eq!(upright_bgr(&f, 3, 1), [50, 100, 200], "{t:?}: the top band is painted");
            assert_eq!(upright_bgr(&f, 3, 2), [0, 0, 0], "{t:?}: the row below isn't");
        }
    }
}
