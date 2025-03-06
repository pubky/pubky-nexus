use crate::db::migrations::manager::Migration;
use crate::models::post::PostCounts;
use crate::models::user::UserCounts;
use crate::types::DynError;
use crate::db::RedisOps;
use crate::db::get_redis_conn;
use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, error, info};

const BATCH_SIZE: u32 = 500;

pub struct TagCountsReset1739459180;

#[async_trait]
impl Migration for TagCountsReset1739459180 {
    fn id(&self) -> &'static str {
        "TagCountsReset1739459180"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        // We only need a backfill phase because there is only one source
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Start deleting redis indexes
        let pubky_list = dump_counts_from_index("User:Counts:*").await?;
        let posts_list = dump_counts_from_index("Post:Counts:*").await?;
        // Retrieve from graph and index the counts
        compute_counts(pubky_list).await?;
        compute_counts(posts_list).await?;

        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        // Not necessary
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        // There is not extra cleanup because we did the necessary clean
        // in the backfill phase
        Ok(())
    }
}

pub async fn compute_counts(list: Vec<(String, Option<String>)>) -> Result<(), DynError> {
    let list_size = list.len();
    let mut success_count = 0;
    let mut error_count = 0;
    info!(
        "Starting compute_counts for {} items at {}",
        list_size,
        chrono::Utc::now()
    );

    for (pubky, post_id) in list.into_iter() {
        if let Some(id) = post_id {
            // Processing post counts
            match PostCounts::get_from_graph(&pubky, &id).await {
                Ok(Some((post_counts, _))) => {
                    if let Err(e) = post_counts.put_index_json(&[&pubky, &id], None, None).await {
                        error!("Failed to add Post:Counts:{pubky}:{id}, {e}");
                        error_count += 1;
                    } else {
                        success_count += 1;
                        debug!("Successfully added Post:Counts:{pubky}:{id}");
                    }
                }
                Ok(None) => {
                    error!("Not found from graph Post:Counts:{pubky}:{id}");
                    error_count += 1;
                }
                Err(e) => {
                    error!("Error fetching from graph for Post:Counts:{pubky}:{id}, {e}");
                    error_count += 1;
                }
            }
        } else {
            // Processing user counts
            match UserCounts::get_from_graph(&pubky).await {
                Ok(Some(user_counts)) => {
                    if let Err(e) = user_counts.put_index_json(&[&pubky], None, None).await {
                        error!("Failed to add User:Counts:{pubky}, {e}");
                        error_count += 1;
                    } else {
                        success_count += 1;
                        debug!("Successfully added User:Counts:{pubky}");
                    }
                }
                Ok(None) => {
                    error!("User:Counts not found from graph for User:Counts:{pubky}");
                    error_count += 1;
                }
                Err(e) => {
                    error!("Error fetching from graph, User:Counts:{pubky}, {e}");
                    error_count += 1;
                }
            }
        }
    }

    info!(
        "compute_counts completed. Total: {}, success operations: {}, failed operations: {}, Timestamp: {}",
        list_size,
        success_count,
        error_count,
        chrono::Utc::now()
    );

    Ok(())
}

pub async fn dump_counts_from_index(
    pattern: &str,
) -> Result<Vec<(String, Option<String>)>, DynError> {
    let mut cursor = 0;
    let mut values = Vec::new();
    let mut total_keys_processed = 0;
    let mut redis_connection = get_redis_conn().await?;

    info!(
        "Starting Redis SCAN for pattern '{}', batch size: {}. Timestamp: {}",
        pattern,
        BATCH_SIZE,
        Utc::now()
    );

    loop {
        let start_time = Utc::now();
        // Scan for keys matching the pattern
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(BATCH_SIZE)
            .query_async(&mut redis_connection)
            .await?;

        let batch_size = keys.len();
        total_keys_processed += batch_size;

        // If keys are found, delete them in a pipeline
        if !keys.is_empty() {
            let mut pipe = redis::pipe();
            for key in keys {
                // Collect key information
                values.push(remove_first_two_segments(&key));
                // Add the command to the pipeline
                pipe.del(key);
            }
            let _: () = pipe.query_async(&mut redis_connection).await?;

            info!(
                "Deleted {} keys in this batch. Batch completed in {:?} seconds.",
                batch_size,
                (Utc::now() - start_time).num_milliseconds() as f64 / 1000.0
            );
        } else {
            info!(
                "No keys found in this batch. Cursor: {}. Continuing...",
                new_cursor
            );
        }

        // Continue scanning until SCAN finishes
        if new_cursor == 0 {
            break;
        }
        cursor = new_cursor;
    }
    info!(
        "=> Redis SCAN completed. Total keys processed: {}",
        total_keys_processed
    );
    Ok(values)
}

pub fn remove_first_two_segments(key: &str) -> (String, Option<String>) {
    let mut parts = key.split(':').skip(2);
    let pubky = parts.next().unwrap_or_default().to_string();
    let optional_post_id = parts.next().map(|s| s.to_string());
    (pubky, optional_post_id)
}
