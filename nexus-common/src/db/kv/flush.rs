use crate::db::get_redis_conn;
use crate::db::kv::{setup_cache, RedisResult};

pub async fn clear_redis() -> RedisResult<()> {
    {
        let mut redis_conn = get_redis_conn().await?;
        // ASYNC: freeing a production-sized keyspace outlives the client timeout;
        // the swap itself is immediate, so writes after this land in the fresh db.
        let _: () = redis::cmd("FLUSHDB")
            .arg("ASYNC")
            .query_async(&mut redis_conn)
            .await?;
    }
    // FLUSHDB drops the RediSearch index along with the keys, so re-apply the schema here.
    // The FLUSHDB connection is returned to the pool first; setup_cache checks out its own.
    setup_cache().await?;
    Ok(())
}
