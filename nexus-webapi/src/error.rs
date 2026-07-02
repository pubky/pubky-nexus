use crate::models::{ErrorResponsePayload, PostId, PubkyId};
use axum::http::header::InvalidHeaderValue;
use axum::http::uri::InvalidUri;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use nexus_common::db::kv::RedisError;
use nexus_common::models::error::ModelError;
use nexus_common::types::DynError;
use std::io;
use thiserror::Error;
use tracing::{debug, error, warn};

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("User not found: {user_id}")]
    UserNotFound { user_id: String },
    #[error("Post not found: {author_id} {post_id}")]
    PostNotFound { author_id: String, post_id: String },
    #[error("Internal server error: {source}")]
    InternalServerError { source: DynError },
    #[error("Tags not found")]
    TagsNotFound { reach: String },
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },
    #[error("File not found.")]
    FileNotFound {},
    #[error("Tag {tag_id} of {tagger_id} not found")]
    TagNotFound { tag_id: String, tagger_id: String },
    #[error("Resource not found: {resource_id}")]
    ResourceNotFound { resource_id: String },
    #[error("Forbidden: {message}")]
    Forbidden { message: String },
    // Add other custom errors here
}

impl Error {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Error::InvalidInput {
            message: message.into(),
        }
    }

    pub fn resource_not_found(resource_id: impl Into<String>) -> Self {
        Error::ResourceNotFound {
            resource_id: resource_id.into(),
        }
    }

    pub fn user_not_found(user_id: PubkyId) -> Self {
        Error::UserNotFound {
            user_id: user_id.to_string(),
        }
    }

    pub fn post_not_found(author_id: PubkyId, post_id: PostId) -> Self {
        Error::PostNotFound {
            author_id: author_id.to_string(),
            post_id: post_id.to_string(),
        }
    }

    pub fn tag_not_found(tag_id: String, tagger_id: PubkyId) -> Self {
        Error::TagNotFound {
            tag_id,
            tagger_id: tagger_id.to_string(),
        }
    }
}

impl From<ModelError> for Error {
    fn from(source: ModelError) -> Self {
        match source {
            ModelError::HsBlacklisted { hs_id } => Error::Forbidden {
                message: format!("Homeserver is blacklisted: {hs_id}"),
            },
            other => Error::InternalServerError {
                source: other.into(),
            },
        }
    }
}

impl From<RedisError> for Error {
    fn from(source: RedisError) -> Self {
        Error::InternalServerError {
            source: source.into(),
        }
    }
}

impl From<DynError> for Error {
    fn from(source: DynError) -> Self {
        Error::InternalServerError { source }
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(source: InvalidHeaderValue) -> Self {
        Error::InternalServerError {
            source: Box::new(source),
        }
    }
}

impl From<InvalidUri> for Error {
    fn from(source: InvalidUri) -> Self {
        Error::InternalServerError {
            source: Box::new(source),
        }
    }
}

impl From<io::Error> for Error {
    fn from(source: io::Error) -> Self {
        Error::InternalServerError {
            source: Box::new(source),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        // HTTP Status codes
        let status_code = match self {
            Error::UserNotFound { .. } => StatusCode::NOT_FOUND,
            Error::PostNotFound { .. } => StatusCode::NOT_FOUND,
            Error::FileNotFound { .. } => StatusCode::NOT_FOUND,
            Error::TagsNotFound { .. } => StatusCode::NOT_FOUND,
            Error::InvalidInput { .. } => StatusCode::BAD_REQUEST,
            Error::InternalServerError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Error::TagNotFound { .. } => StatusCode::NOT_FOUND,
            Error::ResourceNotFound { .. } => StatusCode::NOT_FOUND,
            Error::Forbidden { .. } => StatusCode::FORBIDDEN,
            // Map other errors to appropriate status codes
        };

        // Logging. Client errors (4xx) are ordinary and must not be logged at
        // ERROR on a public, unauthenticated API, else a scanner crawling random
        // ids floods the error stream and buries genuine 5xx. Only 5xx is ERROR.
        match &self {
            Error::UserNotFound { user_id } => debug!("User not found: {}", user_id),
            Error::PostNotFound { author_id, post_id } => {
                debug!("Post not found: {} {}", author_id, post_id)
            }
            Error::FileNotFound {} => {
                debug!("File not found.")
            }
            Error::TagsNotFound { reach } => {
                debug!("Tags not found: {}", reach)
            }
            Error::InvalidInput { message } => {
                debug!("Invalid input: {}", message)
            }
            Error::TagNotFound { tag_id, tagger_id } => {
                debug!("Tag not found: {} of {}", tag_id, tagger_id)
            }
            Error::ResourceNotFound { resource_id } => {
                debug!("Resource not found: {}", resource_id)
            }
            Error::Forbidden { message } => {
                warn!("Forbidden: {}", message)
            }
            Error::InternalServerError { source } => error!("Internal server error: {:?}", source),
        };

        let body = ErrorResponsePayload::new(self.to_string());

        (status_code, axum::Json(body)).into_response()
    }
}
