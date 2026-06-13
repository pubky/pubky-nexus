use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{models::post::create_post_content_index, types::DynError};

pub struct PostContentIndexSetup1780444800;

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
        create_post_content_index().await?;
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
