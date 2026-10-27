use crate::db::{fetch_all_rows_from_graph, graph::Query};
use crate::models::error::{ModelError, ModelResult};
use crate::universal_tag::normalize::normalize_uri;
use pubky_social_specs::PubkySocialAvailabilityContent;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AvailabilityView {
    pub author: String,
    pub post_uri: String,
    pub target: String,
    pub updated_at: i64,
    pub schedule: PubkySocialAvailabilityContent,
}

/// One current schedule per author and normalized target. Older posts remain
/// available for fallback when the current post is deleted or changes kind/target.
pub async fn availability_stream(
    author: Option<&str>,
    target: Option<&str>,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<AvailabilityView>> {
    let target = target
        .map(|uri| {
            normalize_uri(uri)
                .map(|v| v.0)
                .map_err(ModelError::InvalidInput)
        })
        .transpose()?;
    let start = if target.is_some() {
        "MATCH (p:Post {availability_target:$target})<-[:AUTHORED]-(u:User)"
    } else {
        "MATCH (u:User {id:$author})-[:AUTHORED]->(p:Post)"
    };
    let cypher = format!(
        "{start}{}",
        r"
        WHERE p.social_kind='availability' AND p.availability_content IS NOT NULL
          AND NOT coalesce(p.deleted,false) AND NOT coalesce(u.deleted,false)
          AND ($author IS NULL OR u.id=$author)
          AND ($target IS NULL OR p.availability_target=$target)
        WITH u,p ORDER BY p.social_updated_at DESC,p.id DESC
        WITH u,p.availability_target AS target,collect(p)[0] AS p
        RETURN u.id AS author,p.uri AS uri,target,p.social_updated_at AS updated_at,p.availability_content AS content
        ORDER BY updated_at DESC,author,target SKIP $skip LIMIT $limit
    "
    );
    let rows = fetch_all_rows_from_graph(
        Query::new("availability_stream", cypher)
            .param("author", author)
            .param("target", target)
            .param("skip", skip.min(10_000) as i64)
            .param("limit", limit.min(200) as i64),
    )
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(AvailabilityView {
                author: r.get("author")?,
                post_uri: r.get("uri")?,
                target: r.get("target")?,
                updated_at: r.get("updated_at")?,
                schedule: serde_json::from_str(&r.get::<String>("content")?)
                    .map_err(ModelError::from_generic)?,
            })
        })
        .collect()
}
