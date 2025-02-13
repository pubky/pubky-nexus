use async_trait::async_trait;
use log::{error, info};

use crate::db::migrations::manager::Migration;
use crate::models::post::PostCounts;
use crate::models::user::UserCounts;
use crate::{get_redis_conn, RedisOps};
use crate::types::DynError;

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
    for (pubky, post_id) in list {
        if post_id.is_some() {
            let id = post_id.unwrap();
            let response = PostCounts::get_from_graph(&pubky, &id).await?;
            match response {
                Some((post_counts, _)) => post_counts.put_index_json(&[&pubky, &id], None, None).await?,
                None => error!("Error while adding index, Post:Counts:{pubky}:{id}"),
            }
        } else {
            let response = UserCounts::get_from_graph(&pubky).await?;
            match response {
                Some(user_counts) => user_counts.put_index_json(&[&pubky], None, None).await?,
                None => error!("Error while adding index, User:Counts:{pubky}")
            }
        }
    }
    Ok(())
}

pub async fn dump_counts_from_index(pattern: &str) -> Result<Vec<(String, Option<String>)>, DynError> {
    let mut keys_cursor = 0;
    let mut values= Vec::new();
    let mut redis_connection = get_redis_conn().await?;
    
    info!("Redis scan starting... cursor=0");
    loop {
        // Scan for keys matching the pattern
        let (cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(keys_cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(BATCH_SIZE)
            .query_async(&mut redis_connection)
            .await?;

        // If keys are found, delete them in a pipeline
        if !keys.is_empty() {
            let mut pipe = redis::pipe();
            for key in keys {
                values.push(remove_first_two_segments(&key));
                pipe.del(key);
            }
            let _: () = pipe.query_async(&mut redis_connection).await?;
        }

        info!("Found 100 index, cursor level = {:?}", cursor);

        // Continue scanning until SCAN finishes
        if cursor == 0 {
            break;
        }
        keys_cursor = cursor; 
    }
    info!("All counts keys deleted!");
    Ok(values)
}

pub fn remove_first_two_segments(key: &String) -> (String, Option<String>) {
    let parts: Vec<&str> = key.split(':').collect();
    let data: Vec<&str> = parts.into_iter().skip(2).collect();
    let mut info = (data[0].to_string(), None);
    if data.len() == 2 {
        info.1 = Some(data[1].to_string());
    }
    info
}