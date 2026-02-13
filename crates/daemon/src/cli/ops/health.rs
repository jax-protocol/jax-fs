use clap::Args;

use jax_daemon::state::AppState;

#[derive(Args, Debug, Clone)]
pub struct Health;

#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("Health check failed: {0}")]
    Failed(String),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Health {
    type Error = HealthError;
    type Output = String;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut lines = Vec::new();

        // 1. Check config directory
        lines.push("Config:".to_string());
        match AppState::load(ctx.config_path.clone()) {
            Ok(state) => {
                lines.push(format!("  directory:    {}", state.jax_dir.display()));
                lines.push("  config.toml:  OK".to_string());
                lines.push("  db.sqlite:    OK".to_string());
                lines.push("  key.pem:      OK".to_string());
                lines.push("  blobs/:       OK".to_string());
                lines.push(format!("  api_port:     {}", state.config.api_port));
                lines.push(format!("  gateway_port: {}", state.config.gateway_port));
            }
            Err(e) => {
                lines.push(format!("  error: {}", e));
            }
        }

        // 2. Check daemon liveness
        let base = ctx.client.base_url();
        let client = ctx.client.http_client();

        lines.push(String::new());
        lines.push(format!("Daemon ({}):", base));

        let livez_url = format!("{}/_status/livez", base.as_str().trim_end_matches('/'));
        match client.get(&livez_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                lines.push("  livez:  OK".to_string());
            }
            Ok(resp) => {
                lines.push(format!("  livez:  UNHEALTHY ({})", resp.status()));
            }
            Err(_) => {
                lines.push("  livez:  NOT REACHABLE".to_string());
            }
        }

        // 3. Check daemon readiness
        let readyz_url = format!("{}/_status/readyz", base.as_str().trim_end_matches('/'));
        match client.get(&readyz_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                lines.push("  readyz: OK".to_string());
            }
            Ok(resp) => {
                lines.push(format!("  readyz: UNHEALTHY ({})", resp.status()));
            }
            Err(_) => {
                lines.push("  readyz: NOT REACHABLE".to_string());
            }
        }

        Ok(lines.join("\n"))
    }
}
