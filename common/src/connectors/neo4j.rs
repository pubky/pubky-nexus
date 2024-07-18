pub use neo4rs::{query, Graph, Node};
use once_cell::sync::OnceCell;
use std::fmt;

pub struct Neo4jConnector {
    graph: OnceCell<Graph>,
}

impl Neo4jConnector {
    pub fn new() -> Self {
        Self {
            graph: OnceCell::new(),
        }
    }

    pub async fn connect(
        &self,
        uri: &str,
        user: &str,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let graph = Graph::new(uri, user, password).await?;
        self.graph
            .set(graph)
            .map_err(|_| "Failed to set graph instance")?;
        Ok(())
    }

    pub async fn get_user_by_id(
        &self,
        user_id: &str,
    ) -> Result<Option<Node>, Box<dyn std::error::Error>> {
        let graph = self.graph.get().expect("Not connected to Neo4j");
        let query = query("MATCH (u:User {id: $id}) RETURN u").param("id", user_id);
        let mut result = graph.execute(query).await?;
        if let Some(row) = result.next().await? {
            let node: Node = row.get("u").unwrap();
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }
}

impl fmt::Debug for Neo4jConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Neo4jConnector")
            .field("graph", &"Graph instance")
            .finish()
    }
}

pub static GLOBAL_NEO4J_CONNECTOR: OnceCell<Neo4jConnector> = OnceCell::new();
