use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

/// Error de la API. `code` es un identificador estable que el front traduce;
/// el detalle interno solo va al log, nunca al cliente.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("password_change_required")]
    PasswordChangeRequired,
    #[error("not found")]
    NotFound,
    #[error("too many attempts")]
    TooManyAttempts,
    /// Datos inválidos; el texto es el código que ve el front (por ejemplo `invalid_email`).
    #[error("bad request: {0}")]
    BadRequest(&'static str),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => Self::NotFound,
            other => Self::Internal(other.into()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code) = match &self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            Self::PasswordChangeRequired => (StatusCode::FORBIDDEN, "password_change_required"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::TooManyAttempts => (StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"),
            Self::BadRequest(code) => (StatusCode::BAD_REQUEST, *code),
            Self::Conflict(code) => (StatusCode::CONFLICT, *code),
            Self::Internal(e) => {
                tracing::error!(error = ?e, "error interno");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal")
            }
        };
        (status, Json(json!({ "error": code }))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
