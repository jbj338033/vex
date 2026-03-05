use std::io::Write;

use anyhow::Result;
use vex_core::schema::{ApiResponse, AppResponse};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format, TextDisplay};

impl TextDisplay for Vec<AppResponse> {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        if self.is_empty() {
            return writeln!(w, "  No apps found");
        }

        let max_name = self.iter().map(|a| a.name.len()).max().unwrap_or(4).max(4);

        writeln!(w, "  {:<max_name$}  CREATED", "NAME")?;
        for app in self {
            let created = app
                .created_at
                .get(..16)
                .unwrap_or(&app.created_at)
                .replace('T', " ");
            writeln!(w, "  {:<max_name$}  {created}", app.name)?;
        }
        Ok(())
    }
}

pub async fn run(format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<Vec<AppResponse>> = client.get("/apps").await?;
    output::print_api(&response, format);
    Ok(())
}
