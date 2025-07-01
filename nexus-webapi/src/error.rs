use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;
use tracing::error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },
    #[error("Stream is empty: {message}")]
    EmptyStream { message: String },
    #[error("Post not found: {author_id} {post_id}")]
    PostNotFound { author_id: String, post_id: String },
    #[error("Internal server error: {source}")]
    InternalServerError { source: Box<dyn std::error::Error> },
    #[error("Bookmarks not found: {user_id}")]
    BookmarksNotFound { user_id: String },
    #[error("Tags not found")]
    TagsNotFound { reach: String },
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },
    #[error("File not found.")]
    FileNotFound {},
    // Add other custom errors here
}

impl Error {
    pub fn invalid_input(message: &str) -> Self {
        Error::InvalidInput {
            message: message.to_string(),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // HTTP Status codes
        let status_code = match self {
            Error::UserNotFound { .. } => StatusCode::NOT_FOUND,
            Error::PostNotFound { .. } => StatusCode::NOT_FOUND,
            Error::EmptyStream { .. } => StatusCode::NO_CONTENT,
            Error::FileNotFound { .. } => StatusCode::NOT_FOUND,
            Error::BookmarksNotFound { .. } => StatusCode::NOT_FOUND,
            Error::TagsNotFound { .. } => StatusCode::NOT_FOUND,
            Error::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            Error::InternalServerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            // Map other errors to appropriate status codes
        };

        // Logging
        match &self {
            Error::UserNotFound { user_id } => error!("User not found: {}", user_id),
            Error::PostNotFound { author_id, post_id } => {
                error!("Post not found: {} {}", author_id, post_id)
            }
            Error::EmptyStream { message } => error!("Empty stream: {}", message),
            Error::FileNotFound {} => {
                error!("File not found.")
            }
            Error::BookmarksNotFound { user_id } => {
                error!("Bookmarks not found: {}", user_id)
            }
            Error::TagsNotFound { reach } => {
                error!("Tags not found: {}", reach)
            }
            Error::InvalidInput { message } => {
                error!("Invalid input: {}", message)
            }
            Error::InternalServerError { source } => error!("Internal server error: {:?}", source),
        };

        // Handle NO_CONTENT status code with an empty body
        if status_code == StatusCode::NO_CONTENT {
            return (status_code, ()).into_response();
        }

        let body = serde_json::json!({
            "error": self.to_string()
        });

        (status_code, axum::Json(body)).into_response()
    }
}
