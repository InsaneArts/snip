//! `snip`: talks to snipd over ctl.sock. Exit codes: 0 ok, 1 cancelled,
//! 2 error, 3 daemon unavailable (docs/02-architecture.md §6.9).

use snip_ipc::{call, Request};
use std::process::ExitCode;
use std::time::Duration;

const USAGE: &str = "usage: snip [--json] <command>

capture
  area | fullscreen | window | previous [--monitor NAME]
captures (default: the most recent)
  copy [ID]        put a capture on the clipboard
  save [ID]        save to the export folder (and copy)
  list [-n N]      recent captures from history
  restore          re-open the last closed Quick Access card
  close-all        close every Quick Access card
daemon
  status | reload | quit
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
        "area" | "fullscreen" | "window" | "previous" => {
            let monitor = flag(&mut args, "--monitor");
            (Request::Capture { mode: cmd.clone(), monitor }, Duration::from_secs(600))
        }
        "copy" => (Request::Copy { id: arg1 }, Duration::from_secs(10)),
        "save" => (Request::Save { id: arg1 }, Duration::from_secs(10)),
        "list" => {
            let n = flag(&mut args, "-n").and_then(|v| v.parse().ok()).unwrap_or(10);
            (Request::List { n }, Duration::from_secs(5))
        }
        "restore" => (Request::RestoreLast, Duration::from_secs(5)),
        "close-all" => (Request::CloseAll, Duration::from_secs(5)),
        "status" => (Request::Status, Duration::from_secs(5)),
        "reload" => (Request::Reload, Duration::from_secs(5)),
        "quit" => (Request::Quit, Duration::from_secs(5)),
        "ping" => (Request::Ping, Duration::from_secs(2)),
        "debug" => (Request::Debug { op: arg1.unwrap_or_default(), args: args.iter().skip(2).cloned().collect() }, Duration::from_secs(10)),
        "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        other => {
            eprintln!("snip: unknown command '{other}'\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match call(&req, timeout) {
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
