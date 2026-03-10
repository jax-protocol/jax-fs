//! Log viewer IPC commands
//!
//! Provides access to log files on disk and live tail streaming via Tauri events.

use std::io::{BufRead, Read, Seek, SeekFrom};
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::AppState;

/// Metadata for a single log file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileInfo {
    pub filename: String,
    pub size: u64,
    pub modified: String,
}

/// A page of log file contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogFileContents {
    pub lines: Vec<String>,
    pub total_lines: usize,
    pub has_more: bool,
}

/// List available log files with metadata.
#[tauri::command]
pub async fn list_log_files(state: State<'_, AppState>) -> Result<Vec<LogFileInfo>, String> {
    let inner = state.inner.read().await;
    let daemon = inner.as_ref().ok_or("Daemon not connected")?;
    let log_dir = &daemon.log_dir;

    if !log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let entries =
        std::fs::read_dir(log_dir).map_err(|e| format!("Failed to read log dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Ok(meta) = std::fs::metadata(&path) {
                    let modified = meta
                        .modified()
                        .ok()
                        .and_then(|t| {
                            let duration = t.duration_since(std::time::UNIX_EPOCH).ok()?;
                            Some(duration.as_secs().to_string())
                        })
                        .unwrap_or_default();

                    files.push(LogFileInfo {
                        filename: name.to_string(),
                        size: meta.len(),
                        modified,
                    });
                }
            }
        }
    }

    // Sort by modified time descending (newest first)
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}

/// Read contents of a specific log file with pagination.
#[tauri::command]
pub async fn read_log_file(
    state: State<'_, AppState>,
    filename: String,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<LogFileContents, String> {
    let inner = state.inner.read().await;
    let daemon = inner.as_ref().ok_or("Daemon not connected")?;
    let log_path = daemon.log_dir.join(&filename);

    // Prevent path traversal
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err("Invalid filename".to_string());
    }

    if !log_path.exists() {
        return Err(format!("Log file not found: {}", filename));
    }

    let file =
        std::fs::File::open(&log_path).map_err(|e| format!("Failed to open log file: {}", e))?;
    let reader = std::io::BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    let total_lines = all_lines.len();

    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(500);

    let lines: Vec<String> = all_lines.into_iter().skip(offset).take(limit).collect();
    let has_more = offset + limit < total_lines;

    Ok(LogFileContents {
        lines,
        total_lines,
        has_more,
    })
}

/// Read the last N lines of the most recent log file.
#[tauri::command]
pub async fn tail_log_file(
    state: State<'_, AppState>,
    lines: Option<usize>,
) -> Result<Vec<String>, String> {
    let inner = state.inner.read().await;
    let daemon = inner.as_ref().ok_or("Daemon not connected")?;
    let log_dir = &daemon.log_dir;

    let latest = find_latest_log(log_dir)?;
    let n = lines.unwrap_or(200);
    tail_n_lines(&latest, n)
}

/// Subscribe to live log streaming. Emits "log-line" events to the frontend.
#[tauri::command]
pub async fn subscribe_logs(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Check if already streaming
    if state.log_streaming.load(Ordering::SeqCst) {
        return Ok(());
    }
    state.log_streaming.store(true, Ordering::SeqCst);

    let inner = state.inner.read().await;
    let daemon = inner.as_ref().ok_or("Daemon not connected")?;
    let log_dir = daemon.log_dir.clone();
    let streaming = state.log_streaming.clone();

    tokio::spawn(async move {
        if let Err(e) = tail_stream(&log_dir, &app_handle, &streaming).await {
            tracing::warn!("Log streaming ended: {}", e);
        }
        streaming.store(false, Ordering::SeqCst);
    });

    Ok(())
}

/// Unsubscribe from live log streaming.
#[tauri::command]
pub async fn unsubscribe_logs(state: State<'_, AppState>) -> Result<(), String> {
    state.log_streaming.store(false, Ordering::SeqCst);
    Ok(())
}

/// Find the most recently modified log file in the directory.
fn find_latest_log(log_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if !log_dir.exists() {
        return Err("Log directory does not exist".to_string());
    }

    let mut latest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
    let entries =
        std::fs::read_dir(log_dir).map_err(|e| format!("Failed to read log dir: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Ok(meta) = std::fs::metadata(&path) {
                let modified = meta.modified().unwrap_or(std::time::UNIX_EPOCH);
                if latest.as_ref().is_none_or(|(_, t)| modified > *t) {
                    latest = Some((path, modified));
                }
            }
        }
    }

    latest
        .map(|(p, _)| p)
        .ok_or_else(|| "No log files found".to_string())
}

/// Read the last N lines of a file.
fn tail_n_lines(path: &std::path::Path, n: usize) -> Result<Vec<String>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let reader = std::io::BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().map_while(Result::ok).collect();
    let start = all_lines.len().saturating_sub(n);
    Ok(all_lines[start..].to_vec())
}

/// Background task that tails the log file and emits events.
async fn tail_stream(
    log_dir: &std::path::Path,
    app_handle: &tauri::AppHandle,
    streaming: &std::sync::atomic::AtomicBool,
) -> Result<(), String> {
    let path = find_latest_log(log_dir)?;
    let mut file =
        std::fs::File::open(&path).map_err(|e| format!("Failed to open log file: {}", e))?;

    // Seek to end
    file.seek(SeekFrom::End(0))
        .map_err(|e| format!("Failed to seek: {}", e))?;

    let mut buf = String::new();
    let mut current_path = path.clone();

    while streaming.load(Ordering::SeqCst) {
        // Check if a newer log file appeared (daily rotation)
        if let Ok(latest) = find_latest_log(log_dir) {
            if latest != current_path {
                file = std::fs::File::open(&latest)
                    .map_err(|e| format!("Failed to open new log file: {}", e))?;
                current_path = latest;
            }
        }

        buf.clear();
        match file.read_to_string(&mut buf) {
            Ok(0) => {
                // No new data, wait
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
            Ok(_) => {
                for line in buf.lines() {
                    if !line.is_empty() {
                        let _ = app_handle.emit("log-line", line);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Error reading log file: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }

    Ok(())
}
