use crate::models::user::PubkyId;
use std::convert::TryFrom;

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
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        let mut parsed_uri = ParsedUri::default();

        // Ensure the URI starts with the correct prefix
        if !uri.starts_with("pubky://") {
            return Err("Invalid URI, must start with pubky://".into());
        }

        // Extract the user_id from the initial part of the URI
        if let Some(user_id) = extract_segment(uri, "pubky://", "/pub/") {
            parsed_uri.user_id = PubkyId::try_from(user_id)?;
        } else {
            return Err("Uri Pubky ID is invalid".into());
        }

        // Ensure that the URI belongs to pubky.app
        if let Some(app_segment) = extract_segment(uri, "/pub/", "/") {
            if app_segment != "pubky.app" {
                return Err("The Event URI does not belong to pubky.app".into());
            }
        } else {
            return Err("The Event URI is malformed".into());
        }

        // Extract post_id if present
        if let Some(post_id) = extract_segment(uri, "/posts/", "/") {
            parsed_uri.post_id = Some(post_id.to_string());
        }

        // Extract follow_id if present
        if let Some(follow_id) = extract_segment(uri, "/follows/", "/") {
            parsed_uri.follow_id = Some(PubkyId::try_from(follow_id)?);
        }

        // Extract muted_id if present
        if let Some(muted_id) = extract_segment(uri, "/mutes/", "/") {
            parsed_uri.muted_id = Some(PubkyId::try_from(muted_id)?);
        }

        // Extract bookmark_id if present
        if let Some(bookmark_id) = extract_segment(uri, "/bookmarks/", "/") {
            parsed_uri.bookmark_id = Some(bookmark_id.to_string());
        }

        // Extract tag_id if present
        if let Some(tag_id) = extract_segment(uri, "/tags/", "/") {
            parsed_uri.tag_id = Some(tag_id.to_string());
        }

        // Extract file_id if present
        if let Some(file_id) = extract_segment(uri, "/files/", "/") {
            parsed_uri.file_id = Some(file_id.to_string());
        }

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
