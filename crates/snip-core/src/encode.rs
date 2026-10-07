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

/// Decode a PNG into tightly packed RGB or RGBA (8-bit).
pub fn decode_png(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32, bool), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("png too large")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    buf.truncate(info.buffer_size());
    let (buf, alpha) = match info.color_type {
        png::ColorType::Rgba => (buf, true),
        png::ColorType::Rgb => (buf, false),
        png::ColorType::GrayscaleAlpha => (buf.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(), true),
        _ => (buf.iter().flat_map(|&g| [g, g, g]).collect(), false),
    };
    Ok((buf, info.width, info.height, alpha))
}

/// Export format for Save (SET-06). The history keeps PNG; Save transcodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    Png,
    /// quality 1..=100; transparency is flattened onto white
    Jpg(u8),
    /// lossless
    Webp,
}

impl Export {
    pub fn ext(self) -> &'static str {
        match self {
            Export::Png => "png",
            Export::Jpg(_) => "jpg",
            Export::Webp => "webp",
        }
    }
}

/// PNG bytes -> the export format.
pub fn transcode(png_bytes: &[u8], to: Export) -> Result<Vec<u8>, String> {
    if to == Export::Png {
        return Ok(png_bytes.to_vec());
    }
    let (px, w, h, alpha) = decode_png(png_bytes)?;
    let mut out = Vec::new();
    match to {
        Export::Png => unreachable!(),
        Export::Jpg(q) => {
            let rgb: Vec<u8> = if alpha {
                px.chunks_exact(4)
                    .flat_map(|p| {
                        let a = p[3] as u32;
                        let f = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                        [f(p[0]), f(p[1]), f(p[2])]
                    })
                    .collect()
            } else {
                px
            };
            let enc = jpeg_encoder::Encoder::new(&mut out, q.clamp(1, 100));
            enc.encode(&rgb, w as u16, h as u16, jpeg_encoder::ColorType::Rgb).map_err(|e| e.to_string())?;
        }
        Export::Webp => {
            let enc = image_webp::WebPEncoder::new(&mut out);
            let ct = if alpha { image_webp::ColorType::Rgba8 } else { image_webp::ColorType::Rgb8 };
            enc.encode(&px, w, h, ct).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

/// Rotate tightly packed pixels 90° counter-clockwise (CleanShot's "Rotate Left").
pub fn rotate_left(data: &[u8], w: u32, h: u32, bpp: usize) -> (Vec<u8>, u32, u32) {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; data.len()];
    for y in 0..h {
        for x in 0..w {
            // (x, y) -> (y, w - 1 - x) in a h-wide image
            let (nx, ny) = (y, w - 1 - x);
            let s = (y * w + x) * bpp;
            let d = (ny * h + nx) * bpp;
            out[d..d + bpp].copy_from_slice(&data[s..s + bpp]);
        }
    }
    (out, h as u32, w as u32)
}

/// Mirror tightly packed pixels left-right ("Flip Horizontal").
pub fn flip_horizontal(data: &[u8], w: u32, h: u32, bpp: usize) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let mut out = vec![0u8; data.len()];
    for y in 0..h {
        for x in 0..w {
            let s = (y * w + x) * bpp;
            let d = (y * w + (w - 1 - x)) * bpp;
            out[d..d + bpp].copy_from_slice(&data[s..s + bpp]);
        }
    }
    out
}

/// Mirror tightly packed pixels top-bottom ("Flip Vertical").
pub fn flip_vertical(data: &[u8], w: u32, bpp: usize) -> Vec<u8> {
    let row = w as usize * bpp;
    data.chunks_exact(row).rev().flatten().copied().collect()
}

/// Rotate tightly packed pixels 90° clockwise. Returns the new width and height.
pub fn rotate_right(data: &[u8], w: u32, h: u32, bpp: usize) -> (Vec<u8>, u32, u32) {
    let (l, w2, h2) = rotate_left(data, w, h, bpp);
    let (flipped, fw, fh) = (flip_horizontal(&flip_vertical(&l, w2, bpp), w2, h2, bpp), w2, h2);
    (flipped, fw, fh)
}

/// Resize tightly packed pixels: an area average when shrinking, bilinear
/// when growing (the editor's Resize, ANN-11).
pub fn resize(data: &[u8], w: u32, h: u32, bpp: usize, nw: u32, nh: u32) -> Vec<u8> {
    let (w, h, nw, nh) = (w as usize, h as usize, nw.max(1) as usize, nh.max(1) as usize);
    let px = |x: usize, y: usize, c: usize| data[(y.min(h - 1) * w + x.min(w - 1)) * bpp + c] as f32;
    let (fx, fy) = (w as f32 / nw as f32, h as f32 / nh as f32);
    let mut out = vec![0u8; nw * nh * bpp];
    for y in 0..nh {
        for x in 0..nw {
            for c in 0..bpp {
                let v = if fx >= 1.0 && fy >= 1.0 {
                    // every source pixel the output pixel covers
                    let (x0, x1) = ((x as f32 * fx) as usize, (((x + 1) as f32 * fx).ceil() as usize).min(w));
                    let (y0, y1) = ((y as f32 * fy) as usize, (((y + 1) as f32 * fy).ceil() as usize).min(h));
                    let mut sum = 0.0;
                    for sy in y0..y1.max(y0 + 1) {
                        for sx in x0..x1.max(x0 + 1) {
                            sum += px(sx, sy, c);
                        }
                    }
                    sum / ((y1.max(y0 + 1) - y0) * (x1.max(x0 + 1) - x0)) as f32
                } else {
                    let (sx, sy) = (((x as f32 + 0.5) * fx - 0.5).max(0.0), ((y as f32 + 0.5) * fy - 0.5).max(0.0));
                    let (x0, y0) = (sx as usize, sy as usize);
                    let (tx, ty) = (sx - x0 as f32, sy - y0 as f32);
                    let top = px(x0, y0, c) * (1.0 - tx) + px(x0 + 1, y0, c) * tx;
                    let bot = px(x0, y0 + 1, c) * (1.0 - tx) + px(x0 + 1, y0 + 1, c) * tx;
                    top * (1.0 - ty) + bot * ty
                };
                out[(y * nw + x) * bpp + c] = v.round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Ignore the X byte of XRGB when it isn't meaningful; used by tests.
pub fn is_opaque_format(f: PixelFormat) -> bool {
    !f.has_alpha()
}

#[cfg(test)]
mod editor_transform_tests {
    use super::*;

    #[test]
    fn rotate_right_is_three_lefts_and_flips_mirror() {
        let (w, h) = (3u32, 2u32);
        let px: Vec<u8> = (0..6).collect(); // one byte per pixel
        let (r, rw, rh) = rotate_right(&px, w, h, 1);
        let (l1, a, b) = rotate_left(&px, w, h, 1);
        let (l2, c, d) = rotate_left(&l1, a, b, 1);
        let (l3, e, f) = rotate_left(&l2, c, d, 1);
        assert_eq!((r, rw, rh), (l3, e, f));
        assert_eq!(flip_vertical(&px, w, 1), vec![3, 4, 5, 0, 1, 2]);
    }

    #[test]
    fn resize_averages_down_and_keeps_size_up() {
        // 4 x 2 RGB: left half black, right half white -> 2 x 1
        let px: Vec<u8> = [0u8, 0, 255, 255, 0, 0, 255, 255].iter().flat_map(|&v| [v, v, v]).collect();
        assert_eq!(resize(&px, 4, 2, 3, 2, 1), vec![0, 0, 0, 255, 255, 255]);
        assert_eq!(resize(&px, 4, 2, 3, 8, 4).len(), 8 * 4 * 3);
        assert_eq!(resize(&px, 4, 2, 3, 4, 2), px, "same size, same pixels");
    }
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
    fn transcodes_to_jpg_and_webp() {
        let f = synthetic(64, 32, Transform::Normal);
        let png = crop_and_encode(&f, Rect::new(0, 0, 64, 32)).png;
        let jpg = transcode(&png, Export::Jpg(90)).unwrap();
        assert_eq!(&jpg[..3], &[0xFF, 0xD8, 0xFF]);
        let webp = transcode(&png, Export::Webp).unwrap();
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
        let (_, w, h, _) = decode_png(&png).unwrap();
        assert_eq!((w, h), (64, 32));
    }

    #[test]
    fn rotate_left_and_flip() {
        // 3x2 RGB: values identify pixels
        let px: Vec<u8> = (0..6u8).flat_map(|i| [i, 0, 0]).collect();
        let (r, w, h) = rotate_left(&px, 3, 2, 3);
        assert_eq!((w, h), (2, 3));
        // top-right pixel (index 2) goes to the top-left
        assert_eq!(r[0], 2);
        // four rotations are the identity
        let (r2, w2, h2) = rotate_left(&r, w, h, 3);
        let (r3, w3, h3) = rotate_left(&r2, w2, h2, 3);
        let (r4, _, _) = rotate_left(&r3, w3, h3, 3);
        assert_eq!(r4, px);
        let f = flip_horizontal(&px, 3, 2, 3);
        assert_eq!((f[0], f[6]), (2, 0));
        assert_eq!(flip_horizontal(&f, 3, 2, 3), px);
    }

    #[test]
    fn png_roundtrip_header() {
        let f = synthetic(32, 16, Transform::Normal);
        let e = crop_and_encode(&f, Rect::new(0, 0, 32, 16));
        assert_eq!(&e.png[1..4], b"PNG");
        assert_eq!((e.width, e.height), (32, 16));
    }
}
