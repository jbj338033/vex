use axum::extract::{Multipart, Path, State};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use uuid::Uuid;
use vex_core::error::Error;
use vex_core::model::{Deployment, DeploymentStatus};
use vex_core::schema::{ApiResponse, DeploymentResponse};

use super::{AppError, AppState, fetch_app};
use crate::services::proxy::RouteTarget;
use crate::services::{builder, deployer, tls};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/apps/{name}/deploy", post(deploy))
        .route("/apps/{name}/deployments", get(list_deployments))
}

async fn deploy(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<DeploymentResponse>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let temp_dir = tempfile::TempDir::new().map_err(|e| Error::Internal(e.to_string()))?;

    while let Ok(Some(field)) = multipart.next_field().await {
        let is_file = field.name() == Some("file");
        if is_file {
            let data: bytes::Bytes = field
                .bytes()
                .await
                .map_err(|e| Error::Internal(e.to_string()))?;

            let decoder = flate2::read::GzDecoder::new(&data[..]);
            let mut archive = tar::Archive::new(decoder);
            archive
                .unpack(temp_dir.path())
                .map_err(|e| Error::Internal(e.to_string()))?;
        }
    }

    let deployment_id = Uuid::now_v7();
    let deployment = sqlx::query_as::<_, Deployment>(
        "INSERT INTO deployments (id, app_id, status) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(deployment_id)
    .bind(app.id)
    .bind(DeploymentStatus::Pending)
    .fetch_one(&state.pool)
    .await?;

    let app_name = app.name.clone();
    let build_dir = temp_dir.keep();

    tokio::spawn(async move {
        run_deploy_pipeline(state, deployment_id, app.id, &app_name, &build_dir).await;
        let _ = std::fs::remove_dir_all(&build_dir);
    });

    Ok(Json(ApiResponse::success(DeploymentResponse::from(
        deployment,
    ))))
}

async fn run_deploy_pipeline(
    state: AppState,
    deployment_id: Uuid,
    app_id: Uuid,
    app_name: &str,
    build_dir: &std::path::Path,
) {
    let image_tag = format!("vex-{app_name}:{deployment_id}");

    set_status(&state.pool, deployment_id, DeploymentStatus::Building, None).await;

    let project_type = vex_builder::detect(build_dir);
    if let Some(ref pt) = project_type {
        if let Some(dockerfile) = vex_builder::generate(pt)
            && let Err(e) = std::fs::write(build_dir.join("Dockerfile"), dockerfile)
        {
            set_status(
                &state.pool,
                deployment_id,
                DeploymentStatus::Failed,
                Some(e.to_string()),
            )
            .await;
            return;
        }
    } else if !build_dir.join("Dockerfile").exists() {
        set_status(
            &state.pool,
            deployment_id,
            DeploymentStatus::Failed,
            Some("unsupported project type".into()),
        )
        .await;
        return;
    }

    if let Err(e) = builder::build_image(&state.docker, build_dir, &image_tag).await {
        set_status(
            &state.pool,
            deployment_id,
            DeploymentStatus::Failed,
            Some(e.to_string()),
        )
        .await;
        return;
    }

    set_status(
        &state.pool,
        deployment_id,
        DeploymentStatus::Deploying,
        None,
    )
    .await;

    let env_vars =
        sqlx::query_as::<_, vex_core::model::EnvVar>("SELECT * FROM env_vars WHERE app_id = $1")
            .bind(app_id)
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|e| format!("{}={}", e.key, e.value))
            .collect();

    let info = match deployer::create_and_start(&state.docker, &image_tag, app_name, env_vars).await
    {
        Ok(info) => info,
        Err(e) => {
            set_status(
                &state.pool,
                deployment_id,
                DeploymentStatus::Failed,
                Some(e.to_string()),
            )
            .await;
            return;
        }
    };

    if let Err(e) = deployer::wait_for_ready(info.host_port).await {
        set_status(
            &state.pool,
            deployment_id,
            DeploymentStatus::Failed,
            Some(e.to_string()),
        )
        .await;
        let _ = deployer::stop_existing(&state.docker, app_name).await;
        return;
    }

    let _ = sqlx::query(
        "UPDATE deployments SET status = $1, container_id = $2, image_tag = $3, port = $4, updated_at = NOW() WHERE id = $5",
    )
    .bind(DeploymentStatus::Running)
    .bind(&info.container_id)
    .bind(&image_tag)
    .bind(info.host_port as i32)
    .bind(deployment_id)
    .execute(&state.pool)
    .await;

    state.route_table.insert(
        app_name.to_string(),
        RouteTarget {
            host_port: info.host_port,
            container_id: info.container_id,
        },
    );

    if state.config.tls.enabled
        && let (Some(challenges), Some(certs)) = (&state.challenge_store, &state.cert_store)
    {
        let fqdn = format!("{app_name}.{}", state.config.domain);
        if let Err(e) = tls::provision_app(&fqdn, &state.config.tls, challenges, certs).await {
            tracing::error!("cert provisioning failed for {fqdn}: {e}");
        }
    }

    tracing::info!("deployed {app_name} on port {}", info.host_port);
}

async fn set_status(
    pool: &sqlx::PgPool,
    id: Uuid,
    status: DeploymentStatus,
    error: Option<String>,
) {
    let _ = sqlx::query(
        "UPDATE deployments SET status = $1, error_message = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(status)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await;
}

async fn list_deployments(
    State(state): State<AppState>,
    Extension(user_id): Extension<Uuid>,
    Path(name): Path<String>,
) -> Result<Json<ApiResponse<Vec<DeploymentResponse>>>, AppError> {
    let app = fetch_app(&state.pool, user_id, &name).await?;

    let deployments = sqlx::query_as::<_, Deployment>(
        "SELECT * FROM deployments WHERE app_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(app.id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(ApiResponse::success(
        deployments
            .into_iter()
            .map(DeploymentResponse::from)
            .collect(),
    )))
}
