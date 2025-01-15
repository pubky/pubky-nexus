use std::any::Any;

use anyhow::Result;

use async_trait::async_trait;
use neo4rs::query;
use pubky_nexus::{get_neo4j_graph, get_redis_conn, types::DynError, Migration, MigrationManager};

#[tokio::main]
async fn main() -> Result<()> {
    // this goes inside the application logic code, where the original write is happening.
    let data = Box::new(NewUserField {
        id: "user_id".to_string(),
        field: "new_field".to_string(),
    });
    let dual_write_result = MigrationManager::dual_write::<ExampleMigration>(data).await;
    println!("Dual write result: {:?}", dual_write_result);
    Ok(())
}

struct NewUserField {
    id: String,
    field: String,
}

pub struct ExampleMigration;

#[async_trait]
impl Migration for ExampleMigration {
    fn id(&self) -> &'static str {
        "ExampleMigration"
    }

    fn is_multi_staged(&self) -> bool {
        true
    }

    async fn dual_write(data: Box<(dyn Any + Send + 'static)>) -> Result<(), DynError> {
        // Implement your dual write logic here
        let new_field_data = match data.downcast_ref::<NewUserField>() {
            Some(data) => data,
            None => {
                return Err("Received data of unknown type".into());
            }
        };
        let graph = get_neo4j_graph()?;
        graph
            .lock()
            .await
            .run(query(
                format!(
                    "MERGE (n:User {{id: \"{}\"}}) SET n.new_field = \"{}\"",
                    new_field_data.id, new_field_data.field
                )
                .as_str(),
            ))
            .await?;
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // Your backfill logic here
        println!("Running backfill for ExampleMigration");
        // Run Graph queries here
        let graph = get_neo4j_graph()?;
        graph
            .lock()
            .await
            .run(query(
                "MERGE (n:User) SET n.new_field = n.field WHERE n.new_field IS NULL",
            ))
            .await?;
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        // Your cutover logic here
        // for redis you can rename from new to old key here.
        let mut redis_connection = get_redis_conn().await?;
        redis::cmd("rename")
            .arg("Migration:User:Details")
            .arg("User:Details")
            .query_async::<()>(&mut redis_connection)
            .await
            .unwrap();
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        // Your cleanup logic here.
        // delete node/relationships from graph.
        // delete old keys from redis.
        Ok(())
    }
}
