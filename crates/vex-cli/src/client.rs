use anyhow::{Context, Result};
use reqwest::multipart;
use serde::de::DeserializeOwned;
use std::path::Path;

use crate::config::CliConfig;

pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl Client {
    pub fn new(config: &CliConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: config.server_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/v1{path}", self.base_url)
    }

    async fn parse_response<T: DeserializeOwned>(&self, resp: reqwest::Response) -> Result<T> {
        let body = resp.text().await?;
        serde_json::from_str(&body).context("failed to parse response")
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .get(self.url(path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to connect to server")?;
        self.parse_response(resp).await
    }

    pub async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        json: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.api_key)
            .json(json)
            .send()
            .await
            .context("failed to connect to server")?;
        self.parse_response(resp).await
    }

    pub async fn put<T: DeserializeOwned>(
        &self,
        path: &str,
        json: &serde_json::Value,
    ) -> Result<T> {
        let resp = self
            .http
            .put(self.url(path))
            .bearer_auth(&self.api_key)
            .json(json)
            .send()
            .await
            .context("failed to connect to server")?;
        self.parse_response(resp).await
    }

    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let resp = self
            .http
            .delete(self.url(path))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to connect to server")?;
        self.parse_response(resp).await
    }

    pub async fn deploy_tarball<T: DeserializeOwned>(
        &self,
        app_name: &str,
        tarball: Vec<u8>,
    ) -> Result<T> {
        let part = multipart::Part::bytes(tarball)
            .file_name("source.tar.gz")
            .mime_str("application/gzip")?;

        let form = multipart::Form::new().part("file", part);

        let resp = self
            .http
            .post(self.url(&format!("/apps/{app_name}/deploy")))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .context("failed to upload")?;
        self.parse_response(resp).await
    }

    pub async fn stream_logs(
        &self,
        app_name: &str,
        deployment_id: Option<&str>,
    ) -> Result<reqwest::Response> {
        let mut url = self.url(&format!("/apps/{app_name}/logs/stream"));
        if let Some(id) = deployment_id {
            url.push_str(&format!("?deployment_id={id}"));
        }
        self.http
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .context("failed to connect to log stream")
    }
}

pub fn create_tarball(dir: &Path) -> Result<Vec<u8>> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());

    {
        let mut ar = tar::Builder::new(&mut encoder);
        ar.append_dir_all(".", dir)
            .context("failed to create tarball")?;
        ar.finish()?;
    }

    encoder.finish().context("failed to compress tarball")
}
