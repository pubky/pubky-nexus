use crate::db::connectors::redis::get_redis_conn;
use std::error::Error;

pub async fn clear_redis() -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    redis::cmd("FLUSHDB").query_async(&mut redis_conn).await?;
    Ok(())
}
