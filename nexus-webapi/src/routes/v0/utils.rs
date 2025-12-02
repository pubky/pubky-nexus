use crate::Result;
use axum::Json;
use serde::Serialize;

/// It is used in Axum route handlers to streamline the logic for
/// returning a JSON-wrapped response from a list of items, including when the list is empty.
/// Returns HTTP 200 OK with an empty array when no items are found.
/// # Arguments
///
/// * `items` - The vector of items to be returned in the JSON response
/// * `_model` - A string (unused) previously used for error messages
pub fn json_array_or_empty<T>(items: Vec<T>, _model: &str) -> Result<Json<Vec<T>>>
where
    T: Serialize,
{
    Ok(Json(items))
}
