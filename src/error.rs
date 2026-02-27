use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("upstream schema changed: {0}")]
    UpstreamSchema(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("rate limit: {0}")]
    RateLimit(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidArgument(_) => "INVALID_ARGUMENT",
            Self::Network(_) => "NETWORK",
            Self::UpstreamSchema(_) => "UPSTREAM_SCHEMA",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Unauthorized(_) => "UNAUTHORIZED",
            Self::RateLimit(_) => "RATE_LIMIT",
            Self::Database(_) => "DATABASE",
            Self::Internal(_) => "INTERNAL",
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Database(value.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        if value.status() == Some(reqwest::StatusCode::UNAUTHORIZED) {
            return Self::Unauthorized(value.to_string());
        }
        if value.status() == Some(reqwest::StatusCode::TOO_MANY_REQUESTS) {
            return Self::RateLimit(value.to_string());
        }
        Self::Network(value.to_string())
    }
}
