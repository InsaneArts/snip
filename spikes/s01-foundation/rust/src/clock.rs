//! CLOCK_MONOTONIC timestamps and JSON-lines timing output.
//! Hyprland stamps global-shortcut events with steady_clock (= CLOCK_MONOTONIC),
//! so all marks are directly comparable with the hotkey timestamp.

use std::io::Write;

pub fn now_ns() -> u64 {
    let ts = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

/// Print one timing record: {"t_ns":..., "ev":"...", <extra>}
pub fn mark(ev: &str, extra: &str) {
    let line = if extra.is_empty() {
        format!("{{\"t_ns\":{},\"ev\":\"{}\"}}\n", now_ns(), ev)
    } else {
        format!("{{\"t_ns\":{},\"ev\":\"{}\",{}}}\n", now_ns(), ev, extra)
    };
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.flush();
}
