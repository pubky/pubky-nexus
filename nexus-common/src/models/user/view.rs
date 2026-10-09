use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Relationship, SocialGraphStatus, UserCounts, UserDetails, UserHomeserver};
use crate::db::RedisOps;
use crate::models::error::{ModelError, ModelResult};
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::tag::TagDetails;
use crate::types::WotDepth;
use crate::StackManager;
use futures::stream::{self, StreamExt};
use futures::TryStreamExt;

/// Represents a Pubky user with relational data including tags, counts, bookmark and relationship with other posts.
#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub struct UserView {
    pub details: UserDetails,
    pub counts: UserCounts,
    pub tags: Vec<TagDetails>,
    pub relationship: Relationship,
    /// How established the account is in the follow graph. `None` when no
    /// ranking is available, which is not the same as ranking as new.
    pub social_graph_status: Option<SocialGraphStatus>,
    /// Light mode only: the user's homeserver, where a client fetches the profile this
    /// Nexus does not keep. Absent in full mode, and when the user has no known one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homeserver: Option<UserHomeserver>,
}

impl UserView {
    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> ModelResult<Option<Self>> {
        // Perform all operations concurrently
        let (details, counts, relationship, social_graph_status) = tokio::try_join!(
            UserDetails::get_by_id(user_id),
            UserCounts::get_by_id(user_id),
            Relationship::get_by_id(user_id, viewer_id),
            // The others surface ModelError; this one is Redis-only.
            async {
                SocialGraphStatus::get_by_id(user_id)
                    .await
                    .map_err(ModelError::from)
            },
        )?;

        let Some(details) = details else {
            return Ok(None);
        };
        let counts = counts.unwrap_or_default();
        let relationship = relationship.unwrap_or_default();

        // Before fetching post tags, check if the post has any tags
        // Without this check, the index search will return a NONE because the tag index
        // doesn't exist, leading us to query the graph unnecessarily, assuming the data wasn't indexed
        let tags = match counts.tags {
            0 => Vec::new(),
            _ => TagUser::get_by_id(
                user_id,
                None,
                None,
                None,
                None,
                viewer_id,
                depth.and_then(|d| WotDepth::new(d).ok()),
            )
            .await?
            .unwrap_or_default(),
        };

        let homeserver = match StackManager::mode().is_light() {
            true => UserHomeserver::get_by_user_ids(&[user_id])
                .await?
                .pop()
                .flatten(),
            false => None,
        };

        Ok(Some(Self {
            details,
            counts,
            relationship,
            tags,
            social_graph_status,
            homeserver,
        }))
    }

    /// Retrieves multiple users by their IDs using batch Redis operations for better performance.
    ///
    /// This method uses the new `mget` operation to fetch user details and counts in bulk,
    /// significantly improving performance when retrieving multiple users.
    pub async fn get_by_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> ModelResult<Vec<Option<Self>>> {
        // Use mget to fetch all user details and counts in bulk. The social
        // graph lookup belongs here, batched, not in the per-user fan-out below.
        let (details_list, counts_list, social_graph_list) = tokio::try_join!(
            UserDetails::mget(user_ids),
            UserCounts::mget(user_ids),
            SocialGraphStatus::get_by_ids(user_ids),
        )?;
        // Light mode only, one graph query for the whole list.
        let homeserver_list = match StackManager::mode().is_light() {
            true => {
                let ids: Vec<&str> = user_ids.iter().map(String::as_str).collect();
                UserHomeserver::get_by_user_ids(&ids).await?
            }
            false => vec![None; user_ids.len()],
        };
        // Each returns one slot per id; the positional zip below relies on it.
        debug_assert_eq!(details_list.len(), user_ids.len());
        debug_assert_eq!(counts_list.len(), user_ids.len());
        debug_assert_eq!(social_graph_list.len(), user_ids.len());
        debug_assert_eq!(homeserver_list.len(), user_ids.len());

        // Bounded to protect the pool; `buffered` preserves order so results stay
        // aligned with `user_ids`; inputs owned so the future stays `Send`.
        let viewer_id = viewer_id.map(str::to_string);
        let user_views: Vec<Option<Self>> = stream::iter(
            user_ids
                .iter()
                .cloned()
                .zip(details_list)
                .zip(counts_list)
                .zip(social_graph_list)
                .zip(homeserver_list)
                .map(
                    |((((user_id, details), counts), social_graph_status), homeserver)| {
                        let viewer_id = viewer_id.clone();
                        async move {
                            let Some(details) = details else {
                                return Ok::<_, ModelError>(None);
                            };

                            let counts = counts.unwrap_or_default();
                            let relationship =
                                Relationship::get_by_id(&user_id, viewer_id.as_deref())
                                    .await?
                                    .unwrap_or_default();

                            // Before fetching post tags, check if the post has any tags
                            let tags = match counts.tags {
                                0 => Vec::new(),
                                _ => TagUser::get_by_id(
                                    &user_id,
                                    None,
                                    None,
                                    None,
                                    None,
                                    viewer_id.as_deref(),
                                    depth.and_then(|d| WotDepth::new(d).ok()),
                                )
                                .await?
                                .unwrap_or_default(),
                            };

                            Ok(Some(Self {
                                details,
                                counts,
                                relationship,
                                tags,
                                social_graph_status,
                                homeserver,
                            }))
                        }
                    },
                ),
        )
        .buffered(8)
        .try_collect()
        .await?;

        Ok(user_views)
    }
}
