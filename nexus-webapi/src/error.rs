use crate::media::MediaProcessorError;
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
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },
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

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Error::ServiceUnavailable {
            message: message.into(),
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

/// A media failure on its way to a client, carrying its cause but not showing it.
///
/// `InternalServerError` renders its source into the response body through `Display`, and a
/// processor's own `Display` is deliberately verbose: `CommandFailed` interpolates the
/// converter's stderr, and ImageMagick names the file it choked on, i.e. an absolute path under
/// `files_path`. Any user can reach that by declaring a malformed blob `image/png` and asking
/// for `/small`. So the client gets the variant name only -- what `ModelError`'s terse `Display`
/// used to give it -- while `Debug` keeps the whole chain for the `{:?}` log line below.
#[derive(Debug, Error)]
#[error("MediaProcessorError")]
struct RedactedMediaError(#[source] MediaProcessorError);

impl From<MediaProcessorError> for Error {
    fn from(source: MediaProcessorError) -> Self {
        // Load shed: the client-facing message stays generic, the cause is logged server-side.
        if source.is_load_shed() {
            return Error::service_unavailable("service temporarily unavailable");
        }
        Error::InternalServerError {
            source: Box::new(RedactedMediaError(source)),
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
            Error::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
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
            Error::ServiceUnavailable { message } => warn!("Service unavailable: {}", message),
            Error::InternalServerError { source } => error!("Internal server error: {:?}", source),
        };

        let body = ErrorResponsePayload::new(self.to_string());

        (status_code, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use std::time::Duration;

    use crate::media::MediaProcessorError;
    use axum::response::IntoResponse;

    use super::Error;

    // A killed subprocess means "no variant right now", the same answer as a full gate, so it
    // must degrade rather than surface as a server fault.
    #[test]
    fn test_media_shed_errors_map_to_503() {
        let shed = [
            MediaProcessorError::AtCapacity,
            MediaProcessorError::Timeout {
                command: String::from("convert"),
                deadline: Duration::from_secs(180),
            },
        ];

        for source in shed {
            let error = Error::from(source);
            assert!(matches!(error, Error::ServiceUnavailable { .. }));
            assert_eq!(
                error.into_response().status(),
                StatusCode::SERVICE_UNAVAILABLE
            );
        }
    }

    // A genuine processing failure is still a server fault, not a shed -- and the converter's
    // own words must not travel with it: ImageMagick's stderr names the file it choked on,
    // which is an absolute path under `files_path`. The response body is `to_string()`, so
    // asserting on it is asserting on what the client reads.
    #[test]
    fn test_command_failure_maps_to_500_without_leaking_the_converter_message() {
        let error = Error::from(MediaProcessorError::command_failed(
            "ImageMagick format extraction failed: insufficient image data in file \
             `/srv/nexus/static/files/o4dksqu3/0034A0X7NJ52G/main' @ error/png.c/ReadPNGImage/4201.",
        ));

        assert_eq!(
            error.to_string(),
            "Internal server error: MediaProcessorError"
        );
        // The cause still has to reach the operator, via the `{:?}` line in `into_response`.
        assert!(format!("{error:?}").contains("insufficient image data"));

        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
