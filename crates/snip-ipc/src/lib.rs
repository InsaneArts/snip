//! `$XDG_RUNTIME_DIR/snip/ctl.sock`: newline-delimited JSON, one request and
//! one response per line. Only peers with our uid are served (SO_PEERCRED);
//! the directory is 0700. Commands mirror CleanShot's URL API (API-01).

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "kebab-case")]
pub enum Request {
    Ping,
    /// Capture: area | fullscreen | window | previous | text | all-in-one
    Capture {
        mode: String,
        #[serde(default)]
        monitor: Option<String>,
        /// Override `capture.show_cursor` for this capture
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<bool>,
        /// Self-timer (CAP-06): seconds of countdown before the capture
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delay: Option<u32>,
    },
    /// Copy a capture (default: the most recent) to the clipboard
    Copy {
        #[serde(default)]
        id: Option<String>,
    },
    /// Save a capture (default: the most recent) to the export folder
    Save {
        #[serde(default)]
        id: Option<String>,
    },
    /// Pin a capture (default: the most recent), or an image file, above
    /// every window (PIN-01)
    Pin {
        #[serde(default)]
        id: Option<String>,
        #[serde(default)]
        path: Option<String>,
    },
    /// Every pin at once (PIN-05, 07): "toggle" hides or shows them, "close",
    /// "unlock" (a locked pin takes no input)
    Pins { action: String },
    /// Add an image file as a capture with a Quick Access card
    Open { path: String },
    /// Add an image file as a capture and open it in Snip's editor (ANN-19;
    /// OMA-06: Snip as OMARCHY_SCREENSHOT_EDITOR)
    Edit { path: String },
    /// Open the image on the clipboard in Snip's editor, as a new capture (ANN-19)
    EditClipboard,
    /// Search history by the text in captures, window titles and apps
    Search {
        query: String,
        #[serde(default = "default_n")]
        n: u32,
    },
    /// Recent captures from history
    List {
        #[serde(default = "default_n")]
        n: u32,
    },
    /// Show the capture history window (HIS-01)
    History,
    /// Show the settings window (SET-02), on a tab
    Settings {
        #[serde(default)]
        tab: Option<String>,
    },
    /// Re-open the most recently closed Quick Access card
    RestoreLast,
    CloseAll,
    Status,
    Reload,
    Quit,
    /// Test/automation helpers (synthetic input), used by tests/e2e
    Debug {
        op: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

fn default_n() -> u32 {
    10
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub result: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    pub fn ok(result: serde_json::Value) -> Self {
        Self { ok: true, result, error: None }
    }

    pub fn err(e: impl Into<String>) -> Self {
        Self { ok: false, result: serde_json::Value::Null, error: Some(e.into()) }
    }
}

/// Send one request; `timeout` bounds the whole exchange (captures wait for
/// the user, so the CLI passes a long one for those).
pub fn call(req: &Request, timeout: Duration) -> std::io::Result<Response> {
    let mut s = UnixStream::connect(snip_core::paths::ctl_socket())?;
    s.set_read_timeout(Some(timeout))?;
    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    s.write_all(line.as_bytes())?;
    let mut resp = String::new();
    BufReader::new(s).read_line(&mut resp)?;
    serde_json::from_str(&resp).map_err(std::io::Error::other)
}

/// Bind ctl.sock and serve requests on a background thread. `handle` runs on
/// that thread and may block (e.g. a capture waiting for the user).
pub fn serve(handle: impl Fn(Request) -> Response + Send + Sync + 'static) -> std::io::Result<()> {
    let dir = snip_core::paths::runtime_dir();
    snip_core::paths::ensure_private_dir(&dir)?;
    let path = snip_core::paths::ctl_socket();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    let handle = std::sync::Arc::new(handle);
    let uid = rustix::process::getuid();
    std::thread::Builder::new().name("snip-ctl".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            let ok_peer = rustix::net::sockopt::socket_peercred(&stream).map(|c| c.uid == uid).unwrap_or(false);
            if !ok_peer {
                continue;
            }
            let handle = handle.clone();
            // one thread per connection: a capture may wait on the user
            std::thread::spawn(move || {
                let mut out = match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return,
                };
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let resp = match serde_json::from_str::<Request>(&line) {
                        Ok(req) => handle(req),
                        Err(e) => Response::err(format!("bad request: {e}")),
                    };
                    let mut s = serde_json::to_string(&resp).unwrap_or_else(|_| "{\"ok\":false}".into());
                    s.push('\n');
                    if out.write_all(s.as_bytes()).is_err() {
                        break;
                    }
                }
            });
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_stable() {
        let r = Request::Capture { mode: "area".into(), monitor: None, cursor: None, delay: None };
        assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"cmd":"capture","mode":"area","monitor":null}"#);
        let parsed: Request = serde_json::from_str(r#"{"cmd":"copy"}"#).unwrap();
        assert_eq!(parsed, Request::Copy { id: None });
        let parsed: Request = serde_json::from_str(r#"{"cmd":"list"}"#).unwrap();
        assert_eq!(parsed, Request::List { n: 10 });
        assert_eq!(serde_json::to_string(&Response::err("x")).unwrap(), r#"{"ok":false,"error":"x"}"#);
    }
}
