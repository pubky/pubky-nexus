use crate::db::get_redis_conn;
use crate::types::DynError;

pub async fn clear_redis() -> Result<(), DynError> {
    let redis_conn_arc = get_redis_conn().await?;
    let mut redis_conn = redis_conn_arc.lock().await;
    let _: () = redis::cmd("FLUSHDB").query_async(&mut *redis_conn).await?;
    Ok(())
}
