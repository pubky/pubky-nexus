/// Generates an `impl FromRequestParts<S>` for a type that implements `TryFrom<String>`.
///
/// The rejection type is always `crate::Error`. Path extraction failures classified by
/// axum as client errors (e.g. invalid UTF-8 in a path segment) map to
/// `crate::Error::InvalidInput`; server-error rejections (e.g. router misconfiguration)
/// map to `crate::Error::InternalServerError`. `TryFrom` failures map to
/// `crate::Error::invalid_input(&e.to_string())`.
///
/// # Usage
/// ```ignore
/// path_extractor_impl!(PostId);
/// ```
#[macro_export]
macro_rules! path_extractor_impl {
    ($type:ty) => {
        impl<S: Send + Sync> ::axum::extract::FromRequestParts<S> for $type {
            type Rejection = crate::Error;

            async fn from_request_parts(
                parts: &mut ::axum::http::request::Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                let ::axum::extract::Path(s): ::axum::extract::Path<String> =
                    ::axum::extract::Path::from_request_parts(parts, state)
                        .await
                        .map_err(|e| {
                            if e.status().is_client_error() {
                                crate::Error::invalid_input(&e.body_text())
                            } else {
                                crate::Error::InternalServerError {
                                    source: Box::new(e),
                                }
                            }
                        })?;
                Self::try_from(s).map_err(|e| crate::Error::invalid_input(&e.to_string()))
            }
        }
    };
}
