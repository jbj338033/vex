use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub listen_addr: String,
    pub domain: String,
    pub proxy_listen_addr: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub tls: TlsConfig,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub enabled: bool,
    pub acme_email: String,
    pub cert_dir: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            listen_addr: std::env::var("VEX_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
            domain: std::env::var("VEX_DOMAIN").unwrap_or_else(|_| "localhost".to_string()),
            proxy_listen_addr: std::env::var("VEX_PROXY_LISTEN_ADDR")
                .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            github_client_id: std::env::var("GITHUB_CLIENT_ID")
                .context("GITHUB_CLIENT_ID must be set")?,
            github_client_secret: std::env::var("GITHUB_CLIENT_SECRET")
                .context("GITHUB_CLIENT_SECRET must be set")?,
            tls: TlsConfig::from_env(),
        })
    }
}

impl TlsConfig {
    fn from_env() -> Self {
        let enabled = std::env::var("VEX_TLS_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);

        Self {
            enabled,
            acme_email: std::env::var("VEX_ACME_EMAIL").unwrap_or_default(),
            cert_dir: std::env::var("VEX_CERT_DIR")
                .unwrap_or_else(|_| "/var/lib/vex/certs".to_string()),
        }
    }
}
