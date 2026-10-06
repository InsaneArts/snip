//! Monotonic timestamps and structured trace lines (JSON on stderr, which the
//! systemd user unit sends to the journal). `SNIP_TRACE=1` enables timing
//! traces; warnings and errors are always written.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn init() {
    ENABLED.store(std::env::var_os("SNIP_TRACE").is_some_and(|v| v != "0"), Ordering::Relaxed);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// CLOCK_MONOTONIC in ns: the same clock Hyprland stamps shortcut events with.
pub fn now_ns() -> u64 {
    let ts = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

pub fn ms_since(t0: u64) -> f64 {
    now_ns().saturating_sub(t0) as f64 / 1e6
}

fn emit(level: &str, ev: &str, fields: &str) {
    let line = if fields.is_empty() {
        format!("{{\"t_ns\":{},\"level\":\"{level}\",\"ev\":\"{ev}\"}}\n", now_ns())
    } else {
        format!("{{\"t_ns\":{},\"level\":\"{level}\",\"ev\":\"{ev}\",{fields}}}\n", now_ns())
    };
    let _ = std::io::stderr().lock().write_all(line.as_bytes());
}

/// Timing / debug trace (only with SNIP_TRACE=1). `fields` is a JSON fragment.
pub fn trace(ev: &str, fields: &str) {
    if enabled() {
        emit("trace", ev, fields);
    }
}

pub fn warn(ev: &str, fields: &str) {
    emit("warn", ev, fields);
}

pub fn error(ev: &str, fields: &str) {
    emit("error", ev, fields);
}

/// Escape a string for embedding in a JSON fragment.
pub fn js(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}
