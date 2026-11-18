use nexus_common::models::notification::{self, PostChangedSource};
use serde::Serialize;
use utoipa::ToSchema;

use super::PostKind;

/// A notification, as `/v0`'s [`notification::Notification`] with the post kind under its `/v1`
/// name.
#[derive(Serialize, ToSchema, Debug)]
pub struct Notification {
    pub timestamp: i64,
    pub body: NotificationBody,
}

impl From<notification::Notification> for Notification {
    fn from(notification: notification::Notification) -> Self {
        Self {
            timestamp: notification.timestamp,
            body: notification.body.into(),
        }
    }
}

/// A notification's body, variant for variant `/v0`'s [`notification::NotificationBody`]: the
/// stored addresses are kept as they are, only `post_kind` is renamed.
#[derive(Serialize, ToSchema, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NotificationBody {
    Follow {
        followed_by: String,
    },
    NewFriend {
        followed_by: String,
    },
    LostFriend {
        unfollowed_by: String,
    },
    TagPost {
        tagged_by: String,
        tag_label: String,
        post_uri: String,
        post_kind: PostKind,
    },
    TagProfile {
        tagged_by: String,
        tag_label: String,
    },
    UntagPost {
        untagged_by: String,
        tag_label: String,
        post_uri: String,
        post_kind: PostKind,
    },
    UntagProfile {
        untagged_by: String,
        tag_label: String,
    },
    Reply {
        replied_by: String,
        parent_post_uri: String,
        reply_uri: String,
        post_kind: PostKind,
    },
    Repost {
        reposted_by: String,
        embed_uri: String,
        repost_uri: String,
        post_kind: PostKind,
    },
    Mention {
        mentioned_by: String,
        post_uri: String,
        post_kind: PostKind,
    },
    PostDeleted {
        delete_source: PostChangedSource,
        deleted_by: String,
        deleted_uri: String,
        linked_uri: String,
        post_kind: PostKind,
    },
    PostEdited {
        edit_source: PostChangedSource,
        edited_by: String,
        edited_uri: String,
        linked_uri: String,
        post_kind: PostKind,
    },
}

impl From<notification::NotificationBody> for NotificationBody {
    fn from(body: notification::NotificationBody) -> Self {
        use notification::NotificationBody as Stored;
        match body {
            Stored::Follow { followed_by } => Self::Follow { followed_by },
            Stored::NewFriend { followed_by } => Self::NewFriend { followed_by },
            Stored::LostFriend { unfollowed_by } => Self::LostFriend { unfollowed_by },
            Stored::TagPost {
                tagged_by,
                tag_label,
                post_uri,
                post_kind,
            } => Self::TagPost {
                tagged_by,
                tag_label,
                post_uri,
                post_kind: post_kind.into(),
            },
            Stored::TagProfile {
                tagged_by,
                tag_label,
            } => Self::TagProfile {
                tagged_by,
                tag_label,
            },
            Stored::UntagPost {
                untagged_by,
                tag_label,
                post_uri,
                post_kind,
            } => Self::UntagPost {
                untagged_by,
                tag_label,
                post_uri,
                post_kind: post_kind.into(),
            },
            Stored::UntagProfile {
                untagged_by,
                tag_label,
            } => Self::UntagProfile {
                untagged_by,
                tag_label,
            },
            Stored::Reply {
                replied_by,
                parent_post_uri,
                reply_uri,
                post_kind,
            } => Self::Reply {
                replied_by,
                parent_post_uri,
                reply_uri,
                post_kind: post_kind.into(),
            },
            Stored::Repost {
                reposted_by,
                embed_uri,
                repost_uri,
                post_kind,
            } => Self::Repost {
                reposted_by,
                embed_uri,
                repost_uri,
                post_kind: post_kind.into(),
            },
            Stored::Mention {
                mentioned_by,
                post_uri,
                post_kind,
            } => Self::Mention {
                mentioned_by,
                post_uri,
                post_kind: post_kind.into(),
            },
            Stored::PostDeleted {
                delete_source,
                deleted_by,
                deleted_uri,
                linked_uri,
                post_kind,
            } => Self::PostDeleted {
                delete_source,
                deleted_by,
                deleted_uri,
                linked_uri,
                post_kind: post_kind.into(),
            },
            Stored::PostEdited {
                edit_source,
                edited_by,
                edited_uri,
                linked_uri,
                post_kind,
            } => Self::PostEdited {
                edit_source,
                edited_by,
                edited_uri,
                linked_uri,
                post_kind: post_kind.into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::models::post::PostKind as StoredKind;
    use serde_json::{json, Value};

    /// Every stored body converts to the same JSON, with `post_kind` under its `/v1` name
    /// where the variant carries one and nothing else changed.
    #[test]
    fn bodies_differ_from_v0_only_in_the_kind_name() {
        use notification::NotificationBody as Stored;
        let who = "pubky://user/pub/social/v1/posts/P".to_string();
        let bodies = [
            Stored::Follow {
                followed_by: "a".into(),
            },
            Stored::NewFriend {
                followed_by: "a".into(),
            },
            Stored::LostFriend {
                unfollowed_by: "a".into(),
            },
            Stored::TagPost {
                tagged_by: "a".into(),
                tag_label: "l".into(),
                post_uri: who.clone(),
                post_kind: StoredKind::Short,
            },
            Stored::TagProfile {
                tagged_by: "a".into(),
                tag_label: "l".into(),
            },
            Stored::UntagPost {
                untagged_by: "a".into(),
                tag_label: "l".into(),
                post_uri: who.clone(),
                post_kind: StoredKind::Long,
            },
            Stored::UntagProfile {
                untagged_by: "a".into(),
                tag_label: "l".into(),
            },
            Stored::Reply {
                replied_by: "a".into(),
                parent_post_uri: who.clone(),
                reply_uri: who.clone(),
                post_kind: StoredKind::Short,
            },
            Stored::Repost {
                reposted_by: "a".into(),
                embed_uri: who.clone(),
                repost_uri: who.clone(),
                post_kind: StoredKind::Unknown,
            },
            Stored::Mention {
                mentioned_by: "a".into(),
                post_uri: who.clone(),
                post_kind: StoredKind::Image,
            },
            Stored::PostDeleted {
                delete_source: PostChangedSource::Reply,
                deleted_by: "a".into(),
                deleted_uri: who.clone(),
                linked_uri: who.clone(),
                post_kind: StoredKind::Long,
            },
            Stored::PostEdited {
                edit_source: PostChangedSource::TaggedPost,
                edited_by: "a".into(),
                edited_uri: who.clone(),
                linked_uri: who,
                post_kind: StoredKind::Collection,
            },
        ];
        for body in bodies {
            let mut v0 = serde_json::to_value(&body).unwrap();
            let v1 = serde_json::to_value(NotificationBody::from(body)).unwrap();
            if let Some(kind) = v0.get("post_kind").cloned() {
                let stored: StoredKind = serde_json::from_value(kind).unwrap();
                assert_eq!(v1["post_kind"], stored.v1_name(), "{v1}");
                v0["post_kind"] = Value::from(stored.v1_name());
            }
            assert_eq!(v1, v0);
        }
    }

    /// The timestamp travels with the body.
    #[test]
    fn notification_keeps_its_timestamp() {
        let stored = notification::Notification {
            timestamp: 42,
            body: notification::NotificationBody::Mention {
                mentioned_by: "a".into(),
                post_uri: "u".into(),
                post_kind: StoredKind::Short,
            },
        };
        assert_eq!(
            serde_json::to_value(Notification::from(stored)).unwrap(),
            json!({
                "timestamp": 42,
                "body": {"type": "mention", "mentioned_by": "a", "post_uri": "u", "post_kind": "note"}
            })
        );
    }
}
