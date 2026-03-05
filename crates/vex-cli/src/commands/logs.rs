use std::io::Write;

use anyhow::Result;
use console::style;
use vex_core::schema::{ApiResponse, LogEntry};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format, TextDisplay};

impl TextDisplay for LogEntry {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        let ts = style(&self.timestamp).dim();
        writeln!(w, "{ts}  {}", self.message)?;
        Ok(())
    }
}

impl TextDisplay for Vec<LogEntry> {
    fn fmt_text(&self, w: &mut dyn Write) -> std::io::Result<()> {
        for entry in self {
            entry.fmt_text(w)?;
        }
        Ok(())
    }
}

pub async fn run(
    app: &str,
    follow: bool,
    n: u64,
    deployment_id: Option<&str>,
    format: Format,
) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    if follow {
        let response = client.stream_logs(app, deployment_id).await?;
        let mut stream = response.bytes_stream();
        use tokio_stream::StreamExt;

        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find("\n\n") {
                let event = &buffer[..pos];
                if let Some(data) = event
                    .strip_prefix("data: ")
                    .or_else(|| event.lines().find_map(|l| l.strip_prefix("data: ")))
                    && let Ok(entry) = serde_json::from_str::<LogEntry>(data)
                {
                    output::print(&entry, format);
                }
                buffer = buffer[pos + 2..].to_string();
            }
        }
    } else {
        let mut path = format!("/apps/{app}/logs?n={n}");
        if let Some(id) = deployment_id {
            path.push_str(&format!("&deployment_id={id}"));
        }
        let response: ApiResponse<Vec<LogEntry>> = client.get(&path).await?;
        output::print_api(&response, format);
    }

    Ok(())
}
