//! Captured frames: memfd-backed shared memory written by the compositor and
//! read zero-copy by the UI (texture upload) and the encoder.

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;

use rustix::fs::{ftruncate, memfd_create, MemfdFlags};
use rustix::mm::{madvise, mmap, munmap, Advice, MapFlags, ProtFlags};

use crate::geometry::{Rect, Transform};

/// wl_shm formats Snip handles (fourcc values match wl_shm.format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 0: little-endian B,G,R,A in memory
    Argb8888,
    /// 1: little-endian B,G,R,X in memory
    Xrgb8888,
}

impl PixelFormat {
    pub fn wl_shm(self) -> u32 {
        match self {
            PixelFormat::Argb8888 => 0,
            PixelFormat::Xrgb8888 => 1,
        }
    }

    pub fn has_alpha(self) -> bool {
        self == PixelFormat::Argb8888
    }
}

pub struct ShmBuf {
    fd: OwnedFd,
    ptr: *mut u8,
    len: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}

// Plain shared memory: the compositor writes before `ready`, readers read
// after; synchronisation is by protocol.
unsafe impl Send for ShmBuf {}
unsafe impl Sync for ShmBuf {}

impl ShmBuf {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> std::io::Result<Self> {
        let stride = width * 4;
        let len = (stride as usize) * height as usize;
        let fd = memfd_create("snip-frame", MemfdFlags::CLOEXEC)?;
        ftruncate(&fd, len as u64)?;
        let ptr = unsafe { mmap(std::ptr::null_mut(), len, ProtFlags::READ | ProtFlags::WRITE, MapFlags::SHARED, &fd, 0)? } as *mut u8;
        unsafe {
            // Pre-fault so the compositor's first copy doesn't pay page faults,
            // and keep screenshots (possibly secrets) out of core dumps.
            std::ptr::write_bytes(ptr, 0, len);
            let _ = madvise(ptr.cast(), len, Advice::LinuxDontDump);
        }
        Ok(Self { fd, ptr, len, width, height, stride, format })
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }

    /// For tests and synthetic frames.
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
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

/// One output's frozen frame.
#[derive(Clone)]
pub struct Frame {
    pub output: String,
    pub buf: Arc<ShmBuf>,
    pub transform: Transform,
    /// Output scale (e.g. 1.25) and logical position/size, for mapping.
    pub scale: f64,
    pub logical: Rect,
    pub epoch: u64,
}

impl Frame {
    /// Upright (as displayed) size in physical pixels.
    pub fn upright_size(&self) -> (i32, i32) {
        crate::geometry::upright_size(self.buf.width as i32, self.buf.height as i32, self.transform)
    }
}
