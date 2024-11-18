use crate::db::connectors::redis::get_redis_conn;
use crate::types::DynError;

pub async fn redis_is_empty() -> Result<bool, DynError> {
    let mut redis_conn = get_redis_conn().await?;

    let key_count: u64 = redis::cmd("DBSIZE").query_async(&mut redis_conn).await?;

    Ok(key_count == 0)
}
