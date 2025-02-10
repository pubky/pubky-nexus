use crate::types::{DynError, Pagination};
use crate::RedisOps;
use async_trait::async_trait;

use super::collection::CACHE_SET_PREFIX;

// TODO: There is another struct with the same name. model/tag/stream
pub type Taggers = Vec<String>;

#[async_trait]
pub trait TaggersCollection
where
    Self: RedisOps + AsRef<[String]>,
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
    ///   1. **Checking if the viewer is in the taggers list**.
    ///   2. **Retrieving Web of Trust (WoT) tags** when combined with `depth`.
    /// * `depth` - An optional depth parameter, used to determine the distance in WoT relationships.
    ///
    /// # Returns
    /// A result containing an `Option<(Taggers, bool)>`:
    /// - `Some((taggers, is_member))` where:
    ///   - `taggers` is the retrieved list of taggers.
    ///   - `is_member` is `true` if `viewer_id` is in the taggers list, otherwise `false`.
    /// - `None` if no taggers are available.
    /// - An error if the retrieval process fails.
    async fn get_tagger_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        pagination: Pagination,
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> Result<Option<(Taggers, bool)>, DynError> {
        // Set default params for pagination
        let skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(40);
        let mut prefix = None;
        let key_parts;
        // Get WoT tags. If we do not first hit the graph using `TagUser::get_by_id` function
        // for example using, user/{user_id}/tags?viewer_id={viewer_id}&depth={distance} endpoint
        // we get empty array because it was not cached the WoT tags
        if viewer_id.is_some() && depth.is_some() && extra_param.is_none() {
            prefix = Some(CACHE_SET_PREFIX.to_string());
            key_parts = Self::create_label_index(user_id, viewer_id, label, true);
        } else {
            key_parts = Self::create_label_index(user_id, extra_param, label, false);
        }

        Self::get_from_index(key_parts, viewer_id, Some(skip), Some(limit), prefix).await
    }

    async fn get_from_index(
        key_parts: Vec<&str>,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
        prefix: Option<String>,
    ) -> Result<Option<(Taggers, bool)>, DynError> {
        let taggers = Self::try_from_index_set(&key_parts, skip, limit, prefix).await?;
        if let Some(taggers) = taggers {
            let is_member = match viewer_id {
                Some(member) => Self::check_set_member(&key_parts, member).await?.1,
                None => false,
            };
            return Ok(Some((taggers, is_member)));
        }
        Ok(None)
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
    ) -> Result<(), DynError> {
        let key = match extra_param {
            Some(post_id) => vec![author_id, post_id, tag_label],
            None => vec![author_id, tag_label],
        };
        self.remove_from_index_set(&key).await
    }
}
