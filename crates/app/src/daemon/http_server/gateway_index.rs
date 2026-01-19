use axum::extract::State;
use axum::response::{Html, IntoResponse, Response};

use crate::ServiceState;

/// Root page handler for the gateway.
/// Displays the gateway's public identity (NodeId).
pub async fn handler(State(state): State<ServiceState>) -> Response {
    let node_id = state.peer().id().to_string();

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>JAX Gateway</title>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', Arial, sans-serif;
            background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            color: #e5e5e5;
        }}
        .container {{
            text-align: center;
            padding: 2rem;
            max-width: 800px;
        }}
        h1 {{
            font-size: 2.5rem;
            margin-bottom: 1rem;
            background: linear-gradient(135deg, #10b981 0%, #059669 100%);
            -webkit-background-clip: text;
            -webkit-text-fill-color: transparent;
            background-clip: text;
        }}
        .subtitle {{
            color: #9ca3af;
            margin-bottom: 2rem;
            font-size: 1.1rem;
        }}
        .identity-card {{
            background: rgba(255, 255, 255, 0.05);
            border: 1px solid rgba(255, 255, 255, 0.1);
            border-radius: 12px;
            padding: 2rem;
            margin-bottom: 2rem;
        }}
        .label {{
            color: #9ca3af;
            font-size: 0.875rem;
            margin-bottom: 0.5rem;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }}
        .node-id {{
            font-family: 'Monaco', 'Courier New', monospace;
            font-size: 0.9rem;
            background: rgba(0, 0, 0, 0.3);
            padding: 1rem;
            border-radius: 8px;
            word-break: break-all;
            color: #10b981;
            border: 1px solid rgba(16, 185, 129, 0.2);
        }}
        .endpoints {{
            text-align: left;
            background: rgba(255, 255, 255, 0.03);
            border-radius: 8px;
            padding: 1.5rem;
        }}
        .endpoints h3 {{
            color: #e5e5e5;
            margin-bottom: 1rem;
            font-size: 1rem;
        }}
        .endpoint {{
            display: flex;
            justify-content: space-between;
            padding: 0.5rem 0;
            border-bottom: 1px solid rgba(255, 255, 255, 0.05);
        }}
        .endpoint:last-child {{
            border-bottom: none;
        }}
        .endpoint-path {{
            font-family: 'Monaco', 'Courier New', monospace;
            color: #60a5fa;
        }}
        .endpoint-desc {{
            color: #9ca3af;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>JAX Gateway</h1>
        <p class="subtitle">Content-addressed storage gateway</p>

        <div class="identity-card">
            <div class="label">Node Identity</div>
            <div class="node-id">{node_id}</div>
        </div>

        <div class="endpoints">
            <h3>Available Endpoints</h3>
            <div class="endpoint">
                <span class="endpoint-path">/gw/:bucket_id/*path</span>
                <span class="endpoint-desc">Serve bucket content</span>
            </div>
            <div class="endpoint">
                <span class="endpoint-path">/_status/identity</span>
                <span class="endpoint-desc">Node identity (JSON)</span>
            </div>
            <div class="endpoint">
                <span class="endpoint-path">/_status/livez</span>
                <span class="endpoint-desc">Liveness check</span>
            </div>
            <div class="endpoint">
                <span class="endpoint-path">/_status/readyz</span>
                <span class="endpoint-desc">Readiness check</span>
            </div>
            <div class="endpoint">
                <span class="endpoint-path">/_status/version</span>
                <span class="endpoint-desc">Version info</span>
            </div>
        </div>
    </div>
</body>
</html>"#
    );

    Html(html).into_response()
}
