use std::fmt;

use clap::Args;
use owo_colors::OwoColorize;

use crate::cli::ui;
use jax_daemon::http_server::api::client::{resolve_bucket, ApiError};
use jax_daemon::http_server::api::v0::bucket::stat::{StatRequest, StatResponse};

#[derive(Args, Debug, Clone)]
pub struct Stat {
    /// Bucket name or UUID
    pub bucket: String,
}

#[derive(Debug)]
pub struct StatOutput {
    pub response: StatResponse,
}

impl fmt::Display for StatOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = &self.response;

        if ui::is_plain() {
            writeln!(f, "Bucket: {}", r.name)?;
            writeln!(f, "ID: {}", r.bucket_id)?;
            writeln!(f, "Version: {}", r.link.hash())?;
            writeln!(f, "Height: {}", r.height)?;
            writeln!(f, "Published: {}", if r.published { "yes" } else { "no" })?;

            if !r.peers.is_empty() {
                writeln!(f)?;
                let rows: Vec<Vec<String>> = r
                    .peers
                    .iter()
                    .map(|p| {
                        let marker = if p.is_self { "(you)" } else { "" };
                        vec![p.public_key.clone(), p.role.clone(), marker.to_string()]
                    })
                    .collect();
                ui::write_plain_rows(f, &rows)?;
            }

            return Ok(());
        }

        writeln!(f, "{}", ui::label("Bucket", &r.name))?;
        writeln!(f, "{}", ui::label("ID", &r.bucket_id))?;
        writeln!(
            f,
            "{}",
            ui::label("Version", &ui::truncate(&r.link.hash().to_string(), 16))
        )?;
        writeln!(f, "{}", ui::label("Height", &r.height))?;
        writeln!(f, "{}", ui::label("Published", &ui::yes_no(r.published)))?;

        if !r.peers.is_empty() {
            writeln!(f)?;
            let mut table = ui::styled_table(vec!["PEER", "ROLE", ""]);
            for p in &r.peers {
                let marker = if p.is_self {
                    "(you)".dimmed().to_string()
                } else {
                    String::new()
                };
                table.add_row(vec![
                    ui::truncate(&p.public_key, 24),
                    ui::colored_role(&p.role),
                    marker,
                ]);
            }
            write!(f, "{table}")?;
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StatError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),
}

#[async_trait::async_trait]
impl crate::cli::op::Op for Stat {
    type Error = StatError;
    type Output = StatOutput;

    async fn execute(&self, ctx: &crate::cli::op::OpContext) -> Result<Self::Output, Self::Error> {
        let mut client = ctx.client.clone();
        let bucket_id = resolve_bucket(&mut client, &self.bucket).await?;
        let request = StatRequest { bucket_id };
        let response: StatResponse = client.call(request).await?;

        Ok(StatOutput { response })
    }
}
