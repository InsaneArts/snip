//! Crop a BGRX frame and encode it as PNG (fdeflate fast path).

use crate::clock::now_ns;
use crate::frames::ShmBuf;

pub struct Encoded {
    pub png: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub crop_ms: f64,
    pub encode_ms: f64,
}

pub fn crop_png(b: &ShmBuf, x: i32, y: i32, w: i32, h: i32) -> Encoded {
    let t0 = now_ns();
    let x = x.clamp(0, b.width as i32) as usize;
    let y = y.clamp(0, b.height as i32) as usize;
    let w = (w.max(1) as usize).min(b.width as usize - x);
    let h = (h.max(1) as usize).min(b.height as usize - y);
    let (src, stride) = (b.bytes(), b.stride as usize);
    // XRGB8888 little-endian = B,G,R,X in memory -> RGB
    let mut rgb = Vec::with_capacity(w * h * 3);
    for row in y..y + h {
        let line = &src[row * stride + x * 4..row * stride + (x + w) * 4];
        for px in line.chunks_exact(4) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    let t_crop = now_ns();
    let mut png_bytes = Vec::with_capacity(w * h);
    {
        let mut enc = png::Encoder::new(&mut png_bytes, w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut wr = enc.write_header().expect("png header");
        wr.write_image_data(&rgb).expect("png data");
    }
    let t_enc = now_ns();
    Encoded {
        png: png_bytes,
        w,
        h,
        crop_ms: (t_crop - t0) as f64 / 1e6,
        encode_ms: (t_enc - t_crop) as f64 / 1e6,
    }
}

/// Store an encoded capture under ~/.cache/snip-spike/captures/ (stand-in for
/// the history store) via temp + rename. Returns the path.
pub fn store(png: &[u8], stem: &str) -> String {
    let dir = format!("{}/.cache/snip-spike/captures", std::env::var("HOME").unwrap_or_default());
    let _ = std::fs::create_dir_all(&dir);
    let ts = now_ns();
    let path = format!("{dir}/{stem}-{ts}.png");
    let tmp = format!("{path}.tmp");
    if std::fs::write(&tmp, png).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
    path
}
