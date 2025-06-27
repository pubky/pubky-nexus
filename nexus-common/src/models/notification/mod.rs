use crate::db::kv::SortOrder;
use crate::db::{get_neo4j_graph, queries, RedisOps};
use crate::types::DynError;
use crate::types::Pagination;
use chrono::Utc;
use neo4rs::Row;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PostChangedSource {
    Reply,       // A reply to you was deleted/edited.
    Repost,      // A repost of your post was deleted/edited.
    Bookmark,    // A post you bookmarked was deleted/edited.
    ReplyParent, // The parent post of your reply was deleted/edited.
    RepostEmbed, // The embedded post on your repost was deleted/edited.
    TaggedPost,  // A post you tagged was deleted/edited.
}

pub enum PostChangedType {
    Edited,
    Deleted,
}

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct Notification {
    pub timestamp: i64,
    pub body: NotificationBody,
}

#[derive(Serialize, Deserialize, Clone, ToSchema, Debug)]
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
        delete_source: PostChangedSource,
        deleted_by: String,
        deleted_uri: String,
        linked_uri: String,
    },
    PostEdited {
        edit_source: PostChangedSource,
        edited_by: String,
        edited_uri: String,
        linked_uri: String,
    },
}

type QueryFunction = fn(&str, &str) -> neo4rs::Query;
type ExtractFunction = Box<dyn Fn(&Row) -> (String, String) + Send>;

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
    async fn put_to_index(&self, user_id: &str) -> Result<(), DynError> {
        let notification_body_json = serde_json::to_string(&self.body)?;
        let score = self.timestamp as f64;

        Notification::put_index_sorted_set(
            &["Notification", user_id],
            &[(score, notification_body_json.as_str())],
            None,
            None,
        )
        .await
    }

    /// Lists notifications from the sorted set for the user, based on skip and limit, or timestamp range.
    pub async fn get_by_id(user_id: &str, pagination: Pagination) -> Result<Vec<Self>, DynError> {
        // Set the default params for pagination
        let skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(20);

        let notifications = Notification::try_from_index_sorted_set(
            &["Notification", user_id],
            pagination.start,
            pagination.end,
            Some(skip),
            Some(limit),
            SortOrder::Descending, // StreamSorting in descending order by score (timestamp)
            None,
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
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
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
    ) -> Result<(), DynError> {
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
    ) -> Result<Option<String>, DynError> {
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
    ) -> Result<(), DynError> {
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

    pub async fn post_children_changed(
        user_id: &str,
        linked_uri: &str,
        linked_post_author: &str,
        changed_uri: &str,
        change_source: PostChangedSource,
        changed_type: &PostChangedType,
    ) -> Result<(), DynError> {
        if user_id == linked_post_author {
            return Ok(());
        }
        let body = match changed_type {
            PostChangedType::Deleted => NotificationBody::PostDeleted {
                delete_source: change_source,
                deleted_by: user_id.to_string(),
                deleted_uri: changed_uri.to_string(),
                linked_uri: linked_uri.to_string(),
            },
            PostChangedType::Edited => NotificationBody::PostEdited {
                edit_source: change_source,
                edited_by: user_id.to_string(),
                edited_uri: changed_uri.to_string(),
                linked_uri: linked_uri.to_string(),
            },
        };
        let notification = Notification::new(body);
        notification.put_to_index(linked_post_author).await
    }

    // Delete and Edit post notifications to users who interacted

    // A post you replied/reposted/tagged/bookmarked was edited or deleted
    pub async fn changed_post(
        author_id: &str,
        post_id: &str,
        changed_uri: &str,
        changed_type: &PostChangedType,
    ) -> Result<(), DynError> {
        // Define the notification types and associated data
        let notification_types: Vec<(QueryFunction, PostChangedSource, ExtractFunction)> = vec![
            (
                queries::get::get_post_replies as QueryFunction,
                PostChangedSource::ReplyParent,
                Box::new(|row: &Row| {
                    let replier_id: &str = row.get("replier_id").unwrap_or_default();
                    let reply_id: &str = row.get("reply_id").unwrap_or_default();
                    let linked_uri = format!("pubky://{replier_id}/pub/pubky.app/posts/{reply_id}");
                    (replier_id.to_string(), linked_uri)
                }),
            ),
            (
                queries::get::get_post_tags as QueryFunction,
                PostChangedSource::TaggedPost,
                Box::new(|row: &Row| {
                    let tagger_id: &str = row.get("tagger_id").unwrap_or_default();
                    let tag_id: &str = row.get("tag_id").unwrap_or_default();
                    let linked_uri = format!("pubky://{tagger_id}/pub/pubky.app/tags/{tag_id}");
                    (tagger_id.to_string(), linked_uri)
                }),
            ),
            (
                queries::get::get_post_bookmarks as QueryFunction,
                PostChangedSource::Bookmark,
                Box::new(|row: &Row| {
                    let bookmarker_id: &str = row.get("bookmarker_id").unwrap_or_default();
                    let bookmark_id: &str = row.get("bookmark_id").unwrap_or_default();
                    let linked_uri =
                        format!("pubky://{bookmarker_id}/pub/pubky.app/bookmarks/{bookmark_id}");
                    (bookmarker_id.to_string(), linked_uri)
                }),
            ),
            (
                queries::get::get_post_reposts as QueryFunction,
                PostChangedSource::RepostEmbed,
                Box::new(|row: &Row| {
                    let reposter_id: &str = row.get("reposter_id").unwrap_or_default();
                    let repost_id: &str = row.get("repost_id").unwrap_or_default();
                    let linked_uri =
                        format!("pubky://{reposter_id}/pub/pubky.app/posts/{repost_id}");
                    (reposter_id.to_string(), linked_uri)
                }),
            ),
        ];

        for (query_fn, post_changed_source, extract_fn) in notification_types {
            let mut result;
            {
                let graph = get_neo4j_graph()?;
                let query = query_fn(author_id, post_id);

                let graph = graph.lock().await;
                result = graph.execute(query).await?;
            }

            while let Some(row) = result.next().await? {
                let (user_id, linked_uri) = extract_fn(&row);

                if author_id == user_id {
                    // Do not notify the author themselves
                    continue;
                }

                let notification_body = match changed_type {
                    PostChangedType::Deleted => NotificationBody::PostDeleted {
                        delete_source: post_changed_source.clone(),
                        deleted_by: author_id.to_string(),
                        deleted_uri: changed_uri.to_string(),
                        linked_uri,
                    },
                    PostChangedType::Edited => NotificationBody::PostEdited {
                        edit_source: post_changed_source.clone(),
                        edited_by: author_id.to_string(),
                        edited_uri: changed_uri.to_string(),
                        linked_uri,
                    },
                };

                let notification = Notification::new(notification_body);
                notification.put_to_index(&user_id).await?;
            }
        }
        Ok(())
    }
}
