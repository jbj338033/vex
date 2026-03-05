use std::io::Write;

use anyhow::Result;
use vex_core::schema::{ApiResponse, StatusResponse};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format, TextDisplay};

impl TextDisplay for StatusResponse {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        writeln!(w, "  App:     {}", self.app.name)?;
        if let Some(d) = &self.deployment {
            writeln!(w, "  Status:  {}", output::status_text(d.status))?;
        }
        if let Some(url) = &self.url {
            writeln!(w, "  URL:     {url}")?;
        }
        Ok(())
    }
}

pub async fn run(app: &str, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<StatusResponse> = client.get(&format!("/apps/{app}/status")).await?;
    output::print_api(&response, format);
    Ok(())
}
