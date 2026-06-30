use async_trait::async_trait;

use crate::migrations::manager::Migration;
use nexus_common::{
    db::{
        fetch_all_rows_from_graph,
        graph::Query,
        reindex::{keyset_scan, REINDEX_BATCH_SIZE},
    },
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
        let query = Query::new(
            "get_all_user_ids",
            "MATCH (u:User) WHERE u.id > $cursor RETURN u.id AS id ORDER BY u.id LIMIT $limit",
        );
        keyset_scan(REINDEX_BATCH_SIZE, "UsersByPkReindex backfill", |cursor| {
            let query = query.clone();
            async move {
                let batch = fetch_all_rows_from_graph(
                    query
                        .param("cursor", cursor.as_str())
                        .param("limit", REINDEX_BATCH_SIZE as i64),
                )
                .await?;
                let count = batch.len();
                let mut users_details = vec![];
                let mut last_id = String::new();
                for row in batch {
                    let user_id = match row.get::<String>("id") {
                        Ok(id) => id,
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to extract user id from graph row");
                            continue;
                        }
                    };
                    last_id = user_id.clone();
                    match UserDetails::get_by_id(&user_id).await {
                        Ok(Some(details)) => users_details.push(details),
                        Ok(None) => tracing::warn!("No UserDetails for {user_id}"),
                        Err(e) => {
                            tracing::warn!("Failed to reindex UserDetails for {user_id}: {e}")
                        }
                    }
                }
                if !users_details.is_empty() {
                    let users_details_refs = users_details.iter().collect::<Vec<&UserDetails>>();
                    UserSearch::put_to_index(&users_details_refs).await?;
                }
                Ok((
                    count,
                    if last_id.is_empty() {
                        None
                    } else {
                        Some(last_id)
                    },
                ))
            }
        })
        .await
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}
