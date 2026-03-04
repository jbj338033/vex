use anyhow::{Context, Result};
use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, ListContainersOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions,
};
use bollard::models::HostConfig;
use std::collections::HashMap;

pub struct ContainerInfo {
    pub container_id: String,
    pub host_port: u16,
}

pub async fn create_and_start(
    docker: &Docker,
    image: &str,
    app_name: &str,
    env_vars: Vec<String>,
) -> Result<ContainerInfo> {
    stop_existing(docker, app_name).await?;

    let labels: HashMap<&str, &str> = HashMap::from([("vex.app", app_name)]);

    let host_config = HostConfig {
        publish_all_ports: Some(true),
        ..Default::default()
    };

    let config = Config {
        image: Some(image.to_string()),
        env: Some(env_vars),
        labels: Some(
            labels
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ),
        host_config: Some(host_config),
        ..Default::default()
    };

    let options = CreateContainerOptions {
        name: format!("vex-{app_name}"),
        ..Default::default()
    };

    let container = docker
        .create_container(Some(options), config)
        .await
        .context("failed to create container")?;

    docker
        .start_container(&container.id, None::<StartContainerOptions<String>>)
        .await
        .context("failed to start container")?;

    let inspect = docker
        .inspect_container(&container.id, None)
        .await
        .context("failed to inspect container")?;

    let host_port = extract_host_port(&inspect)?;

    Ok(ContainerInfo {
        container_id: container.id,
        host_port,
    })
}

pub async fn stop_existing(docker: &Docker, app_name: &str) -> Result<()> {
    let label_filter = format!("vex.app={app_name}");
    let filters: HashMap<&str, Vec<&str>> = HashMap::from([("label", vec![label_filter.as_str()])]);

    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        }))
        .await
        .context("failed to list containers")?;

    for container in containers {
        if let Some(id) = &container.id {
            let _ = docker
                .stop_container(id, Some(StopContainerOptions { t: 5 }))
                .await;
            docker
                .remove_container(
                    id,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .context("failed to remove old container")?;
        }
    }

    Ok(())
}

pub async fn wait_for_ready(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let mut delay = std::time::Duration::from_millis(250);

    for _ in 0..20 {
        if tokio::net::TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(std::time::Duration::from_secs(2));
    }

    anyhow::bail!("container not ready on port {port} after healthcheck timeout")
}

fn extract_host_port(inspect: &bollard::models::ContainerInspectResponse) -> Result<u16> {
    let ports = inspect
        .network_settings
        .as_ref()
        .and_then(|ns| ns.ports.as_ref())
        .context("no port mappings found")?;

    for bindings in ports.values().flatten() {
        for binding in bindings {
            if let Some(port_str) = &binding.host_port {
                return port_str.parse::<u16>().context("failed to parse host port");
            }
        }
    }

    anyhow::bail!("no host port binding found")
}
