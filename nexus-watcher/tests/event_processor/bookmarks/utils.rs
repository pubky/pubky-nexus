use anyhow::Result;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::PostStream;
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark};

/// A bookmark of `author_id`'s post `post_id`, made now.
pub fn bookmark_of(author_id: &str, post_id: &str) -> PubkyAppBookmark {
    PubkyAppBookmark {
        uri: post_uri_builder(author_id.to_string(), post_id.to_string()),
        created_at: chrono::Utc::now().timestamp_millis(),
    }
}

/// The keys of the posts `user_id` bookmarked, newest bookmark first.
pub async fn bookmarked_post_keys(user_id: &str) -> Result<Vec<String>> {
    let stream =
        PostStream::get_bookmarked_posts(user_id, SortOrder::Descending, None, None, None, None)
            .await?;
    Ok(stream.post_keys)
}
