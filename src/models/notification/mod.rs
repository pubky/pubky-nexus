use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{db::kv::index::sorted_sets::Sorting, RedisOps};

#[derive(Serialize, Deserialize, ToSchema, Clone)]
pub enum PostDeleteType {
    Reply,       // A reply to you was deleted.
    Repost,      // A repost of your post was deleted.
    ReplyParent, // The parent post of your reply was deleted.
    RepostEmbed, // The embedded post of your repost was deleted.
    ThreadRoot,  // The root post of the thread of your reply was deleted.
    ThreadReply, // A reply on the thread of your root post was deleted.
    TaggedPost,  // A post you tagged was deleted.
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct Notification {
    pub timestamp: i64,
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
        tag_label: String,
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

impl Default for NotificationBody {
    fn default() -> Self {
        NotificationBody::Follow {
            followed_by: String::new(),
        }
    }
}

impl RedisOps for Notification {}

impl Notification {
    pub fn new(body: NotificationBody) -> Self {
        Self {
            body,
            timestamp: Utc::now().timestamp_millis(), //milliseconds to avoid sub second collision
        }
    }

    /// Stores the `NotificationBody` in the sorted set for the user using the timestamp as the score.
    pub async fn to_index(
        &self,
        user_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let notification_body_json = serde_json::to_string(&self.body)?;
        let score = self.timestamp as f64;

        Notification::put_index_sorted_set(
            &["Notification", user_id],
            &[(score, notification_body_json.as_str())],
        )
        .await
    }

    /// Lists notifications from the sorted set for the user, based on skip and limit, or timestamp range.
    pub async fn get_by_id(
        user_id: &str,
        limit: Option<usize>,
        skip: Option<usize>,
        start: Option<f64>, // Timestamp as f64 for range query
        end: Option<f64>,
    ) -> Result<Vec<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let notifications = Notification::try_from_index_sorted_set(
            &["Notification", user_id],
            start,
            end,
            skip,
            limit,
            Sorting::Descending, // Sorting in descending order by score (timestamp)
        )
        .await?;

        let mut result = Vec::new();
        if let Some(notifications) = notifications {
            for (notification_body_str, score) in notifications {
                if let Ok(body) = serde_json::from_str::<NotificationBody>(&notification_body_str) {
                    let notification = Notification {
                        timestamp: score as i64,
                        body,
                    };
                    result.push(notification);
                }
            }
        }

        Ok(result)
    }

    pub async fn new_follow(
        user_id: &str,
        followee_id: &str,
        new_friend: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::Follow {
            followed_by: user_id.to_string(),
        };
        let notification = Notification::new(body.clone());
        notification.to_index(followee_id).await?;

        if new_friend {
            let body = NotificationBody::NewFriend {
                followed_by: user_id.to_string(),
            };
            let notification = Notification::new(body);
            notification.to_index(followee_id).await?;
        }

        Ok(())
    }

    pub async fn lost_follow(
        user_id: &str,
        followee_id: &str,
        was_friend: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::LostFriend {
            unfollowed_by: user_id.to_string(),
        };
        let notification = Notification::new(body);
        if was_friend {
            notification.to_index(followee_id).await?;
        }

        Ok(())
    }

    pub async fn new_post_tag(
        user_id: &str,
        author_id: &str,
        label: &str,
        post_uri: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == author_id {
            return Ok(());
        }
        let body = NotificationBody::TagPost {
            tagged_by: user_id.to_string(),
            tag_label: label.to_string(),
            post_uri: post_uri.to_string(),
        };
        let notification = Notification::new(body);
        notification.to_index(author_id).await
    }

    pub async fn new_profile_tag(
        user_id: &str,
        profile_id: &str,
        label: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let body = NotificationBody::TagProfile {
            tagged_by: user_id.to_string(),
            tag_label: label.to_string(),
        };
        let notification = Notification::new(body);
        notification.to_index(profile_id).await
    }

    pub async fn new_post_reply(
        user_id: &str,
        parent_uri: &str,
        reply_uri: &str,
        parent_post_author: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == parent_post_author {
            return Ok(());
        }
        let body = NotificationBody::Reply {
            replied_by: user_id.to_string(),
            parent_post_uri: parent_uri.to_string(),
            reply_uri: reply_uri.to_string(),
        };
        let notification = Notification::new(body);
        notification.to_index(parent_post_author).await
    }

    pub async fn new_repost(
        user_id: &str,
        embed_uri: &str,
        repost_uri: &str,
        embed_post_author: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == embed_post_author {
            return Ok(());
        }
        let body = NotificationBody::Repost {
            reposted_by: user_id.to_string(),
            embed_uri: embed_uri.to_string(),
            repost_uri: repost_uri.to_string(),
        };
        let notification = Notification::new(body);
        notification.to_index(embed_post_author).await
    }

    pub async fn deleted_post(
        user_id: &str,
        linked_uri: &str,
        linked_post_author: &str,
        deleted_uri: &str,
        delete_type: PostDeleteType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == linked_post_author {
            return Ok(());
        }
        let body = NotificationBody::PostDeleted {
            delete_type,
            deleted_by: user_id.to_string(),
            deleted_uri: deleted_uri.to_string(),
            linked_uri: linked_uri.to_string(),
        };
        let notification = Notification::new(body);
        notification.to_index(linked_post_author).await
    }
}
