use anyhow::{Context, Result, bail};
use vex_core::schema::{ApiResponse, AuthResponse, DeviceCodeResponse};

use crate::config::{self, CliConfig};

pub async fn run(url: &str) -> Result<()> {
    let base_url = url.trim_end_matches('/');
    let http = reqwest::Client::new();

    let resp: ApiResponse<DeviceCodeResponse> = http
        .post(format!("{base_url}/v1/auth/device-code"))
        .send()
        .await
        .context("failed to connect to server")?
        .json()
        .await
        .context("failed to parse device code response")?;

    let data = resp.data.context("failed to get device code")?;

    eprintln!("Open this URL in your browser: {}", data.verification_uri);
    eprintln!("Enter code: {}", data.user_code);
    eprintln!();

    let _ = open::that(&data.verification_uri);

    let interval = std::time::Duration::from_secs(data.interval.max(5));

    loop {
        tokio::time::sleep(interval).await;

        let resp = http
            .post(format!("{base_url}/v1/auth/device-token"))
            .json(&serde_json::json!({ "device_code": data.device_code }))
            .send()
            .await
            .context("failed to connect to server")?;

        let status = resp.status().as_u16();

        if status == 428 {
            continue;
        }

        let body: ApiResponse<AuthResponse> = resp
            .json()
            .await
            .context("failed to parse token response")?;

        if !body.ok {
            let err = body
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| "unknown error".to_string());
            bail!("authentication failed: {err}");
        }

        let auth = body.data.context("missing auth data")?;

        config::save(&CliConfig {
            api_key: auth.api_key,
            server_url: base_url.to_string(),
        })?;

        println!("{{\"ok\": true, \"message\": \"authenticated successfully\"}}");
        return Ok(());
    }
}
