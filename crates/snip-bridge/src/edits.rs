//! Rendered editor images on their way from C++ to the engine. The editor's
//! export (`Platform.grabExact`) hands its pixels over under a token, and
//! `Engine.editor_commit` takes them by that token.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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
