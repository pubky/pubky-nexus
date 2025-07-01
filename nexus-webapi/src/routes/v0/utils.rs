use crate::{Error, Result};
use axum::Json;
use serde::Serialize;

/// It is used in Axum route handlers to streamline the logic for
/// returning a JSON-wrapped response from a list of items, while gracefully handling the case
/// where the list is empty
/// # Arguments
///
/// * `items` - The vector of items to be returned in the JSON response
/// * `model` - A string used in the error message to indicate which kind of item was expected
pub fn json_array_or_no_content<T>(items: Vec<T>, model: &str) -> Result<Json<Vec<T>>>
where
    T: Serialize,
{
    if items.is_empty() {
        Err(Error::EmptyStream {
            message: format!("No {model} found for the given criteria"),
        })
    } else {
        Ok(Json(items))
    }
}
