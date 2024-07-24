pub mod info;
pub mod post;
pub mod profile;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

use crate::db::kv::index;

/// A trait for operations involving Redis storage. Implement this trait for types that need to be stored
/// and retrieved from Redis with serialization and deserialization capabilities.
#[async_trait]
pub trait RedisOps: Serialize + DeserializeOwned + Send + Sync {
    async fn prefix() -> String {
        let type_name = std::any::type_name::<Self>();
        let struct_name = type_name.split("::").last().unwrap_or_default();
        format!("{}!", struct_name)
    }

    /// Sets the data in Redis using the provided key.
    ///
    /// This method serializes the data and stores it in Redis under the given key.
    /// It can also set an expiration time for the key if required.
    ///
    /// # Arguments
    ///
    /// * `key` - A string slice that represents the key under which the value is stored.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn set_index(&self, key: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        index::set(&Self::prefix().await, key, self, None, None).await
    }

    /// Retrieves data from Redis using the provided key.
    ///
    /// This method deserializes the data stored under the given key in Redis. If the key is not found,
    /// it returns `None`.
    ///
    /// # Arguments
    ///
    /// * `key` - A string slice that represents the key under which the value is stored.
    ///
    /// # Returns
    ///
    /// An `Option<Self>` containing the deserialized data if found, or `None` if the key does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if the operation fails, such as if the Redis connection is unavailable.
    async fn try_from_index(
        key: &str,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        index::get(&Self::prefix().await, &key, None).await
    }
}
