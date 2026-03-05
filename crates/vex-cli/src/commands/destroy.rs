use anyhow::Result;
use vex_core::schema::ApiResponse;

use crate::client::Client;
use crate::config;
use crate::output::{self, Format};

pub async fn run(app: &str, force: bool, format: Format) -> Result<()> {
    if !force {
        output::error(&format!("use --force to confirm deletion of app '{app}'"));
        std::process::exit(1);
    }

    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<()> = client.delete(&format!("/apps/{app}")).await?;

    match format {
        Format::Text => {
            if let Some(err) = &response.error {
                output::error(&err.message);
                std::process::exit(1);
            }
            output::success(&format!("App {app} deleted"));
        }
        Format::Json => {
            println!("{}", serde_json::to_string_pretty(&response).unwrap());
        }
    }
    Ok(())
}
