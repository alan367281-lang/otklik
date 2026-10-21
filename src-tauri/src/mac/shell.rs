//! Открыть сайт, папку, файл или программу — как `open` в Терминале.

use std::process::Command;

/// `~/Documents` и `$HOME/Documents` → полный путь.
fn expand(s: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let s = if let Some(rest) = s.strip_prefix("~/") {
        format!("{home}/{rest}")
    } else {
        s.to_string()
    };
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        let mut name = String::new();
        while let Some(&n) = chars.peek() {
            if n.is_ascii_alphanumeric() || n == '_' {
                name.push(n);
                chars.next();
            } else {
                break;
            }
        }
        match std::env::var(&name) {
            Ok(v) if !name.is_empty() => out.push_str(&v),
            _ => {
                out.push('$');
                out.push_str(&name);
            }
        }
    }
    out
}

pub fn open(target: &str, args: Option<&str>) -> Result<(), String> {
    let target = expand(target.trim().trim_matches('"'));
    let mut cmd = Command::new("open");
    // Программа — по имени («Telegram») или путь к .app; остальное — как есть.
    let bare = !target.contains('/') && !target.contains(':');
    if bare && (target.ends_with(".app") || !target.contains('.')) {
        cmd.arg("-a").arg(target.trim_end_matches(".app"));
    } else {
        cmd.arg(&target);
    }
    if let Some(a) = args.filter(|a| !a.trim().is_empty()) {
        cmd.arg("--args").args(a.split_whitespace().map(expand));
    }
    let out = cmd
        .output()
        .map_err(|e| format!("не удалось запустить open: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if err.is_empty() {
            format!("macOS не смогла открыть {target}")
        } else {
            format!("не открылось {target}: {err}")
        })
    }
}
