use anyhow::Result;
use vex_core::schema::{ApiResponse, EnvVarResponse};

use super::EnvCommand;
use crate::client::Client;
use crate::config;
use crate::output::{self, Format};

pub async fn run(command: EnvCommand, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    match command {
        EnvCommand::List { app } => {
            let response: ApiResponse<Vec<EnvVarResponse>> =
                client.get(&format!("/apps/{app}/env")).await?;
            output::print(&response, format);
        }
        EnvCommand::Set { app, vars } => {
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
            output::print(&response, format);
        }
        EnvCommand::Unset { app, keys } => {
            for key in &keys {
                let response: ApiResponse<()> =
                    client.delete(&format!("/apps/{app}/env/{key}")).await?;
                output::print(&response, format);
            }
        }
    }

    Ok(())
}
