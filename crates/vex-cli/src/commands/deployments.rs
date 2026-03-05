use std::io::Write;

use anyhow::Result;
use vex_core::schema::{ApiResponse, DeploymentResponse};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format, TextDisplay, status_text};

impl TextDisplay for Vec<DeploymentResponse> {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        if self.is_empty() {
            return writeln!(w, "  No deployments found");
        }

        writeln!(w, "  {:<14}  {:<20}  ID", "STATUS", "CREATED")?;
        for d in self {
            let short_id = &d.id[..12];
            let created = d.created_at.get(..16).unwrap_or(&d.created_at);
            writeln!(
                w,
                "  {:<14}  {:<20}  {short_id}..",
                status_text(d.status),
                created,
            )?;
        }
        Ok(())
    }
}

pub async fn run(app: &str, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<Vec<DeploymentResponse>> =
        client.get(&format!("/apps/{app}/deployments")).await?;
    output::print_api(&response, format);
    Ok(())
}
