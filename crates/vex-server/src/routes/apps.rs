use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::App;
use vex_core::schema::{
    ApiResponse, AppResponse, CreateAppRequest, DeploymentResponse, StatusResponse,
};

use super::{AppError, AppState, fetch_app};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apps", post(create_app).get(list_apps))
        .route("/apps/{name}", get(get_app).delete(delete_app))
        .route("/apps/{name}/status", get(get_status))
}

async fn create_app(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Json(body): Json<CreateAppRequest>,
) -> Result<impl IntoResponse, AppError> {
    let id = Uuid::now_v7();

    let result = sqlx::query_as::<_, App>(
        "INSERT INTO apps (id, user_id, name) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(id)
    .bind(user_id)
    .bind(&body.name)
    .fetch_one(&state.pool)
    .await;

    match result {
        Ok(app) => Ok((
            axum::http::StatusCode::CREATED,
            Json(ApiResponse::success(AppResponse::from(app))),
        )),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(Error::AppAlreadyExists(body.name).into())
        }
        Err(e) => Err(Error::Database(e).into()),
    }
}

async fn list_apps(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
) -> Result<Json<ApiResponse<Vec<AppResponse>>>, AppError> {
    let apps =
        sqlx::query_as::<_, App>("SELECT * FROM apps WHERE user_id = $1 ORDER BY created_at DESC")
            .bind(user_id)
            .fetch_all(&state.pool)
            .await?;

    Ok(Json(ApiResponse::success(
        apps.into_iter().map(AppResponse::from).collect(),
    )))
}

async fn get_app(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<AppResponse>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;
    Ok(Json(ApiResponse::success(AppResponse::from(app))))
}

async fn delete_app(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;
    let _ = crate::services::deployer::stop_existing(&state.docker, &app.name).await;
    state.route_table.remove(&app.name);
    sqlx::query("DELETE FROM apps WHERE id = $1")
        .bind(app.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(ApiResponse::success(())))
}

async fn get_status(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<StatusResponse>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let deployment = sqlx::query_as::<_, vex_core::model::Deployment>(
        "SELECT * FROM deployments WHERE app_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(app.id)
    .fetch_optional(&state.pool)
    .await?;

    let url = state
        .route_table
        .get(&app.name)
        .map(|_| format!("http://{}.{}", app.name, state.config.domain));

    Ok(Json(ApiResponse::success(StatusResponse {
        app: AppResponse::from(app),
        deployment: deployment.map(DeploymentResponse::from),
        url,
    })))
}
