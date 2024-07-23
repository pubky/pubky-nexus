use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use log::{debug, error};
use thiserror::Error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },
    #[error("Post not found: {author_id} {post_id}")]
    PostNotFound { author_id: String, post_id: String },
    #[error("Internal server error: {source}")]
    InternalServerError { source: Box<dyn std::error::Error> },
    // Add other custom errors here
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // HTTP Status codes
        let status_code = match self {
            Error::UserNotFound { .. } => StatusCode::NOT_FOUND,
            Error::PostNotFound { .. } => StatusCode::NOT_FOUND,
            Error::InternalServerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            // Map other errors to appropriate status codes
        };

        // Logging
        match &self {
            Error::UserNotFound { user_id } => debug!("User not found: {}", user_id),
            Error::PostNotFound { author_id, post_id } => {
                debug!("Post not found: {} {}", author_id, post_id)
            }
            Error::InternalServerError { source } => error!("Internal server error: {:?}", source),
        };

        let body = serde_json::json!({
            "error": self.to_string()
        });

        (status_code, axum::Json(body)).into_response()
    }
}
