use crate::{Error, Result};
use axum::Json;

/// It is used in Axum route handlers to streamline the logic for
/// returning a JSON-wrapped response from a list of items, while gracefully handling the case
/// where the list is empty
/// # Arguments
///
/// * `items` - The vector of items to be returned in the JSON response
/// * `model` - A string used in the error message to indicate which kind of item was expected
pub fn as_json_or_error<T>(items: Vec<T>, model: &str) -> Result<Json<Vec<T>>> {
    if items.is_empty() {
        Err(Error::EmptyStream {
            message: format!("No {:?} found for the given criteria", model),
        })
    } else {
        Ok(Json(items))
    }
}
