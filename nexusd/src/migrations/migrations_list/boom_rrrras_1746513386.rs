use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::types::DynError;

pub struct BoomRrrras1746513386;

#[async_trait]
impl Migration for BoomRrrras1746513386 {
    fn id(&self) -> &'static str {
        "BoomRrrras1746513386"
    }

    fn is_multi_staged(&self) -> bool {
        true
    }

    async fn dual_write(data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        // Implement your dual write logic here. Downcast data to your struct type.
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Your backfill logic here
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        // Your cutover logic here
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        // Your cleanup logic here
        Ok(())
    }
}
