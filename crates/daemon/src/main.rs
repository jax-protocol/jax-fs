// CLI modules
mod cli;

use clap::{Parser, Subcommand};
use cli::{args::Args, op::Op, ui, Bucket, Daemon, Health, Init, Update, Version};
use std::io::IsTerminal;

#[cfg(feature = "fuse")]
use cli::Mount;

#[cfg(feature = "fuse")]
command_enum! {
    (Bucket, Bucket),
    (Daemon, Daemon),
    (Health, Health),
    (Init, Init),
    (Mount, Mount),
    (Update, Update),
    (Version, Version),
}

#[cfg(not(feature = "fuse"))]
command_enum! {
    (Bucket, Bucket),
    (Daemon, Daemon),
    (Health, Health),
    (Init, Init),
    (Update, Update),
    (Version, Version),
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    // Configure plain mode (disables colors and table borders)
    if args.plain {
        ui::set_plain(true);
        owo_colors::set_override(false);
    }

    // Resolve remote URL: explicit flag > config api_port > hardcoded 5001
    let remote = cli::op::resolve_remote(args.remote, args.config_path.clone());

    // Build context - always has API client initialized
    let mp = indicatif::MultiProgress::new();
    if !std::io::stdout().is_terminal() || args.plain {
        mp.set_draw_target(indicatif::ProgressDrawTarget::hidden());
    }

    let ctx = match cli::op::OpContext::new(remote, args.config_path, mp) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("{}", ui::format_error(&e));
            std::process::exit(1);
        }
    };

    match args.command.execute(&ctx).await {
        Ok(output) => {
            println!("{output}");
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{}", ui::format_error(&e));
            std::process::exit(1);
        }
    }
}
