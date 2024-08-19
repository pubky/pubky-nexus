use axum::async_trait;
use neo4rs::Query;

use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::RedisOps;
use std::fmt::Debug;

#[async_trait]
pub trait Collection
where
    Self: RedisOps + Clone + Debug + Default,
{
    /// Retrieves records by their IDs, first attempting to fetch them from a cache (e.g., Redis),
    /// and then querying a graph database (e.g., Neo4j) if necessary.
    ///
    /// # Arguments
    ///
    /// * `id_list` - A slice of string slices representing the IDs to query.
    /// * `collection_type` - An enum value of `CollectionType` specifying the type of collection (e.g., User, Post, Tag).
    ///
    /// # Returns
    ///
    /// This function returns a `Result` containing a vector of `Option<Self>`. Each `Option` corresponds to
    /// a queried ID, containing `Some(record)` if the record was found in either the cache or the graph database,
    /// or `None` if it was not found in either.
    async fn get_by_ids(
        id_list: &[&str],
    ) -> Result<Vec<Option<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<&[&str]> = id_list.iter().map(std::slice::from_ref).collect();
        let mut collection = Self::try_from_index_multiple_json(&key_parts_list).await?;

        let mut missing: Vec<(usize, &str)> = Vec::new();

        for (i, details) in collection.iter().enumerate() {
            if details.is_none() {
                missing.push((i, id_list[i]));
            }
        }

        if !missing.is_empty() {
            let missing_ids: Vec<&str> = missing.iter().map(|&(_, id)| id).collect();
            let fetched_details = Self::from_graph(&missing_ids).await?;

            for (i, (original_index, _)) in missing.iter().enumerate() {
                collection[*original_index].clone_from(&fetched_details[i]);
            }
        }

        Ok(collection)
    }

    /// Queries a Neo4j graph database to retrieve records based on the provided IDs and collection type.
    ///
    /// # Arguments
    ///
    /// * `missing_ids` - A slice of string slices representing the IDs that were not found in the cache.
    /// * `collection_type` - An enum value of `CollectionType` specifying the type of collection to query (e.g., User, Post, Tag).
    ///
    /// # Returns
    ///
    /// This function returns a `Result` containing a vector of `Option<Self>`. Each `Option` corresponds to
    /// a queried ID, containing `Some(record)` if the record was found in the graph database, or `None` if it was not found.
    async fn from_graph(
        missing_ids: &[&str],
    ) -> Result<Vec<Option<Self>>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = Self::graph_query(missing_ids);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut missing_records = Vec::with_capacity(missing_ids.len());

        while let Some(row) = result.next().await? {
            let record: Option<Self> = row.get("record").unwrap_or_default();
            missing_records.push(record);
        }

        if !missing_records.is_empty() {
            let mut existing_records = Vec::new();
            let mut existing_record_ids = Vec::new();

            for (detail, id) in missing_records.iter().zip(missing_ids.iter()) {
                if let Some(record) = detail {
                    existing_records.push(Some(record.clone()));
                    existing_record_ids.push(*id);
                }
            }

            Self::to_index(&existing_record_ids, existing_records).await?;
        }

        Self::add_to_sorted_sets(&missing_records).await;
        Ok(missing_records)
    }

    /// Indexes collection of records in Redis for faster access in future queries.
    ///
    /// # Arguments
    ///
    /// * `user_ids` - A slice of string slices representing the IDs of the records to index.
    /// * `existing_records` - A vector of `Option<Self>` containing the records to be indexed.
    ///   Each `Option` corresponds to an ID in `user_ids`.
    ///
    /// # Returns
    ///
    /// This function returns a `Result` indicating success or failure. A successful result indicates that the
    /// records were successfully indexed in the cache.
    async fn to_index(
        user_ids: &[&str],
        existing_records: Vec<Option<Self>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key_parts_list: Vec<Vec<&str>> = user_ids.iter().map(|id| vec![*id]).collect();
        let keys_refs: Vec<&[&str]> = key_parts_list.iter().map(|key| &key[..]).collect();

        Self::put_multiple_json_indexes(&keys_refs, existing_records).await
    }

    fn graph_query(id_list: &[&str]) -> Query;
    async fn add_to_sorted_sets(elements: &[std::option::Option<Self>]);
}
