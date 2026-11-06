//! What the post streams read differently on `/v1`: the `source=` without `bookmarks`, and
//! the `by_ids` body without `include_attachment_metadata`.
//!
//! `social/v1` bookmarks live under `/priv/`, which Nexus never sees, so a bookmark stream is
//! not a source a `/v1` client can ask for: `source=bookmarks` is an unknown source there, a
//! 400 like any other name. The `BOOKMARKED` edges, the Redis keys and the `/v0` source stay
//! until R1. A `social/v1` file has no `File` node, so there is no metadata a `/v1` client can
//! ask the `by_ids` stream to attach.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::{GlobalPostIds, PubkyId};
use crate::routes::v0::stream::posts;

/// The body of `POST /stream/posts/by_ids` on `/v1`: `/v0`'s [`posts::PostStreamByIdsRequest`]
/// without `include_attachment_metadata`. Serde reads a JSON body leniently, so a client that
/// sends the flag is read as if it had sent any unknown field: ignored, with no 400. Named as
/// the `/v0` component is, so a client reads one `PostStreamByIdsRequest` in its document.
#[derive(Deserialize, ToSchema, Debug)]
pub struct PostStreamByIdsRequest {
    pub post_ids: GlobalPostIds,
    pub viewer_id: Option<PubkyId>,
}

/// The shared body the one handler reads, with no metadata fetched.
impl From<PostStreamByIdsRequest> for posts::PostStreamByIdsRequest {
    fn from(request: PostStreamByIdsRequest) -> Self {
        Self {
            post_ids: request.post_ids,
            viewer_id: request.viewer_id,
            include_attachment_metadata: false,
        }
    }
}

/// The source of a post stream on `/v1`, variant for variant `/v0`'s
/// [`posts::StreamSourceKind`] without `Bookmarks`. Serde derives the strict reading, so an
/// unknown name is rejected naming the ones a client may use.
#[derive(Deserialize, Debug, ToSchema, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StreamSourceKind {
    PostReplies,
    Following,
    Followers,
    Friends,
    Author,
    AuthorReplies,
    Collection,
    PostCollections,
    Wot,
    WotDomain,
    #[default]
    All,
}

/// The names a `/v1` client may write in `source=`, as the parameter description lists them.
pub const SOURCE_NAMES: &str = "following, followers, friends, post_replies, author, author_replies, collection, post_collections, wot, wot_domain, all";

/// The shared source the stream builder reads, for the one body behind both versions.
impl From<StreamSourceKind> for posts::StreamSourceKind {
    fn from(source: StreamSourceKind) -> Self {
        match source {
            StreamSourceKind::PostReplies => Self::PostReplies,
            StreamSourceKind::Following => Self::Following,
            StreamSourceKind::Followers => Self::Followers,
            StreamSourceKind::Friends => Self::Friends,
            StreamSourceKind::Author => Self::Author,
            StreamSourceKind::AuthorReplies => Self::AuthorReplies,
            StreamSourceKind::Collection => Self::Collection,
            StreamSourceKind::PostCollections => Self::PostCollections,
            StreamSourceKind::Wot => Self::Wot,
            StreamSourceKind::WotDomain => Self::WotDomain,
            StreamSourceKind::All => Self::All,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::enum_values;
    use serde_json::Value;
    use utoipa::PartialSchema;

    /// The `/v1` sources are the `/v0` sources without `bookmarks`, in the same order, and
    /// `SOURCE_NAMES` lists them as the `/v0` description does.
    #[test]
    fn sources_are_the_v0_sources_without_bookmarks() {
        let v0 = enum_values::<posts::StreamSourceKind>();
        let v1 = enum_values::<StreamSourceKind>();
        assert!(v0.contains(&"bookmarks".to_string()), "{v0:?}");
        let expected: Vec<&str> = v0
            .iter()
            .map(String::as_str)
            .filter(|name| *name != "bookmarks")
            .collect();
        assert_eq!(v1, expected);
        for name in &v1 {
            assert!(
                SOURCE_NAMES.split(", ").any(|listed| listed == name),
                "{name}"
            );
        }
        assert_eq!(SOURCE_NAMES.split(", ").count(), v1.len());
    }

    /// Every `/v1` name reads into the shared source of the same name; `bookmarks` is an error
    /// that lists the names a client may use.
    #[test]
    fn every_name_reads_into_the_shared_source() {
        for name in enum_values::<StreamSourceKind>() {
            let v1: StreamSourceKind = serde_json::from_value(Value::from(name.as_str())).unwrap();
            let v0: posts::StreamSourceKind =
                serde_json::from_value(Value::from(name.as_str())).unwrap();
            assert_eq!(posts::StreamSourceKind::from(v1), v0, "{name}");
        }
        let error = serde_json::from_value::<StreamSourceKind>(Value::from("bookmarks"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown variant `bookmarks`"), "{error}");
        assert!(error.contains("`all`"), "{error}");
        assert_eq!(StreamSourceKind::default(), StreamSourceKind::All);
    }

    /// The `/v1` body reads `post_ids` and `viewer_id` into the shared body with no fetch, and
    /// an `include_attachment_metadata` a client sends is ignored, `true` included; its schema
    /// has no such property and keeps the `/v0` component's name.
    #[test]
    fn by_ids_body_reads_without_the_metadata_flag() {
        let author = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
        for body in [
            serde_json::json!({ "post_ids": [format!("{author}:0000000000001")] }),
            serde_json::json!({
                "post_ids": [format!("{author}:0000000000001")],
                "viewer_id": author,
                "include_attachment_metadata": true
            }),
        ] {
            let request: PostStreamByIdsRequest = serde_json::from_value(body.clone()).unwrap();
            let shared = posts::PostStreamByIdsRequest::from(request);
            assert_eq!(shared.post_ids.0.len(), 1, "{body}");
            assert_eq!(
                shared.viewer_id.is_some(),
                body.get("viewer_id").is_some(),
                "{body}"
            );
            assert!(!shared.include_attachment_metadata, "{body}");
        }

        assert_eq!(
            <PostStreamByIdsRequest as ToSchema>::name(),
            <posts::PostStreamByIdsRequest as ToSchema>::name()
        );
        let schema = serde_json::to_value(PostStreamByIdsRequest::schema()).unwrap();
        let properties = schema["properties"].as_object().unwrap();
        assert!(
            !properties.contains_key("include_attachment_metadata"),
            "{schema}"
        );
        assert!(properties.contains_key("post_ids"), "{schema}");
        assert!(properties.contains_key("viewer_id"), "{schema}");
        assert_eq!(
            schema["required"],
            Value::from(vec!["post_ids"]),
            "{schema}"
        );
    }
}
