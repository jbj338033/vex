use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use std::convert::Infallible;
use tokio_stream::StreamExt;
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::{Deployment, DeploymentStatus};
use vex_core::schema::{ApiResponse, LogEntry};

use super::{AppError, AppState, fetch_app};
use crate::services::logger;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apps/{name}/logs", get(get_logs))
        .route("/apps/{name}/logs/stream", get(stream_logs))
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default = "default_tail")]
    n: u64,
}

fn default_tail() -> u64 {
    100
}

async fn get_logs(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<ApiResponse<Vec<LogEntry>>>, AppError> {
    let container_id = find_running_container(&state, user_id, &name).await?;
    let logs = logger::fetch_logs(&state.docker, &container_id, query.n)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    Ok(Json(ApiResponse::success(logs)))
}

async fn stream_logs(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, AppError> {
    let container_id = find_running_container(&state, user_id, &name).await?;

    let stream = logger::stream_logs(state.docker.clone(), container_id);
    let sse_stream = stream.map(|result| {
        Ok::<_, Infallible>(match result {
            Ok(entry) => Event::default()
                .json_data(entry)
                .unwrap_or_else(|_| Event::default().data("error")),
            Err(_) => Event::default().data("error"),
        })
    });

    Ok(Sse::new(sse_stream))
}

async fn find_running_container(
    state: &AppState,
    user_id: Uuid,
    name: &str,
) -> Result<String, Error> {
    let app = fetch_app(&state.pool, user_id, name).await?;

    let deployment = sqlx::query_as::<_, Deployment>(
        "SELECT * FROM deployments WHERE app_id = $1 AND status = $2 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(app.id)
    .bind(DeploymentStatus::Running)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| Error::DeploymentNotFound(name.to_string()))?;

    deployment.container_id.ok_or(Error::NoContainer)
}
