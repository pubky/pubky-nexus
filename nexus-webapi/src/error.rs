use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nexus_common::types::DynError;
use thiserror::Error;
use tracing::error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },
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
    #[error("Tag {tag_id} of {tagger_id} not found")]
    TagNotFound { tag_id: String, tagger_id: String },
    // Add other custom errors here
}

impl Error {
    pub fn invalid_input(message: &str) -> Self {
        Error::InvalidInput {
            message: message.to_string(),
        }
    }
}

impl From<DynError> for Error {
    fn from(source: DynError) -> Self {
        Error::InternalServerError { source }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // HTTP Status codes
        let status_code = match self {
            Error::UserNotFound { .. } => StatusCode::NOT_FOUND,
            Error::PostNotFound { .. } => StatusCode::NOT_FOUND,
            Error::FileNotFound { .. } => StatusCode::NOT_FOUND,
            Error::BookmarksNotFound { .. } => StatusCode::NOT_FOUND,
            Error::TagsNotFound { .. } => StatusCode::NOT_FOUND,
            Error::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            Error::InternalServerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::TagNotFound { .. } => StatusCode::NOT_FOUND,
            // Map other errors to appropriate status codes
        };

        // Logging
        match &self {
            Error::UserNotFound { user_id } => error!("User not found: {}", user_id),
            Error::PostNotFound { author_id, post_id } => {
                error!("Post not found: {} {}", author_id, post_id)
            }
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
            Error::TagNotFound { tag_id, tagger_id } => {
                error!("Tag not found: {} of {}", tag_id, tagger_id)
            }
            Error::InternalServerError { source } => error!("Internal server error: {:?}", source),
        };

        let body = serde_json::json!({
            "error": self.to_string()
        });

        (status_code, axum::Json(body)).into_response()
    }
}
