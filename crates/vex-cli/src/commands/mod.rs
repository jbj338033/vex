mod deploy;
mod destroy;
mod env;
mod login;
mod logs;
mod status;

use crate::output::Format;
use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vex", version, about = "AI-agent friendly PaaS")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true, default_value = "json", value_enum)]
    pub format: Format,
}

#[derive(Subcommand)]
pub enum Command {
    Login {
        #[arg(long, default_value = "http://localhost:3000")]
        url: String,
    },
    Deploy {
        path: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        git: Option<String>,
    },
    Logs {
        app: String,
        #[arg(short, long)]
        follow: bool,
        #[arg(short, long, default_value = "100")]
        n: u64,
    },
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    Status {
        app: String,
    },
    Destroy {
        app: String,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum EnvCommand {
    List {
        app: String,
    },
    Set {
        app: String,
        #[arg(required = true, num_args = 1..)]
        vars: Vec<String>,
    },
    Unset {
        app: String,
        #[arg(required = true, num_args = 1..)]
        keys: Vec<String>,
    },
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Login { url } => login::run(&url).await,
        Command::Deploy { path, name, git } => deploy::run(path, name, git, cli.format).await,
        Command::Logs { app, follow, n } => logs::run(&app, follow, n, cli.format).await,
        Command::Env { command } => env::run(command, cli.format).await,
        Command::Status { app } => status::run(&app, cli.format).await,
        Command::Destroy { app, force } => destroy::run(&app, force, cli.format).await,
    }
}
