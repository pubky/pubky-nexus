//! The `source=` of the post streams on `/v1`: `/v0`'s sources without `bookmarks`.
//!
//! `social/v1` bookmarks live under `/priv/`, which Nexus never sees, so a bookmark stream is
//! not a source a `/v1` client can ask for: `source=bookmarks` is an unknown source there, a
//! 400 like any other name. The `BOOKMARKED` edges, the Redis keys and the `/v0` source stay
//! until R1.

use serde::Deserialize;
use utoipa::ToSchema;

use crate::routes::v0::stream::posts;

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
    use serde_json::Value;
    use utoipa::PartialSchema;

    /// The `enum` values of a source's schema.
    fn enum_values<T: PartialSchema>() -> Vec<String> {
        let schema = serde_json::to_value(T::schema()).unwrap();
        schema["enum"]
            .as_array()
            .unwrap_or_else(|| panic!("no enum in {schema}"))
            .iter()
            .map(|name| name.as_str().unwrap().to_string())
            .collect()
    }

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
}
