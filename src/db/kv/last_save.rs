use crate::db::connectors::redis::get_redis_conn;
use std::error::Error;

pub async fn get_last_rdb_save_time() -> Result<u64, Box<dyn Error + Send + Sync>> {
    let mut redis_conn = get_redis_conn().await?;
    let info: String = redis::cmd("INFO")
        .arg("persistence")
        .query_async(&mut redis_conn)
        .await?;
    for line in info.lines() {
        if line.starts_with("rdb_last_save_time:") {
            if let Some(value_str) = line.split(':').nth(1) {
                let timestamp = value_str.trim().parse::<u64>()?;
                return Ok(timestamp);
            }
        }
    }
    Err("Could not find rdb_last_save_time in the info output".into())
}
