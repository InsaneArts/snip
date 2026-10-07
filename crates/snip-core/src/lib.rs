//! Snip core: pure logic with no Wayland or Qt dependencies, so it is unit
//! tested in isolation. See docs/02-architecture.md §11.

pub mod balance;
pub mod config;
pub mod encode;
pub mod frame;
pub mod geometry;
pub mod history;
pub mod naming;
pub mod ocr;
pub mod paths;
pub mod project;
pub mod qr;
pub mod redact;
pub mod stitch;
pub mod store;
pub mod theme;
pub mod trace;
pub mod window;
