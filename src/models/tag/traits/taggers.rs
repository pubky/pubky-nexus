use axum::async_trait;
use crate::RedisOps;
use super::DynError;

// TODO: There is another struct with the same name. model/tag/stream
pub type Taggers = Vec<String>;

#[async_trait]
pub trait TaggersCollection
where
    Self: RedisOps,
{
    async fn try_from_index(
        user_id: &str,
        extra_param: Option<&str>,
        label: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Taggers>, DynError> {
        let skip = skip.unwrap_or(0);
        let limit = limit.unwrap_or(40);
        let key_parts = Self::create_label_index(user_id, extra_param, label);
        Ok(Self::try_from_index_set(&key_parts, Some(skip), Some(limit)).await?)
        
    }

    /// Constructs an index key based on user key, an optional extra parameter and a tag label.
    /// # Arguments
    /// * user_id - The key of the user.
    /// * extra_param - An optional parameter for specifying additional context (e.g., an post_id)
    /// * label - The label of the tag.
    /// # Returns
    /// A string representing the index key.
    fn create_label_index<'a>(user_id: &'a str, extra_param: Option<&'a str>, label: &'a str) -> Vec<&'a str> {
        match extra_param {
            Some(extra_id) => vec![user_id, extra_id, label],
            None => vec![user_id, label],
        }
    }
}