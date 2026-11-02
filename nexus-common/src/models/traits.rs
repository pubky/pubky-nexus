use crate::db::graph::Query;
use crate::db::{exec_single_row, fetch_all_rows_from_graph, GraphResult, RedisOps};
use crate::models::error::ModelResult;
use async_trait::async_trait;
use core::fmt;
use std::fmt::Debug;

#[async_trait]
pub trait Collection<T>
where
    Self: RedisOps + Clone + Debug,
    T: fmt::Debug + Sync + Send + Copy,
{
    /// Retrieves records by their IDs: one slot per id, in the order given,
    /// `None` where no record was found.
    async fn get_by_ids(ids: &[T]) -> ModelResult<Vec<Option<Self>>>;

    /// Queries a Neo4j graph database to retrieve records based on the provided IDs and collection type.
    ///
    /// # Arguments
    ///
    /// * `ids` - A slice of string slices representing the IDs.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` containing a vector of `Option<Self>`. Each `Option` corresponds to
    /// a queried ID, containing `Some(record)` if the record was found in the graph database, or `None` if it was not found.
    async fn get_from_graph(ids: &[T]) -> GraphResult<Vec<Option<Self>>> {
        let query = Self::collection_details_graph_query(ids);
        let rows = fetch_all_rows_from_graph(query).await?;

        let mut records = Vec::with_capacity(ids.len());

        for row in rows {
            let record: Option<Self> = row.get("record").ok();
            records.push(record);
        }
        Ok(records)
    }

    // Save new graph node
    async fn put_to_graph(&self) -> GraphResult<()> {
        exec_single_row(self.put_graph_query()?).await
    }

    /// Returns the neo4j query to return a list records by passing a list of ids.
    /// The query should return each record in the "record" attribute of the node.
    fn collection_details_graph_query(id_list: &[T]) -> Query;

    /// Returns the neo4j query to put a record into the graph.
    fn put_graph_query(&self) -> GraphResult<Query>;
}
