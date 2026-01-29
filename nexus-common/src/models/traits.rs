use crate::db::kv::RedisResult;
use crate::db::{exec_single_row, fetch_all_rows_from_graph, RedisOps};
use crate::types::DynError;
use async_trait::async_trait;
use core::fmt;
use neo4rs::Query;
use std::fmt::Debug;

pub trait CollectionId {
    fn to_string_id(self) -> String;
}

impl CollectionId for &str {
    fn to_string_id(self) -> String {
        String::from(self)
    }
}

impl CollectionId for &[&str] {
    fn to_string_id(self) -> String {
        self.join(":")
    }
}

#[async_trait]
pub trait Collection<T>
where
    Self: RedisOps + Clone + Debug + Default,
    T: CollectionId + fmt::Debug + Sync + Send + Copy,
{
    /// Retrieves records by their IDs, first attempting to fetch them from a cache (e.g., Redis),
    /// and then querying a graph database (e.g., Neo4j) if necessary.
    ///
    /// # Arguments
    ///
    /// * `ids` - A slice of id slices representing the IDs to query.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` containing a vector of `Option<Self>`. Each `Option` corresponds to
    /// a queried ID, containing `Some(record)` if the record was found in either the cache or the graph database,
    /// or `None` if it was not found in either.
    async fn get_by_ids(ids: &[T]) -> Result<Vec<Option<Self>>, DynError> {
        let key_parts_list: Vec<String> = ids.iter().map(|id| id.to_string_id()).collect();

        let keys_refs: Vec<Vec<&str>> = key_parts_list.iter().map(|id| vec![id.as_str()]).collect();

        let keys: Vec<&[&str]> = keys_refs.iter().map(|arr| &arr[..]).collect();

        let mut collection = Self::get_from_index(keys).await?;

        let mut missing_ids: Vec<(usize, T)> = Vec::new();
        for (i, details) in collection.iter().enumerate() {
            if details.is_none() {
                missing_ids.push((i, ids[i]));
            }
        }

        if !missing_ids.is_empty() {
            let flat_missing_ids: Vec<T> = missing_ids.iter().map(|&(_, id)| id).collect();
            let fetched_details = Self::get_from_graph(&flat_missing_ids).await?;

            if !fetched_details.is_empty() {
                for (i, (original_index, _)) in missing_ids.iter().enumerate() {
                    collection[*original_index].clone_from(&fetched_details[i]);
                }
                Self::put_to_index(&flat_missing_ids, fetched_details).await?;
            }
        }

        Ok(collection)
    }

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
    async fn get_from_graph(ids: &[T]) -> Result<Vec<Option<Self>>, DynError> {
        let query = Self::collection_details_graph_query(ids);
        let rows = fetch_all_rows_from_graph(query).await?;

        let mut records = Vec::with_capacity(ids.len());

        for row in rows {
            let record: Option<Self> = row.get("record").ok();
            records.push(record);
        }
        Ok(records)
    }

    async fn get_from_index(keys: Vec<&[&str]>) -> RedisResult<Vec<Option<Self>>> {
        Self::try_from_index_multiple_json(&keys).await
    }

    /// Indexes collection of records in Redis for faster access in future queries.
    ///
    /// # Arguments
    ///
    /// * `ids` - A slice of id slices representing the IDs of the records to index.
    /// * `records` - A vector of `Option<Self>` containing the records to be indexed.
    ///   Each `Option` corresponds to an ID.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` indicating success or failure. A successful result indicates that the
    /// records were successfully indexed in the cache.
    async fn put_to_index(ids: &[T], records: Vec<Option<Self>>) -> RedisResult<()> {
        let mut found_records = Vec::with_capacity(records.len());
        let mut found_record_ids = Vec::with_capacity(records.len());

        for (detail, id) in records.iter().zip(ids.iter()) {
            if let Some(value) = detail {
                found_records.push(Some(value.clone()));
                found_record_ids.push(*id);
            }
        }
        let key_parts_list: Vec<String> = found_record_ids
            .iter()
            .map(|id| id.to_string_id())
            .collect();

        let keys_refs: Vec<Vec<&str>> = key_parts_list.iter().map(|id| vec![id.as_str()]).collect();

        let keys: Vec<&[&str]> = keys_refs.iter().map(|arr| &arr[..]).collect();

        Self::put_multiple_json_indexes(&keys, found_records).await?;
        Self::extend_on_index_miss(&records).await?;
        Ok(())
    }

    // Save new graph node
    async fn put_to_graph(&self) -> Result<(), DynError> {
        exec_single_row(self.put_graph_query()?).await
    }

    async fn reindex(collection_ids: &[T]) -> Result<(), DynError> {
        match Self::get_from_graph(collection_ids).await {
            Ok(collection_details_list) => {
                if !collection_details_list.is_empty() {
                    Self::put_to_index(collection_ids, collection_details_list).await?;
                }
            }
            Err(e) => tracing::error!("Error: Could not find any element of the collection: {}", e),
        }
        Ok(())
    }

    /// Returns the neo4j query to return a list records by passing a list of ids.
    /// The query should return each record in the "record" attribute of the node.
    fn collection_details_graph_query(id_list: &[T]) -> Query;

    /// Returns the neo4j query to put a record into the graph.
    fn put_graph_query(&self) -> Result<Query, DynError>;

    async fn extend_on_index_miss(elements: &[std::option::Option<Self>]) -> RedisResult<()>;
}
