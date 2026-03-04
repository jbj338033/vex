use anyhow::{Context, Result};
use bollard::Docker;
use bollard::container::{LogOutput, LogsOptions};
use tokio_stream::StreamExt;
use vex_core::schema::LogEntry;

fn parse_log_output(output: LogOutput) -> LogEntry {
    let line = output.to_string();
    let (timestamp, message) = line.split_once(' ').unwrap_or(("", &line));
    LogEntry {
        timestamp: timestamp.to_string(),
        message: message.to_string(),
    }
}

pub async fn fetch_logs(docker: &Docker, container_id: &str, tail: u64) -> Result<Vec<LogEntry>> {
    let options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: tail.to_string(),
        timestamps: true,
        ..Default::default()
    };

    let mut stream = docker.logs(container_id, Some(options));
    let mut entries = Vec::new();

    while let Some(result) = stream.next().await {
        let output = result.context("failed to read log")?;
        entries.push(parse_log_output(output));
    }

    Ok(entries)
}

pub fn stream_logs(
    docker: Docker,
    container_id: String,
) -> impl tokio_stream::Stream<Item = Result<LogEntry>> {
    let options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        follow: true,
        tail: "0".to_string(),
        timestamps: true,
        ..Default::default()
    };

    docker.logs(&container_id, Some(options)).map(|result| {
        let output = result.context("failed to read log")?;
        Ok(parse_log_output(output))
    })
}
