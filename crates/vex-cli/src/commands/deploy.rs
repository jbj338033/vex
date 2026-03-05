use std::io::Write;

use anyhow::{Context, Result};
use std::path::PathBuf;
use vex_core::schema::{ApiResponse, DeploymentResponse};

use crate::client::{self, Client};
use crate::config;
use crate::output::{self, Format, TextDisplay};

impl TextDisplay for DeploymentResponse {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        writeln!(w, "  Deployment:  {}", &self.id[..12])?;
        writeln!(w, "  Status:      {}", output::status_text(self.status))?;
        Ok(())
    }
}

pub async fn run(
    path: Option<String>,
    name: Option<String>,
    git_url: Option<String>,
    format: Format,
) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);
    let is_text = format == Format::Text;

    if let Some(git_url) = git_url {
        let app_name = name.context("--name is required when using --git")?;

        let sp = is_text.then(|| output::spinner("Creating app..."));
        let _ = client
            .post::<serde_json::Value>("/apps", &serde_json::json!({"name": &app_name}))
            .await;
        if let Some(s) = sp {
            s.finish_and_clear();
            output::success("App created");
        }

        let sp = is_text.then(|| output::spinner("Deploying..."));
        let response: ApiResponse<DeploymentResponse> = client
            .post(
                &format!("/apps/{app_name}/deploy"),
                &serde_json::json!({"git_url": git_url}),
            )
            .await?;
        if let Some(s) = sp {
            s.finish_and_clear();
            output::success("Deployment started");
        }

        output::print_api(&response, format);
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

    let sp = is_text.then(|| output::spinner("Creating app..."));
    let _ = client
        .post::<serde_json::Value>("/apps", &serde_json::json!({"name": &app_name}))
        .await;
    if let Some(s) = sp {
        s.finish_and_clear();
        output::success("App created");
    }

    let sp = is_text.then(|| output::spinner("Uploading..."));
    let tarball = client::create_tarball(&dir)?;
    let size = tarball.len();

    let response: ApiResponse<DeploymentResponse> =
        client.deploy_tarball(&app_name, tarball).await?;
    if let Some(s) = sp {
        s.finish_and_clear();
        let size_label = if size < 1_048_576 {
            format!("{:.1} KB", size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", size as f64 / 1_048_576.0)
        };
        output::success(&format!("Uploaded ({size_label})"));
        output::success("Deployment started");

        eprintln!();
        eprintln!("  App:  {app_name}");
        if let Some(data) = &response.data {
            eprintln!("  ID:   {}", &data.id[..12]);
        }
    }

    if !is_text {
        output::print_api(&response, format);
    }
    Ok(())
}
