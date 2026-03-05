use std::io::Write;

use anyhow::Result;
use vex_core::schema::{ApiResponse, EnvVarResponse};

use super::{EnvCommand, resolve_app_name};
use crate::client::Client;
use crate::config;
use crate::output::{self, Format, TextDisplay};

impl TextDisplay for Vec<EnvVarResponse> {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        if self.is_empty() {
            return writeln!(w, "  No environment variables set");
        }

        let max_key = self.iter().map(|e| e.key.len()).max().unwrap_or(3).max(3);

        writeln!(w, "  {:<max_key$}  VALUE", "KEY")?;
        for var in self {
            writeln!(w, "  {:<max_key$}  {}", var.key, var.value)?;
        }
        Ok(())
    }
}

pub async fn run(command: EnvCommand, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    match command {
        EnvCommand::List { app } => {
            let app = resolve_app_name(app)?;
            let response: ApiResponse<Vec<EnvVarResponse>> =
                client.get(&format!("/apps/{app}/env")).await?;
            output::print_api(&response, format);
        }
        EnvCommand::Set { app, vars } => {
            let app = resolve_app_name(app)?;
            let mut map = std::collections::HashMap::new();
            for var in &vars {
                let (key, value) = var.split_once('=').unwrap_or((var, ""));
                map.insert(key.to_string(), value.to_string());
            }

            let response: ApiResponse<Vec<EnvVarResponse>> = client
                .put(
                    &format!("/apps/{app}/env"),
                    &serde_json::json!({"vars": map}),
                )
                .await?;

            match format {
                Format::Text => {
                    if let Some(err) = &response.error {
                        output::error(&err.message);
                        std::process::exit(1);
                    }
                    output::success("Environment variables updated");
                }
                Format::Json => {
                    println!("{}", serde_json::to_string_pretty(&response).unwrap());
                }
            }
        }
        EnvCommand::Unset { app, keys } => {
            let app = resolve_app_name(app)?;
            for key in &keys {
                let response: ApiResponse<()> =
                    client.delete(&format!("/apps/{app}/env/{key}")).await?;

                match format {
                    Format::Text => {
                        if let Some(err) = &response.error {
                            output::error(&err.message);
                            std::process::exit(1);
                        }
                        output::success(&format!("Environment variable {key} removed"));
                    }
                    Format::Json => {
                        println!("{}", serde_json::to_string_pretty(&response).unwrap());
                    }
                }
            }
        }
    }

    Ok(())
}
