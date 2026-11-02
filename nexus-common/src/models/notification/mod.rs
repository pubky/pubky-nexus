use crate::db::graph::keyset_scan_composite;
use crate::db::kv::{RedisError, RedisResult, SortOrder};
use crate::db::{fetch_all_rows_from_graph, queries, RedisOps};
use crate::models::error::{ModelError, ModelResult};
use crate::types::Pagination;
use chrono::Utc;
use neo4rs::Row;
use pubky_app_specs::{bookmark_uri_builder, PubkyAppPostKind, PubkyId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

#[derive(Clone)]
pub enum PostChangedType {
    Edited,
    Deleted,
}

fn unknown_post_kind() -> PubkyAppPostKind {
    PubkyAppPostKind::Unknown
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
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    TagProfile {
        tagged_by: String,
        tag_label: String,
    },
    UntagPost {
        untagged_by: String,
        tag_label: String,
        post_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    UntagProfile {
        untagged_by: String,
        tag_label: String,
    },
    Reply {
        replied_by: String,
        parent_post_uri: String,
        reply_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    Repost {
        reposted_by: String,
        embed_uri: String,
        repost_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    Mention {
        mentioned_by: String,
        post_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    PostDeleted {
        delete_source: PostChangedSource,
        deleted_by: String,
        deleted_uri: String,
        linked_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
    PostEdited {
        edit_source: PostChangedSource,
        edited_by: String,
        edited_uri: String,
        linked_uri: String,
        #[serde(default = "unknown_post_kind")]
        post_kind: PubkyAppPostKind,
    },
}

/// The `type` tag of a [`NotificationBody`], used to filter a notification list.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    Follow,
    NewFriend,
    LostFriend,
    TagPost,
    TagProfile,
    UntagPost,
    UntagProfile,
    Reply,
    Repost,
    Mention,
    PostDeleted,
    PostEdited,
}

impl std::str::FromStr for NotificationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "follow" => Self::Follow,
            "new_friend" => Self::NewFriend,
            "lost_friend" => Self::LostFriend,
            "tag_post" => Self::TagPost,
            "tag_profile" => Self::TagProfile,
            "untag_post" => Self::UntagPost,
            "untag_profile" => Self::UntagProfile,
            "reply" => Self::Reply,
            "repost" => Self::Repost,
            "mention" => Self::Mention,
            "post_deleted" => Self::PostDeleted,
            "post_edited" => Self::PostEdited,
            other => return Err(format!("unknown notification type: {other}")),
        })
    }
}

impl NotificationBody {
    pub fn notification_type(&self) -> NotificationType {
        match self {
            Self::Follow { .. } => NotificationType::Follow,
            Self::NewFriend { .. } => NotificationType::NewFriend,
            Self::LostFriend { .. } => NotificationType::LostFriend,
            Self::TagPost { .. } => NotificationType::TagPost,
            Self::TagProfile { .. } => NotificationType::TagProfile,
            Self::UntagPost { .. } => NotificationType::UntagPost,
            Self::UntagProfile { .. } => NotificationType::UntagProfile,
            Self::Reply { .. } => NotificationType::Reply,
            Self::Repost { .. } => NotificationType::Repost,
            Self::Mention { .. } => NotificationType::Mention,
            Self::PostDeleted { .. } => NotificationType::PostDeleted,
            Self::PostEdited { .. } => NotificationType::PostEdited,
        }
    }
}

const NOTIFICATION_FANOUT_PAGE_SIZE: i64 = 1_000;

/// Entries read from Redis per round when filtering by type.
const NOTIFICATION_FILTER_CHUNK: usize = 500;
/// Upper bound on entries scanned for one filtered request, so a rare or absent
/// type cannot make a request walk a whole history.
pub const NOTIFICATION_FILTER_MAX_SCAN: usize = 10_000;

type QueryFunction = fn(&str, &str, &[String], i64) -> crate::db::graph::Query;
type ExtractFunction = Arc<dyn Fn(&Row) -> (String, String) + Send + Sync>;

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
    async fn put_to_index(&self, user_id: &str) -> RedisResult<()> {
        let notification_body_json = serde_json::to_string(&self.body)
            .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;
        let score = self.timestamp as f64;

        Notification::put_index_sorted_set(
            &["Notification", user_id],
            &[(score, notification_body_json.as_str())],
            None,
            None,
        )
        .await
    }

    /// Like [`Self::put_to_index`], but keeps an identical notification already in
    /// the set, timestamp included. The body carries no timestamp, so the event that
    /// created it can be replayed (retry, recovery) without re-surfacing it.
    async fn put_to_index_once(&self, user_id: &str) -> RedisResult<()> {
        let notification_body_json = serde_json::to_string(&self.body)
            .map_err(|e| RedisError::SerializationFailed(Box::new(e)))?;
        let score = self.timestamp as f64;

        Notification::add_index_sorted_set_if_absent(
            &["Notification", user_id],
            score,
            notification_body_json.as_str(),
            None,
        )
        .await
    }

    /// Lists notifications from the sorted set for the user, based on skip and limit, or timestamp range.
    pub async fn get_by_id(user_id: &str, pagination: Pagination) -> RedisResult<Vec<Self>> {
        // Set the default params for pagination
        let skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(20);

        let rows = Notification::try_from_index_sorted_set(
            &["Notification", user_id],
            pagination.start,
            pagination.end,
            Some(skip),
            Some(limit),
            SortOrder::Descending, // StreamSorting in descending order by score (timestamp)
            None,
        )
        .await?
        .unwrap_or_default();

        Ok(rows.into_iter().filter_map(Self::from_row).collect())
    }

    /// Like [`Self::get_by_id`], keeping only notifications whose type is in `types`.
    /// `skip` and `limit` count matching notifications. The set is read newest-first
    /// in chunks and the scan stops after [`NOTIFICATION_FILTER_MAX_SCAN`] entries.
    ///
    /// Returns the page and, when the scan stopped at that cap before filling the
    /// page, the `start` to resume from (one below the oldest timestamp scanned),
    /// so a short or empty page is never mistaken for the end of the history.
    pub async fn get_by_id_filtered(
        user_id: &str,
        pagination: Pagination,
        types: &[NotificationType],
    ) -> RedisResult<(Vec<Self>, Option<i64>)> {
        let mut to_skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(20);
        let mut start = pagination.start;
        let mut result = Vec::new();
        let mut offset = 0;
        let mut oldest_scanned: Option<f64> = None;
        let mut exhausted = false;

        while result.len() < limit && offset < NOTIFICATION_FILTER_MAX_SCAN {
            let chunk = NOTIFICATION_FILTER_CHUNK.min(NOTIFICATION_FILTER_MAX_SCAN - offset);
            let rows = Notification::try_from_index_sorted_set(
                &["Notification", user_id],
                start,
                pagination.end,
                Some(offset),
                Some(chunk),
                SortOrder::Descending,
                None,
            )
            .await?
            .unwrap_or_default();
            let read = rows.len();
            if read < chunk {
                exhausted = true;
            }
            // Pin the upper bound to the newest row seen, so a notification written
            // mid-scan cannot shift the offsets and repeat a row in the next chunk.
            if start.is_none() {
                start = rows.first().map(|(_, score)| *score);
            }
            if let Some((_, score)) = rows.last() {
                oldest_scanned = Some(*score);
            }
            offset += read;

            for notification in rows.into_iter().filter_map(Self::from_row) {
                if !types.contains(&notification.body.notification_type()) {
                    continue;
                }
                if to_skip > 0 {
                    to_skip -= 1;
                    continue;
                }
                result.push(notification);
                if result.len() == limit {
                    break;
                }
            }

            if exhausted {
                break;
            }
        }

        let next_start = match (result.len() < limit && !exhausted, oldest_scanned) {
            (true, Some(score)) => Some(score as i64 - 1),
            _ => None,
        };

        Ok((result, next_start))
    }

    fn from_row((body_str, score): (String, f64)) -> Option<Self> {
        match serde_json::from_str::<NotificationBody>(&body_str) {
            Ok(body) => Some(Notification {
                timestamp: score as i64,
                body,
            }),
            Err(e) => {
                tracing::warn!("Failed to deserialize notification, body: {body_str}, reason: {e}");
                None
            }
        }
    }

    pub async fn new_follow(user_id: &str, followee_id: &str, new_friend: bool) -> RedisResult<()> {
        let body = match new_friend {
            true => NotificationBody::NewFriend {
                followed_by: user_id.to_string(),
            },
            false => NotificationBody::Follow {
                followed_by: user_id.to_string(),
            },
        };

        let notification = Notification::new(body);
        notification.put_to_index(followee_id).await
    }

    pub async fn lost_follow(
        user_id: &str,
        followee_id: &str,
        were_friends: bool,
    ) -> RedisResult<()> {
        if !were_friends {
            return Ok(());
        }

        let body = NotificationBody::LostFriend {
            unfollowed_by: user_id.to_string(),
        };
        let notification = Notification::new(body);
        notification.put_to_index(followee_id).await
    }

    pub async fn new_post_tag(
        user_id: &str,
        author_id: &str,
        label: &str,
        post_uri: &str,
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<()> {
        if user_id == author_id {
            return Ok(());
        }
        let body = NotificationBody::TagPost {
            tagged_by: user_id.to_string(),
            tag_label: label.to_string(),
            post_uri: post_uri.to_string(),
            post_kind,
        };
        let notification = Notification::new(body);
        notification.put_to_index(author_id).await
    }

    pub async fn new_user_tag(
        tagger_user_id: &str,
        tagged_user_id: &str,
        label: &str,
    ) -> RedisResult<()> {
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

    pub async fn new_post_untag(
        user_id: &str,
        author_id: &str,
        label: &str,
        post_uri: &str,
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<()> {
        if user_id == author_id {
            return Ok(());
        }
        let body = NotificationBody::UntagPost {
            untagged_by: user_id.to_string(),
            tag_label: label.to_string(),
            post_uri: post_uri.to_string(),
            post_kind,
        };
        let notification = Notification::new(body);
        notification.put_to_index(author_id).await
    }

    pub async fn new_user_untag(
        tagger_user_id: &str,
        tagged_user_id: &str,
        label: &str,
    ) -> RedisResult<()> {
        if tagger_user_id == tagged_user_id {
            return Ok(());
        }
        let body = NotificationBody::UntagProfile {
            untagged_by: tagger_user_id.to_string(),
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
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<()> {
        if user_id == parent_post_author {
            return Ok(());
        }
        let body = NotificationBody::Reply {
            replied_by: user_id.to_string(),
            parent_post_uri: parent_uri.to_string(),
            reply_uri: reply_uri.to_string(),
            post_kind,
        };
        let notification = Notification::new(body);
        notification.put_to_index_once(parent_post_author).await
    }

    pub async fn new_mention(
        user_id: &PubkyId,
        mentioned_id: &PubkyId,
        post_uri: &str,
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<Option<PubkyId>> {
        if user_id == mentioned_id {
            return Ok(None);
        }
        let body = NotificationBody::Mention {
            mentioned_by: user_id.to_string(),
            post_uri: post_uri.to_string(),
            post_kind,
        };
        let notification = Notification::new(body);
        notification.put_to_index_once(mentioned_id).await?;

        Ok(Some(mentioned_id.clone()))
    }

    pub async fn new_repost(
        user_id: &str,
        embed_uri: &str,
        repost_uri: &str,
        embed_post_author: &str,
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<()> {
        if user_id == embed_post_author {
            return Ok(());
        }
        let body = NotificationBody::Repost {
            reposted_by: user_id.to_string(),
            embed_uri: embed_uri.to_string(),
            repost_uri: repost_uri.to_string(),
            post_kind,
        };
        let notification = Notification::new(body);
        notification.put_to_index_once(embed_post_author).await
    }

    pub async fn post_children_changed(
        user_id: &str,
        linked_uri: &str,
        linked_post_author: &str,
        changed_uri: &str,
        change_source: PostChangedSource,
        changed_type: &PostChangedType,
        post_kind: PubkyAppPostKind,
    ) -> RedisResult<()> {
        if user_id == linked_post_author {
            return Ok(());
        }
        let body = match changed_type {
            PostChangedType::Deleted => NotificationBody::PostDeleted {
                delete_source: change_source,
                deleted_by: user_id.to_string(),
                deleted_uri: changed_uri.to_string(),
                linked_uri: linked_uri.to_string(),
                post_kind,
            },
            PostChangedType::Edited => NotificationBody::PostEdited {
                edit_source: change_source,
                edited_by: user_id.to_string(),
                edited_uri: changed_uri.to_string(),
                linked_uri: linked_uri.to_string(),
                post_kind,
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
        post_kind: PubkyAppPostKind,
    ) -> ModelResult<()> {
        // Define the notification types and associated data
        let notification_types: Vec<(&str, QueryFunction, PostChangedSource, ExtractFunction)> = vec![
            (
                "get_post_replies",
                queries::get::get_post_replies as QueryFunction,
                PostChangedSource::ReplyParent,
                Arc::new(|row: &Row| {
                    let replier_id: &str = row.get("replier_id").unwrap_or_default();
                    let linked_uri: String = row.get("reply_uri").unwrap_or_default();
                    (replier_id.to_string(), linked_uri)
                }),
            ),
            (
                "get_post_tags",
                queries::get::get_post_tags as QueryFunction,
                PostChangedSource::TaggedPost,
                Arc::new(|row: &Row| {
                    let tagger_id: &str = row.get("tagger_id").unwrap_or_default();
                    let linked_uri: String = row.get("tag_uri").unwrap_or_default();
                    (tagger_id.to_string(), linked_uri)
                }),
            ),
            (
                "get_post_bookmarks",
                queries::get::get_post_bookmarks as QueryFunction,
                PostChangedSource::Bookmark,
                Arc::new(|row: &Row| {
                    let bookmarker_id: &str = row.get("bookmarker_id").unwrap_or_default();
                    let bookmark_id: &str = row.get("bookmark_id").unwrap_or_default();
                    let linked_uri = bookmark_uri_builder(bookmarker_id.into(), bookmark_id.into());
                    (bookmarker_id.to_string(), linked_uri)
                }),
            ),
            (
                "get_post_reposts",
                queries::get::get_post_reposts as QueryFunction,
                PostChangedSource::RepostEmbed,
                Arc::new(|row: &Row| {
                    let reposter_id: &str = row.get("reposter_id").unwrap_or_default();
                    let linked_uri: String = row.get("repost_uri").unwrap_or_default();
                    (reposter_id.to_string(), linked_uri)
                }),
            ),
        ];

        let author_id = author_id.to_string();
        let post_id = post_id.to_string();
        let changed_uri = changed_uri.to_string();
        let changed_type = changed_type.clone();

        for (label, query_fn, post_changed_source, extract_fn) in notification_types {
            let author_id = author_id.clone();
            let post_id = post_id.clone();
            let changed_uri = changed_uri.clone();
            let changed_type = changed_type.clone();
            keyset_scan_composite(NOTIFICATION_FANOUT_PAGE_SIZE as usize, label, |cursor| {
                let author_id = author_id.clone();
                let post_id = post_id.clone();
                let changed_uri = changed_uri.clone();
                let changed_type = changed_type.clone();
                let kind = post_kind.clone();
                let post_changed_source = post_changed_source.clone();
                let extract_fn = Arc::clone(&extract_fn);
                async move {
                    let query =
                        query_fn(&author_id, &post_id, &cursor, NOTIFICATION_FANOUT_PAGE_SIZE);
                    let rows = fetch_all_rows_from_graph(query).await?;
                    let count = rows.len();
                    let mut last_cursor = None;
                    for row in &rows {
                        last_cursor = Some(row.get::<Vec<String>>("cursor")?);
                        let (user_id, linked_uri) = extract_fn(row);
                        if author_id == user_id {
                            // Do not notify the author themselves
                            continue;
                        }
                        let notification_body = match changed_type {
                            PostChangedType::Deleted => NotificationBody::PostDeleted {
                                delete_source: post_changed_source.clone(),
                                deleted_by: author_id.clone(),
                                deleted_uri: changed_uri.clone(),
                                linked_uri,
                                post_kind: kind.clone(),
                            },
                            PostChangedType::Edited => NotificationBody::PostEdited {
                                edit_source: post_changed_source.clone(),
                                edited_by: author_id.clone(),
                                edited_uri: changed_uri.clone(),
                                linked_uri,
                                post_kind: kind.clone(),
                            },
                        };
                        let notification = Notification::new(notification_body);
                        notification.put_to_index(&user_id).await?;
                    }
                    Ok::<(usize, Option<Vec<String>>), ModelError>((count, last_cursor))
                }
            })
            .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_rows_without_post_kind_default_to_unknown() {
        // A TagPost stored before `post_kind` existed must still deserialize.
        let old = r#"{"type":"tag_post","tagged_by":"a","tag_label":"l","post_uri":"u"}"#;
        let body: NotificationBody = serde_json::from_str(old).unwrap();
        match body {
            NotificationBody::TagPost { post_kind, .. } => {
                assert_eq!(post_kind, PubkyAppPostKind::Unknown);
            }
            other => panic!("expected TagPost, got {other:?}"),
        }
    }

    #[test]
    fn post_kind_round_trips_as_lowercase() {
        for (kind, wire) in [
            (PubkyAppPostKind::Collection, "collection"),
            (PubkyAppPostKind::Unknown, "unknown"),
        ] {
            let body = NotificationBody::TagPost {
                tagged_by: "a".into(),
                tag_label: "l".into(),
                post_uri: "u".into(),
                post_kind: kind.clone(),
            };
            let json = serde_json::to_string(&body).unwrap();
            assert!(
                json.contains(&format!(r#""post_kind":"{wire}""#)),
                "got {json}"
            );
            match serde_json::from_str::<NotificationBody>(&json).unwrap() {
                NotificationBody::TagPost { post_kind, .. } => assert_eq!(post_kind, kind),
                other => panic!("expected TagPost, got {other:?}"),
            }
        }
    }
}
