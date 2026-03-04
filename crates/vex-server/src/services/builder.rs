use anyhow::{Context, Result, bail};
use bollard::Docker;
use bollard::image::BuildImageOptions;
use bollard::models::BuildInfo;
use std::path::Path;
use tokio_stream::StreamExt;

pub async fn build_image(docker: &Docker, context_dir: &Path, tag: &str) -> Result<()> {
    let dockerfile_content = std::fs::read_to_string(context_dir.join("Dockerfile"))
        .context("failed to read Dockerfile")?;

    let tarball = create_build_context(context_dir)?;

    let options = BuildImageOptions {
        dockerfile: "Dockerfile".to_string(),
        t: tag.to_string(),
        rm: true,
        ..Default::default()
    };

    drop(dockerfile_content);

    let mut stream = docker.build_image(options, None, Some(tarball.into()));

    while let Some(result) = stream.next().await {
        let info: BuildInfo = result.context("docker build stream error")?;
        if let Some(error) = info.error {
            bail!("docker build failed: {error}");
        }
        if let Some(stream) = &info.stream {
            tracing::debug!("{}", stream.trim());
        }
    }

    Ok(())
}

fn create_build_context(dir: &Path) -> Result<Vec<u8>> {
    let mut ar = tar::Builder::new(Vec::new());
    ar.append_dir_all(".", dir)
        .context("failed to create build context tarball")?;
    ar.finish()?;
    Ok(ar.into_inner()?)
}
