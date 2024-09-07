use super::index::*;
use axum::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use sorted_sets::{ScoreAction, Sorting, SORTED_PREFIX};
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

    // ############################################################
    // ################# JSON related functions ###################
    // ############################################################

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

    async fn put_param_index_json(
        key_parts: &[&str],
        field: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Fetch the current value from Redis
        let json_entry: Option<Self> = Self::try_from_index_json(key_parts).await?;
        
        if let Some(instance) = json_entry {
            let value = json::put_json_param(instance, field, 1)?;
            // Deserialize the modified JSON back into the PostMetrics struct
            let incremented_instance: Self = serde_json::from_value(value)?;

            incremented_instance.put_index_json(key_parts).await?;
            return Ok(());

        }

        Err("Could not increment by one".into())
    }

    /// Stores multiple key-value pairs in Redis, where each key is constructed from the provided key parts
    /// and each value is an item from the given collection.
    ///
    /// This method serializes each item in the collection and stores it in Redis under keys generated
    /// by joining the elements of the corresponding slices in `key_parts_list`. It efficiently handles
    /// the setting of multiple key-value pairs in a single operation.
    ///
    /// # Arguments
    ///
    /// * `key_parts_list` - A slice of slices, where each inner slice is a list of string slices representing
    ///   the components used to generate the Redis key for the corresponding value in the `collection`.
    ///   Each slice in this list must align with the corresponding index in `collection`.
    ///
    /// * `collection` - A vector of `Option<Self>` representing the values to be stored in Redis. Each value is serialized
    ///   before being stored, and the vector should be of the same length as `key_parts_list`.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` indicating success or failure. A successful result means that
    /// all key-value pairs were successfully stored in Redis.
    async fn put_multiple_json_indexes(
        key_parts_list: &[&[&str]],
        collection: Vec<Option<Self>>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> // The items in the collection must be serializable
    {
        let mut data = Vec::with_capacity(key_parts_list.len());
        for (i, key_parts) in key_parts_list.iter().enumerate() {
            let key = key_parts.join(":");
            data.push((key, &collection[i]));
        }

        json::put_multiple(&Self::prefix().await, &data).await
    }

    /// Removes multiple JSON objects from Redis using the provided key parts.
    ///
    /// This method deletes the data stored under the keys generated from the provided `key_parts_list` in Redis.
    /// It returns a result indicating success or failure.
    ///
    /// # Arguments
    ///
    /// * `key_parts_list` - A slice of slices, where each inner slice contains string slices representing
    ///   the parts used to form the key under which the corresponding value is stored.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or failure. If successful, all keys are removed from Redis.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn remove_from_index_multiple_json(
        key_parts_list: &[&[&str]],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        let keys: Vec<String> = key_parts_list
            .iter()
            .map(|key_parts| key_parts.join(":"))
            .collect();

        json::del_multiple(&prefix, &keys).await
    }

    // ############################################################
    // ################# List related functions ###################
    // ############################################################

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

    // ############################################################
    // ################# SET related functions ###################
    // ############################################################

    /// Adds elements to a Redis set using the provided key parts.
    ///
    /// This method adds elements to a Redis set under the key generated from the provided `key_parts`.
    /// It ensures that each element in the set is unique.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    /// * `values` - A list of string that represents the value to add in the index
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with serialization.
    async fn put_index_set(key_parts: &[&str], values: &[String]) -> Result<(), Box<dyn Error + Send + Sync>>
    {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");

        let values_ref: Vec<&str> = values.iter().map(|id| id.as_str()).collect();

        // Store the values in the Redis set
        sets::put(&prefix, &key, &values_ref).await
    }

    /// Removes elements from a Redis set using the provided key parts.
    ///
    /// This method removes elements from a Redis set stored under the key generated from the provided `key_parts`.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    /// * `values` - A slice of string slices representing the elements to be removed from the set.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn remove_from_index_set<T>(
        &self,
        key_parts: &[&str],
    ) -> Result<(), Box<dyn Error + Send + Sync>>
    where
        Self: AsRef<[T]>,            // Self can be dereferenced into a slice of T
        T: AsRef<str> + Send + Sync, // The items must be convertible to &str
    {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");

        // Directly use the string representations of items without additional serialization
        let collection = self.as_ref();
        let values: Vec<&str> = collection.iter().map(|item| item.as_ref()).collect();

        // Remove the values from the Redis set
        sets::del(&prefix, &key, &values).await
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

    /// Fetches multiple sets from Redis using the specified key components.
    ///
    /// This asynchronous function retrieves multiple sets from Redis based on the provided key components.
    /// It returns a vector where each element is an optional vector containing the elements of the corresponding set.
    /// If a particular set does not exist, the corresponding position in the returned vector will be `None`.
    ///
    /// # Arguments
    ///
    /// * `key_parts_list` - A slice of string slices, where each inner slice represents the components
    ///   used to construct the Redis key for the corresponding set.
    /// * `limit` - An optional parameter specifying the maximum number of elements to fetch from each set.
    ///   If `None`, all elements will be retrieved.
    ///
    /// # Returns
    ///
    /// A `Vec<Option<Vec<String>>>` where:
    /// * Each inner `Vec<String>` contains the elements of a set retrieved from Redis.
    /// * `None` indicates that the set does not exist for the corresponding key.
    ///
    /// # Errors
    ///
    /// This function will return an error if the operation fails, such as in cases of a Redis connection issue.
    async fn try_from_multiple_sets(
        key_parts_list: &[&str],
        limit: Option<usize>,
    ) -> Result<Vec<Option<(Vec<String>, usize)>>, Box<dyn Error + Send + Sync>> {
        let prefix = Self::prefix().await;
        sets::get_multiple_sets(&prefix, key_parts_list, limit).await
    }

    /// Adds elements to multiple Redis sets using the provided keys and collections.
    ///
    /// This asynchronous function allows you to add elements to multiple Redis sets,
    /// with each set identified by a key generated from the `common_key` and `index_ref`.
    /// The function ensures that each element in each set is unique.
    ///
    /// # Arguments
    ///
    /// * `common_key` - A slice of string slices representing the common part of the Redis keys.
    ///   This will be combined with each element in `index` to generate the full Redis key.
    /// * `index` - A slice of string slices representing the unique identifiers to append to the `common_key` to form the full Redis keys.
    /// * `collections_refs` - A slice of vectors, where each inner vector contains elements to be added to the corresponding Redis set
    ///
    /// # Returns
    ///
    /// This function returns a `Result` indicating success or failure. A successful result means that
    /// all elements were successfully added to their respective Redis sets.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn put_multiple_set_indexes(
        common_key: &[&str],
        index: &[&str],
        collections_refs: &[Vec<&str>],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Ensure the lengths of keys_refs and collections_refs match
        if index.len() != collections_refs.len() {
            // TODO: Maybe create redis related errors
            return Err("Keys refs and collections refs length mismatch".into());
        }

        // Get the prefix for the Redis keys
        let prefix = Self::prefix().await;

        let refs: Vec<&[&str]> = collections_refs
            .iter()
            .map(|inner_vec| inner_vec.as_slice())
            .collect();
        let slice: &[&[&str]] = refs.as_slice();

        sets::put_multiple_sets(&prefix, common_key, index, slice).await
    }

    // ############################################################
    // ########### SORTED SET related functions ###################
    // ############################################################

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
        sorted_sets::put(SORTED_PREFIX, &key, elements).await
    }

    async fn put_score_index_sorted_set(
        key_parts: &[&str],
        member: &[&str],
        score_mutation: ScoreAction
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let key = key_parts.join(":");
        let member_key = member.join(":");
        sorted_sets::put_score(SORTED_PREFIX, &key, &member_key, score_mutation).await
    }

    /// Removes elements from a Redis sorted set using the provided key parts.
    ///
    /// This method removes the specified elements from the Redis sorted set identified by the key generated
    /// from the provided `key_parts`.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `items` - A slice of string slices representing the elements to be removed from the sorted set.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn remove_from_index_sorted_set(
        key_parts: &[&str],
        items: &[&str],
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        if items.is_empty() {
            return Ok(());
        }

        // Create the key by joining the key parts
        let key = key_parts.join(":");

        // Call the sorted_sets::del function to remove the items from the sorted set
        sorted_sets::del("Sorted", &key, items).await
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
        skip: Option<usize>,
        limit: Option<usize>,
        sorting: Sorting,
    ) -> Result<Option<Vec<(String, f64)>>, Box<dyn Error + Send + Sync>> {
        let key = key_parts.join(":");
        sorted_sets::get_range("Sorted", &key, start, end, skip, limit, sorting).await
    }

    /// Retrieves a lexicographical range of elements from a Redis sorted set using the provided key parts.
    ///
    /// This method fetches elements from a Redis sorted set stored under the key generated from the provided `key_parts`.
    /// The range is defined by `min` and `max` lexicographical bounds.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `min` - The minimum lexicographical bound (inclusive).
    /// * `max` - The maximum lexicographical bound (exclusive).
    /// * `limit` - An optional number of elements to return (useful for pagination).
    ///
    /// # Returns
    ///
    /// Returns a vector of elements if they exist, or an empty vector if no matching elements are found.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index_sorted_set_lex(
        key_parts: &[&str],
        min: &str,
        max: &str,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Option<Vec<String>>, Box<dyn Error + Send + Sync>> {
        let key = key_parts.join(":");
        sorted_sets::get_lex_range("Sorted", &key, min, max, skip, limit).await
    }
}
