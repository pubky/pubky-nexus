use super::{PostRelationships, PostStream};
use crate::db::kv::RedisResult;
use crate::db::{
    execute_graph_operation, fetch_row_from_graph, queries, GraphResult, OperationOutcome, RedisOps,
};
use crate::models::error::ModelResult;
use chrono::Utc;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyId};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents post data with content, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug, PartialEq)]
#[cfg_attr(test, derive(Clone))]
// NOTE: Might not be necessary the default values for serde because before PUT a PostDetails node
// we do sanity check
pub struct PostDetails {
    /// Written by a person, so a light Nexus keeps it nowhere: it is blanked in the graph
    /// and left out of the cached JSON and of API responses. `content_hash` stays.
    #[serde(default, skip_serializing_if = "crate::omit_in_light_mode")]
    pub content: String,
    pub id: String,
    pub indexed_at: i64,
    pub author: String,
    pub kind: PubkyAppPostKind,
    pub uri: String,
    pub attachments: Option<Vec<String>>,
    /// `pubky://` URL of the lock server; `None` when the post is unlocked.
    /// `default` keeps pre-lock cached JSON (no `lock` key) deserializing.
    #[serde(default)]
    pub lock: Option<String>,
    /// Set on the cleared node left behind when a post with relationships is
    /// deleted. `default` keeps cached JSON written before the flag (no
    /// `deleted` key) deserializing as a live post.
    #[serde(default)]
    pub deleted: bool,
    /// blake3 of `content`, hex encoded. Lets an edit be detected without the
    /// stored content. `default` keeps posts written before the hash (no
    /// `content_hash` key or property) deserializing as `None`.
    #[serde(default)]
    pub content_hash: Option<String>,
}

impl RedisOps for PostDetails {}

impl PostDetails {
    /// Retrieves post details by author ID and post ID, first trying to get from Redis, then from Neo4j if not found.
    pub async fn get_by_id(author_id: &str, post_id: &str) -> ModelResult<Option<PostDetails>> {
        match Self::get_from_index(author_id, post_id).await? {
            Some(details) => Ok(Some(details)),
            None => {
                let graph_response = Self::get_from_graph(author_id, post_id).await?;
                if let Some((post_details, reply)) = graph_response {
                    post_details.put_to_index(author_id, reply, false).await?;
                    return Ok(Some(post_details));
                }
                Ok(None)
            }
        }
    }

    pub async fn get_from_index(
        author_id: &str,
        post_id: &str,
    ) -> RedisResult<Option<PostDetails>> {
        Self::try_from_index_json(&[author_id, post_id], None).await
    }

    /// Retrieves the post fields from Neo4j.
    pub async fn get_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> GraphResult<Option<(PostDetails, Option<(String, String)>)>> {
        let query = queries::get::get_post_by_id(author_id, post_id);
        let maybe_row = fetch_row_from_graph(query).await?;

        let Some(row) = maybe_row else {
            return Ok(None);
        };

        let post: PostDetails = row.get("details")?;
        let reply_value: Vec<(String, String)> = row.get("reply").unwrap_or(Vec::new());
        let reply_key = match reply_value.is_empty() {
            true => None,
            false => Some(reply_value[0].clone()),
        };
        Ok(Some((post, reply_key)))
    }

    /// The links stored on the post node: `(mentioned_ids, collection_items)`. They are
    /// written with the post and let its MENTIONED and COLLECTED edges be rebuilt without
    /// the content. `None` when the post is not in the graph; a list is `None` on a post
    /// written before the lists existed.
    pub async fn get_link_lists_from_graph(
        author_id: &str,
        post_id: &str,
    ) -> GraphResult<Option<(Option<Vec<String>>, Option<Vec<String>>)>> {
        let query = queries::get::get_post_link_lists(author_id, post_id);
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok(None);
        };
        Ok(Some((
            row.get("mentioned_ids")?,
            row.get("collection_items")?,
        )))
    }

    pub async fn put_to_index(
        &self,
        author_id: &str,
        parent_key_wrapper: Option<(String, String)>,
        is_edit: bool,
    ) -> RedisResult<()> {
        self.put_index_json(&[author_id, &self.id], None, None)
            .await?;
        // When we delete a post that has ancestor, ignore other index updates
        if is_edit {
            return Ok(());
        }
        // Replies are not indexed in the global feeds — they live in the
        // per-parent reply set instead.
        match parent_key_wrapper {
            None => {
                PostStream::add_to_timeline_sorted_set(self).await?;
                PostStream::add_to_per_user_sorted_set(self).await?;
            }
            Some((parent_author_id, parent_post_id)) => {
                PostStream::add_to_post_reply_sorted_set(
                    &[&parent_author_id, &parent_post_id],
                    author_id,
                    &self.id,
                    self.indexed_at,
                )
                .await?;
                PostStream::add_to_replies_per_user_sorted_set(self).await?;
            }
        }
        Ok(())
    }

    /// `uri` is the address the post was read from: the event path.
    pub fn from_homeserver(
        homeserver_post: PubkyAppPost,
        uri: String,
        author_id: &PubkyId,
        post_id: &str,
    ) -> Self {
        PostDetails {
            uri,
            content_hash: Some(Self::hash_content(&homeserver_post.content)),
            content: homeserver_post.content,
            id: post_id.to_string(),
            indexed_at: Utc::now().timestamp_millis(),
            author: author_id.to_string(),
            kind: homeserver_post.kind,
            attachments: homeserver_post.attachments,
            lock: homeserver_post.lock,
            deleted: false,
        }
    }

    /// The `content_hash` of `content`: its full blake3 digest, hex encoded.
    pub fn hash_content(content: &str) -> String {
        blake3::hash(content.as_bytes()).to_hex().to_string()
    }

    pub async fn reindex(author_id: &str, post_id: &str) -> ModelResult<()> {
        match Self::get_from_graph(author_id, post_id).await? {
            Some((details, reply)) => details.put_to_index(author_id, reply, false).await?,
            None => {
                tracing::error!("{author_id}:{post_id} Could not find post counts in the graph")
            }
        }
        Ok(())
    }

    // Save new graph node
    pub async fn put_to_graph(
        &self,
        post_relationships: &PostRelationships,
    ) -> GraphResult<OperationOutcome> {
        let query = queries::put::create_post(self, post_relationships)?;
        execute_graph_operation(query).await
    }

    /// Removes the post details JSON entry and its sorted-set memberships from Redis.
    /// Idempotent: every operation is a JSON.DEL or ZREM, safe to retry.
    /// Does NOT touch the graph — callers must run `delete_post` separately to
    /// preserve the graph-last invariant.
    pub async fn delete_from_index(
        author_id: &str,
        post_id: &str,
        parent_post_key_wrapper: Option<(String, String)>,
    ) -> RedisResult<()> {
        // Delete post details on Redis
        Self::remove_from_index_multiple_json(&[&[author_id, post_id]]).await?;
        // The replies are not indexed in the global feeds
        match parent_post_key_wrapper {
            None => {
                PostStream::remove_from_timeline_sorted_set(author_id, post_id).await?;
                PostStream::remove_from_per_user_sorted_set(author_id, post_id).await?;
            }
            Some((parent_author_id, parent_post_id)) => {
                PostStream::remove_from_post_reply_sorted_set(
                    &[&parent_author_id, &parent_post_id],
                    author_id,
                    post_id,
                )
                .await?;
                PostStream::remove_from_replies_per_user_sorted_set(author_id, post_id).await?;
            }
        }
        Ok(())
    }

    /// True when the post's visible content (content or attachments) changed.
    /// Deliberately excludes `lock` so a lock toggle is not treated as a content edit.
    ///
    /// Content is compared by `content_hash` when both sides have one, so the
    /// check works when the stored side holds no content. A post stored before
    /// the hash existed has none and falls back to comparing the content.
    pub fn content_differs_from(&self, other: &PostDetails) -> bool {
        let content_changed = match (&self.content_hash, &other.content_hash) {
            (Some(own), Some(other_hash)) => own != other_hash,
            _ => self.content != other.content,
        };
        content_changed || self.attachments != other.attachments
    }

    /// True when any cached field changed and the index needs refreshing. Unlike
    /// [`Self::content_differs_from`] this includes `lock` and `deleted`, so a
    /// lock-only toggle refreshes the cache without counting as a content edit.
    pub fn is_different_than(&self, other: &PostDetails) -> bool {
        self.content_differs_from(other) || self.lock != other.lock || self.deleted != other.deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_app_specs::PubkyAppPostKind;

    #[tokio_shared_rt::test(shared)]
    async fn test_is_different_than() {
        // Create a base PostDetails
        let base_post = PostDetails {
            content: "Original content".into(),
            id: "post1".into(),
            indexed_at: 123456789,
            author: "author1".into(),
            kind: PubkyAppPostKind::Short,
            uri: "uri1".into(),
            attachments: Some(vec!["image1.jpg".into(), "image2.jpg".into()]),
            lock: None,
            deleted: false,
            content_hash: None,
        };

        // Test with same content and attachments
        let same_post = base_post.clone();
        assert!(!base_post.is_different_than(&same_post));

        // Test with same attachments but different order
        let different_order_attachments_post = PostDetails {
            attachments: Some(vec!["image2.jpg".into(), "image1.jpg".into()]),
            ..base_post.clone()
        };
        assert!(base_post.is_different_than(&different_order_attachments_post));

        // Test with different content
        let different_content_post = PostDetails {
            content: "Updated content".to_string(),
            ..base_post.clone()
        };
        assert!(base_post.is_different_than(&different_content_post));

        // Test with different attachments
        let different_attachments_post = PostDetails {
            attachments: Some(vec!["image3.jpg".to_string()]),
            ..base_post.clone()
        };
        assert!(base_post.is_different_than(&different_attachments_post));

        // Test with no attachments
        let no_attachments_post = PostDetails {
            attachments: None,
            ..base_post.clone()
        };
        assert!(base_post.is_different_than(&no_attachments_post));

        // Test with a different lock (lock toggle must count as an edit)
        let locked_post = PostDetails {
            lock: Some("pubky://lockserver.example/pub/pubky.app/lock".into()),
            ..base_post.clone()
        };
        assert!(base_post.is_different_than(&locked_post));
    }

    #[test]
    fn test_deserialize_legacy_json_without_lock() {
        // A pre-lock cached PostDetails JSON has no `lock` key. It must still
        // deserialize, with `lock` defaulting to None, so old Redis entries
        // stay readable after deploy.
        let legacy = r#"{
            "content": "hi",
            "id": "post1",
            "indexed_at": 123456789,
            "author": "author1",
            "kind": "short",
            "uri": "pubky://author1/pub/pubky.app/posts/post1",
            "attachments": null
        }"#;
        let details: PostDetails = serde_json::from_str(legacy).unwrap();
        assert_eq!(details.lock, None);
    }

    #[test]
    fn test_deserialize_deleted_flag() {
        // Cached JSON written before the flag has no `deleted` key and must
        // read as a live post; a present `deleted: true` is kept.
        let legacy = r#"{
            "content": "hi",
            "id": "post1",
            "indexed_at": 123456789,
            "author": "author1",
            "kind": "short",
            "uri": "pubky://author1/pub/pubky.app/posts/post1",
            "attachments": null,
            "lock": null
        }"#;
        let details: PostDetails = serde_json::from_str(legacy).unwrap();
        assert!(!details.deleted);

        let mut tombstone: serde_json::Value = serde_json::from_str(legacy).unwrap();
        tombstone["content"] = "".into();
        tombstone["deleted"] = true.into();
        let details: PostDetails = serde_json::from_value(tombstone).unwrap();
        assert!(details.deleted);
    }

    #[test]
    fn test_content_differs_from_ignores_lock() {
        let base = PostDetails {
            content: "c".into(),
            id: "p".into(),
            indexed_at: 1,
            author: "a".into(),
            kind: PubkyAppPostKind::Short,
            uri: "u".into(),
            attachments: None,
            lock: None,
            deleted: false,
            content_hash: None,
        };
        let locked = PostDetails {
            lock: Some("pubky://host/pub/lock".into()),
            ..base.clone()
        };
        // A lock-only toggle is a cache difference but not a content edit.
        assert!(base.is_different_than(&locked));
        assert!(!base.content_differs_from(&locked));
        // A content change is both.
        let edited = PostDetails {
            content: "c2".into(),
            ..base.clone()
        };
        assert!(base.content_differs_from(&edited));
    }

    #[test]
    fn test_content_differs_from_compares_hashes() {
        let stored = PostDetails {
            content_hash: Some(PostDetails::hash_content("hello")),
            ..PostDetails::default()
        };
        // The stored side holds no content, only its hash: same hash, no edit.
        let same = PostDetails {
            content: "hello".into(),
            content_hash: Some(PostDetails::hash_content("hello")),
            ..PostDetails::default()
        };
        assert!(!stored.content_differs_from(&same));
        let edited = PostDetails {
            content: "hello!".into(),
            content_hash: Some(PostDetails::hash_content("hello!")),
            ..PostDetails::default()
        };
        assert!(stored.content_differs_from(&edited));
    }

    #[test]
    fn test_content_differs_from_falls_back_without_hash() {
        // A post stored before the hash existed is compared by content.
        let legacy = PostDetails {
            content: "hello".into(),
            ..PostDetails::default()
        };
        let incoming = PostDetails {
            content: "hello".into(),
            content_hash: Some(PostDetails::hash_content("hello")),
            ..PostDetails::default()
        };
        assert!(!legacy.content_differs_from(&incoming));
        let edited = PostDetails {
            content: "bye".into(),
            content_hash: Some(PostDetails::hash_content("bye")),
            ..PostDetails::default()
        };
        assert!(legacy.content_differs_from(&edited));
    }

    #[test]
    fn test_from_homeserver_sets_content_hash() {
        let post = PubkyAppPost {
            content: "hello".into(),
            kind: PubkyAppPostKind::Short,
            parent: None,
            embed: None,
            attachments: None,
            lock: None,
        };
        let author = PubkyId::try_from("ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny")
            .expect("valid pubky id");
        let details = PostDetails::from_homeserver(post, "uri".into(), &author, "post1");
        assert_eq!(
            details.content_hash.as_deref(),
            Some(PostDetails::hash_content("hello").as_str())
        );
    }

    #[test]
    fn test_deleted_flag_is_not_content() {
        // Deleting a repost with no content or attachments changes no content,
        // but the cache still has to pick up the flag.
        let live = PostDetails::default();
        let tombstone = PostDetails {
            deleted: true,
            ..PostDetails::default()
        };
        assert!(!live.content_differs_from(&tombstone));
        assert!(live.is_different_than(&tombstone));
    }
}
