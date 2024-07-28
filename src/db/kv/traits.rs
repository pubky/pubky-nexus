use super::index::{self, RangeReturnType};
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::error::Error;

/// A trait for operations involving Redis storage. Implement this trait for types that need to be stored
/// and retrieved from Redis with serialization and deserialization capabilities.
#[async_trait]
pub trait RedisOps: Serialize + DeserializeOwned + Send + Sync {
    /// Provides a prefix string for the Redis key.
    ///
    /// This method should return a prefix string that helps namespace the keys in Redis,
    /// preventing key collisions. The prefix is typically derived from the struct name.
    ///
    /// # Returns
    ///
    /// A `String` representing the prefix for Redis keys.
    async fn prefix() -> String {
        let type_name = std::any::type_name::<Self>();
        let struct_name = type_name.split("::").last().unwrap_or_default();
        String::from(struct_name)
    }

    async fn try_from_bool_index(
        key_parts: &[&str],
    ) -> Result<Option<bool>, Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        index::get_bool(&prefix, &key).await
    }

    /// Sets the data in Redis using the provided key parts.
    ///
    /// This method serializes the data and stores it in Redis under the key generated
    /// from the provided `key_parts`. It can also set an expiration time for the key if required.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the value is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn set_index(&self, key_parts: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>> {
        index::set(
            &Self::prefix().await,
            &key_parts.join(":"),
            self,
            None,
            None,
        )
        .await
    }

    /// Sets multiple indexes in Redis using the provided list of key parts for each value in the collection.
    ///
    /// This method serializes each item in the collection and stores it in Redis under the keys generated
    /// from the provided `key_parts_list`. It supports setting multiple key-value pairs efficiently.
    ///
    /// # Arguments
    ///
    /// * `key_parts_list` - A slice of slices, where each inner slice contains string slices representing
    ///   the parts used to form the key under which the corresponding value in the collection is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with serialization.
    async fn set_multiple_indexes<T>(
        &self,
        key_parts_list: &[&[&str]],
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        Self: AsRef<[T]>,           // Assuming Self can be dereferenced into a slice of T
        T: Serialize + Send + Sync, // The items in the collection must be serializable
    {
        let collection = self.as_ref();

        let mut data = Vec::with_capacity(key_parts_list.len());
        for (i, key_parts) in key_parts_list.iter().enumerate() {
            let key = key_parts.join(":");
            data.push((key, &collection[i]));
        }

        index::set_multiple(&Self::prefix().await, &data).await
    }

    /// Retrieves data from Redis using the provided key parts.
    ///
    /// This method deserializes the data stored under the key generated from the provided `key_parts` in Redis.
    /// If the key is not found, it returns `None`.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the value is stored.
    ///
    /// # Returns
    ///
    /// An `Option<Self>` containing the deserialized data if found, or `None` if the key does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index(
        key_parts: &[&str],
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        index::get(&Self::prefix().await, &key_parts.join(":"), None).await
    }

    /// TODO: THIS IS PROB NOT OK, NOT USEFUL AS IS
    /// Retrieves a range of data from Redis using a pattern to match keys, with optional pagination.
    ///
    /// This method fetches and deserializes data stored under keys matching the provided `pattern`.
    /// It supports pagination through the `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `pattern` - An optional string slice representing the pattern to match keys. If not provided,
    ///   defaults to "*" which matches all keys under the prefix.
    /// * `skip` - An optional number of keys to skip (useful for pagination).
    /// * `limit` - An optional number of keys to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of deserialized values if they exist, or an empty vector if no matching keys are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with deserialization.
    async fn try_from_index_range(
        pattern: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<Self>>, Box<dyn Error + Send + Sync>>
    where
        Self: Sized,
    {
        let values = index::get_range::<Self>(&Self::prefix().await, pattern, skip, limit).await?;

        Ok(Some(values))
    }

    /// TODO: THIS IS PROB NOT OK, NOT USEFUL AS IS
    /// Retrieves a range of boolean values from Redis using a pattern to match keys, with optional pagination.
    ///
    /// This method fetches boolean values stored under keys matching the provided `pattern`.
    /// It supports pagination through the `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `pattern` - An optional string slice representing the pattern to match keys. If not provided,
    ///   defaults to "*" which matches all keys under the prefix.
    /// * `skip` - An optional number of keys to skip (useful for pagination).
    /// * `limit` - An optional number of keys to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of boolean values if they exist, or an empty vector if no matching keys are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with retrieving the data.
    async fn try_from_bool_index_range(
        pattern: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<bool>>, Box<dyn Error + Send + Sync>> {
        let (_, values) = index::get_bool_range(
            &Self::prefix().await,
            pattern,
            skip,
            limit,
            RangeReturnType::Values,
        )
        .await?;

        // If values are found, return them; otherwise, return an empty vector.
        Ok(values)
    }
}
