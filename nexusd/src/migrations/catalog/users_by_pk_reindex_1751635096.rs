use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{
    db::reindex::get_all_user_ids,
    models::user::{UserDetails, UserSearch},
    types::DynError,
};

pub struct UsersByPkReindex1751635096;

#[async_trait]
impl Migration for UsersByPkReindex1751635096 {
    fn id(&self) -> &'static str {
        "UsersByPkReindex1751635096"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Collect all existing UserDetails and reindex them

        let mut users_details = vec![];
        for user_id in get_all_user_ids().await? {
            match UserDetails::get_by_id(&user_id).await {
                Ok(opt) => match opt {
                    Some(details) => users_details.push(details),
                    None => tracing::warn!("No UserDetails for {user_id}"),
                },
                Err(e) => tracing::warn!("Failed to reindex UserDetails for {user_id}: {e}"),
            }
        }

        let users_details_refs = users_details.iter().collect::<Vec<&UserDetails>>();
        UserSearch::put_to_index(&users_details_refs)
            .await
            .map_err(Into::into)
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
