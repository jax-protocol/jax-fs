use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::ServiceState;

#[derive(Debug, Clone)]
pub struct LogEntryDisplay {
    pub height: u64,
    pub name: String,
    pub current_link: String,
    pub current_link_short: String,
    pub previous_link: Option<String>,
    pub previous_link_full: String,
    pub previous_link_short: String,
    pub created_at_formatted: String,
}

#[derive(Template)]
#[template(path = "bucket_logs.html")]
pub struct BucketLogsTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub entries: Vec<LogEntryDisplay>,
    pub page: u32,
    pub page_display: u32,
    pub page_size: u32,
    pub total_entries: i64,
    pub total_pages: u32,
    pub start_entry: u32,
    pub end_entry: u32,
    pub has_next: bool,
    pub prev_page: u32,
    pub next_page: u32,
    pub last_page: u32,
}

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

fn default_page_size() -> u32 {
    20
}

fn shorten_link(link: &str) -> String {
    if link.len() > 16 {
        format!("{}...{}", &link[..8], &link[link.len() - 8..])
    } else {
        link.to_string()
    }
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<LogsQuery>,
) -> askama_axum::Response {
    // Validate and cap page size
    let page_size = query.page_size.min(100).max(1);
    let page = query.page;

    // Get bucket info to show the bucket name
    let bucket = match state.database().get_bucket_info(&bucket_id).await {
        Ok(Some(bucket)) => bucket,
        Ok(None) => return error_response("Bucket not found"),
        Err(e) => return error_response(&format!("Database error: {}", e)),
    };

    // Get total count
    let total_entries = match state.database().get_bucket_log_count(&bucket_id).await {
        Ok(count) => count,
        Err(e) => return error_response(&format!("Failed to get log count: {}", e)),
    };

    if total_entries == 0 {
        let template = BucketLogsTemplate {
            bucket_id: bucket_id.to_string(),
            bucket_name: bucket.name,
            entries: vec![],
            page: 0,
            page_display: 1,
            page_size,
            total_entries,
            total_pages: 0,
            start_entry: 0,
            end_entry: 0,
            has_next: false,
            prev_page: 0,
            next_page: 0,
            last_page: 0,
        };
        return template.into_response();
    }

    // Calculate pagination
    let total_pages = ((total_entries as f64) / (page_size as f64)).ceil() as u32;
    let page = page.min(total_pages.saturating_sub(1));

    // Get log entries
    let entries = match state
        .database()
        .get_bucket_logs(&bucket_id, page, page_size)
        .await
    {
        Ok(entries) => entries,
        Err(e) => return error_response(&format!("Failed to get log entries: {}", e)),
    };

    // Convert to display format
    let entries_display: Vec<LogEntryDisplay> = entries
        .into_iter()
        .map(|entry| {
            let current_link = entry.current_link.to_string();
            let current_link_short = shorten_link(&current_link);

            let (previous_link, previous_link_full, previous_link_short) =
                if let Some(prev) = entry.previous_link {
                    let prev_str = prev.to_string();
                    let prev_short = shorten_link(&prev_str);
                    (Some(prev_str.clone()), prev_str, prev_short)
                } else {
                    (None, String::new(), String::new())
                };

            let created_at_formatted = entry
                .created_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_else(|_| "Invalid date".to_string());

            LogEntryDisplay {
                height: entry.height,
                name: entry.name,
                current_link,
                current_link_short,
                previous_link,
                previous_link_full,
                previous_link_short,
                created_at_formatted,
            }
        })
        .collect();

    // Calculate display info
    let start_entry = (page * page_size) + 1;
    let end_entry = ((page + 1) * page_size).min(total_entries as u32);
    let has_next = (page + 1) < total_pages;

    let template = BucketLogsTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        entries: entries_display,
        page,
        page_display: page + 1,
        page_size,
        total_entries,
        total_pages,
        start_entry,
        end_entry,
        has_next,
        prev_page: page.saturating_sub(1),
        next_page: page + 1,
        last_page: total_pages.saturating_sub(1),
    };

    template.into_response()
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
