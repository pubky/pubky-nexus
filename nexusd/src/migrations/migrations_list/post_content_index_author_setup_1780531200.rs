/// Adds $.author TAG CASESENSITIVE + $.kind TAG CASESENSITIVE to the post content full-text index.
///
/// # Deployment expectation: stop → migrate → start
///
/// Between `drop_post_content_index` and `create_post_content_index` the
/// index is absent, so global content search returns empty results. After
/// FT.CREATE the PREFIX clause triggers a background scan that indexes all
/// existing PostDetails JSON documents against the new schema — both `author`
/// and `kind` fields are already present in every document, so no re-persistence
/// is needed. For zero-downtime deployments, schedule this migration during a
/// maintenance window or accept a brief gap where search is unavailable.
use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{
    models::post::{create_post_content_index, drop_post_content_index},
    types::DynError,
};
use tracing::info;

pub struct PostContentIndexAuthorSetup1780531200;

#[async_trait]
impl Migration for PostContentIndexAuthorSetup1780531200 {
    fn id(&self) -> &'static str {
        "PostContentIndexAuthorSetup1780531200"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Drop existing index (idempotent — no-ops if already absent).
        drop_post_content_index().await?;
        info!("Dropped post content index for schema upgrade");

        // Recreate with the new schema ($.content TEXT + $.author TAG + $.kind TAG).
        // FT.CREATE with PREFIX triggers a background scan that indexes all existing
        // PostDetails JSON documents against the new schema — both `author` and `kind`
        // fields are already present in every document, so no re-persistence is needed.
        create_post_content_index().await?;
        info!("Recreated post content index with author and kind fields");

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
