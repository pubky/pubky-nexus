use crate::db::get_redis_conn;
use crate::db::kv::RedisResult;

pub async fn get_last_rdb_save_time() -> RedisResult<Option<String>> {
    let mut redis_conn = get_redis_conn().await?;
    let info: String = redis::cmd("INFO")
        .arg("persistence")
        .query_async(&mut redis_conn)
        .await?;
    for line in info.lines() {
        if line.starts_with("rdb_last_save_time:") {
            if let Some(value_str) = line.split(':').nth(1) {
                return Ok(Some(value_str.trim().to_string()));
            }
        }
    }
    Ok(None)
}
