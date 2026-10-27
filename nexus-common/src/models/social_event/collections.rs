use super::{stream, EventQuery, Occurrence};
use crate::db::{fetch_all_rows_from_graph, graph::Query};
use crate::models::{
    error::{ModelError, ModelResult},
    post::{PostKind, PostView},
};
use pubky_social_specs::PubkySocialCollectionContent;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CalendarView {
    pub post: PostView,
    pub collection: PubkySocialCollectionContent,
    pub occurrences: Vec<Occurrence>,
}
#[derive(Debug, Default)]
pub struct CalendarQuery {
    pub author: Option<String>,
    pub curator: Option<String>,
    pub from: i64,
    pub to: i64,
    pub skip: usize,
    pub limit: usize,
    pub occurrences: usize,
}

pub async fn calendar_view(
    author: &str,
    id: &str,
    from: i64,
    to: i64,
    limit: usize,
) -> ModelResult<Option<CalendarView>> {
    let Some(post) = PostView::get_by_id(author, id, None, Some(10), Some(10)).await? else {
        return Ok(None);
    };
    if post.details.deleted
        || post.details.kind != PostKind::Collection
        || !post.details.uri.contains("/pub/social/v1/posts/")
    {
        return Ok(None);
    }
    let collection =
        serde_json::from_str(&post.details.content).map_err(ModelError::from_generic)?;
    let occurrences = stream(EventQuery {
        from,
        to,
        collection: Some(post.details.uri.clone()),
        limit: limit.min(50),
        ..Default::default()
    })
    .await?;
    Ok(Some(CalendarView {
        post,
        collection,
        occurrences,
    }))
}

/// A social collection is owned and curated by its author. No separate membership
/// or permission model is inferred from unrecognized collection extensions.
pub async fn calendar_stream(q: CalendarQuery) -> ModelResult<Vec<CalendarView>> {
    let rows = fetch_all_rows_from_graph(Query::new("calendar_stream", r"
        MATCH (u:User)-[:AUTHORED]->(c:Post {kind:'collection'})-[:COLLECTED]->(event:Post)
        WHERE c.uri CONTAINS '/pub/social/v1/posts/' AND NOT coalesce(c.deleted,false)
          AND NOT coalesce(u.deleted,false) AND NOT coalesce(event.deleted,false)
          AND event.event_content IS NOT NULL
          AND ($author IS NULL OR u.id=$author) AND ($curator IS NULL OR u.id=$curator)
        CALL (event) {
            MATCH (o:EventOccurrence {master:event.uri}) USING INDEX o:EventOccurrence(master) RETURN o
            UNION
            MATCH (o:EventOccurrence {post_uri:event.uri}) USING INDEX o:EventOccurrence(post_uri) RETURN o
        }
        WITH u,c,o WHERE o.end_at>$from AND o.start_at<$to
        WITH u,c,min(o.start_at) AS next_start
        RETURN u.id AS author,c.id AS id ORDER BY next_start,u.id,c.id SKIP $skip LIMIT $limit
    ").param("author",q.author).param("curator",q.curator).param("from",q.from).param("to",q.to).param("skip",q.skip.min(10_000) as i64).param("limit",q.limit.min(50) as i64)).await?;
    let mut views = Vec::new();
    for row in rows {
        if let Some(view) = calendar_view(
            &row.get::<String>("author")?,
            &row.get::<String>("id")?,
            q.from,
            q.to,
            q.occurrences,
        )
        .await?
        {
            views.push(view);
        }
    }
    Ok(views)
}
