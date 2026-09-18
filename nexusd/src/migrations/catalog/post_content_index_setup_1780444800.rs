use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{db::get_redis_conn, db::RedisOps, models::post::PostDetails, types::DynError};
use tracing::info;

pub struct PostContentIndexSetup1780444800;

const POST_CONTENT_INDEX: &str = "postContentIdx";

/// Creates the original v1 post content index: $.content TEXT only.
/// Includes NOOFFSETS, NOHL, NOFIELDS optimisations.
/// Idempotent: no-ops if the index already exists.
async fn create_post_content_index_v1() -> Result<(), DynError> {
    let prefix = format!("{}:", PostDetails::prefix().await);
    let mut conn = get_redis_conn().await?;

    let result = redis::cmd("FT.CREATE")
        .arg(POST_CONTENT_INDEX)
        .arg("ON")
        .arg("JSON")
        .arg("PREFIX")
        .arg("1")
        .arg(&prefix)
        .arg("NOOFFSETS")
        .arg("NOHL")
        .arg("NOFIELDS")
        .arg("SCHEMA")
        .arg("$.content")
        .arg("AS")
        .arg("content")
        .arg("TEXT")
        .query_async::<()>(&mut conn)
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("already exists") => {
            info!("RediSearch index '{POST_CONTENT_INDEX}' already exists");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[async_trait]
impl Migration for PostContentIndexSetup1780444800 {
    fn id(&self) -> &'static str {
        "PostContentIndexSetup1780444800"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        create_post_content_index_v1().await?;
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
