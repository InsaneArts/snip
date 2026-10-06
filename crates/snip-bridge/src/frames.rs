//! Frames of the current overlay session, shared with the C++ render code.
//! C++ wraps a frame in a QImage without copying; `acquire` hands it an owned
//! reference (released by the QImage cleanup), so dropping a session never
//! unmaps memory the render thread is still uploading.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use snip_core::frame::{Frame, ShmBuf};

fn map() -> &'static Mutex<HashMap<String, Frame>> {
    static M: OnceLock<Mutex<HashMap<String, Frame>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

pub fn publish(frames: Vec<Frame>) {
    let mut m = map().lock().unwrap();
    m.clear();
    for f in frames {
        m.insert(f.output.clone(), f);
    }
}

pub fn get(output: &str) -> Option<Frame> {
    map().lock().unwrap().get(output).cloned()
}

pub fn clear() {
    map().lock().unwrap().clear();
}

pub fn acquire(buf: &Arc<ShmBuf>) -> usize {
    Box::into_raw(Box::new(buf.clone())) as usize
}

/// # Safety
/// `handle` must come from `acquire` and be released once.
pub unsafe fn release(handle: usize) {
    if handle != 0 {
        drop(Box::from_raw(handle as *mut Arc<ShmBuf>));
    }
}
