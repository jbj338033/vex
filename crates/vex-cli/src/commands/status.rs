use anyhow::Result;
use vex_core::schema::{ApiResponse, StatusResponse};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format};

pub async fn run(app: &str, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<StatusResponse> = client.get(&format!("/apps/{app}/status")).await?;
    output::print(&response, format);
    Ok(())
}
