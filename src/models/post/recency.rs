use crate::db::connectors::redis::get_redis_conn;
use chrono::Utc;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::error::Error;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct PostRecencyIndex {
    poster_id: String,
    post_id: String,
    indexed_at: f64, // Using f64 to store the timestamp
}

impl PostRecencyIndex {
    pub fn new(poster_id: &str, post_id: &str, indexed_at: f64) -> Self {
        Self {
            poster_id: poster_id.to_string(),
            post_id: post_id.to_string(),
            indexed_at,
        }
    }

    pub fn key(&self) -> String {
        format!("{}:{}", self.poster_id, self.post_id)
    }
}
