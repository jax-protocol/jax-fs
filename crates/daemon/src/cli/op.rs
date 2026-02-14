use std::error::Error;
use std::path::PathBuf;

use url::Url;

use jax_daemon::http_server::api::client::{ApiClient, ApiError};
use jax_daemon::state::AppState;

/// Resolve the remote URL for the API client.
///
/// Priority: explicit `--remote` flag > config file `api_port` > hardcoded 5001.
pub fn resolve_remote(explicit: Option<Url>, config_path: Option<PathBuf>) -> Url {
    if let Some(url) = explicit {
        return url;
    }
    if let Ok(state) = AppState::load(config_path) {
        if let Ok(url) = Url::parse(&format!("http://localhost:{}", state.config.api_port)) {
            return url;
        }
    }
    Url::parse("http://localhost:5001").expect("hardcoded URL must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_remote_explicit_wins() {
        let explicit = Url::parse("http://example.com:9999").unwrap();
        let result = resolve_remote(Some(explicit.clone()), None);
        assert_eq!(result, explicit);
    }

    #[test]
    fn test_resolve_remote_falls_back_to_default() {
        // No explicit URL, no valid config path → hardcoded 5001
        let result = resolve_remote(None, Some(PathBuf::from("/nonexistent")));
        assert_eq!(result.as_str(), "http://localhost:5001/");
    }

    #[test]
    fn test_resolve_remote_no_args() {
        let result = resolve_remote(None, None);
        assert_eq!(result.port().unwrap(), 5001);
    }
}

#[derive(Clone)]
pub struct OpContext {
    /// API client (always initialized with default or custom URL)
    pub client: ApiClient,
    /// Optional custom config path (defaults to ~/.jax)
    pub config_path: Option<PathBuf>,
}

impl OpContext {
    /// Create context with custom remote URL and optional config path
    pub fn new(remote: Url, config_path: Option<PathBuf>) -> Result<Self, ApiError> {
        Ok(Self {
            client: ApiClient::new(&remote)?,
            config_path,
        })
    }
}

#[async_trait::async_trait]
pub trait Op: Send + Sync {
    type Error: Error + Send + Sync + 'static;
    type Output;

    async fn execute(&self, ctx: &OpContext) -> Result<Self::Output, Self::Error>;
}

#[macro_export]
macro_rules! command_enum {
    ($(($variant:ident, $type:ty)),* $(,)?) => {
        #[derive(Subcommand, Debug, Clone)]
        pub enum Command {
            $($variant($type),)*
        }

        #[derive(Debug)]
        pub enum OpOutput {
            $($variant(<$type as $crate::cli::op::Op>::Output),)*
        }

        #[derive(Debug, thiserror::Error)]
        pub enum OpError {
            $(
                #[error(transparent)]
                $variant(<$type as $crate::cli::op::Op>::Error),
            )*
        }

        #[async_trait::async_trait]
        impl $crate::cli::op::Op for Command {
            type Output = OpOutput;
            type Error = OpError;

            async fn execute(&self, ctx: &$crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
                match self {
                    $(
                        Command::$variant(op) => {
                            op.execute(ctx).await
                                .map(OpOutput::$variant)
                                .map_err(OpError::$variant)
                        },
                    )*
                }
            }
        }

        impl std::fmt::Display for OpOutput {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(
                        OpOutput::$variant(output) => write!(f, "{}", output),
                    )*
                }
            }
        }
    };
}
