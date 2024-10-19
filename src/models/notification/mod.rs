use crate::{db::kv::index::sorted_sets::Sorting, RedisOps};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PostChangedType {
    Reply,       // Implemented: Del / Edit. A reply to you was deleted/edited.
    Repost,      // Implemented: Del. A repost of your post was deleted/edited.
    Bookmark,    // A post you bookmarked was deleted/edited.
    ReplyParent, // Implemented: Del. The parent post of your reply was deleted/edited.
    RepostEmbed, // The embedded post of your repost was deleted/edited.
    ThreadReply, // A reply on the thread of your root post was deleted/edited.
    TaggedPost,  // A post you tagged was deleted/edited.
}

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Notification {
    pub timestamp: i64,
    pub body: NotificationBody,
}

#[derive(Serialize, Deserialize, Clone, ToSchema, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
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
    Mention {
        mentioned_by: String,
        post_uri: String,
    },
    PostDeleted {
        delete_type: PostChangedType,
        deleted_by: String,
        deleted_uri: String,
        linked_uri: String,
    },
    PostEdited {
        edit_type: PostChangedType,
        edited_by: String,
        edited_uri: String,
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
    async fn put_to_index(
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
        let body = match new_friend {
            true => NotificationBody::NewFriend {
                followed_by: user_id.to_string(),
            },
            false => NotificationBody::Follow {
                followed_by: user_id.to_string(),
            },
        };

        let notification = Notification::new(body);
        notification.put_to_index(followee_id).await?;

        Ok(())
    }

    pub async fn lost_follow(
        user_id: &str,
        followee_id: &str,
        were_friends: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !were_friends {
            return Ok(());
        }

        let body = NotificationBody::LostFriend {
            unfollowed_by: user_id.to_string(),
        };
        let notification = Notification::new(body);
        notification.put_to_index(followee_id).await?;

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
        notification.put_to_index(author_id).await
    }

    pub async fn new_user_tag(
        tagger_user_id: &str,
        tagged_user_id: &str,
        label: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if tagger_user_id == tagged_user_id {
            return Ok(());
        }
        let body = NotificationBody::TagProfile {
            tagged_by: tagger_user_id.to_string(),
            tag_label: label.to_string(),
        };
        let notification = Notification::new(body);
        notification.put_to_index(tagged_user_id).await
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
        notification.put_to_index(parent_post_author).await
    }

    pub async fn new_mention(
        user_id: &str,
        mentioned_id: &str,
        post_id: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        if user_id == mentioned_id {
            return Ok(None);
        }
        let body = NotificationBody::Mention {
            mentioned_by: user_id.to_string(),
            post_uri: format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}"),
        };
        let notification = Notification::new(body);
        notification.put_to_index(mentioned_id).await?;

        Ok(Some(mentioned_id.to_string()))
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
        notification.put_to_index(embed_post_author).await
    }

    pub async fn deleted_post(
        user_id: &str,
        linked_uri: &str,
        linked_post_author: &str,
        deleted_uri: &str,
        delete_type: PostChangedType,
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
        notification.put_to_index(linked_post_author).await
    }

    pub async fn edited_post(
        user_id: &str,
        linked_uri: &str,
        linked_post_author: &str,
        edited_uri: &str,
        edit_type: PostChangedType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if user_id == linked_post_author {
            return Ok(());
        }
        let body = NotificationBody::PostEdited {
            edit_type,
            edited_by: user_id.to_string(),
            edited_uri: edited_uri.to_string(),
            linked_uri: linked_uri.to_string(),
        };
        let notification = Notification::new(body);
        notification.put_to_index(linked_post_author).await
    }
}
