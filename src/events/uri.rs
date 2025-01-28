use crate::types::DynError;
use crate::types::PubkyId;
use std::convert::TryFrom;

use super::error::EventProcessorError;

#[derive(Default, Debug)]
pub struct ParsedUri {
    pub user_id: PubkyId,
    pub post_id: Option<String>,
    pub follow_id: Option<PubkyId>,
    pub muted_id: Option<PubkyId>,
    pub bookmark_id: Option<String>,
    pub tag_id: Option<String>,
    pub file_id: Option<String>,
}

impl TryFrom<&str> for ParsedUri {
    type Error = DynError;

    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        let mut parsed_uri = ParsedUri::default();

        // Ensure the URI starts with the correct prefix
        if !uri.starts_with("pubky://") {
            return Err(EventProcessorError::InvalidEventLine {
                message: format!("Invalid URI, must start with pubky://,  {}", uri),
            }
            .into());
        }

        // Extract the user_id from the initial part of the URI
        if let Some(user_id) = extract_segment(uri, "pubky://", "/pub/") {
            parsed_uri.user_id = PubkyId::try_from(user_id)?;
        } else {
            return Err(EventProcessorError::InvalidEventLine {
                message: format!("Uri Pubky ID is invalid,  {}", uri),
            }
            .into());
        }

        // Ensure that the URI belongs to pubky.app
        if let Some(app_segment) = extract_segment(uri, "/pub/", "/") {
            if app_segment != "pubky.app" {
                return Err(EventProcessorError::InvalidEventLine {
                    message: format!("The Event URI does not belong to pubky.app,  {}", uri),
                }
                .into());
            }
        } else {
            return Err(EventProcessorError::InvalidEventLine {
                message: format!("The Event URI is malformed,  {}", uri),
            }
            .into());
        }

        // Extract post_id if present
        parsed_uri.post_id = extract_segment(uri, "/posts/", "/")
            .filter(|id| !id.is_empty())
            .map(String::from);

        // Extract follow_id if present
        parsed_uri.follow_id = extract_segment(uri, "/follows/", "/")
            .map(PubkyId::try_from)
            .transpose()
            .map_err(|e| EventProcessorError::InvalidEventLine {
                message: format!("{}, {}", e, uri),
            })?;

        // Extract muted_id if present
        parsed_uri.muted_id = extract_segment(uri, "/mutes/", "/")
            .map(PubkyId::try_from)
            .transpose()
            .map_err(|e| EventProcessorError::InvalidEventLine {
                message: format!("{}, {}", e, uri),
            })?;

        // Extract bookmark_id if present
        parsed_uri.bookmark_id = extract_segment(uri, "/bookmarks/", "/")
            .filter(|id| !id.is_empty())
            .map(String::from);

        // Extract tag_id if present
        parsed_uri.tag_id = extract_segment(uri, "/tags/", "/")
            .filter(|id| !id.is_empty())
            .map(String::from);

        // Extract file_id if present
        parsed_uri.file_id = extract_segment(uri, "/files/", "/")
            .filter(|id| !id.is_empty())
            .map(String::from);

        Ok(parsed_uri)
    }
}

fn extract_segment<'a>(uri: &'a str, start_pattern: &str, end_pattern: &str) -> Option<&'a str> {
    let start_idx = uri.find(start_pattern)? + start_pattern.len();
    let end_idx = uri[start_idx..]
        .find(end_pattern)
        .map(|i| i + start_idx)
        .unwrap_or_else(|| uri.len());

    Some(&uri[start_idx..end_idx])
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_bookmark_uri() {
        let uri =
            "pubky://phbhg3qgcttn95guepmbud1nzcxhg3xc5j5k4h7i8a4b6wb3nw1o/pub/pubky.app/bookmarks/";
        let parsed_uri = ParsedUri::try_from(uri).unwrap_or_default();
        assert_eq!(
            parsed_uri.bookmark_id, None,
            "The provided URI has bookmark_id"
        );
    }
}
