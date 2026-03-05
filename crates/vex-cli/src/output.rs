use std::io::{self, Write};

use clap::ValueEnum;
use console::{Style, style};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use vex_core::model::DeploymentStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    Text,
}

pub trait TextDisplay {
    fn fmt_text(&self, w: &mut dyn Write) -> io::Result<()>;
}

pub fn print<T: Serialize + TextDisplay>(value: &T, format: Format) {
    match format {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(value).unwrap());
        }
        Format::Text => {
            let mut out = io::stdout().lock();
            value.fmt_text(&mut out).ok();
        }
    }
}

pub fn print_api<T: Serialize + TextDisplay>(
    response: &vex_core::schema::ApiResponse<T>,
    format: Format,
) {
    match format {
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(response).unwrap());
        }
        Format::Text => {
            if let Some(err) = &response.error {
                error(&err.message);
                std::process::exit(1);
            }
            if let Some(data) = &response.data {
                let mut out = io::stdout().lock();
                data.fmt_text(&mut out).ok();
            }
        }
    }
}

pub fn success(msg: &str) {
    let green = Style::new().green();
    eprintln!("{} {msg}", green.apply_to("✔"));
}

pub fn error(msg: &str) {
    let red = Style::new().red();
    eprintln!("{} {msg}", red.apply_to("✗"));
}

pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("◐◓◑◒ ")
            .template("{spinner} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

pub fn status_dot(status: DeploymentStatus) -> String {
    let dot = "●";
    match status {
        DeploymentStatus::Running => style(dot).green().to_string(),
        DeploymentStatus::Failed => style(dot).red().to_string(),
        DeploymentStatus::Stopped => style(dot).red().to_string(),
        DeploymentStatus::Pending | DeploymentStatus::Building | DeploymentStatus::Deploying => {
            style(dot).yellow().to_string()
        }
    }
}

pub fn status_text(status: DeploymentStatus) -> String {
    format!("{} {status}", status_dot(status))
}

impl TextDisplay for () {
    fn fmt_text(&self, _w: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }
}
