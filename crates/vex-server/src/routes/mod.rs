mod apps;
mod auth;
mod deploy;
mod env;
mod logs;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, Router, middleware as axum_middleware};
use bollard::Docker;
use sqlx::PgPool;
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::App;
use vex_core::schema::ApiResponse;

use crate::config::Config;
use crate::middleware::auth_middleware;
use crate::services::proxy::RouteTable;
use crate::services::tls::{CertStore, ChallengeStore};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
    pub docker: Docker,
    pub route_table: RouteTable,
    pub challenge_store: Option<ChallengeStore>,
    pub cert_store: Option<CertStore>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        config: Config,
        challenge_store: Option<ChallengeStore>,
        cert_store: Option<CertStore>,
    ) -> Self {
        let docker = Docker::connect_with_local_defaults().expect("failed to connect to docker");
        let route_table = RouteTable::new();
        Self {
            pool,
            config,
            docker,
            route_table,
            challenge_store,
            cert_store,
        }
    }
}

pub struct AppError(Error);

impl<E: Into<Error>> From<E> for AppError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = Json(ApiResponse::<()>::error(self.0.code(), self.0.to_string()));
        (status, body).into_response()
    }
}

async fn fetch_app(pool: &PgPool, user_id: Uuid, name: &str) -> Result<App, Error> {
    sqlx::query_as::<_, App>("SELECT * FROM apps WHERE user_id = $1 AND name = $2")
        .bind(user_id)
        .bind(name)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| Error::AppNotFound(name.to_string()))
}

pub fn router(state: AppState) -> Router {
    let authenticated = Router::new()
        .merge(apps::router())
        .merge(deploy::router())
        .merge(env::router())
        .merge(logs::router())
        .route_layer(axum_middleware::from_fn_with_state(
            state.pool.clone(),
            auth_middleware,
        ));

    let public = Router::new().merge(auth::router());

    Router::new()
        .nest("/v1", authenticated)
        .nest("/v1", public)
        .with_state(state)
}
