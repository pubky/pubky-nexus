use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ErrorResponsePayload {
    pub error: String,
}

impl ErrorResponsePayload {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
        }
    }
}

impl IntoResponse for ErrorResponsePayload {
    fn into_response(self) -> axum::response::Response {
        axum::Json(self).into_response()
    }
}
