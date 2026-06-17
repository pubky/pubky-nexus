mod errors;

use crate::db::{kv::RedisResult, RedisOps};
use serde::{Deserialize, Serialize};
use tracing::error;

pub use errors::EventProcessorError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLine(pub String);

#[async_trait::async_trait]
impl RedisOps for EventLine {
    async fn prefix() -> String {
        "Event".to_string()
    }
}

impl AsRef<[String]> for EventLine {
    fn as_ref(&self) -> &[String] {
        std::slice::from_ref(&self.0)
    }
}

impl EventLine {
    #[tracing::instrument(name = "event.index.write", skip_all)]
    pub async fn store(&self) -> RedisResult<()> {
        self.put_index_list(&["Events"]).await
    }

    pub async fn get_events_from_redis(
        cursor: Option<u64>,
        limit: usize,
    ) -> RedisResult<(Vec<String>, u64)> {
        let start = cursor.unwrap_or(0);
        let start_u = usize::try_from(start).unwrap_or(usize::MAX);
        let result = EventLine::try_from_index_list(&["Events"], Some(start_u), Some(limit)).await;

        let events = match result {
            Ok(r) => r.unwrap_or_default(),
            Err(error) => {
                error!("IndexReadFailed: Failed to read from list due to Redis error: {error}");
                return Err(error);
            }
        };

        let next_cursor = start + events.len() as u64;
        Ok((events, next_cursor))
    }
}
