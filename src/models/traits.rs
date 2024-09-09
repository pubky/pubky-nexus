use axum::async_trait;
use neo4rs::Query;

use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::RedisOps;
use core::fmt;
use std::fmt::Debug;

pub trait CollectionId {
    fn to_string_id(self) -> String;
}

impl CollectionId for &str {
    fn to_string_id(self) -> String {
        return String::from(self);
    }
}

impl CollectionId for &[&str] {
    fn to_string_id(self) -> String {
        return self.join(":");
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
    async fn get_by_ids(
        ids: &[T],
    ) -> Result<Vec<Option<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<String> = ids.iter().map(|id| id.to_string_id()).collect();

        let keys_refs: Vec<Vec<&str>> = key_parts_list.iter().map(|id| vec![id.as_str()]).collect();

        let keys: Vec<&[&str]> = keys_refs.iter().map(|arr| &arr[..]).collect();

        let mut collection = Self::try_from_index_multiple_json(&keys).await?;

        let mut missing_ids: Vec<(usize, T)> = Vec::new();
        for (i, details) in collection.iter().enumerate() {
            if details.is_none() {
                missing_ids.push((i, ids[i]));
            }
        }

        if !missing_ids.is_empty() {
            let flat_missing_ids: Vec<T> = missing_ids.iter().map(|&(_, id)| id).collect();
            let fetched_details = Self::from_graph(&flat_missing_ids).await?;

            if fetched_details.len() > 0 {
                for (i, (original_index, _)) in missing_ids.iter().enumerate() {
                    collection[*original_index].clone_from(&fetched_details[i]);
                }
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
    async fn from_graph(
        ids: &[T],
    ) -> Result<Vec<Option<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = Self::graph_query(ids);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut records = Vec::with_capacity(ids.len());

        while let Some(row) = result.next().await? {
            let record = row.get::<Option<Self>>("record").unwrap_or_default();
            records.push(record);
        }

        if !records.is_empty() {
            let mut found_records = Vec::new();
            let mut found_record_ids = Vec::new();

            for (detail, id) in records.iter().zip(ids.iter()) {
                if let Some(value) = detail {
                    found_records.push(Some(value.clone()));
                    found_record_ids.push(*id);
                }
            }

            Self::to_index(&found_record_ids, found_records).await?;
        }

        Self::extend_on_cache_miss(&records).await;
        Ok(records)
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
    async fn to_index(
        ids: &[T],
        records: Vec<Option<Self>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<String> = ids.iter().map(|id| id.to_string_id()).collect();

        let keys_refs: Vec<Vec<&str>> = key_parts_list.iter().map(|id| vec![id.as_str()]).collect();

        let keys: Vec<&[&str]> = keys_refs.iter().map(|arr| &arr[..]).collect();

        Self::put_multiple_json_indexes(&keys, records).await
    }

    /// Returns the neo4j query to return a list records by passing a list of ids.
    /// The query should return each record in the "record" attribute of the node.
    fn graph_query(id_list: &[T]) -> Query;
    async fn extend_on_cache_miss(elements: &[std::option::Option<Self>]);
}
