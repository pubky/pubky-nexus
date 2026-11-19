//! Adds $.indexed_at NUMERIC SORTABLE to the post content full-text index, so a
//! search scoped to one author can sort newest first.
//!
//! Same deployment expectation as `PostContentIndexAuthorSetup1780531200`: stop → migrate →
//! start. The index is absent between the drop and the create, and an author-scoped search
//! fails until it has the field. FT.CREATE rescans every PostDetails JSON document, which
//! already carries `indexed_at`, so nothing is re-persisted.
//!
//! The FT.CREATE arg list is frozen here on purpose: the live schema is owned by
//! `nexus_common::db::setup_cache`, and this migration must keep producing the v3 index even
//! after that schema evolves.
use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{db::get_redis_conn, db::RedisOps, models::post::PostDetails, types::DynError};
use tracing::info;

pub struct PostContentIndexSortSetup1795086590;

const POST_CONTENT_INDEX: &str = "postContentIdx";

/// Creates the v3 post content index: the v2 fields plus $.indexed_at NUMERIC SORTABLE.
/// Idempotent: no-ops if the index already exists.
async fn create_post_content_index_v3() -> Result<(), DynError> {
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
        .arg("SCHEMA")
        .arg("$.content")
        .arg("AS")
        .arg("content")
        .arg("TEXT")
        .arg("$.author")
        .arg("AS")
        .arg("author")
        .arg("TAG")
        .arg("CASESENSITIVE")
        .arg("$.kind")
        .arg("AS")
        .arg("kind")
        .arg("TAG")
        .arg("CASESENSITIVE")
        .arg("$.indexed_at")
        .arg("AS")
        .arg("indexed_at")
        .arg("NUMERIC")
        .arg("SORTABLE")
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

/// Drops the post content index without deleting the documents (no `DD` flag).
/// Idempotent: swallows "Unknown index name" so repeated calls are safe.
async fn drop_post_content_index() -> Result<(), DynError> {
    let mut conn = get_redis_conn().await?;

    let result = redis::cmd("FT.DROPINDEX")
        .arg(POST_CONTENT_INDEX)
        .query_async::<()>(&mut conn)
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("unknown index name") || msg.contains("no such index") {
                info!("RediSearch index '{POST_CONTENT_INDEX}' already absent");
                Ok(())
            } else {
                Err(e.into())
            }
        }
    }
}

#[async_trait]
impl Migration for PostContentIndexSortSetup1795086590 {
    fn id(&self) -> &'static str {
        "PostContentIndexSortSetup1795086590"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        drop_post_content_index().await?;
        info!("Dropped post content index for schema upgrade");

        create_post_content_index_v3().await?;
        info!("Recreated post content index with a sortable indexed_at");

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
