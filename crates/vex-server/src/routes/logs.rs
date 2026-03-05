use std::convert::Infallible;
use std::pin::Pin;

use axum::extract::{Path, Query, State};
use axum::response::sse::{Event, Sse};
use axum::routing::get;
use axum::{Extension, Json, Router};
use serde::Deserialize;
use tokio_stream::StreamExt;
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::{BuildLog, Deployment, DeploymentStatus};
use vex_core::schema::{ApiResponse, LogEntry};

use super::{AppError, AppState, fetch_app};
use crate::services::{build_logger, logger};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apps/{name}/logs", get(get_logs))
        .route("/apps/{name}/logs/stream", get(stream_logs))
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default = "default_tail")]
    n: u64,
    deployment_id: Option<Uuid>,
}

fn default_tail() -> u64 {
    100
}

async fn find_deployment(
    state: &AppState,
    user_id: Uuid,
    name: &str,
    deployment_id: Option<Uuid>,
) -> Result<Deployment, Error> {
    let app = fetch_app(&state.pool, user_id, name).await?;

    let deployment = if let Some(id) = deployment_id {
        sqlx::query_as::<_, Deployment>("SELECT * FROM deployments WHERE id = $1 AND app_id = $2")
            .bind(id)
            .bind(app.id)
            .fetch_optional(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Deployment>(
            "SELECT * FROM deployments WHERE app_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(app.id)
        .fetch_optional(&state.pool)
        .await?
    };

    deployment.ok_or_else(|| Error::DeploymentNotFound(name.to_string()))
}

async fn fetch_build_logs(state: &AppState, deployment_id: Uuid) -> Result<Vec<LogEntry>, Error> {
    let logs = sqlx::query_as::<_, BuildLog>(
        "SELECT * FROM build_logs WHERE deployment_id = $1 ORDER BY created_at ASC",
    )
    .bind(deployment_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(logs
        .into_iter()
        .map(|l| LogEntry {
            timestamp: l.created_at.to_rfc3339(),
            message: l.message,
        })
        .collect())
}

async fn get_logs(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<ApiResponse<Vec<LogEntry>>>, AppError> {
    let deployment = find_deployment(&state, user_id, &name, query.deployment_id).await?;

    let logs = if deployment.status == DeploymentStatus::Running {
        if let Some(container_id) = &deployment.container_id {
            logger::fetch_logs(&state.docker, container_id, query.n)
                .await
                .map_err(|e| Error::Internal(e.to_string()))?
        } else {
            fetch_build_logs(&state, deployment.id).await?
        }
    } else {
        fetch_build_logs(&state, deployment.id).await?
    };

    Ok(Json(ApiResponse::success(logs)))
}

type SseStream = Pin<Box<dyn tokio_stream::Stream<Item = Result<Event, Infallible>> + Send>>;

fn log_entry_to_event(entry: LogEntry) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(entry)
        .unwrap_or_else(|_| Event::default().data("error")))
}

async fn stream_logs(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
    Query(query): Query<LogsQuery>,
) -> Result<Sse<SseStream>, AppError> {
    let deployment = find_deployment(&state, user_id, &name, query.deployment_id).await?;

    let stream: SseStream = match deployment.status {
        DeploymentStatus::Running => {
            let container_id = deployment.container_id.ok_or(Error::NoContainer)?;
            let stream = logger::stream_logs(state.docker.clone(), container_id);
            Box::pin(stream.map(|result| match result {
                Ok(entry) => log_entry_to_event(entry),
                Err(_) => Ok(Event::default().data("error")),
            }))
        }
        DeploymentStatus::Building => {
            let rx = build_logger::subscribe(&state.build_log_channels, deployment.id);
            Box::pin(
                tokio_stream::wrappers::BroadcastStream::new(rx).map(|result| match result {
                    Ok(entry) => log_entry_to_event(entry),
                    Err(_) => Ok(Event::default().data("error")),
                }),
            )
        }
        _ => {
            let logs = fetch_build_logs(&state, deployment.id)
                .await
                .map_err(|e| Error::Internal(e.to_string()))?;
            Box::pin(tokio_stream::iter(logs.into_iter().map(log_entry_to_event)))
        }
    };

    Ok(Sse::new(stream))
}
