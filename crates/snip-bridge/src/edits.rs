//! Pixels the editor shares with C++: renders on their way to the engine
//! (`Platform.grabExact` hands them over under a token, `editor_commit`
//! takes them), and decoded images the Pixelate tiles are built from.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Straight (not premultiplied) RGBA, tightly packed.
pub struct Rendered {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

fn map() -> &'static Mutex<HashMap<u32, Rendered>> {
    static M: OnceLock<Mutex<HashMap<u32, Rendered>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

pub fn put(token: u32, data: &[u8], width: u32, height: u32, stride: u32) {
    let row = width as usize * 4;
    let mut rgba = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let at = y * stride as usize;
        rgba.extend_from_slice(&data[at..at + row]);
    }
    let mut m = map().lock().unwrap();
    m.insert(token, Rendered { rgba, width, height });
    // An editor that never committed (closed mid-export) leaves at most a few.
    if m.len() > 8 {
        let oldest = *m.keys().min().unwrap();
        m.remove(&oldest);
    }
}

pub fn take(token: u32) -> Option<Rendered> {
    map().lock().unwrap().remove(&token)
}

pub struct Decoded {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bpp: usize,
}

fn cache() -> &'static Mutex<Vec<(String, Arc<Decoded>)>> {
    static CACHE: OnceLock<Mutex<Vec<(String, Arc<Decoded>)>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// The image at `path`, decoded once: a Pixelate region asks for new tiles
/// on every pointer move.
pub fn decoded(path: &str) -> Option<Arc<Decoded>> {
    let cache = cache();
    if let Some(d) = cache.lock().unwrap().iter().find(|(p, _)| p == path).map(|(_, d)| d.clone()) {
        return Some(d);
    }
    let (data, width, height, alpha) = snip_core::encode::decode_png(&std::fs::read(path).ok()?).ok()?;
    let d = Arc::new(Decoded { data, width, height, bpp: if alpha { 4 } else { 3 } });
    let mut c = cache.lock().unwrap();
    c.insert(0, (path.to_string(), d.clone()));
    c.truncate(2);
    Some(d)
}

/// The decoded images, once no editor needs them (up to ~100 MB each).
pub fn forget_decoded() {
    cache().lock().unwrap().clear();
}
