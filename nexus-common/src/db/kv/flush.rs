use crate::db::get_redis_conn;
use crate::types::DynError;

pub async fn clear_redis() -> Result<(), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let _: () = redis::cmd("FLUSHDB").query_async(&mut redis_conn).await?;
    Ok(())
}
