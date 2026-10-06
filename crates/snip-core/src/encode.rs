//! Crop a frame (upright physical rectangle), un-rotate, and encode PNG on the
//! fast path (png crate + fdeflate: ~1 GP/s, spike S01/S12 research).

use crate::frame::{Frame, PixelFormat};
use crate::geometry::{untransform_block, upright_to_buffer, Rect};

pub struct Encoded {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub crop_ms: f64,
    pub encode_ms: f64,
}

/// Tightly packed upright pixels (RGB or RGBA) of `sel` (upright physical px).
pub fn crop_upright(frame: &Frame, sel: Rect) -> (Vec<u8>, u32, u32, bool) {
    let buf = &frame.buf;
    let (bw, bh) = (buf.width as i32, buf.height as i32);
    let (uw, uh) = frame.upright_size();
    let sel = sel.intersect(&Rect::new(0, 0, uw, uh));
    let br = upright_to_buffer(sel, bw, bh, frame.transform).intersect(&Rect::new(0, 0, bw, bh));
    let alpha = buf.format.has_alpha();
    let bpp = if alpha { 4 } else { 3 };
    let (src, stride) = (buf.bytes(), buf.stride as usize);
    let mut block = Vec::with_capacity(br.area() as usize * bpp);
    for row in br.y..br.bottom() {
        let start = row as usize * stride + br.x as usize * 4;
        let line = &src[start..start + br.w as usize * 4];
        for px in line.chunks_exact(4) {
            // BGRA/BGRX in memory -> RGB(A)
            block.extend_from_slice(&[px[2], px[1], px[0]]);
            if alpha {
                block.push(px[3]);
            }
        }
    }
    let (data, w, h) = untransform_block(&block, br.w as usize, br.h as usize, bpp, frame.transform);
    (data, w as u32, h as u32, alpha)
}

pub fn encode_png(data: &[u8], width: u32, height: u32, alpha: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() / 3);
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(if alpha { png::ColorType::Rgba } else { png::ColorType::Rgb });
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut w = enc.write_header().expect("png header");
        w.write_image_data(data).expect("png data");
    }
    out
}

pub fn crop_and_encode(frame: &Frame, sel: Rect) -> Encoded {
    let t0 = std::time::Instant::now();
    let (data, w, h, alpha) = crop_upright(frame, sel);
    let t1 = std::time::Instant::now();
    let png = encode_png(&data, w, h, alpha);
    Encoded {
        png,
        width: w,
        height: h,
        crop_ms: (t1 - t0).as_secs_f64() * 1e3,
        encode_ms: t1.elapsed().as_secs_f64() * 1e3,
    }
}

/// Ignore the X byte of XRGB when it isn't meaningful; used by tests.
pub fn is_opaque_format(f: PixelFormat) -> bool {
    !f.has_alpha()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::ShmBuf;
    use crate::geometry::Transform;
    use std::sync::Arc;

    fn synthetic(w: u32, h: u32, t: Transform) -> Frame {
        let mut b = ShmBuf::new(w, h, PixelFormat::Xrgb8888).unwrap();
        let bytes = b.bytes_mut();
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                bytes[i..i + 4].copy_from_slice(&[x as u8, y as u8, 7, 255]); // B=x, G=y, R=7
            }
        }
        Frame { output: "T".into(), buf: Arc::new(b), transform: t, scale: 1.0, logical: Rect::new(0, 0, w as i32, h as i32), epoch: 1 }
    }

    #[test]
    fn crop_normal_converts_bgrx_to_rgb() {
        let f = synthetic(16, 8, Transform::Normal);
        let (d, w, h, a) = crop_upright(&f, Rect::new(3, 2, 4, 2));
        assert_eq!((w, h, a), (4, 2, false));
        assert_eq!(&d[0..3], &[7, 2, 3]); // R=7, G=y=2, B=x=3
    }

    #[test]
    fn crop_is_clamped_to_frame() {
        let f = synthetic(16, 8, Transform::Normal);
        let (_, w, h, _) = crop_upright(&f, Rect::new(10, 6, 100, 100));
        assert_eq!((w, h), (6, 2));
    }

    #[test]
    fn rotated_frame_crop_is_upright_sized() {
        // buffer 16x8 with 90° -> upright 8x16
        let f = synthetic(16, 8, Transform::R90);
        assert_eq!(f.upright_size(), (8, 16));
        let (_, w, h, _) = crop_upright(&f, Rect::new(1, 2, 3, 5));
        assert_eq!((w, h), (3, 5));
    }

    #[test]
    fn png_roundtrip_header() {
        let f = synthetic(32, 16, Transform::Normal);
        let e = crop_and_encode(&f, Rect::new(0, 0, 32, 16));
        assert_eq!(&e.png[1..4], b"PNG");
        assert_eq!((e.width, e.height), (32, 16));
    }
}
