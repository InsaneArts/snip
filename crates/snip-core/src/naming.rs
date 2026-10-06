//! File name templates (product SET-04, CleanShot's token set):
//! %y year, %m month, %n month name, %d day, %w weekday, %H hour, %M minute,
//! %S second, %p AM/PM, %t window title, %a app name, %r random, %i counter.

use chrono::{DateTime, Datelike, Local, Timelike};

pub const DEFAULT_TEMPLATE: &str = "Snip %y-%m-%d at %H.%M.%S";

#[derive(Clone, Debug, Default)]
pub struct NameContext {
    pub window_title: Option<String>,
    pub app_name: Option<String>,
    /// value for %i; the caller increments and persists it
    pub counter: u64,
    /// 4-char random token for %r (deterministic in tests)
    pub random: String,
}

/// Characters that are illegal or troublesome in file names on Linux and in
/// the apps people drop screenshots into.
fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | '\0' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    let out: String = trimmed.chars().take(120).collect();
    if !out.chars().any(|c| c.is_alphanumeric()) {
        return "Snip".into();
    }
    out
}

pub fn render(template: &str, now: DateTime<Local>, ctx: &NameContext) -> String {
    let mut out = String::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('y') => out.push_str(&format!("{:04}", now.year())),
            Some('m') => out.push_str(&format!("{:02}", now.month())),
            Some('n') => out.push_str(&now.format("%B").to_string()),
            Some('d') => out.push_str(&format!("{:02}", now.day())),
            Some('w') => out.push_str(&now.format("%A").to_string()),
            Some('H') => out.push_str(&format!("{:02}", now.hour())),
            Some('M') => out.push_str(&format!("{:02}", now.minute())),
            Some('S') => out.push_str(&format!("{:02}", now.second())),
            Some('p') => out.push_str(if now.hour() < 12 { "AM" } else { "PM" }),
            Some('t') => out.push_str(&sanitize(ctx.window_title.as_deref().unwrap_or("Screen"))),
            Some('a') => out.push_str(&sanitize(ctx.app_name.as_deref().unwrap_or("Desktop"))),
            Some('r') => out.push_str(&ctx.random),
            Some('i') => out.push_str(&ctx.counter.to_string()),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    sanitize(&out)
}

/// Pick a non-existing path `dir/stem.ext`, appending " (2)", " (3)", ... on
/// collision. Never overwrites.
pub fn unique_path(dir: &std::path::Path, stem: &str, ext: &str) -> std::path::PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2.. {
        let p = dir.join(format!("{stem} ({n}).{ext}"));
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 6, 21, 5, 9).unwrap()
    }

    #[test]
    fn default_template() {
        assert_eq!(render(DEFAULT_TEMPLATE, t(), &NameContext::default()), "Snip 2026-10-06 at 21.05.09");
    }

    #[test]
    fn tokens_and_sanitising() {
        let ctx = NameContext {
            window_title: Some("main.rs — ~/src/snip: VS Code".into()),
            app_name: Some("code".into()),
            counter: 42,
            random: "x7k2".into(),
        };
        assert_eq!(render("%a %t #%i %r %p", t(), &ctx), "code main.rs — ~-src-snip- VS Code #42 x7k2 PM");
        assert_eq!(render("100%% %q", t(), &ctx), "100% %q");
        assert_eq!(render("../..", t(), &ctx), "Snip");
    }

    #[test]
    fn unique_path_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("snip-naming-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = unique_path(&dir, "Shot", "png");
        std::fs::write(&a, b"x").unwrap();
        let b = unique_path(&dir, "Shot", "png");
        assert_eq!(b.file_name().unwrap(), "Shot (2).png");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
