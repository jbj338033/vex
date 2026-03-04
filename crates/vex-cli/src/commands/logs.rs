use anyhow::Result;
use vex_core::schema::{ApiResponse, LogEntry};

use crate::client::Client;
use crate::config;
use crate::output::{self, Format};

pub async fn run(app: &str, follow: bool, n: u64, format: Format) -> Result<()> {
    let cfg = config::load()?;
    let client = Client::new(&cfg);

    if follow {
        let response = client.stream_logs(app).await?;
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
        let response: ApiResponse<Vec<LogEntry>> =
            client.get(&format!("/apps/{app}/logs?n={n}")).await?;
        output::print(&response, format);
    }

    Ok(())
}
