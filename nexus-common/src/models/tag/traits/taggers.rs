use crate::db::kv::RedisResult;
use crate::db::RedisOps;
use crate::models::error::ModelResult;
use crate::models::tag::Taggers;
use crate::types::{Pagination, WotDepth};
use async_trait::async_trait;

use super::collection::{TagCollection, CACHE_SET_PREFIX, MAX_TAG_PAGE};

/// The taggers of a label, and the address of the viewer's tag file for it.
pub type TaggersTuple = (Taggers, Option<String>);

#[async_trait]
pub trait TaggersCollection
where
    Self: RedisOps + AsRef<[String]> + TagCollection,
{
    /// Retrieves taggers associated with a given user ID and label.
    ///
    /// This function queries taggers linked to a specified user and label,
    /// with optional parameters for pagination and viewer context.
    ///
    /// # Arguments
    /// * `user_id` - The ID of the user whose taggers are being retrieved.
    /// * `extra_param` - An optional parameter for additional context (e.g., post ID).
    /// * `label` - The tag label used to filter the taggers.
    /// * `pagination` - A struct containing optional pagination parameters (`skip` and `limit`).
    /// * `viewer_id` - An optional viewer ID, used for two purposes:
    ///   1. **Reading the viewer's tag address** for the label.
    ///   2. **Retrieving Web of Trust (WoT) tags** when combined with `depth`.
    /// * `depth` - An optional validated `WotDepth`; its presence (with `viewer_id`) selects the WoT-tagger index.
    ///
    /// # Returns
    /// A result containing `(Taggers, Option<String>)`:
    /// - `taggers` is the retrieved list of taggers (empty if no taggers are available).
    /// - `tag_uri` is the stored `uri` of the viewer's tag on the label, or `None` if
    ///   they haven't tagged it or there is no viewer.
    /// - An error if the retrieval process fails.
    async fn get_tagger_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        pagination: Pagination,
        viewer_id: Option<&str>,
        depth: Option<WotDepth>,
    ) -> ModelResult<TaggersTuple> {
        // Set default params for pagination
        let skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(40).min(MAX_TAG_PAGE);
        let is_wot = viewer_id.is_some() && depth.is_some() && extra_param.is_none();
        // Get WoT tags. If we do not first hit the graph using `TagUser::get_by_id` function
        // for example using, user/{user_id}/tags?viewer_id={viewer_id}&depth={distance} endpoint
        // we get empty array because it was not cached the WoT tags.
        // The WoT taggers sets leave the viewer out, so the membership is checked below
        // against the global taggers set instead.
        let (key_param, prefix, member) = if is_wot {
            (viewer_id, Some(CACHE_SET_PREFIX.to_string()), None)
        } else {
            (extra_param, None, viewer_id)
        };
        let key_parts =
            <Self as TaggersCollection>::create_label_index(user_id, key_param, label, is_wot);
        let (taggers, is_member) = <Self as TaggersCollection>::get_from_index(
            key_parts,
            member,
            Some(skip),
            Some(limit),
            prefix,
        )
        .await?;

        let tag_uri = match viewer_id {
            Some(viewer_id) => {
                let is_viewer_tagger = if is_wot {
                    let global_key = <Self as TaggersCollection>::create_label_index(
                        user_id,
                        extra_param,
                        label,
                        false,
                    );
                    let (exists, is_member) =
                        Self::check_set_member(&global_key, viewer_id).await?;
                    exists.then_some(is_member)
                } else {
                    Some(is_member)
                };
                let labels = vec![(label.to_string(), is_viewer_tagger)];
                Self::flagged_viewer_tag_uris(user_id, extra_param, viewer_id, labels)
                    .await?
                    .remove(label)
            }
            None => None,
        };
        Ok((taggers, tag_uri))
    }

    async fn get_from_index(
        key_parts: Vec<&str>,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
        prefix: Option<String>,
    ) -> RedisResult<(Taggers, bool)> {
        let taggers = Self::try_from_index_set(&key_parts, skip, limit, prefix).await?;
        let is_member = match viewer_id {
            Some(member) => Self::check_set_member(&key_parts, member).await?.1,
            None => false,
        };
        let users = taggers.unwrap_or_default();
        Ok((users, is_member))
    }

    /// Constructs an index key based on user key, an optional extra parameter and a tag label.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// * label - The label of the tag.
    /// # Returns
    /// A string representing the index key.
    fn create_label_index<'a>(
        user_id: &'a str,
        extra_param: Option<&'a str>,
        label: &'a str,
        is_cache: bool,
    ) -> Vec<&'a str> {
        match extra_param {
            Some(extra_id) => match is_cache {
                true => vec![extra_id, user_id, label],
                false => vec![user_id, extra_id, label],
            },
            None => vec![user_id, label],
        }
    }

    /// Remove a tagger from the label tagger list
    async fn del_from_index(
        &self,
        author_id: &str,
        extra_param: Option<&str>,
        tag_label: &str,
    ) -> RedisResult<()> {
        let key = match extra_param {
            Some(post_id) => vec![author_id, post_id, tag_label],
            None => vec![author_id, tag_label],
        };
        self.remove_from_index_set(&key).await
    }
}
