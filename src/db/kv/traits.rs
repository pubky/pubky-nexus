use super::index::*;
use axum::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sorted_sets::Sorting;
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

        // Insert ":" before each uppercase letter except the first one
        let mut prefixed_name = String::new();
        let chars = struct_name.chars().peekable();

        for c in chars {
            if c.is_uppercase() && !prefixed_name.is_empty() {
                prefixed_name.push(':');
            }
            prefixed_name.push(c);
        }

        prefixed_name
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
    async fn put_index_json(&self, key_parts: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>> {
        json::put(
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
    async fn put_multiple_json_indexes(
        &self,
        key_parts_list: &[&[&str]],
        collection: Vec<Option<Self>>
    ) -> Result<(), Box<dyn Error + Send + Sync>> // The items in the collection must be serializable
    {
        let mut data = Vec::with_capacity(key_parts_list.len());
        for (i, key_parts) in key_parts_list.iter().enumerate() {
            let key = key_parts.join(":");
            data.push((key, &collection[i]));
        }

        json::put_multiple(&Self::prefix().await, &data).await
    }

    /// Adds elements to a Redis list using the provided key parts.
    ///
    /// This method serializes the data and appends it to a Redis list under the key generated
    /// from the provided `key_parts`.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the list is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with serialization.
    async fn put_index_list<T>(
        &self,
        key_parts: &[&str],
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        Self: AsRef<[T]>,            // Self can be dereferenced into a slice of T
        T: AsRef<str> + Send + Sync, // The items must be convertible to &str
    {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");

        // TODO: Unsafe. If re-indexed it will duplicate follower/following list entries.
        // Need reading, matching out the duplicates then storing. Inneficient.
        // Needs mode safety for double-write.

        // Directly use the string representations of items without additional serialization
        let collection = self.as_ref();
        let values: Vec<&str> = collection.iter().map(|item| item.as_ref()).collect();

        // Store the values in the Redis list
        lists::put(&prefix, &key, &values).await
    }

    /// Adds elements to a Redis set using the provided key parts.
    ///
    /// This method adds elements to a Redis set under the key generated from the provided `key_parts`.
    /// It ensures that each element in the set is unique.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with serialization.
    async fn put_index_set<T>(&self, key_parts: &[&str]) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        Self: AsRef<[T]>,            // Self can be dereferenced into a slice of T
        T: AsRef<str> + Send + Sync, // The items must be convertible to &str
    {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");

        // Directly use the string representations of items without additional serialization
        let collection = self.as_ref();
        let values: Vec<&str> = collection.iter().map(|item| item.as_ref()).collect();

        // Store the values in the Redis set
        sets::put(&prefix, &key, &values).await
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
    async fn try_from_index_json(
        key_parts: &[&str],
    ) -> Result<Option<Self>, Box<dyn Error + Send + Sync>> {
        json::get(&Self::prefix().await, &key_parts.join(":"), None).await
    }

    /// Retrieves multiple JSON objects from Redis using the provided key parts.
    ///
    /// This method deserializes the data stored under the keys generated from the provided `key_parts_list` in Redis.
    /// It returns a vector of options, where each option corresponds to the existence of the key in Redis.
    ///
    /// # Arguments
    ///
    /// * `key_parts_list` - A slice of slices, where each inner slice contains string slices representing
    ///   the parts used to form the key under which the corresponding value is stored.
    ///
    /// # Returns
    ///
    /// A `Vec<Option<Self>>` containing the deserialized data if found, or `None` if a key does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index_multiple_json(
        key_parts_list: &[&[&str]],
    ) -> Result<Vec<Option<Self>>, Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let keys: Vec<String> = key_parts_list
            .iter()
            .map(|key_parts| key_parts.join(":"))
            .collect();

        json::get_multiple(&prefix, &keys, None).await
    }

    /// Retrieves a range of elements from a Redis list using the provided key parts.
    ///
    /// This method fetches elements from a Redis list stored under the key generated from the provided `key_parts`.
    /// The range is defined by `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the list is stored.
    /// * `skip` - An optional number of elements to skip (useful for pagination).
    /// * `limit` - An optional number of elements to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of deserialized elements if they exist, or an empty vector if no matching elements are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index_list(
        key_parts: &[&str],
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        lists::get_range(&prefix, &key, skip, limit).await
    }

    /// Retrieves a range of elements from a Redis set using the provided key parts.
    ///
    /// This method fetches elements from a Redis set stored under the key generated from the provided `key_parts`.
    /// The range is defined by `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    /// * `skip` - An optional number of elements to skip (useful for pagination).
    /// * `limit` - An optional number of elements to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of deserialized elements if they exist, or an empty vector if no matching elements are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index_set(
        key_parts: &[&str],
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        sets::get_range(&prefix, &key, skip, limit).await
    }

    /// Checks if a member exists in a Redis set and if the set exists using the provided key parts.
    ///
    /// This method checks if a specific member is present in the Redis set stored under the key
    /// generated from the provided `key_parts`. It also determines if the set itself exists.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    /// * `member` - A string slice representing the member to check for existence in the set.
    ///
    /// # Returns
    ///
    /// Returns `Ok((true, true))` if the set exists and the member is in the set,
    /// `Ok((true, false))` if the set exists but the member is not in the set,
    /// `Ok((false, false))` if the set does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn check_set_member(
        key_parts: &[&str],
        member: &str,
    ) -> Result<(bool, bool), Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        sets::check_set_member(&prefix, &key, member).await
    }

    /// Adds elements to a Redis sorted set using the provided key parts.
    ///
    /// This method adds elements to a Redis sorted set under the key generated from the provided `key_parts`.
    /// The elements are associated with scores, which determine their order in the set.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `elements` - A slice of tuples where each tuple contains a reference to a string slice representing
    ///                the element and a f64 representing the score of the element.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn put_index_sorted_set(
        key_parts: &[&str],
        elements: &[(f64, &str)],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let key = key_parts.join(":");
        // Store the elements in the Redis sorted set
        sorted_sets::put("Sorted", &key, elements).await
    }

    /// Retrieves a range of elements from a Redis sorted set using the provided key parts.
    ///
    /// This method fetches elements from a Redis sorted set stored under the key generated from the provided `key_parts`.
    /// The range is defined by `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `skip` - An optional number of elements to skip (useful for pagination).
    /// * `limit` - An optional number of elements to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of tuples containing the elements and their scores if they exist, or an empty vector if no matching elements are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index_sorted_set(
        key_parts: &[&str],
        start: Option<f64>,
        end: Option<f64>,
        skip: Option<isize>,
        limit: Option<isize>,
        sorting: Sorting,
    ) -> Result<Option<Vec<(String, f64)>>, Box<dyn Error + Send + Sync>> {
        let key = key_parts.join(":");
        sorted_sets::get_range("Sorted", &key, start, end, skip, limit, sorting).await
    }
}
