use crate::db::get_redis_conn;
use crate::db::kv::{setup_cache, RedisResult};

pub async fn clear_redis() -> RedisResult<()> {
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis::cmd("FLUSHDB").query_async(&mut redis_conn).await?;
    // FLUSHDB drops the RediSearch index along with the keys, so re-apply the schema here.
    setup_cache().await?;
    Ok(())
}
