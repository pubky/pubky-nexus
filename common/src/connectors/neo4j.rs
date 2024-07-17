pub use neo4rs::{query, Graph, Node};
use tokio::sync::OnceCell;

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
