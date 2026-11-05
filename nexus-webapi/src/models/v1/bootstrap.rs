//! The bootstrap payload on `/v1`: `/v0`'s with the users, posts and notifications in their
//! `/v1` shapes.
//!
//! The lists it embeds are the ones the user stream, the post stream and the notification
//! route serve, so a client reads one shape for a user and one for a post wherever they
//! appear: no `bookmarks` in a user's counts, no `bookmark` on a post, the post kinds under
//! their `social/v1` names. `files` stays as `/v0` lists it until its own ticket.

use nexus_common::models::bootstrap::{self, BootstrapIds};
use nexus_common::models::file::FileDetails;
use serde::Serialize;
use utoipa::ToSchema;

use super::{Notification, PostStream, UserStream};

/// Everything the client app loads at sign-in, as `/v0`'s [`bootstrap::Bootstrap`] with
/// each embedded list in its `/v1` shape.
#[derive(Serialize, ToSchema, Debug, Default)]
pub struct Bootstrap {
    /// The user objects shown to the given user ID
    pub users: UserStream,
    /// The posts objects shown to the given user ID
    pub posts: PostStream,
    /// File metadata for all attachments referenced in posts and replies
    pub files: Vec<FileDetails>,
    /// IDs of objects shown to this user on the home page of the FE
    pub ids: BootstrapIds,
    /// Whether or not this user is already indexed
    pub indexed: bool,
    /// Latest notifications
    pub notifications: Vec<Notification>,
}

impl From<bootstrap::Bootstrap> for Bootstrap {
    fn from(bootstrap: bootstrap::Bootstrap) -> Self {
        let bootstrap::Bootstrap {
            users,
            posts,
            files,
            ids,
            indexed,
            notifications,
        } = bootstrap;
        Self {
            users: users.into(),
            posts: posts.into(),
            files,
            ids,
            indexed,
            notifications: notifications.into_iter().map(Notification::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::notification;
    use nexus_common::models::post::{self, PostKind};
    use nexus_common::models::user;
    use serde_json::{json, Value};

    /// Every key of `value` named `name`, at any depth.
    fn keys_named(value: &Value, name: &str) -> usize {
        match value {
            Value::Object(fields) => fields
                .iter()
                .map(|(key, value)| usize::from(key == name) + keys_named(value, name))
                .sum(),
            Value::Array(items) => items.iter().map(|item| keys_named(item, name)).sum(),
            _ => 0,
        }
    }

    /// The `/v1` bootstrap is the stored one with each list converted: the users' counts lose
    /// `bookmarks`, the posts lose `bookmark` and name their kind the `/v1` way, the
    /// notifications name `post_kind` the `/v1` way; `files`, `ids` and `indexed` are as stored.
    #[test]
    fn lists_take_their_v1_shape_and_the_rest_is_as_stored() {
        let stored_user: user::UserView = serde_json::from_value(json!({
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
                "collections": 0, "following": 0, "followers": 0, "friends": 0, "bookmarks": 2
            },
            "tags": [],
            "relationship": { "following": false, "followed_by": false },
            "social_graph_status": null
        }))
        .unwrap();
        let stored = bootstrap::Bootstrap {
            users: user::UserStream(vec![stored_user]),
            posts: post::PostStream(vec![post::PostView {
                bookmark: Some(post::Bookmark {
                    id: "bookmark1".into(),
                    indexed_at: 1,
                }),
                ..Default::default()
            }]),
            files: Vec::new(),
            ids: BootstrapIds {
                stream: vec!["author:post".into()],
                ..Default::default()
            },
            indexed: true,
            notifications: vec![notification::Notification {
                timestamp: 7,
                body: notification::NotificationBody::Mention {
                    mentioned_by: "a".into(),
                    post_uri: "u".into(),
                    post_kind: PostKind::Long,
                },
            }],
        };
        let mut v0 = serde_json::to_value(&stored).unwrap();
        let v1 = serde_json::to_value(Bootstrap::from(stored)).unwrap();

        assert_eq!(v0["users"][0]["counts"]["bookmarks"], 2, "{v0}");
        assert_eq!(v0["posts"][0]["bookmark"]["id"], "bookmark1", "{v0}");
        assert_eq!(keys_named(&v1, "bookmarks"), 0, "{v1}");
        assert_eq!(keys_named(&v1, "bookmark"), 0, "{v1}");
        assert_eq!(v1["posts"][0]["details"]["kind"], "note", "{v1}");
        assert_eq!(v1["notifications"][0]["body"]["post_kind"], "article");
        assert_eq!(v1["ids"]["stream"], json!(["author:post"]));
        assert_eq!(v1["indexed"], true);
        assert_eq!(v1["files"], json!([]));

        // Written the `/v1` way, the stored payload is the same document.
        v0["users"][0]["counts"]
            .as_object_mut()
            .unwrap()
            .remove("bookmarks");
        v0["posts"][0].as_object_mut().unwrap().remove("bookmark");
        v0["posts"][0]["details"]["kind"] = json!("note");
        v0["notifications"][0]["body"]["post_kind"] = json!("article");
        assert_eq!(v1, v0);
    }
}
