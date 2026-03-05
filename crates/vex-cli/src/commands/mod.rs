mod deploy;
mod deployments;
mod destroy;
mod env;
mod list;
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

    #[arg(long, global = true, default_value = "text", value_enum)]
    pub format: Format,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = "Authenticate with GitHub")]
    Login {
        #[arg(long, default_value_t = default_api_url())]
        url: String,
    },
    #[command(about = "Deploy an application")]
    Deploy {
        path: Option<String>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        git: Option<String>,
    },
    #[command(about = "List all apps")]
    List,
    #[command(about = "Show deployment history")]
    Deployments { app: Option<String> },
    #[command(about = "View application logs")]
    Logs {
        app: Option<String>,
        #[arg(short, long)]
        follow: bool,
        #[arg(short, long, default_value = "100")]
        n: u64,
        #[arg(short, long)]
        deployment: Option<String>,
    },
    #[command(about = "Manage environment variables")]
    Env {
        #[command(subcommand)]
        command: EnvCommand,
    },
    #[command(about = "Show app status")]
    Status { app: Option<String> },
    #[command(about = "Delete an application")]
    Destroy {
        app: Option<String>,
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand)]
pub enum EnvCommand {
    #[command(about = "List environment variables")]
    List { app: Option<String> },
    #[command(about = "Set environment variables")]
    Set {
        app: Option<String>,
        #[arg(required = true, num_args = 1..)]
        vars: Vec<String>,
    },
    #[command(about = "Unset environment variables")]
    Unset {
        app: Option<String>,
        #[arg(required = true, num_args = 1..)]
        keys: Vec<String>,
    },
}

pub fn resolve_app_name(app: Option<String>) -> Result<String> {
    if let Some(name) = app {
        return Ok(name);
    }
    let dir = std::env::current_dir()?;
    dir.file_name()
        .and_then(|n| n.to_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("could not determine app name from current directory"))
}

fn default_api_url() -> String {
    if cfg!(debug_assertions) {
        "http://localhost:3000".to_string()
    } else {
        "https://api.proxia.kr".to_string()
    }
}

pub async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Login { url } => login::run(&url).await,
        Command::Deploy { path, name, git } => deploy::run(path, name, git, cli.format).await,
        Command::List => list::run(cli.format).await,
        Command::Deployments { app } => {
            let app = resolve_app_name(app)?;
            deployments::run(&app, cli.format).await
        }
        Command::Logs {
            app,
            follow,
            n,
            deployment,
        } => {
            let app = resolve_app_name(app)?;
            logs::run(&app, follow, n, deployment.as_deref(), cli.format).await
        }
        Command::Env { command } => env::run(command, cli.format).await,
        Command::Status { app } => {
            let app = resolve_app_name(app)?;
            status::run(&app, cli.format).await
        }
        Command::Destroy { app, force } => {
            let app = resolve_app_name(app)?;
            destroy::run(&app, force, cli.format).await
        }
    }
}
