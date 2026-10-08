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
    decode_grey(&grey(data, bpp), w as usize, h as usize)
}

fn grey(data: &[u8], bpp: usize) -> Vec<u8> {
    data.chunks_exact(bpp).map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8).collect()
}

/// Codes besides QR (OCR-03): Data Matrix, Aztec and PDF417 always; 1D
/// barcodes (EAN, UPC, Code 128…) only when `one_d`, since they misread busy
/// UI. Decoded by rxing (a port of ZXing).
pub fn decode_others(data: &[u8], w: u32, h: u32, bpp: usize, one_d: bool) -> Vec<String> {
    use rxing::BarcodeFormat::*;
    let mut formats: std::collections::HashSet<rxing::BarcodeFormat> = [DATA_MATRIX, AZTEC, PDF_417].into();
    if one_d {
        formats.extend([CODABAR, CODE_39, CODE_93, CODE_128, EAN_8, EAN_13, ITF, UPC_A, UPC_E]);
    }
    let mut hints = rxing::DecodeHints { PossibleFormats: Some(formats), TryHarder: Some(true), ..Default::default() };
    rxing::helpers::detect_multiple_in_luma_with_hints(grey(data, bpp), w, h, &mut hints)
        .map(|found| found.iter().map(|r| r.getText().to_string()).collect())
        .unwrap_or_default()
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

    /// A 1D barcode as RGB: its bars `scale` px wide, `quiet` modules of white around.
    fn barcode(format: rxing::BarcodeFormat, value: &str) -> (Vec<u8>, u32, u32) {
        use rxing::Writer;
        let m = rxing::MultiFormatWriter.encode(value, &format, 0, 0).unwrap();
        let (mw, scale, height, quiet) = (m.getWidth() as usize, 3, 80, 12);
        let w = (mw + 2 * quiet) * scale;
        let mut img = vec![255u8; w * height * 3];
        for x in 0..mw {
            if m.get(x as u32, 0) {
                for y in 10..height - 10 {
                    for dx in 0..scale {
                        let o = (y * w + (x + quiet) * scale + dx) * 3;
                        img[o..o + 3].copy_from_slice(&[0, 0, 0]);
                    }
                }
            }
        }
        (img, w as u32, height as u32)
    }

    #[test]
    fn one_d_barcodes_only_when_asked() {
        let (img, w, h) = barcode(rxing::BarcodeFormat::CODE_128, "SNIP-42");
        assert_eq!(decode_others(&img, w, h, 3, true), vec!["SNIP-42".to_string()]);
        assert!(decode_others(&img, w, h, 3, false).is_empty(), "1D is opt-in");
        let (img, w, h) = barcode(rxing::BarcodeFormat::EAN_13, "4006381333931");
        assert_eq!(decode_others(&img, w, h, 3, true), vec!["4006381333931".to_string()]);
    }
}
