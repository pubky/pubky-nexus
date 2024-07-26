use std::ops::Deref;

use chrono::Utc;
use redis::{AsyncCommands, AsyncIter, JsonAsyncCommands};
use serde::{Deserialize, Serialize};
use crate::db::connectors::redis::get_redis_conn;
use crate::RedisOps;
use crate::db::kv::index;

pub mod user;

impl RedisOps for Tags {}
//impl RedisOps for TagList {}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tag {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: i64,
    tagger_id: String, // TODO#35: or maybe user_id name?
}

impl Default for Tag {
    fn default() -> Self {
        Self {
            tag_id: String::new(),
            indexed_at: Utc::now().timestamp(),
            tagger_id: String::new(),
        }
    }
}

// Define a newtype wrapper
#[derive(Serialize, Deserialize, Debug)]
pub struct Tags(Vec<Tag>);

impl Default for Tags {
    fn default() -> Self {
        Tags(Vec::new())
    }
}

// Implement Deref so TagList can be used like Vec<String>
impl Deref for Tags {
    type Target = Vec<Tag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Tags {
    async fn type_name() -> String {
        Self::prefix().await
    }

    // TODO#35: Try to use RedisOps, with struct Tags(Vec<Tag>)
    pub async fn set_index(key:&str, tag_list: &Tags) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        index::set(
            &Self::type_name().await,
            key,
            tag_list,
            None, 
            None)
        .await?; 
        Ok(())
    }

    pub async fn search_keys_with_pattern(word: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let key_prefix = &Self::type_name().await;
        let pattern = format!("{}{:}*", key_prefix, word);

        // TODO#35: find some way to keep open the connection
        let mut redis_conn = get_redis_conn().await?;

        // Search base on regular expression
        let mut iter: AsyncIter<Option<String>> = redis_conn.scan_match(&pattern).await.unwrap();
        let mut tags_keys: Vec<String> = vec![];

        while let Some(wrapped_key) = iter.next_item().await {
            if let Some(key) = wrapped_key {    
                tags_keys.push(key);
            }
        }
        Ok(tags_keys)
    }

    pub async fn search_key_value(key:&str) -> Result<Tags, Box<dyn std::error::Error + Send + Sync>> {
        let mut redis_conn = get_redis_conn().await?;
        let value: String = redis_conn.json_get(&key, ".").await.unwrap();
        let by: Tags = serde_json::from_str(&value)?;
        Ok(by)
    }
}

