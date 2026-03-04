use anyhow::{Context, Result};
use std::path::PathBuf;
use vex_core::schema::{ApiResponse, DeploymentResponse};

use crate::client::{self, Client};
use crate::config;
use crate::output::{self, Format};

pub async fn run(
    path: Option<String>,
    name: Option<String>,
    git_url: Option<String>,
    format: Format,
) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    if let Some(git_url) = git_url {
        let app_name = name.context("--name is required when using --git")?;

        let _ = client
            .post::<serde_json::Value>("/apps", &serde_json::json!({"name": &app_name}))
            .await;

        let response: ApiResponse<DeploymentResponse> = client
            .post(
                &format!("/apps/{app_name}/deploy"),
                &serde_json::json!({"git_url": git_url}),
            )
            .await?;

        output::print(&response, format);
        return Ok(());
    }

    let dir = PathBuf::from(path.unwrap_or_else(|| ".".to_string()));
    let dir = dir.canonicalize().context("directory not found")?;

    let app_name = name.unwrap_or_else(|| {
        dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app")
            .to_string()
    });

    let _ = client
        .post::<serde_json::Value>("/apps", &serde_json::json!({"name": &app_name}))
        .await;

    let tarball = client::create_tarball(&dir)?;

    let response: ApiResponse<DeploymentResponse> =
        client.deploy_tarball(&app_name, tarball).await?;

    output::print(&response, format);
    Ok(())
}
