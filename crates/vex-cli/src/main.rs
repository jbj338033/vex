mod client;
mod commands;
mod config;
mod output;

use clap::Parser;
use commands::Cli;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = commands::run(cli).await {
        output::error(&format!("{e}"));
        std::process::exit(1);
    }
}
