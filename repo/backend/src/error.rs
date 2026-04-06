use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use tracing::error;

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl AppError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    pub fn locked(message: impl Into<String>) -> Self {
        Self::new(StatusCode::LOCKED, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(ErrorBody { error: self.message })).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        error!(error = %err, "database operation failed");
        Self::internal("internal server error")
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        error!(error = %err, "io operation failed");
        Self::internal("internal server error")
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(err: argon2::password_hash::Error) -> Self {
        error!(error = %err, "password operation failed");
        Self::internal("internal server error")
    }
}

impl From<aes_gcm::Error> for AppError {
    fn from(_: aes_gcm::Error) -> Self {
        error!("encryption operation failed");
        Self::internal("internal server error")
    }
}

impl From<sqlx::migrate::MigrateError> for AppError {
    fn from(err: sqlx::migrate::MigrateError) -> Self {
        error!(error = %err, "migration operation failed");
        Self::internal("internal server error")
    }
}

impl From<http::Error> for AppError {
    fn from(err: http::Error) -> Self {
        error!(error = %err, "http operation failed");
        Self::internal("internal server error")
    }
}
