//! `snip`: talks to snipd over ctl.sock. Exit codes: 0 ok, 1 cancelled,
//! 2 error, 3 daemon unavailable (docs/02-architecture.md §6.9).

use snip_ipc::{call, Request};
use std::io::ErrorKind;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

const USAGE: &str = "usage: snip [--json] <command>

capture
  area | fullscreen | window | previous | text | all-in-one | area-pin [--monitor NAME] [--cursor | --no-cursor]
                   [--action copy|save|annotate|pin]   then that, instead of the After Capture rules
  area --x X --y Y --w W --h H [--monitor NAME]   that area (logical px), with no overlay
  fullscreen --all every monitor, in one image
  text --file FILE | --clipboard   read the text (or QR code) of a PNG, onto the clipboard
  text --line-breaks | --one-line   Capture Text keeping or joining its lines, whatever the setting
  scroll           scrolling capture: choose the area, Start, scroll, Done (Enter; Esc cancels)
  area | fullscreen --delay N   self-timer: N seconds of countdown, then the capture
  open FILE        add an image (or a .snip project) as a capture, with a Quick Access card
  edit FILE        open an image or a .snip project in Snip's editor (as a new capture)
  edit --clipboard open the image on the clipboard in Snip's editor
captures (default: the most recent)
  copy [ID]        put a capture on the clipboard
  save [ID]        save to the export folder (and copy)
  pin [ID|FILE]    float it above every window (drag to move, scroll: opacity, Ctrl+scroll: size)
  pins toggle|close|unlock  hide or show every pin, close them all, or unlock locked ones
  desktop hide|show|toggle  hide desktop clutter: the wallpaper over the bar, notifications held, no pins or cards
recording
  record [--gif]   choose an area (Enter: the monitor, Space: a window), then Record; again: stop
  record area --x X --y Y --w W --h H [--monitor NAME] [--gif]   record that area at once (logical px)
  record fullscreen [--monitor NAME] [--gif]   record a whole monitor at once
  record stop|pause|resume|restart|discard|status
  list [-n N]      recent captures from history
  search TEXT [-n N]  find captures by the text in them (and window title/app)
  history          browse and search past captures (Enter restores one)
  settings [TAB]   open Settings (general, quick-access, screenshots, annotate, advanced, about)
  welcome          show the first-run Welcome again (Snip's keys, start at login)
  restore [ID]     re-open the last closed Quick Access card (or that capture's)
  close-all        close every Quick Access card
daemon
  status | reload | quit
  --version        the version this is
";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let json = if let Some(i) = args.iter().position(|a| a == "--json") {
        args.remove(i);
        true
    } else {
        false
    };
    let flag = |args: &mut Vec<String>, name: &str| -> Option<String> {
        let i = args.iter().position(|a| a == name)?;
        args.remove(i);
        (i < args.len()).then(|| args.remove(i))
    };
    let Some(cmd) = args.first().cloned() else {
        eprint!("{USAGE}");
        return ExitCode::from(2);
    };
    let arg1 = args.get(1).cloned();
    let (req, timeout) = match cmd.as_str() {
        "text" if args.iter().any(|a| a == "--clipboard") => (Request::ReadText { path: None }, Duration::from_secs(30)),
        "text" if args.iter().any(|a| a == "--file") => {
            let Some(p) = flag(&mut args, "--file") else {
                eprintln!("snip: text --file needs a file");
                return ExitCode::from(2);
            };
            let path = std::fs::canonicalize(&p).unwrap_or_else(|_| p.into()).to_string_lossy().into_owned();
            (Request::ReadText { path: Some(path) }, Duration::from_secs(30))
        }
        "area" | "fullscreen" | "window" | "previous" | "text" | "all-in-one" | "area-pin" | "scroll" => {
            let monitor = flag(&mut args, "--monitor");
            let cursor = if args.iter().any(|a| a == "--cursor") {
                Some(true)
            } else if args.iter().any(|a| a == "--no-cursor") {
                Some(false)
            } else {
                None
            };
            let delay = flag(&mut args, "--delay").and_then(|v| v.parse::<u32>().ok());
            let action = flag(&mut args, "--action");
            let n = |args: &mut Vec<String>, name: &str| flag(args, name).and_then(|v| v.parse::<i32>().ok());
            let rect = match (n(&mut args, "--x"), n(&mut args, "--y"), n(&mut args, "--w"), n(&mut args, "--h")) {
                (Some(x), Some(y), Some(w), Some(h)) => Some([x, y, w, h]),
                _ => None,
            };
            let mode = if cmd == "fullscreen" && args.iter().any(|a| a == "--all") {
                "fullscreen-all".to_string()
            } else if cmd == "text" && args.iter().any(|a| a == "--line-breaks") {
                "text-lines".to_string()
            } else if cmd == "text" && args.iter().any(|a| a == "--one-line") {
                "text-one-line".to_string()
            } else {
                cmd.clone()
            };
            (Request::Capture { mode, monitor, cursor, delay, action, rect }, Duration::from_secs(600))
        }
        "record" => {
            let gif = args.iter().any(|a| a == "--gif");
            let monitor = flag(&mut args, "--monitor");
            let n = |args: &mut Vec<String>, name: &str| flag(args, name).and_then(|v| v.parse::<i32>().ok());
            let rect = match (n(&mut args, "--x"), n(&mut args, "--y"), n(&mut args, "--w"), n(&mut args, "--h")) {
                (Some(x), Some(y), Some(w), Some(h)) => Some([x, y, w, h]),
                _ => None,
            };
            let (action, whole) = match arg1.as_deref() {
                Some(a @ ("stop" | "pause" | "resume" | "restart" | "discard" | "status")) => (a.to_string(), false),
                Some("fullscreen") => ("start".to_string(), true),
                Some("area") if rect.is_some() => ("start".to_string(), false),
                _ => ("toggle".to_string(), false),
            };
            // a start is answered when the recording ends: hours, maybe
            let wait = if matches!(action.as_str(), "start" | "toggle") { Duration::from_secs(24 * 3600) } else { Duration::from_secs(30) };
            (Request::Record { action, gif, monitor, rect, whole }, wait)
        }
        "edit" if arg1.as_deref() == Some("--clipboard") => (Request::EditClipboard, Duration::from_secs(10)),
        "open" | "edit" => {
            let Some(p) = arg1 else {
                eprintln!("snip: {cmd} needs a file");
                return ExitCode::from(2);
            };
            let abs = std::fs::canonicalize(&p).unwrap_or_else(|_| p.into());
            let path = abs.to_string_lossy().into_owned();
            (if cmd == "open" { Request::Open { path } } else { Request::Edit { path } }, Duration::from_secs(10))
        }
        "copy" => (Request::Copy { id: arg1 }, Duration::from_secs(10)),
        "save" => (Request::Save { id: arg1 }, Duration::from_secs(10)),
        "pin" => {
            // a file to pin, or a capture's id
            let file = arg1.as_deref().filter(|a| std::path::Path::new(a).is_file()).map(|a| std::fs::canonicalize(a).unwrap_or_else(|_| a.into()).to_string_lossy().into_owned());
            (if file.is_some() { Request::Pin { id: None, path: file } } else { Request::Pin { id: arg1, path: None } }, Duration::from_secs(5))
        }
        "pins" => (Request::Pins { action: arg1.unwrap_or_else(|| "toggle".into()) }, Duration::from_secs(5)),
        "desktop" => (Request::Desktop { action: arg1.unwrap_or_else(|| "toggle".into()) }, Duration::from_secs(5)),
        "search" => {
            let n = flag(&mut args, "-n").and_then(|v| v.parse().ok()).unwrap_or(10);
            let query = args.iter().skip(1).cloned().collect::<Vec<_>>().join(" ");
            if query.trim().is_empty() {
                eprintln!("snip: search needs some text");
                return ExitCode::from(2);
            }
            (Request::Search { query, n }, Duration::from_secs(5))
        }
        "list" => {
            let n = flag(&mut args, "-n").and_then(|v| v.parse().ok()).unwrap_or(10);
            (Request::List { n }, Duration::from_secs(5))
        }
        "history" => (Request::History, Duration::from_secs(5)),
        "settings" => (Request::Settings { tab: arg1 }, Duration::from_secs(5)),
        "welcome" => (Request::Welcome, Duration::from_secs(5)),
        "restore" => (arg1.map_or(Request::RestoreLast, |id| Request::RestoreCard { id }), Duration::from_secs(5)),
        "close-all" => (Request::CloseAll, Duration::from_secs(5)),
        "status" => (Request::Status, Duration::from_secs(5)),
        "reload" => (Request::Reload, Duration::from_secs(5)),
        "quit" => (Request::Quit, Duration::from_secs(5)),
        "ping" => (Request::Ping, Duration::from_secs(2)),
        "debug" => (Request::Debug { op: arg1.unwrap_or_default(), args: args.iter().skip(2).cloned().collect() }, Duration::from_secs(10)),
        "-V" | "--version" | "version" => {
            println!("snip {}", env!("SNIP_VERSION"));
            return ExitCode::SUCCESS;
        }
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("snip: unknown command '{other}'\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let mut answer = call(&req, timeout);
    // Snip isn't running (quit from its menu, say): start it and ask again,
    // so the launcher's "Snip" and every key work either way (OMA-09)
    let down = matches!(&answer, Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused));
    if down && !matches!(req, Request::Quit | Request::Ping | Request::Status | Request::Debug { .. }) && start_snipd() {
        answer = call(&req, timeout);
    }
    match answer {
        Err(e) => {
            eprintln!("snip: daemon unavailable ({e}). Start it with: systemctl --user start snipd");
            ExitCode::from(3)
        }
        Ok(r) if json => {
            println!("{}", serde_json::to_string(&r).unwrap_or_default());
            if r.ok { ExitCode::SUCCESS } else if r.error.as_deref() == Some("cancelled") { ExitCode::from(1) } else { ExitCode::from(2) }
        }
        Ok(r) if r.ok => {
            match &r.result {
                serde_json::Value::Null => {}
                serde_json::Value::String(s) => println!("{s}"),
                v => println!("{}", serde_json::to_string_pretty(v).unwrap_or_default()),
            }
            ExitCode::SUCCESS
        }
        Ok(r) => {
            let e = r.error.unwrap_or_default();
            if e == "cancelled" {
                ExitCode::from(1)
            } else {
                eprintln!("snip: {e}");
                ExitCode::from(2)
            }
        }
    }
}

/// Start snipd (beside this binary, detached) and wait until it answers.
fn start_snipd() -> bool {
    use std::os::unix::process::CommandExt;
    let Some(snipd) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("snipd"))).filter(|p| p.exists()) else {
        return false;
    };
    let started = std::process::Command::new(snipd).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn();
    started.is_ok()
        && (0..50).any(|_| {
            std::thread::sleep(Duration::from_millis(100));
            call(&Request::Ping, Duration::from_secs(1)).is_ok()
        })
}
