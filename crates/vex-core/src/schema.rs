use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::model::{App, Deployment, DeploymentStatus, EnvVar};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
}

#[derive(Debug, Deserialize)]
pub struct DeviceTokenRequest {
    pub device_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "T: Serialize",
    deserialize = "T: serde::de::DeserializeOwned"
))]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }
}

impl ApiResponse<()> {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ApiError {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateAppRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<App> for AppResponse {
    fn from(app: App) -> Self {
        Self {
            id: app.id.to_string(),
            name: app.name,
            created_at: app.created_at.to_rfc3339(),
            updated_at: app.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DeployRequest {
    pub git_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeploymentResponse {
    pub id: String,
    pub app_id: String,
    pub status: DeploymentStatus,
    pub container_id: Option<String>,
    pub image_tag: Option<String>,
    pub port: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Deployment> for DeploymentResponse {
    fn from(d: Deployment) -> Self {
        Self {
            id: d.id.to_string(),
            app_id: d.app_id.to_string(),
            status: d.status,
            container_id: d.container_id,
            image_tag: d.image_tag,
            port: d.port,
            error_message: d.error_message,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SetEnvRequest {
    pub vars: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EnvVarResponse {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

impl From<EnvVar> for EnvVarResponse {
    fn from(e: EnvVar) -> Self {
        Self {
            key: e.key,
            value: e.value,
            updated_at: e.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub app: AppResponse,
    pub deployment: Option<DeploymentResponse>,
    pub url: Option<String>,
}
