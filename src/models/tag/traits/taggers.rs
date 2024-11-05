use super::DynError;
use crate::{routes::v0::queries::PaginationQuery, RedisOps};
use axum::async_trait;

// TODO: There is another struct with the same name. model/tag/stream
pub type Taggers = Vec<String>;

#[async_trait]
pub trait TaggersCollection
where
    Self: RedisOps + AsRef<[String]>,
{
    /// Retrieves taggers associated with a given user ID and label.
    /// # Arguments
    /// * `user_id` - The ID of the user.
    /// * `extra_param` - An optional parameter for additional context (e.g., post ID or other context).
    /// * `label` - The tag label to filter the taggers.
    /// * `skip` - Optional number of taggers to skip for pagination.
    /// * `limit` - Optional limit for the number of taggers to retrieve, defaults to 40.
    ///
    /// # Returns
    /// A result containing a vector of taggers or an error.
    async fn get_tagger_by_id(
        user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        pagination: PaginationQuery,
    ) -> Result<Option<Taggers>, DynError> {
        // Set default params for pagination
        let skip = pagination.skip.unwrap_or(0);
        let limit = pagination.limit.unwrap_or(40);

        let key_parts = Self::create_label_index(user_id, extra_param, label);
        Self::get_from_index(key_parts, Some(skip), Some(limit)).await
    }

    async fn get_from_index(
        key_parts: Vec<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Taggers>, DynError> {
        Ok(Self::try_from_index_set(&key_parts, skip, limit).await?)
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
    ) -> Vec<&'a str> {
        match extra_param {
            Some(extra_id) => vec![user_id, extra_id, label],
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
