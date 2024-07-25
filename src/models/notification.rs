use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::RedisOps;

#[derive(Serialize, Deserialize, ToSchema)]
pub enum NotificationType {
    Follow,
    NewFriend,
    LostFriend,
    TagPost,
    TagProfile,
    Mention,
    Reply,
    Repost,
    PostDeleted,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub enum PostDeleteType {
    Reply,       // A reply to you was deleted.
    Repost,      // A repost of your post was deleted.
    ReplyParent, // The parent post of your reply was deleted.
    RepostEmbed, // The embedded post of your repost was deleted.
    ThreadRoot,  // The root post of the thread of your reply was deleted.
    ThreadReply, // A reply on the thread of your root post was deleted.
    TaggedPost,  // A post you tagged was deleted.
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Notification {
    pub timestamp: i64,
    pub notification_type: NotificationType,
    pub body: NotificationBody,
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[serde(tag = "type")]
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
    },
    TagProfile {
        tagged_by: String,
        tag: String,
    },
    Reply {
        replied_by: String,
        parent_post_uri: String,
        reply_uri: String,
    },
    Repost {
        reposted_by: String,
        embed_uri: String,
        repost_uri: String,
    },
    PostDeleted {
        delete_type: PostDeleteType,
        deleted_by: String,
        deleted_uri: String,
        linked_uri: String,
    },
}

impl RedisOps for Notification {}

impl Notification {
    pub fn new(notification_type: NotificationType, body: NotificationBody) -> Self {
        Self {
            notification_type,
            body,
            timestamp: Utc::now().timestamp_millis(), //milliseconds to avoid sub second collision
        }
    }

    pub async fn index_notification(
        user_id: &str,
        notification: Notification,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        notification
            .set_index(&[user_id, &notification.timestamp])
            .await
    }

    pub async fn list(
        user_id: &str,
        limit: usize,
        start: Option<String>,
        end: Option<String>,
    ) -> Result<Vec<Notification>, Box<dyn std::error::Error + Send + Sync>> {
        let start = start.unwrap_or_else(|| Utc::now().to_rfc3339());
        let end = end.unwrap_or_else(|| "0".to_string());
        let keys = index::get_keys(user_id, &start, &end, limit).await?;
        let mut notifications = Vec::new();

        for key in keys {
            if let Some(notification) = Notification::try_from_index(&[&key]).await? {
                notifications.push(notification);
            }
        }
        Ok(notifications)
    }

    pub async fn new_follow(
        user_id: &str,
        contact_id: &str,
        new_friend: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::Follow {
            followed_by: user_id.to_string(),
        };
        let notification = Notification::new(NotificationType::Follow, body.clone());
        Self::index_notification(contact_id, notification).await?;

        if new_friend {
            let notification = Notification::new(NotificationType::NewFriend, body);
            Self::index_notification(contact_id, notification).await?;
        }

        Ok(())
    }

    pub async fn lost_follow(
        user_id: &str,
        contact_id: &str,
        was_friend: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::LostFriend {
            unfollowed_by: user_id.to_string(),
        };
        let notification = Notification::new(NotificationType::LostFriend, body);
        if was_friend {
            Self::index_notification(contact_id, notification).await?;
        }

        Ok(())
    }

    pub async fn new_post_tag(
        user_id: &str,
        author_id: &str,
        tag: &str,
        post_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == author_id {
            return Ok(());
        }
        let body = NotificationBody::TagPost {
            tagged_by: user_id.to_string(),
            tag: tag.to_string(),
            post_uri: post_uri.to_string(),
        };
        let notification = Notification::new(NotificationType::TagPost, body);
        Self::index_notification(author_id, notification).await
    }

    pub async fn new_profile_tag(
        user_id: &str,
        profile_id: &str,
        tag: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::TagProfile {
            tagged_by: user_id.to_string(),
            tag: tag.to_string(),
        };
        let notification = Notification::new(NotificationType::TagProfile, body);
        Self::index_notification(profile_id, notification).await
    }

    pub async fn new_post_reply(
        user_id: &str,
        parent_uri: &str,
        reply_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parent_post_author = index::get_author_id(parent_uri).await?;
        if user_id == parent_post_author {
            return Ok(());
        }
        let body = NotificationBody::Reply {
            replied_by: user_id.to_string(),
            parent_post_uri: parent_uri.to_string(),
            reply_uri: reply_uri.to_string(),
        };
        let notification = Notification::new(NotificationType::Reply, body);
        Self::index_notification(&parent_post_author, notification).await
    }

    pub async fn new_repost(
        user_id: &str,
        embed_uri: &str,
        repost_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embed_post_author = index::get_author_id(embed_uri).await?;
        if user_id == embed_post_author {
            return Ok(());
        }
        let body = NotificationBody::Repost {
            reposted_by: user_id.to_string(),
            embed_uri: embed_uri.to_string(),
            repost_uri: repost_uri.to_string(),
        };
        let notification = Notification::new(NotificationType::Repost, body);
        Self::index_notification(&embed_post_author, notification).await
    }

    pub async fn deleted_post(
        user_id: &str,
        linked_uri: &str,
        deleted_uri: &str,
        delete_type: PostDeleteType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let linked_post_author = index::get_author_id(linked_uri).await?;
        if user_id == linked_post_author {
            return Ok(());
        }
        let body = NotificationBody::PostDeleted {
            delete_type,
            deleted_by: user_id.to_string(),
            deleted_uri: deleted_uri.to_string(),
            linked_uri: linked_uri.to_string(),
        };
        let notification = Notification::new(NotificationType::PostDeleted, body);
        Self::index_notification(&linked_post_author, notification).await
    }
}
