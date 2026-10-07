//! Capture Text (CAP-09, spike S07): Tesseract 5 with the pipeline that won
//! the bake-off: grey, 2x upscale at low density, invert dark regions, pad
//! with the edge colour, PSM 6, then TSV clean-up (drop low-confidence words,
//! rebuild lines and indentation from word boxes).
//!
//! M1 runs the `tesseract` CLI (~36 ms init per call); linking TessBaseAPI
//! in-process to keep a warm instance is the planned follow-up.

use std::io::Write;
use std::process::{Command, Stdio};

const MIN_CONF: f64 = 40.0;
/// Below this mean word confidence the primary-language result is retried
/// with the other languages (Georgian read as English scores ~20-40).
const RETRY_BELOW: f64 = 70.0;
const PAD: usize = 24;

pub fn installed_languages() -> Vec<String> {
    Command::new("tesseract")
        .arg("--list-langs")
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().skip(1).map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && l != "osd").collect())
        .unwrap_or_default()
}

/// XKB layout -> Tesseract model, for the languages people type in.
fn model_for_layout(layout: &str) -> Option<&'static str> {
    Some(match layout {
        "ge" => "kat",
        "ru" => "rus",
        "ua" => "ukr",
        "by" => "bel",
        "bg" => "bul",
        "rs" => "srp",
        "de" | "at" | "ch" => "deu",
        "fr" | "be" => "fra",
        "es" | "latam" => "spa",
        "it" => "ita",
        "pt" | "br" => "por",
        "nl" => "nld",
        "pl" => "pol",
        "cz" => "ces",
        "sk" => "slk",
        "hu" => "hun",
        "ro" => "ron",
        "gr" => "ell",
        "tr" => "tur",
        "se" => "swe",
        "fi" => "fin",
        "no" => "nor",
        "dk" => "dan",
        "il" => "heb",
        "ara" => "ara",
        "ir" => "fas",
        "am" => "hye",
        "kz" => "kaz",
        "jp" => "jpn",
        "cn" => "chi_sim",
        "tw" => "chi_tra",
        "kr" => "kor",
        "th" => "tha",
        "vn" => "vie",
        "in" => "hin",
        _ => return None,
    })
}

/// English first, then the models for the user's other keyboard layouts
/// (Hyprland `input:kb_layout`, e.g. "us,ge" -> "eng+kat"), when installed.
pub fn languages_for_layouts(layouts: &str, installed: &[String]) -> String {
    let mut langs = vec!["eng".to_string()];
    for l in layouts.split(',').map(str::trim) {
        if let Some(m) = model_for_layout(l) {
            if installed.iter().any(|i| i == m) && !langs.iter().any(|x| x == m) {
                langs.push(m.to_string());
            }
        }
    }
    langs.join("+")
}

/// Greyscale plane from tightly packed RGB(A).
fn grey(data: &[u8], w: usize, h: usize, bpp: usize) -> Vec<u8> {
    let mut g = Vec::with_capacity(w * h);
    for px in data.chunks_exact(bpp) {
        g.push(((px[0] as u32 * 299 + px[1] as u32 * 587 + px[2] as u32 * 114) / 1000) as u8);
    }
    g
}

fn upscale2(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let (ow, oh) = (w * 2, h * 2);
    let mut out = vec![0u8; ow * oh];
    for y in 0..oh {
        let sy = ((y as f32 + 0.5) / 2.0 - 0.5).max(0.0);
        let (y0, fy) = (sy as usize, sy.fract());
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..ow {
            let sx = ((x as f32 + 0.5) / 2.0 - 0.5).max(0.0);
            let (x0, fx) = (sx as usize, sx.fract());
            let x1 = (x0 + 1).min(w - 1);
            let p = |xx: usize, yy: usize| src[yy * w + xx] as f32;
            let top = p(x0, y0) * (1.0 - fx) + p(x1, y0) * fx;
            let bot = p(x0, y1) * (1.0 - fx) + p(x1, y1) * fx;
            out[y * ow + x] = (top * (1.0 - fy) + bot * fy).round() as u8;
        }
    }
    out
}

/// How much `preprocess` enlarges an image (small text reads better at 2×).
fn upscale_factor(w: u32, h: u32, scale: f64) -> usize {
    if scale <= 1.25 && (w as usize) * (h as usize) < 8_000_000 { 2 } else { 1 }
}

/// Preprocess into a PGM (P5) that tesseract reads from stdin.
pub fn preprocess(data: &[u8], w: u32, h: u32, bpp: usize, scale: f64) -> Vec<u8> {
    let factor = upscale_factor(w, h, scale);
    let (mut w, mut h) = (w as usize, h as usize);
    let mut g = grey(data, w, h, bpp);
    if factor == 2 {
        g = upscale2(&g, w, h);
        w *= 2;
        h *= 2;
    }
    let mean = g.iter().map(|&v| v as u64).sum::<u64>() / g.len().max(1) as u64;
    if mean < 128 {
        g.iter_mut().for_each(|v| *v = 255 - *v); // light text on dark: invert
    }
    // pad with the border's median so edge glyphs are not cut lines
    let mut border: Vec<u8> = (0..w).flat_map(|x| [g[x], g[(h - 1) * w + x]]).chain((0..h).flat_map(|y| [g[y * w], g[y * w + w - 1]])).collect();
    border.sort_unstable();
    let bg = border[border.len() / 2];
    let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
    let mut out = format!("P5\n{pw} {ph}\n255\n").into_bytes();
    out.reserve(pw * ph);
    for y in 0..ph {
        for x in 0..pw {
            let inside = x >= PAD && y >= PAD && x < PAD + w && y < PAD + h;
            out.push(if inside { g[(y - PAD) * w + (x - PAD)] } else { bg });
        }
    }
    out
}

/// TSV -> text: confident words only, lines in reading order, indentation
/// rebuilt from the left edge in units of the median character width.
pub fn text_from_tsv(tsv: &str) -> String {
    parse_tsv(tsv).0
}

/// (text, confidence of the kept words weighted by length, kept words).
/// Junk the filter drops does not count: it shows up in every language.
pub fn parse_tsv(tsv: &str) -> (String, f64, usize) {
    let (mut sum, mut n) = (0.0, 0.0);
    for l in tsv.lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() < 12 || f[0] != "5" || f[11].trim().is_empty() {
            continue;
        }
        if let Ok(c) = f[10].parse::<f64>() {
            if c >= MIN_CONF {
                let w = f[11].chars().count() as f64;
                sum += c * w;
                n += w;
            }
        }
    }
    let (text, kept) = words_to_text(tsv);
    (text, if n > 0.0 { sum / n } else { 0.0 }, kept)
}

fn words_to_text(tsv: &str) -> (String, usize) {
    struct W<'a> {
        block: i32,
        key: (i32, i32, i32),
        left: i32,
        top: i32,
        width: i32,
        text: &'a str,
    }
    let words: Vec<W> = tsv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 12 || f[0] != "5" || f[11].trim().is_empty() {
                return None;
            }
            if f[10].parse::<f64>().ok()? < MIN_CONF {
                return None;
            }
            let n = |i: usize| f[i].parse::<i32>().ok();
            Some(W { block: n(2)?, key: (n(2)?, n(3)?, n(4)?), left: n(6)?, top: n(7)?, width: n(8)?, text: f[11] })
        })
        .collect();
    if words.is_empty() {
        return (String::new(), 0);
    }
    let mut cw: Vec<f64> = words.iter().map(|w| w.width as f64 / w.text.chars().count().max(1) as f64).collect();
    cw.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let cw = cw[cw.len() / 2].max(1.0);
    let minleft = words.iter().map(|w| w.left).min().unwrap_or(0);
    let mut lines: Vec<((i32, i32, i32), Vec<&W>)> = Vec::new();
    for w in &words {
        match lines.iter_mut().find(|(k, _)| *k == w.key) {
            Some((_, v)) => v.push(w),
            None => lines.push((w.key, vec![w])),
        }
    }
    lines.sort_by_key(|(_, ws)| ws.iter().map(|w| w.top).min().unwrap_or(0));
    let mut out = String::new();
    let mut last_block = None;
    for (_, mut ws) in lines {
        ws.sort_by_key(|w| w.left);
        if last_block.is_some_and(|b| b != ws[0].block) {
            out.push('\n');
        }
        last_block = Some(ws[0].block);
        let ind = ((ws[0].left - minleft) as f64 / cw).round() as usize;
        out.push_str(&" ".repeat(ind));
        out.push_str(&ws.iter().map(|w| w.text).collect::<Vec<_>>().join(" "));
        out.push('\n');
    }
    (out.trim_end().to_string(), words.len())
}

/// Recognise with the first language; if that reads poorly and there are
/// other languages, retry with them first and keep the more confident read.
pub fn recognize(data: &[u8], w: u32, h: u32, bpp: usize, scale: f64, langs: &str) -> std::io::Result<String> {
    recognize_with(data, w, h, bpp, scale, langs, false)
}

/// A recognised word's box in image px, and the line it sits on.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct WordBox {
    pub line: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Word boxes from Tesseract's TSV, back in image px: `pad` and `factor`
/// undo `preprocess`. Lines are numbered in reading order.
pub fn word_boxes(tsv: &str, pad: f64, factor: f64) -> Vec<WordBox> {
    let mut lines: Vec<(i32, i32, i32)> = vec![];
    let mut out = vec![];
    for row in tsv.lines().skip(1) {
        let f: Vec<&str> = row.split('\t').collect();
        if f.len() < 12 || f[0] != "5" || f[11].trim().is_empty() {
            continue;
        }
        let n = |i: usize| f[i].parse::<i32>().unwrap_or(0);
        let key = (n(2), n(3), n(4));
        let line = lines.iter().position(|k| *k == key).unwrap_or_else(|| {
            lines.push(key);
            lines.len() - 1
        });
        let at = |v: i32| (v as f64 - pad) / factor;
        out.push(WordBox { line: line as u32, x: at(n(6)), y: at(n(7)), w: n(8) as f64 / factor, h: n(9) as f64 / factor });
    }
    out
}

/// Every word's box (the editor's highlighter snaps to them, ANN-10).
pub fn words(data: &[u8], w: u32, h: u32, bpp: usize, scale: f64, lang: &str) -> std::io::Result<Vec<WordBox>> {
    let tsv = run_tesseract(&preprocess(data, w, h, bpp, scale), lang, false)?;
    Ok(word_boxes(&tsv, PAD as f64, upscale_factor(w, h, scale) as f64))
}

/// `background`: run Tesseract at the lowest CPU priority (history indexing).
pub fn recognize_with(data: &[u8], w: u32, h: u32, bpp: usize, scale: f64, langs: &str, background: bool) -> std::io::Result<String> {
    let pgm = preprocess(data, w, h, bpp, scale);
    let mut parts: Vec<&str> = langs.split('+').filter(|l| !l.is_empty()).collect();
    if parts.is_empty() {
        parts.push("eng");
    }
    let first = run_tesseract(&pgm, parts[0], background)?;
    let (text, conf, _) = parse_tsv(&first);
    if parts.len() == 1 || conf >= RETRY_BELOW {
        return Ok(text);
    }
    let mut rest: Vec<&str> = parts[1..].to_vec();
    rest.push(parts[0]);
    let second = run_tesseract(&pgm, &rest.join("+"), background)?;
    let (text2, conf2, _) = parse_tsv(&second);
    Ok(if conf2 > conf { text2 } else { text })
}

fn run_tesseract(pgm: &[u8], langs: &str, background: bool) -> std::io::Result<String> {
    let pgm = pgm.to_vec();
    let mut cmd = if background {
        let mut c = Command::new("nice");
        c.args(["-n", "19", "tesseract"]);
        c
    } else {
        Command::new("tesseract")
    };
    let mut child = cmd
        .args(["stdin", "stdout", "--oem", "1", "--psm", "6", "-l", langs])
        .args(["-c", "preserve_interword_spaces=1", "-c", "tessedit_create_tsv=1"])
        .env("OMP_THREAD_LIMIT", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("stdin");
    let writer = std::thread::spawn(move || stdin.write_all(&pgm));
    let out = child.wait_with_output()?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(std::io::Error::other(format!("tesseract exited with {}", out.status)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_rebuilds_indentation_and_drops_noise() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
5\t1\t1\t1\t1\t1\t10\t10\t30\t12\t90\tfn\n\
5\t1\t1\t1\t1\t2\t50\t10\t60\t12\t90\tmain()\n\
5\t1\t1\t1\t2\t1\t50\t30\t60\t12\t88\treturn\n\
5\t1\t1\t1\t3\t1\t10\t50\t10\t12\t12\t~\n";
        assert_eq!(text_from_tsv(tsv), "fn main()\n    return");
    }

    #[test]
    fn word_boxes_return_to_image_px() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
4\t1\t1\t1\t1\t0\t24\t24\t200\t30\t-1\t\n\
5\t1\t1\t1\t1\t1\t44\t64\t60\t30\t91\thello\n\
5\t1\t1\t1\t1\t2\t124\t64\t40\t30\t88\tyou\n\
5\t1\t1\t1\t2\t1\t44\t124\t80\t30\t90\tthere\n";
        let b = word_boxes(tsv, 24.0, 2.0);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0], WordBox { line: 0, x: 10.0, y: 20.0, w: 30.0, h: 15.0 });
        assert_eq!((b[1].line, b[1].x), (0, 50.0));
        assert_eq!((b[2].line, b[2].y), (1, 50.0));
    }

    #[test]
    fn layouts_pick_installed_models() {
        let installed = vec!["eng".to_string(), "kat".to_string(), "rus".to_string()];
        assert_eq!(languages_for_layouts("us,ge", &installed), "eng+kat");
        assert_eq!(languages_for_layouts("us", &installed), "eng");
        assert_eq!(languages_for_layouts("us,de,ru", &installed), "eng+rus");
    }

    #[test]
    fn preprocess_inverts_dark_and_pads() {
        let (w, h) = (4u32, 2u32);
        let px = vec![10u8; (w * h * 3) as usize];
        let pgm = preprocess(&px, w, h, 3, 2.0);
        let header = format!("P5\n{} {}\n255\n", w as usize + 2 * PAD, h as usize + 2 * PAD);
        assert!(pgm.starts_with(header.as_bytes()));
        assert_eq!(*pgm.last().unwrap(), 245); // inverted background
    }
}
