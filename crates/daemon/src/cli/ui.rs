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
    let char_count = s.chars().count();
    if char_count <= max_len {
        s.to_string()
    } else if max_len <= 1 {
        "\u{2026}".to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 1).collect();
        format!("{truncated}\u{2026}")
    }
}

/// Format a success status line: `✓ action subject`
pub fn success(action: &str, subject: &str) -> String {
    format!(
        "{} {} {}",
        SUCCESS.green(),
        action.green().bold(),
        subject.bold()
    )
}

/// Format a failure status line: `✗ action subject`
pub fn failure(action: &str, subject: &str) -> String {
    format!(
        "{} {} {}",
        FAILURE.red(),
        action.red().bold(),
        subject.bold()
    )
}

/// Format a warning status line: `! message`
pub fn warning(message: &str) -> String {
    format!("{} {}", WARNING.yellow().bold(), message.yellow())
}

/// Format a dimmed label with a value: `  label: value`
pub fn label(label: &str, value: &impl fmt::Display) -> String {
    format!("  {} {value}", format!("{label}:").dimmed())
}

/// Color a mount/daemon status string (green for running/ok, red for stopped/error).
#[cfg(feature = "fuse")]
pub fn colored_status(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "running" | "ok" | "started" | "mounted" => status.green().to_string(),
        "stopped" | "error" | "failed" | "unmounted" => status.red().to_string(),
        _ => status.yellow().to_string(),
    }
}

/// Color a share role string.
pub fn colored_role(role: &str) -> String {
    match role.to_lowercase().as_str() {
        "owner" => role.yellow().bold().to_string(),
        "writer" | "mirror" => role.cyan().to_string(),
        "reader" => role.white().to_string(),
        _ => role.to_string(),
    }
}

/// Color a file type string (dir=blue, file=white).
pub fn colored_type(type_str: &str) -> String {
    match type_str {
        "dir" => type_str.blue().bold().to_string(),
        _ => type_str.to_string(),
    }
}

/// Create a styled table with consistent formatting.
///
/// In plain mode, uses no borders or padding (tab-separated).
/// In normal mode, uses UTF8_FULL_CONDENSED with bold headers.
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
    if value {
        "yes".green().to_string()
    } else {
        "no".dimmed().to_string()
    }
}

/// Format an error with the failure symbol.
///
/// Uses `owo_colors` which respects `set_override(false)` in plain mode.
pub fn format_error(e: &dyn std::error::Error) -> String {
    let mut msg = format!("{} {} {e}", FAILURE.red(), "error:".red().bold());
    let mut source = e.source();
    while let Some(cause) = source {
        msg.push_str(&format!("\n  {} {cause}", "caused by:".yellow()));
        source = cause.source();
    }
    msg
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
