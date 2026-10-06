//! Frame memory: memfd-backed shm buffers shared by the Wayland thread (writer,
//! via the compositor) and the Qt render thread (reader, texture upload).

use std::collections::HashMap;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex, OnceLock};

use rustix::fs::{ftruncate, memfd_create, MemfdFlags};
use rustix::mm::{madvise, mmap, munmap, Advice, MapFlags, ProtFlags};

pub struct ShmBuf {
    pub fd: OwnedFd,
    ptr: *mut u8,
    pub len: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: u32,
}

// The mapping is plain shared memory; synchronisation is by protocol (the
// compositor writes before `ready`, we read after).
unsafe impl Send for ShmBuf {}
unsafe impl Sync for ShmBuf {}

impl ShmBuf {
    pub fn new(width: u32, height: u32, format: u32) -> std::io::Result<Self> {
        let stride = width * 4;
        let len = (stride * height) as usize;
        let fd = memfd_create("snip-frame", MemfdFlags::CLOEXEC)?;
        ftruncate(&fd, len as u64)?;
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                len,
                ProtFlags::READ | ProtFlags::WRITE,
                MapFlags::SHARED,
                &fd,
                0,
            )?
        } as *mut u8;
        unsafe {
            // Pre-fault so the compositor's first copy doesn't pay page faults,
            // and keep screenshots out of core dumps.
            std::ptr::write_bytes(ptr, 0, len);
            let _ = madvise(ptr.cast(), len, Advice::LinuxDontDump);
        }
        Ok(Self { fd, ptr, len, width, height, stride, format })
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    pub fn ptr(&self) -> *const u8 {
        self.ptr
    }
}

impl Drop for ShmBuf {
    fn drop(&mut self) {
        unsafe {
            let _ = munmap(self.ptr.cast(), self.len);
        }
    }
}

#[derive(Clone)]
pub struct FrameSlot {
    pub buf: Arc<ShmBuf>,
    pub epoch: u32,
    pub transform: u32,
}

fn store() -> &'static Mutex<HashMap<String, FrameSlot>> {
    static STORE: OnceLock<Mutex<HashMap<String, FrameSlot>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

pub fn publish(output: &str, slot: FrameSlot) {
    store().lock().unwrap().insert(output.to_owned(), slot);
}

pub fn get(output: &str) -> Option<FrameSlot> {
    store().lock().unwrap().get(output).cloned()
}
