use axum::extract::State;
use axum::{Json, Router, routing::post};
use rand::Rng;
use serde::Deserialize;
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::schema::{ApiResponse, AuthResponse, DeviceCodeResponse, DeviceTokenRequest};

use super::{AppError, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/device-code", post(device_code))
        .route("/auth/device-token", post(device_token))
}

#[derive(Deserialize)]
struct GitHubDeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: u64,
}

#[derive(Deserialize)]
struct GitHubAccessTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct GitHubUser {
    id: i64,
    login: String,
}

async fn device_code(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<DeviceCodeResponse>>, AppError> {
    let http = reqwest::Client::new();
    let resp = http
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", state.config.github_client_id.as_str()),
            ("scope", "read:user"),
        ])
        .send()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?;

    let gh: GitHubDeviceCodeResponse = resp
        .json()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?;

    let data = DeviceCodeResponse {
        device_code: gh.device_code,
        user_code: gh.user_code,
        verification_uri: gh.verification_uri,
        interval: gh.interval,
    };

    Ok(Json(ApiResponse::success(data)))
}

async fn device_token(
    State(state): State<AppState>,
    Json(body): Json<DeviceTokenRequest>,
) -> Result<Json<ApiResponse<AuthResponse>>, AppError> {
    let http = reqwest::Client::new();

    let resp = http
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", state.config.github_client_id.as_str()),
            ("client_secret", state.config.github_client_secret.as_str()),
            ("device_code", body.device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?;

    let token_resp: GitHubAccessTokenResponse = resp
        .json()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?;

    if let Some(err) = &token_resp.error {
        if err == "authorization_pending" || err == "slow_down" {
            return Err(Error::AuthPending.into());
        }
        return Err(Error::OAuthError(err.clone()).into());
    }

    let access_token = token_resp
        .access_token
        .ok_or_else(|| Error::OAuthError("missing access_token".to_string()))?;

    let gh_user: GitHubUser = http
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "vex-server")
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| Error::OAuthError(e.to_string()))?;

    let existing =
        sqlx::query_scalar::<_, String>("SELECT api_key FROM users WHERE github_id = $1")
            .bind(gh_user.id)
            .fetch_optional(&state.pool)
            .await?;

    let api_key = if let Some(key) = existing {
        key
    } else {
        let key = generate_api_key();
        sqlx::query(
            "INSERT INTO users (id, api_key, github_id, github_username) VALUES ($1, $2, $3, $4)",
        )
        .bind(Uuid::now_v7())
        .bind(&key)
        .bind(gh_user.id)
        .bind(&gh_user.login)
        .execute(&state.pool)
        .await?;
        key
    };

    Ok(Json(ApiResponse::success(AuthResponse { api_key })))
}

fn generate_api_key() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("vex_{hex}")
}
