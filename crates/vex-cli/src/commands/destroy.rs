use anyhow::{Result, bail};
use vex_core::schema::ApiResponse;

use crate::client::Client;
use crate::config;
use crate::output::{self, Format};

pub async fn run(app: &str, force: bool, format: Format) -> Result<()> {
    if !force {
        bail!("use --force to confirm deletion of app '{app}'");
    }

    let cfg = config::load()?;
    let client = Client::new(&cfg);

    let response: ApiResponse<()> = client.delete(&format!("/apps/{app}")).await?;
    output::print(&response, format);
    Ok(())
}
