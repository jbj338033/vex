#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("app already exists: {0}")]
    AppAlreadyExists(String),

    #[error("deployment not found: {0}")]
    DeploymentNotFound(String),

    #[error("env var not found: {0}")]
    EnvVarNotFound(String),

    #[error("no running container")]
    NoContainer,

    #[error("invalid api key")]
    InvalidApiKey,

    #[error("missing authorization header")]
    MissingAuth,

    #[error("build failed: {0}")]
    BuildFailed(String),

    #[error("unsupported project type")]
    UnsupportedProject,

    #[error("authorization pending")]
    AuthPending,

    #[error("oauth error: {0}")]
    OAuthError(String),

    #[error("{0}")]
    Internal(String),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::AppNotFound(_) => "app_not_found",
            Self::AppAlreadyExists(_) => "app_already_exists",
            Self::DeploymentNotFound(_) => "deployment_not_found",
            Self::EnvVarNotFound(_) => "env_var_not_found",
            Self::NoContainer => "no_container",
            Self::InvalidApiKey => "invalid_api_key",
            Self::MissingAuth => "missing_auth",
            Self::BuildFailed(_) => "build_failed",
            Self::UnsupportedProject => "unsupported_project",
            Self::AuthPending => "authorization_pending",
            Self::OAuthError(_) => "oauth_error",
            Self::Internal(_) => "internal_error",
            Self::Database(_) => "database_error",
        }
    }

    pub fn status_code(&self) -> u16 {
        match self {
            Self::AppNotFound(_)
            | Self::DeploymentNotFound(_)
            | Self::EnvVarNotFound(_)
            | Self::NoContainer => 404,
            Self::AppAlreadyExists(_) => 409,
            Self::InvalidApiKey | Self::MissingAuth => 401,
            Self::BuildFailed(_) | Self::UnsupportedProject => 422,
            Self::AuthPending => 428,
            Self::Internal(_) | Self::Database(_) => 500,
            Self::OAuthError(_) => 502,
        }
    }
}
