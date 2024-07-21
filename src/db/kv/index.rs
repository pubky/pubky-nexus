use crate::db::connectors::redis::get_redis_conn;
use log::debug;
use redis::AsyncCommands;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub async fn set<T: Serialize>(
    prefix: &str,
    key: &str,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{prefix}{key}");

    let value_json = serde_json::to_string(value)?;

    redis_conn.set(&index_key, value_json).await?;
    debug!("Indexed key: {}", index_key);
    Ok(())
}

pub async fn get<T: DeserializeOwned>(
    prefix: &str,
    key: &str,
) -> Result<Option<T>, Box<dyn std::error::Error>> {
    let mut redis_conn = get_redis_conn().await?;
    let index_key = format!("{}{}", prefix, key);

    if let Ok(indexed_value) = redis_conn.get::<_, String>(&index_key).await {
        let value: T = serde_json::from_str(&indexed_value)?;
        debug!("Restored from key: {}", index_key);
        return Ok(Some(value));
    }

    Ok(None)
}
