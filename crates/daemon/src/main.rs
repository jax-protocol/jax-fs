// CLI modules
mod cli;

use clap::{Parser, Subcommand};
use cli::{args::Args, op::Op, Bucket, Daemon, Health, Init, Version};

#[cfg(feature = "fuse")]
use cli::Mount;

#[cfg(feature = "fuse")]
command_enum! {
    (Bucket, Bucket),
    (Daemon, Daemon),
    (Health, Health),
    (Init, Init),
    (Mount, Mount),
    (Version, Version),
}

#[cfg(not(feature = "fuse"))]
command_enum! {
    (Bucket, Bucket),
    (Daemon, Daemon),
    (Health, Health),
    (Init, Init),
    (Version, Version),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Resolve remote URL: explicit flag > config api_port > hardcoded 5001
    let remote = cli::op::resolve_remote(args.remote, args.config_path.clone());

    // Build context - always has API client initialized
    let ctx = match cli::op::OpContext::new(remote, args.config_path) {
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
