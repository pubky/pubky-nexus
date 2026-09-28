use anyhow::Result;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{fetch_key_from_graph, RedisOps};
use nexus_common::models::resource::stream::{
    ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::models::resource::tag::{TagResource, RESOURCE_TAGS_KEY_PARTS};
use nexus_common::types::Pagination;
use serde::{Deserialize, Serialize};

/// Graph query result for a Resource tag
#[derive(Serialize, Deserialize, Debug)]
pub struct ResourceTagResult {
    pub label: String,
    pub app: Option<String>,
    pub tagger: String,
    pub uri: String,
    pub scheme: String,
}

/// Find a Resource tag in the graph database
pub async fn find_resource_tag(
    resource_id: &str,
    label: &str,
) -> Result<Option<ResourceTagResult>> {
    let query = resource_tag_query(resource_id, label);
    let result = fetch_key_from_graph(query, "details").await.unwrap();
    Ok(result)
}

/// Check if a Resource node exists in the graph
pub async fn resource_exists_in_graph(resource_id: &str) -> Result<bool> {
    let query = Query::new(
        "resource_exists",
        "
        OPTIONAL MATCH (r:Resource {id: $resource_id})
        RETURN r IS NOT NULL AS exists
        ",
    )
    .param("resource_id", resource_id);
    let result: Option<bool> = fetch_key_from_graph(query, "exists").await.unwrap();
    Ok(result.unwrap_or(false))
}

/// Count TAGGED relationships pointing to a Resource
pub async fn count_resource_tags(resource_id: &str) -> Result<i64> {
    let query = Query::new(
        "count_resource_tags",
        "
        MATCH (:User)-[t:TAGGED]->(:Resource {id: $resource_id})
        RETURN count(t) AS tag_count
        ",
    )
    .param("resource_id", resource_id);
    let result: Option<i64> = fetch_key_from_graph(query, "tag_count").await.unwrap();
    Ok(result.unwrap_or(0))
}

/// The newest `indexed_at` among the TAGGED edges on a Resource, optionally
/// restricted to one app namespace. This is the resource's timeline position.
pub async fn latest_tag_indexed_at(resource_id: &str, app: Option<&str>) -> Result<Option<i64>> {
    let app_filter = match app {
        Some(_) => "AND t.app = $app",
        None => "",
    };
    let cypher = format!(
        "
        MATCH (:User)-[t:TAGGED]->(:Resource {{id: $resource_id}})
        WHERE true {app_filter}
        RETURN MAX(t.indexed_at) AS latest
        "
    );
    let mut query = Query::new("latest_tag_indexed_at", &cypher).param("resource_id", resource_id);
    if let Some(a) = app {
        query = query.param("app", a);
    }
    let result: Option<i64> = fetch_key_from_graph(query, "latest").await.unwrap();
    Ok(result)
}

/// `(resource id, taggers count)` pairs the graph-served stream returns for the
/// given app namespace and labels, ordered by taggers count. Read through
/// `get_scored_resource_keys`, because `last_score` carries no count under that
/// sorting.
pub async fn resource_taggers_count(
    app: Option<&str>,
    tags: Option<&[String]>,
) -> Result<Vec<(String, i64)>> {
    let source = match app {
        Some(app) => ResourceStreamSource::App {
            app: app.to_string(),
        },
        None => ResourceStreamSource::All,
    };
    let pagination = Pagination {
        limit: Some(100),
        ..Default::default()
    };
    Ok(ResourceStream::get_scored_resource_keys(
        &source,
        pagination,
        SortOrder::Descending,
        &ResourceSorting::TaggersCount,
        tags,
    )
    .await?)
}

/// Score of `label` in the resource's label-score sorted set, `None` when
/// absent. Read through `TagResource`, which writes the key in
/// `update_index_score`, and from the same key parts, so a key change breaks
/// the tests at compile time. The sorted-set helpers key off `Sorted:`, not
/// `TagResource::prefix()`.
pub async fn resource_label_score(resource_id: &str, label: &str) -> Result<Option<isize>> {
    let key_parts: Vec<&str> = [&RESOURCE_TAGS_KEY_PARTS[..], &[resource_id]].concat();
    let score = TagResource::check_sorted_set_member(None, &key_parts, &[label])
        .await
        .unwrap();
    Ok(score)
}

/// Compute the resource_id for a given URI (for test assertions)
pub fn compute_resource_id(uri: &str) -> String {
    let (normalized, _) = nexus_common::universal_tag::normalize::normalize_uri(uri).unwrap();
    nexus_common::universal_tag::normalize::resource_id(&normalized)
}

fn resource_tag_query(resource_id: &str, label: &str) -> Query {
    Query::new(
        "resource_tag_query",
        "
        MATCH (tagger:User)-[t:TAGGED {label: $label}]->(r:Resource {id: $resource_id})
        RETURN {
            label: t.label,
            app: t.app,
            tagger: tagger.id,
            uri: r.uri,
            scheme: r.scheme
        } AS details
        ",
    )
    .param("resource_id", resource_id)
    .param("label", label)
}
