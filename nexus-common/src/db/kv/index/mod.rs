/// Module for redis Indexing operations split into modules by Redis types
pub mod json;
pub mod lists;
pub mod sets;
pub mod sorted_sets;

use crate::db::get_redis_conn;
use crate::db::kv::RedisError;
use deadpool_redis::Connection;

pub(crate) async fn get_conn() -> Result<Connection, RedisError> {
    get_redis_conn()
        .await
        .map_err(RedisError::ConnectionUnavailable)
}
