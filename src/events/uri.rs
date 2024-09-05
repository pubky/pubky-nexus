use crate::models::user::PubkyId;
use std::convert::TryFrom;

#[derive(Default)]
pub struct ParsedUri {
    pub user_id: Option<PubkyId>,
    pub post_id: Option<String>,
    pub follow_id: Option<PubkyId>,
    pub bookmark_id: Option<String>,
    pub tag_id: Option<String>,
}

impl TryFrom<&str> for ParsedUri {
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        let mut parsed_uri = ParsedUri {
            user_id: None,
            post_id: None,
            follow_id: None,
            bookmark_id: None,
            tag_id: None,
        };

        // Extract user_id if present
        if let Some(user_id) = extract_segment(uri, "pubky://", "/pub/") {
            parsed_uri.user_id = Some(PubkyId::try_from(user_id)?);
        }

        // Extract post_id if present
        if let Some(post_id) = extract_segment(uri, "/posts/", "/") {
            parsed_uri.post_id = Some(post_id.to_string());
        }

        // Extract follow_id if present
        if let Some(follow_id) = extract_segment(uri, "/follows/", "/") {
            parsed_uri.user_id = Some(PubkyId::try_from(follow_id)?);
        }

        // Extract bookmark_id if present
        if let Some(bookmark_id) = extract_segment(uri, "/bookmarks/", "/") {
            parsed_uri.bookmark_id = Some(bookmark_id.to_string());
        }

        // Extract tag_id if present
        if let Some(tag_id) = extract_segment(uri, "/tags/", "/") {
            parsed_uri.tag_id = Some(tag_id.to_string());
        }

        Ok(parsed_uri)
    }
}

fn extract_segment<'a>(uri: &'a str, start_pattern: &str, end_pattern: &str) -> Option<&'a str> {
    let start_idx = uri.find(start_pattern)? + start_pattern.len();
    let end_idx = uri[start_idx..].find(end_pattern)? + start_idx;

    Some(&uri[start_idx..end_idx])
}
