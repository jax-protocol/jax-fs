use regex::Regex;
use std::sync::LazyLock;
use uuid::Uuid;

// Regex patterns for URL rewriting.
//
// These match any URL-like value in HTML attributes or markdown links.
// External URLs (http://, https://, data:, mailto:, #anchors) are filtered
// in the replacement closure since the regex crate doesn't support look-ahead.
static HTML_ATTR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?P<attr>(?:href|src|action|data|srcset))=["'](?P<url>[^"'\s][^"']*)["']"#)
        .unwrap()
});

static MARKDOWN_LINK_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"\]\((?P<url>[^)\s][^)]*)\)"#).unwrap());

/// Rewrites relative URLs in HTML/Markdown content to absolute gateway URLs.
pub(super) fn rewrite_relative_urls(
    content: &str,
    current_path: &str,
    bucket_id: &Uuid,
    host: &str,
    at_hash: Option<&str>,
) -> String {
    let current_dir = extract_current_dir(current_path);

    let content = HTML_ATTR_REGEX.replace_all(content, |caps: &regex::Captures| {
        let attr = &caps["attr"];
        let url = &caps["url"];
        if is_external_url(url) {
            caps[0].to_string()
        } else {
            let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host, at_hash);
            format!(r#"{}="{}""#, attr, absolute_url)
        }
    });

    let content = MARKDOWN_LINK_REGEX.replace_all(&content, |caps: &regex::Captures| {
        let url = &caps["url"];
        if is_external_url(url) {
            caps[0].to_string()
        } else {
            let absolute_url = resolve_relative_url(url, &current_dir, bucket_id, host, at_hash);
            format!("]({})", absolute_url)
        }
    });

    content.to_string()
}

/// Rewrites path-like values in CSV content to absolute gateway URLs.
pub(super) fn rewrite_csv_urls(
    content: &str,
    current_path: &str,
    bucket_id: &Uuid,
    host: &str,
    at_hash: Option<&str>,
) -> String {
    let current_dir = extract_current_dir(current_path);
    let mut result = String::with_capacity(content.len());

    for line in content.split('\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        let mut first_cell = true;
        for cell in parse_csv_cells(line) {
            if !first_cell {
                result.push(',');
            }
            first_cell = false;

            let unquoted = unquote_csv_cell(&cell);
            if looks_like_path(&unquoted) {
                let resolved =
                    resolve_relative_url(&unquoted, &current_dir, bucket_id, host, at_hash);
                // Preserve quoting if original was quoted
                if cell.trim().starts_with('"') {
                    result.push_str(&format!("\"{}\"", resolved));
                } else {
                    result.push_str(&resolved);
                }
            } else {
                result.push_str(&cell);
            }
        }
    }

    result
}

/// Converts CSV content to an HTML table with clickable links for path-like cells.
pub(super) fn csv_to_html(
    csv_content: &str,
    current_path: &str,
    bucket_id: &Uuid,
    host: &str,
    at_hash: Option<&str>,
) -> String {
    let current_dir = extract_current_dir(current_path);
    let mut rows = Vec::new();

    for line in csv_content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<String> = parse_csv_cells(line)
            .into_iter()
            .map(|c| unquote_csv_cell(&c))
            .collect();
        rows.push(cells);
    }

    let mut table_html = String::new();
    table_html.push_str("<table>\n");

    for (i, row) in rows.iter().enumerate() {
        let tag = if i == 0 { "th" } else { "td" };
        table_html.push_str("<tr>");
        for cell in row {
            let cell_html = if i > 0 && looks_like_path(cell.trim()) {
                let url = resolve_relative_url(cell.trim(), &current_dir, bucket_id, host, at_hash);
                format!(
                    "<{tag}><a href=\"{}\">{}</a></{tag}>",
                    html_escape(&url),
                    html_escape(cell.trim())
                )
            } else {
                format!("<{tag}>{}</{tag}>", html_escape(cell))
            };
            table_html.push_str(&cell_html);
        }
        table_html.push_str("</tr>\n");
    }

    table_html.push_str("</table>");

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 1200px; margin: 40px auto; padding: 0 20px; line-height: 1.6; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f4f4f4; }}
        tr:hover {{ background-color: #f9f9f9; }}
        a {{ color: #0366d6; text-decoration: none; }}
        a:hover {{ text-decoration: underline; }}
    </style>
</head>
<body>
{}
</body>
</html>"#,
        table_html
    )
}

/// Converts markdown content to HTML.
pub(super) fn markdown_to_html(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; max-width: 800px; margin: 40px auto; padding: 0 20px; line-height: 1.6; }}
        img {{ max-width: 100%; height: auto; }}
        code {{ background: #f4f4f4; padding: 2px 6px; border-radius: 3px; }}
        pre {{ background: #f4f4f4; padding: 12px; border-radius: 5px; overflow-x: auto; }}
        table {{ border-collapse: collapse; width: 100%; }}
        th, td {{ border: 1px solid #ddd; padding: 8px; text-align: left; }}
        th {{ background-color: #f4f4f4; }}
    </style>
</head>
<body>
{}
</body>
</html>"#,
        html_output
    )
}

/// Returns true for URLs that should not be rewritten (external, data URIs, anchors, etc.)
fn is_external_url(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("data:")
        || url.starts_with("mailto:")
        || url.starts_with('#')
        || url.starts_with("javascript:")
}

fn extract_current_dir(current_path: &str) -> String {
    if current_path == "/" {
        "".to_string()
    } else {
        std::path::Path::new(current_path)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_string()
    }
}

fn resolve_relative_url(
    relative_url: &str,
    current_dir: &str,
    bucket_id: &Uuid,
    host: &str,
    at_hash: Option<&str>,
) -> String {
    let path = if relative_url.starts_with('/') {
        relative_url.to_string()
    } else {
        // For all relative paths (./foo, ../foo, ../../foo, bare foo),
        // join with current_dir and let PathBuf normalization handle it.
        format!("{}/{}", current_dir, relative_url)
    };

    let normalized = std::path::PathBuf::from(&path).components().fold(
        std::path::PathBuf::new(),
        |mut acc, component| {
            match component {
                std::path::Component::ParentDir => {
                    acc.pop();
                }
                std::path::Component::Normal(part) => {
                    acc.push(part);
                }
                _ => {}
            }
            acc
        },
    );

    let normalized_str = normalized.to_str().unwrap_or("");
    let base = format!(
        "{}/gw/{}/{}",
        host.trim_end_matches('/'),
        bucket_id,
        normalized_str
    );

    match at_hash {
        Some(hash) => format!("{}?at={}", base, hash),
        None => base,
    }
}

/// Heuristic: a CSV cell value looks like a file path if it contains `/`
/// or starts with `./` or `../`.
fn looks_like_path(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    value.contains('/') || value.starts_with("./") || value.starts_with("../")
}

/// Parse a single CSV line into cells, respecting quoted fields.
fn parse_csv_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '"' if !in_quotes => {
                in_quotes = true;
                current.push('"');
            }
            '"' if in_quotes => {
                current.push('"');
                if chars.peek() == Some(&'"') {
                    // Escaped quote
                    current.push(chars.next().unwrap());
                } else {
                    in_quotes = false;
                }
            }
            ',' if !in_quotes => {
                cells.push(current);
                current = String::new();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    cells.push(current);
    cells
}

/// Remove surrounding quotes and unescape doubled quotes in a CSV cell.
fn unquote_csv_cell(cell: &str) -> String {
    let trimmed = cell.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        trimmed[1..trimmed.len() - 1].replace("\"\"", "\"")
    } else {
        trimmed.to_string()
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice_bucket() -> Uuid {
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap()
    }

    const HOST: &str = "http://localhost:3000";

    // --- HTML: deep relative paths ---

    #[test]
    fn alice_rewrites_deeply_nested_relative_path_in_html() {
        let html = r#"<a href="../../../../styles/main.css">link</a>"#;
        let result =
            rewrite_relative_urls(html, "/a/b/c/d/index.html", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/styles/main.css"),
            "got: {}",
            result
        );
    }

    #[test]
    fn alice_rewrites_triple_parent_in_html() {
        let html = r#"<img src="../../../img/photo.jpg">"#;
        let result = rewrite_relative_urls(html, "/a/b/c/readme.html", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/img/photo.jpg"),
            "got: {}",
            result
        );
    }

    // --- HTML: bare relative paths ---

    #[test]
    fn alice_rewrites_bare_relative_path_in_html() {
        let html = r#"<img src="assets/2.png">"#;
        let result = rewrite_relative_urls(html, "/docs/page.html", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/docs/assets/2.png"),
            "got: {}",
            result
        );
    }

    // --- External URLs untouched ---

    #[test]
    fn alice_does_not_rewrite_https_urls() {
        let html = r#"<a href="https://example.com/page">link</a>"#;
        let result = rewrite_relative_urls(html, "/index.html", &alice_bucket(), HOST, None);
        assert!(
            result.contains("https://example.com/page"),
            "got: {}",
            result
        );
    }

    #[test]
    fn alice_does_not_rewrite_data_uris() {
        let html = r#"<img src="data:image/png;base64,abc">"#;
        let result = rewrite_relative_urls(html, "/index.html", &alice_bucket(), HOST, None);
        assert!(
            result.contains("data:image/png;base64,abc"),
            "got: {}",
            result
        );
    }

    #[test]
    fn alice_does_not_rewrite_anchor_links() {
        let html = r##"<a href="#section">jump</a>"##;
        let result = rewrite_relative_urls(html, "/index.html", &alice_bucket(), HOST, None);
        assert!(result.contains("#section"), "got: {}", result);
    }

    // --- ?at= propagation ---

    #[test]
    fn bob_version_pinned_urls_include_at_param() {
        let html = r#"<a href="./other.html">link</a>"#;
        let result = rewrite_relative_urls(
            html,
            "/docs/index.html",
            &alice_bucket(),
            HOST,
            Some("bafy2bzaceabc123"),
        );
        assert!(result.contains("?at=bafy2bzaceabc123"), "got: {}", result);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/docs/other.html"),
            "got: {}",
            result
        );
    }

    #[test]
    fn bob_unpinned_urls_have_no_at_param() {
        let html = r#"<a href="./other.html">link</a>"#;
        let result = rewrite_relative_urls(html, "/docs/index.html", &alice_bucket(), HOST, None);
        assert!(!result.contains("?at="), "got: {}", result);
    }

    // --- Markdown: deep relative paths ---

    #[test]
    fn alice_rewrites_deep_relative_in_markdown() {
        let md = r#"[photo](../../../img/photo.jpg)"#;
        let result = rewrite_relative_urls(md, "/a/b/c/readme.md", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/img/photo.jpg"),
            "got: {}",
            result
        );
    }

    // --- CSV: raw rewriting ---

    #[test]
    fn carol_rewrites_paths_in_csv_cells() {
        let csv = "name,link\nitem,./data/report.html\n";
        let result = rewrite_csv_urls(csv, "/reports/index.csv", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/reports/data/report.html"),
            "got: {}",
            result
        );
    }

    #[test]
    fn carol_csv_preserves_non_path_cells() {
        let csv = "name,value\nhello,world\n";
        let result = rewrite_csv_urls(csv, "/data.csv", &alice_bucket(), HOST, None);
        assert_eq!(result, csv, "non-path cells should be unchanged");
    }

    #[test]
    fn carol_csv_with_version_pinning() {
        let csv = "file\n./assets/1.png\n";
        let result = rewrite_csv_urls(
            csv,
            "/index.csv",
            &alice_bucket(),
            HOST,
            Some("bafy2bzaceabc123"),
        );
        assert!(result.contains("?at=bafy2bzaceabc123"), "got: {}", result);
    }

    #[test]
    fn carol_csv_handles_quoted_paths() {
        let csv = "name,path\n\"item\",\"./assets/file.txt\"\n";
        let result = rewrite_csv_urls(csv, "/data.csv", &alice_bucket(), HOST, None);
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/assets/file.txt"),
            "got: {}",
            result
        );
    }

    // --- CSV: HTML rendering ---

    #[test]
    fn carol_csv_to_html_renders_table() {
        let csv = "name,link\nitem,./data/report.html\n";
        let result = csv_to_html(csv, "/reports/index.csv", &alice_bucket(), HOST, None);
        assert!(result.contains("<table>"), "should contain table tag");
        assert!(result.contains("<th>name</th>"), "should have header");
        assert!(result.contains("<a href="), "paths should be links");
        assert!(
            result.contains("/gw/550e8400-e29b-41d4-a716-446655440000/reports/data/report.html"),
            "got: {}",
            result
        );
    }

    #[test]
    fn carol_csv_to_html_does_not_link_header_row() {
        let csv = "path,name\n./file.txt,hello\n";
        let result = csv_to_html(csv, "/data.csv", &alice_bucket(), HOST, None);
        // Header row should use <th>, not <a>
        assert!(result.contains("<th>path</th>"), "got: {}", result);
    }

    #[test]
    fn carol_csv_to_html_escapes_html() {
        let csv = "name\n<script>alert(1)</script>\n";
        let result = csv_to_html(csv, "/data.csv", &alice_bucket(), HOST, None);
        assert!(
            !result.contains("<script>"),
            "HTML should be escaped, got: {}",
            result
        );
        assert!(result.contains("&lt;script&gt;"), "got: {}", result);
    }

    // --- resolve_relative_url edge cases ---

    #[test]
    fn resolve_absolute_path() {
        let result = resolve_relative_url("/assets/img.png", "/docs", &alice_bucket(), HOST, None);
        assert_eq!(
            result,
            "http://localhost:3000/gw/550e8400-e29b-41d4-a716-446655440000/assets/img.png"
        );
    }

    #[test]
    fn resolve_dot_slash() {
        let result = resolve_relative_url("./style.css", "/docs", &alice_bucket(), HOST, None);
        assert_eq!(
            result,
            "http://localhost:3000/gw/550e8400-e29b-41d4-a716-446655440000/docs/style.css"
        );
    }

    #[test]
    fn resolve_traversal_past_root_clamps() {
        // Going up more levels than exist should clamp at root
        let result =
            resolve_relative_url("../../../../file.txt", "/a", &alice_bucket(), HOST, None);
        assert_eq!(
            result,
            "http://localhost:3000/gw/550e8400-e29b-41d4-a716-446655440000/file.txt"
        );
    }
}
