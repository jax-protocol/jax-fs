use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::Extension;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::daemon::http_server::Config;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "pages/buckets/editor.html")]
pub struct FileEditorTemplate {
    pub bucket_id: String,
    pub file_path: String,
    pub file_name: String,
    pub current_path: String,
    pub content: String,
    pub is_new_file: bool,
    pub back_url: String,
    pub api_url: String,
}

#[derive(Debug, Deserialize)]
pub struct EditorQuery {
    pub path: String,
    #[serde(default)]
    pub new: bool,
    /// Where the user came from: "view" means they came from the file viewer
    pub from: Option<String>,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<EditorQuery>,
) -> askama_axum::Response {
    // Load current mount
    let mount = match state.peer().mount(bucket_id).await {
        Ok(mount) => mount,
        Err(e) => {
            tracing::error!("Failed to load mount: {}", e);
            return error_response("Failed to load bucket");
        }
    };

    let (file_path, file_name, content, is_new_file, current_path) = if query.new {
        // Creating a new note
        let dir_path = std::path::PathBuf::from(&query.path);

        // Generate untitled filename
        let filename = match generate_untitled_filename(&mount, &dir_path).await {
            Ok(name) => name,
            Err(e) => {
                tracing::error!("Failed to generate filename: {}", e);
                return error_response("Failed to generate filename");
            }
        };

        let full_path = if query.path == "/" {
            format!("/{}", filename)
        } else {
            format!("{}/{}", query.path.trim_end_matches('/'), filename)
        };

        (
            full_path.clone(),
            filename,
            String::new(), // Empty content for new files
            true,
            query.path.clone(),
        )
    } else {
        // Editing existing file
        let path_buf = std::path::PathBuf::from(&query.path);

        let file_data = match mount.cat(&path_buf).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Failed to get file content: {}", e);
                return error_response("Failed to load file content");
            }
        };

        let content = match String::from_utf8(file_data) {
            Ok(text) => text,
            Err(_) => return error_response("File is not valid UTF-8"),
        };

        let file_name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let parent_path = path_buf
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("/")
            .to_string();

        (query.path.clone(), file_name, content, false, parent_path)
    };

    // Build back URL based on where the user came from
    let back_url = if query.from.as_deref() == Some("view") {
        // They came from the file viewer, so go back to viewing the file
        format!("/buckets/{}/view?path={}", bucket_id, file_path)
    } else {
        // They came from the directory listing, so go back to that
        format!("/buckets/{}?path={}", bucket_id, current_path)
    };

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let template = FileEditorTemplate {
        bucket_id: bucket_id.to_string(),
        file_path,
        file_name,
        current_path,
        content,
        is_new_file,
        back_url,
        api_url,
    };

    template.into_response()
}

async fn generate_untitled_filename(
    mount: &common::mount::Mount,
    dir_path: &std::path::Path,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // List files in the directory
    let items = mount.ls(dir_path).await?;

    // Find all untitled-N.md files
    let mut max_number = 0;
    let untitled_prefix = "untitled-";
    let untitled_suffix = ".md";

    for (path, _) in items {
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(untitled_prefix) && name.ends_with(untitled_suffix) {
                // Extract the number between prefix and suffix
                let number_str = &name[untitled_prefix.len()..name.len() - untitled_suffix.len()];
                if let Ok(num) = number_str.parse::<u32>() {
                    max_number = max_number.max(num);
                }
            }
        }
    }

    Ok(format!("untitled-{}.md", max_number + 1))
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
