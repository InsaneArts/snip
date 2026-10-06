//! inotify on the few files that change Snip's look or behaviour live:
//! ~/.config/snip/config.toml (SET-01) and Omarchy's current-theme symlink
//! (OMA-01). Bursts are coalesced (theme switches touch several files).

use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};

pub enum Change {
    Config,
    Theme,
}

pub fn spawn(f: impl Fn(Change) + Send + 'static) {
    let cfg_dir = snip_core::paths::config_dir();
    let _ = std::fs::create_dir_all(&cfg_dir);
    let theme_dir: PathBuf = snip_core::theme::current_dir();
    std::thread::Builder::new()
        .name("snip-watch".into())
        .spawn(move || {
            let Ok(fd) = inotify::init(CreateFlags::CLOEXEC) else { return };
            let mask = WatchFlags::CLOSE_WRITE | WatchFlags::MOVED_TO | WatchFlags::CREATE | WatchFlags::DELETE;
            let wd_cfg = inotify::add_watch(&fd, &cfg_dir, mask).unwrap_or(-1);
            let wd_theme = inotify::add_watch(&fd, &theme_dir, mask).unwrap_or(-1);
            let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
            loop {
                let (mut cfg, mut theme) = (false, false);
                let mut deadline: Option<Instant> = None;
                loop {
                    // block for the first event, then drain for 150 ms
                    if let Some(d) = deadline {
                        let left = d.saturating_duration_since(Instant::now());
                        if left.is_zero() {
                            break;
                        }
                        let mut pfd = [rustix::event::PollFd::new(&fd, rustix::event::PollFlags::IN)];
                        let ts = rustix::time::Timespec { tv_sec: 0, tv_nsec: left.as_nanos().min(999_999_999) as i64 };
                        if rustix::event::poll(&mut pfd, Some(&ts)).unwrap_or(0) == 0 {
                            break;
                        }
                    }
                    let mut r = inotify::Reader::new(&fd, &mut buf);
                    loop {
                        let ev = match r.next() {
                            Ok(ev) => ev,
                            Err(_) => break,
                        };
                        let name = ev.file_name().map(|n| n.to_bytes().to_vec()).unwrap_or_default();
                        let name = OsStr::new(std::str::from_utf8(&name).unwrap_or(""));
                        if ev.wd() == wd_cfg && name == "config.toml" {
                            cfg = true;
                        } else if ev.wd() == wd_theme && (name == "theme" || name == "theme.name" || name == "background") {
                            theme = true;
                        }
                        let _ = ReadFlags::empty();
                        if r.is_buffer_empty() {
                            break;
                        }
                    }
                    if deadline.is_none() {
                        deadline = Some(Instant::now() + Duration::from_millis(150));
                    }
                }
                if cfg {
                    f(Change::Config);
                }
                if theme {
                    f(Change::Theme);
                }
            }
        })
        .expect("spawn watcher");
}
