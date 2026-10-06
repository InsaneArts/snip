//! QR codes in a capture (OCR-03). Capture Text decodes QR first: a code's
//! value is exact, OCR of the pixels around it is not.

/// Decoded values of every QR code in a greyscale image. Tries the inverted
/// image too (light codes on dark UIs).
pub fn decode_grey(grey: &[u8], w: usize, h: usize) -> Vec<String> {
    let run = |invert: bool| {
        let mut img = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
            let v = grey[y * w + x];
            if invert { 255 - v } else { v }
        });
        img.detect_grids().into_iter().filter_map(|g| g.decode().ok().map(|(_, s)| s)).collect::<Vec<_>>()
    };
    let found = run(false);
    if found.is_empty() { run(true) } else { found }
}

/// Same, from tightly packed RGB(A).
pub fn decode(data: &[u8], w: u32, h: u32, bpp: usize) -> Vec<String> {
    let grey: Vec<u8> = data.chunks_exact(bpp).map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8).collect();
    decode_grey(&grey, w as usize, h as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str, scale: usize, invert: bool) -> (Vec<u8>, usize) {
        let code = qrcode::QrCode::new(text.as_bytes()).unwrap();
        let n = code.width();
        let quiet = 4;
        let side = (n + 2 * quiet) * scale;
        let mut img = vec![255u8; side * side];
        for y in 0..n {
            for x in 0..n {
                if code[(x, y)] == qrcode::Color::Dark {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            img[((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx] = 0;
                        }
                    }
                }
            }
        }
        if invert {
            img.iter_mut().for_each(|v| *v = 255 - *v);
        }
        (img, side)
    }

    #[test]
    fn decodes_dark_and_light_codes() {
        let (img, side) = render("https://omarchy.org/snip", 4, false);
        assert_eq!(decode_grey(&img, side, side), vec!["https://omarchy.org/snip".to_string()]);
        let (img, side) = render("WIFI:S:home;T:WPA;P:secret;;", 3, true);
        assert_eq!(decode_grey(&img, side, side), vec!["WIFI:S:home;T:WPA;P:secret;;".to_string()]);
    }

    #[test]
    fn no_code_no_result() {
        assert!(decode_grey(&vec![200u8; 64 * 64], 64, 64).is_empty());
    }
}
