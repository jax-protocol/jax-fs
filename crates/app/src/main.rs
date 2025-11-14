// CLI modules
mod args;
mod op;
mod ops;
mod state;

// Daemon/service modules (HTTP server, database, P2P sync)
mod daemon;

// Re-export types that daemon modules need
pub use daemon::ServiceState;

use args::Args;
use clap::{Parser, Subcommand};
use op::Op;
use ops::{Bucket, Daemon, Init, Version};

command_enum! {
    (Bucket, Bucket),
    (Daemon, Daemon),
    (Init, Init),
    (Version, Version),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Build context - always has API client initialized
    let ctx = match op::OpContext::new(args.remote, args.config_path) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Error: Failed to create API client: {}", e);
            std::process::exit(1);
        }
    };

    match args.command.execute(&ctx).await {
        Ok(output) => {
            println!("{}", output);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
