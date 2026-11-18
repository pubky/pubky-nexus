//! A user on `/v1`: the counts without `bookmarks`, and the view and the stream that carry
//! them, with the user's tags without `relationship`.
//!
//! `social/v1` bookmarks live under `/priv/`, which Nexus never sees, so `/v1` never shows a
//! count it cannot produce. The stored count, the `BOOKMARKED` edges and the `/v0` field stay
//! until R1.

use nexus_common::models::user::{self, Relationship, SocialGraphStatus, UserDetails};
use serde::Serialize;
use utoipa::ToSchema;

use super::TagDetails;

/// Total counts of a user's relationships, as `/v0`'s [`user::UserCounts`] without
/// `bookmarks`.
#[derive(Serialize, ToSchema, Debug)]
pub struct UserCounts {
    /// The tags the user put on other users and posts.
    pub tagged: u32,
    /// The tags the user received.
    pub tags: u32,
    /// The distinct labels among the tags the user received.
    pub unique_tags: u32,
    pub posts: u32,
    pub replies: u32,
    pub collections: u32,
    pub following: u32,
    pub followers: u32,
    pub friends: u32,
}

impl From<user::UserCounts> for UserCounts {
    fn from(counts: user::UserCounts) -> Self {
        let user::UserCounts {
            tagged,
            tags,
            unique_tags,
            posts,
            replies,
            collections,
            following,
            followers,
            friends,
            bookmarks: _,
        } = counts;
        Self {
            tagged,
            tags,
            unique_tags,
            posts,
            replies,
            collections,
            following,
            followers,
            friends,
        }
    }
}

/// A user with their tags, counts and relationship with the viewer, as `/v0`'s
/// [`user::UserView`] with the counts without `bookmarks` and the tags without
/// `relationship`. The user's own `relationship`, whether the viewer and they follow each
/// other, stays: it is not a tag's flag.
#[derive(Serialize, ToSchema, Debug)]
pub struct UserView {
    pub details: UserDetails,
    pub counts: UserCounts,
    pub tags: Vec<TagDetails>,
    pub relationship: Relationship,
    /// How established the account is in the follow graph. `None` when no
    /// ranking is available, which is not the same as ranking as new.
    pub social_graph_status: Option<SocialGraphStatus>,
}

impl From<user::UserView> for UserView {
    fn from(view: user::UserView) -> Self {
        let user::UserView {
            details,
            counts,
            tags,
            relationship,
            social_graph_status,
        } = view;
        Self {
            details,
            counts: counts.into(),
            tags: tags.into_iter().map(TagDetails::from).collect(),
            relationship,
            social_graph_status,
        }
    }
}

/// A list of [`UserView`]s, as `/v0`'s [`user::UserStream`].
#[derive(Serialize, ToSchema, Debug, Default)]
pub struct UserStream(pub Vec<UserView>);

impl From<user::UserStream> for UserStream {
    fn from(stream: user::UserStream) -> Self {
        Self(stream.0.into_iter().map(UserView::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// The `/v1` counts are the stored counts without `bookmarks` and nothing else.
    #[test]
    fn counts_differ_from_v0_only_by_bookmarks() {
        let stored = user::UserCounts {
            tagged: 1,
            tags: 2,
            unique_tags: 3,
            posts: 4,
            replies: 5,
            collections: 6,
            following: 7,
            followers: 8,
            friends: 9,
            bookmarks: 10,
        };
        let mut v0 = serde_json::to_value(&stored).unwrap();
        let v1 = serde_json::to_value(UserCounts::from(stored)).unwrap();
        assert_eq!(v0["bookmarks"], 10);
        assert!(v1.get("bookmarks").is_none(), "{v1}");
        v0.as_object_mut().unwrap().remove("bookmarks");
        assert_eq!(v1, v0);
    }

    /// The `/v1` view is the stored view with `counts.bookmarks` and each tag's
    /// `relationship` taken out and nothing else, the user's own `relationship` included; an
    /// empty stream is a plain array, as on `/v0`.
    #[test]
    fn view_and_stream_differ_from_v0_only_by_the_bookmarks_count_and_the_tags_flag() {
        let stored: user::UserView = serde_json::from_value(json!({
            "details": {
                "name": "Aldert",
                "bio": null,
                "id": "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
                "links": [],
                "status": null,
                "image": null,
                "indexed_at": 1,
                "deleted": false,
                "epoch": "v1"
            },
            "counts": {
                "tagged": 0, "tags": 0, "unique_tags": 0, "posts": 0, "replies": 0,
                "collections": 0, "following": 0, "followers": 0, "friends": 0, "bookmarks": 3
            },
            "tags": [
                { "label": "rust", "taggers": ["alice"], "taggers_count": 1, "relationship": true,
                  "tag_uri": "pubky://alice/pub/social/v1/tags/T1" }
            ],
            "relationship": { "following": false, "followed_by": true },
            "social_graph_status": "new"
        }))
        .unwrap();
        let mut v0 = serde_json::to_value(&stored).unwrap();
        assert_eq!(v0["counts"]["bookmarks"], 3, "{v0}");
        assert_eq!(v0["tags"][0]["relationship"], true, "{v0}");
        let v1 = serde_json::to_value(UserView::from(stored)).unwrap();
        assert!(v1["counts"].get("bookmarks").is_none(), "{v1}");
        assert!(v1["tags"][0].get("relationship").is_none(), "{v1}");
        assert_eq!(
            v1["tags"][0]["tag_uri"],
            "pubky://alice/pub/social/v1/tags/T1"
        );
        assert_eq!(
            v1["relationship"],
            json!({ "following": false, "followed_by": true })
        );
        for field in [
            "details",
            "counts",
            "tags",
            "relationship",
            "social_graph_status",
        ] {
            assert!(v1.get(field).is_some(), "{field}: {v1}");
        }
        v0["counts"].as_object_mut().unwrap().remove("bookmarks");
        v0["tags"][0]
            .as_object_mut()
            .unwrap()
            .remove("relationship");
        assert_eq!(v1, v0);

        let stream = UserStream::from(user::UserStream::default());
        assert_eq!(serde_json::to_value(stream).unwrap(), json!([]));
    }

    /// The schema lists every count but `bookmarks`, each required.
    #[test]
    fn schema_has_no_bookmarks() {
        let schema = serde_json::to_value(<UserCounts as utoipa::PartialSchema>::schema()).unwrap();
        let properties = schema["properties"].as_object().unwrap();
        assert!(!properties.contains_key("bookmarks"), "{schema}");
        let mut required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        required.sort_unstable();
        assert_eq!(
            required,
            [
                "collections",
                "followers",
                "following",
                "friends",
                "posts",
                "replies",
                "tagged",
                "tags",
                "unique_tags",
            ]
        );
    }
}
