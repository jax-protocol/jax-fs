use std::fmt;
use std::sync::OnceLock;

use comfy_table::{presets, Table};
use owo_colors::OwoColorize;

// Status symbols
pub const SUCCESS: &str = "\u{2713}"; // ✓
pub const PROGRESS: &str = "\u{2192}"; // →
pub const FAILURE: &str = "\u{2717}"; // ✗
pub const WARNING: &str = "!";

// Global plain mode flag
static PLAIN_MODE: OnceLock<bool> = OnceLock::new();

pub fn set_plain(plain: bool) {
    PLAIN_MODE.set(plain).ok();
}

pub fn is_plain() -> bool {
    PLAIN_MODE.get().copied().unwrap_or(false)
}

/// Truncate a string to `max_len` characters, appending "…" if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        "\u{2026}".to_string()
    } else {
        format!("{}\u{2026}", &s[..max_len - 1])
    }
}

/// Format a success status line: `✓ action subject`
pub fn success(action: &str, subject: &str) -> String {
    if is_plain() {
        format!("{action} {subject}")
    } else {
        format!(
            "{} {} {}",
            SUCCESS.green(),
            action.green().bold(),
            subject.bold()
        )
    }
}

/// Format a failure status line: `✗ action subject`
pub fn failure(action: &str, subject: &str) -> String {
    if is_plain() {
        format!("{action} {subject}")
    } else {
        format!(
            "{} {} {}",
            FAILURE.red(),
            action.red().bold(),
            subject.bold()
        )
    }
}

/// Format a progress status line: `→ message`
pub fn progress(message: &str) -> String {
    if is_plain() {
        message.to_string()
    } else {
        format!("{} {}", PROGRESS.cyan(), message)
    }
}

/// Format a warning status line: `! message`
pub fn warning(message: &str) -> String {
    if is_plain() {
        message.to_string()
    } else {
        format!("{} {}", WARNING.yellow().bold(), message.yellow())
    }
}

/// Format a dimmed label with a value: `  label: value`
pub fn label(label: &str, value: &impl fmt::Display) -> String {
    if is_plain() {
        format!("  {label} {value}")
    } else {
        format!("  {} {value}", format!("{label}:").dimmed())
    }
}

/// Color a mount/daemon status string (green for running/ok, red for stopped/error).
pub fn colored_status(status: &str) -> String {
    if is_plain() {
        return status.to_string();
    }
    match status.to_lowercase().as_str() {
        "running" | "ok" | "started" | "mounted" => status.green().to_string(),
        "stopped" | "error" | "failed" | "unmounted" => status.red().to_string(),
        _ => status.yellow().to_string(),
    }
}

/// Color a share role string.
pub fn colored_role(role: &str) -> String {
    if is_plain() {
        return role.to_string();
    }
    match role.to_lowercase().as_str() {
        "owner" => role.yellow().bold().to_string(),
        "writer" | "mirror" => role.cyan().to_string(),
        "reader" => role.white().to_string(),
        _ => role.to_string(),
    }
}

/// Color a file type string (dir=blue, file=white).
pub fn colored_type(type_str: &str) -> String {
    if is_plain() {
        return type_str.to_string();
    }
    match type_str {
        "dir" => type_str.blue().bold().to_string(),
        _ => type_str.to_string(),
    }
}

/// Create a styled table with consistent formatting.
pub fn styled_table(headers: Vec<&str>) -> Table {
    let mut table = Table::new();
    if is_plain() {
        table
            .load_preset(presets::NOTHING)
            .set_header(headers.iter().map(|h| h.to_string()));
    } else {
        table
            .load_preset(presets::UTF8_FULL_CONDENSED)
            .set_header(headers.iter().map(|h| h.bold().to_string()));
    }
    table
}

/// Format a yes/no boolean as colored text.
pub fn yes_no(value: bool) -> String {
    if is_plain() {
        return if value { "yes" } else { "no" }.to_string();
    }
    if value {
        "yes".green().to_string()
    } else {
        "no".dimmed().to_string()
    }
}

/// Write plain output for a list of tab-separated rows.
pub fn write_plain_rows(f: &mut fmt::Formatter<'_>, rows: &[Vec<String>]) -> fmt::Result {
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            writeln!(f)?;
        }
        write!(f, "{}", row.join("\t"))?;
    }
    Ok(())
}

/// Format an error with the failure symbol.
pub fn format_error(e: &dyn std::error::Error) -> String {
    if is_plain() {
        let mut msg = format!("error: {e}");
        let mut source = e.source();
        while let Some(cause) = source {
            msg.push_str(&format!("\n  caused by: {cause}"));
            source = cause.source();
        }
        msg
    } else {
        let mut msg = format!("{} {} {e}", FAILURE.red(), "error:".red().bold());
        let mut source = e.source();
        while let Some(cause) = source {
            msg.push_str(&format!("\n  {} {cause}", "caused by:".yellow()));
            source = cause.source();
        }
        msg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_short() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_exact() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_long() {
        let result = truncate("abcdefghij", 6);
        assert_eq!(result, "abcde\u{2026}");
    }

    #[test]
    fn test_truncate_tiny() {
        assert_eq!(truncate("abcdef", 1), "\u{2026}");
    }

    #[test]
    fn test_yes_no() {
        // In default (non-plain) mode the strings contain ANSI codes,
        // but should include "yes" / "no" text.
        let y = yes_no(true);
        let n = yes_no(false);
        assert!(y.contains("yes"));
        assert!(n.contains("no"));
    }
}
