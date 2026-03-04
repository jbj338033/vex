use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use sqlx::PgPool;
use uuid::Uuid;
use vex_core::error::Error;

use crate::routes::AppError;

pub async fn auth_middleware(
    State(pool): State<PgPool>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::MissingAuth)?;

    let api_key = auth_header
        .strip_prefix("Bearer ")
        .ok_or(Error::InvalidApiKey)?;

    let user_id: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE api_key = $1")
        .bind(api_key)
        .fetch_optional(&pool)
        .await?;

    let (user_id,) = user_id.ok_or(Error::InvalidApiKey)?;

    req.extensions_mut().insert(user_id);
    Ok(next.run(req).await)
}
