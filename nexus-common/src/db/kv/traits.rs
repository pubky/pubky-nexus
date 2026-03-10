use super::index::*;
use crate::types::DynError;
use async_trait::async_trait;
use json::JsonAction;
use serde::{de::DeserializeOwned, Serialize};
use sorted_sets::{ScoreAction, SortOrder, SORTED_PREFIX};

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
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the value is stored
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    /// * `expiration` - An optional `i64` specifying the TTL (in seconds) for the set. If `None`, no TTL will be set.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn put_index_json(
        &self,
        key_parts: &[&str],
        prefix: Option<String>,
        expiration: Option<i64>,
    ) -> Result<(), DynError> {
        let prefix = prefix.unwrap_or(Self::prefix().await);
        json::put(&prefix, &key_parts.join(":"), self, None, expiration).await
    }

    /// Retrieves data from Redis using the provided key parts.
    ///
    /// This method deserializes the data stored under the key generated from the provided `key_parts` in Redis.
    /// If the key is not found, it returns `None`.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the value is stored.
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
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
        prefix: Option<String>,
    ) -> Result<Option<Self>, DynError> {
        let prefix = prefix.unwrap_or(Self::prefix().await);
        json::get(&prefix, &key_parts.join(":"), None).await
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
    async fn try_from_index_multiple_json(
        key_parts_list: &[&[&str]],
    ) -> Result<Vec<Option<Self>>, DynError> {
        let prefix = Self::prefix().await;
        let keys: Vec<String> = key_parts_list
            .iter()
            .map(|key_parts| key_parts.join(":"))
            .collect();

        json::get_multiple(&prefix, &keys, None).await
    }

    /// Retrieves multiple JSON objects from Redis using direct keys (mget operation).
    ///
    /// This method provides a convenient way to batch retrieve multiple objects by their direct keys
    /// in a single Redis call, improving performance over individual get operations.
    ///
    /// # Arguments
    ///
    /// * `keys` - A slice of strings representing the direct keys to retrieve from Redis.
    ///
    /// # Returns
    ///
    /// A `Vec<Option<Self>>` containing the deserialized data if found, or `None` if a key does not exist.
    async fn mget(keys: &[impl AsRef<str> + Send + Sync]) -> Result<Vec<Option<Self>>, DynError> {
        let prefix = Self::prefix().await;
        json::get_multiple(&prefix, keys, None).await
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
    ) -> Result<(), DynError> // The items in the collection must be serializable
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
    async fn remove_from_index_multiple_json(key_parts_list: &[&[&str]]) -> Result<(), DynError> {
        let prefix = Self::prefix().await;
        let keys: Vec<String> = key_parts_list
            .iter()
            .map(|key_parts| key_parts.join(":"))
            .collect();

        json::del_multiple(&prefix, &keys).await
    }

    /// Modifies a numeric field in a Redis JSON object by either incrementing or decrementing it.
    ///
    /// This method performs an operation on a numeric field in Redis JSON at the given path,
    /// either incrementing or decrementing it based on the `JsonAction` provided.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices representing the parts used to form the key under which the JSON object is stored.
    /// * `field` - A string slice representing the field to be modified in the JSON object.
    /// * `action` - A `JsonAction` enum that specifies whether to increment or decrement the field.
    ///
    /// # Returns
    ///
    /// Returns a result indicating success or failure.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or the field is not numeric.
    async fn modify_json_field(
        key_parts: &[&str],
        field: &str,
        action: JsonAction,
    ) -> Result<(), DynError> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        json::modify_json_field(&prefix, &key, field, action, None).await
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
    async fn put_index_list<T>(&self, key_parts: &[&str]) -> Result<(), DynError>
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
    ) -> Result<Option<Vec<String>>, DynError> {
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
    /// * `expiration` - An optional `i64` specifying the TTL (in seconds) for the set. If `None`, no TTL will be set.
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable or
    /// if there is an issue with serialization.
    async fn put_index_set(
        key_parts: &[&str],
        values: &[&str],
        expiration: Option<i64>,
        prefix: Option<String>,
    ) -> Result<(), DynError> {
        let prefix = prefix.unwrap_or(Self::prefix().await);
        let key = key_parts.join(":");
        // Store the values in the Redis set
        sets::put(&prefix, &key, values, expiration).await
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
    async fn remove_from_index_set<T>(&self, key_parts: &[&str]) -> Result<(), DynError>
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
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
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
        prefix: Option<String>,
    ) -> Result<Option<Vec<String>>, DynError> {
        let combined_prefix = match prefix {
            Some(p) => format!("{}:{}", p, Self::prefix().await),
            None => Self::prefix().await,
        };
        let key = key_parts.join(":");
        sets::get_range(&combined_prefix, &key, skip, limit).await
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
    async fn check_set_member(key_parts: &[&str], member: &str) -> Result<(bool, bool), DynError> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        sets::check_member(&prefix, &key, member).await
    }

    /// Retrieves the size of a Redis set using the provided key parts.
    ///
    /// This method retrieves the number of elements in a Redis set stored under the key generated from the provided `key_parts`.
    /// It returns `Ok(Some(size))` if the set exists, or `Ok(None)` if the set does not exist.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the set is stored.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(size))` where `size` is the number of elements in the set, or `Ok(None)` if the set does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn get_set_size(key_parts: &[&str]) -> Result<Option<usize>, DynError> {
        let prefix = Self::prefix().await;
        let key = key_parts.join(":");
        sets::get_size(&prefix, &key).await
    }

    /// Fetches multiple sets from Redis using the specified key components.
    ///
    /// # Arguments
    /// * `key_parts_list` - A slice of string slices, where each inner slice represents the components
    ///   used to construct the Redis keys.
    /// * `prefix` - An optional string representing the prefix for the Redis keys
    /// * `member` - An optional string reference representing a specific element to check for member in each SET
    /// * `limit` - An optional parameter specifying the maximum number of elements to fetch from each SET
    ///   If `None`, all elements will be retrieved.
    async fn try_from_multiple_sets(
        key_parts_list: &[&str],
        prefix: Option<String>,
        member: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<Option<(Vec<String>, usize, bool)>>, DynError> {
        let combined_prefix = match prefix {
            Some(p) => format!("{}:{}", p, Self::prefix().await),
            None => Self::prefix().await,
        };
        sets::get_multiple_sets(&combined_prefix, key_parts_list, member, limit).await
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
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    /// * `expiration` - An optional `i64` specifying the TTL (in seconds) for the set. If `None`, no TTL will be set.
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
        prefix: Option<String>,
        expiration: Option<i64>,
    ) -> Result<(), DynError> {
        // Ensure the lengths of keys_refs and collections_refs match
        if index.len() != collections_refs.len() {
            // TODO: Maybe create redis related errors
            return Err("Keys refs and collections refs length mismatch".into());
        }
        let combined_prefix = match prefix {
            Some(p) => format!("{}:{}", p, Self::prefix().await),
            None => Self::prefix().await,
        };

        let refs: Vec<&[&str]> = collections_refs
            .iter()
            .map(|inner_vec| inner_vec.as_slice())
            .collect();
        let slice: &[&[&str]] = refs.as_slice();

        sets::put_multiple_sets(&combined_prefix, common_key, index, slice, expiration).await
    }

    /// Retrieves random elements from a Redis set using the provided key parts.
    ///
    /// This method fetches random elements from a Redis set stored under the key generated from the provided `key_parts`.
    /// The number of elements retrieved is defined by the `count` parameter.
    /// # Arguments
    ///
    /// * `key_parts` - Components of the key under which the set is stored.
    /// * `count` - The number of random elements to retrieve.
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(Vec<String>))` if the set exists and random elements are retrieved.
    /// Returns `Ok(None)` if the set does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the Redis operation fails.
    async fn try_get_random_from_index_set(
        key_parts: &[&str],
        count: isize,
        prefix: Option<String>,
    ) -> Result<Option<Vec<String>>, DynError> {
        let prefix = prefix.unwrap_or(Self::prefix().await);
        let key = key_parts.join(":");
        sets::get_random_members(&prefix, &key, count).await
    }

    // ############################################################
    // ########### SORTED SET related functions ###################
    // ############################################################

    /// Checks if a member exists in a Redis sorted set and retrieves its score if it exists.
    ///
    /// This method checks if a specific member is present in the Redis sorted set stored under the key
    /// generated from the provided `key_parts`. If the member is found, it returns its score.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `member` - A slice of string slices that represent the parts used to form the key identifying the member within the sorted set.
    async fn check_sorted_set_member(
        prefix: Option<&str>,
        key_parts: &[&str],
        member: &[&str],
    ) -> Result<Option<isize>, DynError> {
        let prefix = prefix.unwrap_or(SORTED_PREFIX);
        let key = key_parts.join(":");
        let member_key = member.join(":");
        sorted_sets::check_member(prefix, &key, &member_key).await
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
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    /// * `expiration` - An optional `i64` specifying the TTL (in seconds) for the set. If `None`, no TTL will be set.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn put_index_sorted_set(
        key_parts: &[&str],
        elements: &[(f64, &str)],
        prefix: Option<&str>,
        expiration: Option<i64>,
    ) -> Result<(), DynError> {
        let prefix = prefix.unwrap_or(SORTED_PREFIX);
        let key = key_parts.join(":");
        // Store the elements in the Redis sorted set
        sorted_sets::put(prefix, &key, elements, expiration).await
    }

    /// Updates the score of a member in a Redis sorted set.
    ///
    /// This method updates the score associated with a specific member in a Redis sorted set
    /// identified by the provided key parts. The score can be mutated (incremented, decremented, or set) based on the `score_mutation` parameter.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `member` - A slice of string slices that represent the parts used to form the key identifying the member within the sorted set.
    /// * `score_mutation` - A `ScoreAction` that defines how the score should be modified (e.g., incremented or decremented).
    async fn put_score_index_sorted_set(
        key_parts: &[&str],
        member: &[&str],
        score_mutation: ScoreAction,
    ) -> Result<(), DynError> {
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
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `items` - A slice of string slices representing the elements to be removed from the sorted set.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn remove_from_index_sorted_set(
        prefix: Option<&str>,
        key_parts: &[&str],
        items: &[&str],
    ) -> Result<(), DynError> {
        if items.is_empty() {
            return Ok(());
        }

        let prefix = prefix.unwrap_or(SORTED_PREFIX);
        // Create the key by joining the key parts
        let key = key_parts.join(":");
        // Call the sorted_sets::del function to remove the items from the sorted set
        sorted_sets::del(prefix, &key, items).await
    }

    /// Retrieves a range of elements from a Redis sorted set using the provided key parts.
    ///
    /// This method fetches elements from a Redis sorted set stored under the key generated from the provided `key_parts`.
    /// The range is defined by `skip` and `limit` parameters.
    ///
    /// # Arguments
    ///
    /// * `key_parts` - A slice of string slices that represent the parts used to form the key under which the sorted set is stored.
    /// * `start` - An optional value representing the beginning of the stream timeframe or score. If `None`, no lower bound is applied.
    /// * `end` - An optional value representing the end of the stream timeframe or score. If `None`, no upper bound is applied.
    /// * `skip` - An optional number of elements to skip (useful for pagination).
    /// * `limit` - An optional number of elements to return (useful for pagination).
    /// * `prefix` - An optional string representing the prefix for the Redis keys. If `Some(String)`, the prefix will be used
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
        sorting: SortOrder,
        prefix: Option<&str>,
    ) -> Result<Option<Vec<(String, f64)>>, DynError> {
        let key = key_parts.join(":");
        let prefix = prefix.unwrap_or("Sorted");

        sorted_sets::get_range(prefix, &key, end, start, skip, limit, sorting).await
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
    /// * `skip` - An optional number of elements to skip (useful for pagination).
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
    ) -> Result<Option<Vec<String>>, DynError> {
        let key = key_parts.join(":");
        sorted_sets::get_lex_range("Sorted", &key, min, max, skip, limit).await
    }
}
