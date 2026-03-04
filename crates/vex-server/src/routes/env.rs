use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Extension, Json, Router};
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::EnvVar;
use vex_core::schema::{ApiResponse, EnvVarResponse, SetEnvRequest};

use super::{AppError, AppState, fetch_app};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apps/{name}/env", get(list_env).put(set_env))
        .route("/apps/{name}/env/{key}", delete(unset_env))
}

async fn list_env(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Vec<EnvVarResponse>>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let vars = sqlx::query_as::<_, EnvVar>("SELECT * FROM env_vars WHERE app_id = $1 ORDER BY key")
        .bind(app.id)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(ApiResponse::success(
        vars.into_iter().map(EnvVarResponse::from).collect(),
    )))
}

async fn set_env(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
    Json(body): Json<SetEnvRequest>,
) -> Result<Json<ApiResponse<Vec<EnvVarResponse>>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let mut results = Vec::new();
    for (key, value) in body.vars {
        let id = Uuid::now_v7();
        let var = sqlx::query_as::<_, EnvVar>(
            "INSERT INTO env_vars (id, app_id, key, value) VALUES ($1, $2, $3, $4)
             ON CONFLICT (app_id, key) DO UPDATE SET value = $4, updated_at = NOW()
             RETURNING *",
        )
        .bind(id)
        .bind(app.id)
        .bind(&key)
        .bind(&value)
        .fetch_one(&state.pool)
        .await?;

        results.push(EnvVarResponse::from(var));
    }

    Ok(Json(ApiResponse::success(results)))
}

async fn unset_env(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path((name, key)): Path<(String, String)>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let result = sqlx::query("DELETE FROM env_vars WHERE app_id = $1 AND key = $2")
        .bind(app.id)
        .bind(&key)
        .execute(&state.pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(Error::EnvVarNotFound(key).into());
    }

    Ok(Json(ApiResponse::success(())))
}
