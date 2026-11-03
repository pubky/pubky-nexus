//! The queries of the watcher's internal migration: what it reads from a `v0` user's graph at
//! the flip and the addresses it writes back in their `social/v1` form. Nothing else uses
//! them; they go when the internal migration goes.

use super::get::MAX_QUERY_LIMIT;
use crate::db::graph::Query;

/// Every `File` node `owner_id` owns, attached or not, as `(id, src, content_type)` rows
/// ordered by id: what the internal migration builds its file map from.
///
/// The `id IS NOT NULL` predicate is load-bearing: the unique constraint on `(owner_id, id)`
/// is a composite index, which seeks only with a predicate on every property; on `owner_id`
/// alone the planner scans every `File` in the graph.
pub fn get_user_files_sources(owner_id: &str) -> Query {
    Query::new(
        "get_user_files_sources",
        "
        MATCH (f:File)
        WHERE f.owner_id = $owner_id AND f.id IS NOT NULL
        RETURN f.id AS id, f.src AS src, f.content_type AS content_type
        ORDER BY f.id ASC
        ",
    )
    .param("owner_id", owner_id)
}

/// One keyset page of the posts `author_id` wrote, as `(id, attachments)` rows strictly after
/// `after_id` (`""` for the first page), ordered by id, up to `limit`. Tombstones are
/// included: every post the user authored counts. `attachments` is null on a post stored
/// without any.
pub fn get_author_posts_page(author_id: &str, after_id: &str, limit: usize) -> Query {
    Query::new(
        "get_author_posts_page",
        "
        MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post)
        WHERE p.id > $after_id
        RETURN p.id AS id, p.attachments AS attachments
        ORDER BY p.id ASC
        LIMIT $limit
        ",
    )
    .param("author_id", author_id)
    .param("after_id", after_id.to_string())
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
}

/// One keyset page of the replies and reposts of `author_id`'s posts `post_ids`, whoever wrote
/// them, as `(author_id, post_id)` rows strictly after the child post id `after_id` (`""` for
/// the first page), ordered by that id, up to `limit`. A post that both replies to and reposts
/// one of them is one row; a child with no author is left out.
///
/// The `WITH` barrier is load-bearing, as in `set_post_addresses`: it pins the seek on
/// `Post(id)` per id, where the planner may otherwise expand every post of the author for each.
pub fn get_post_children_page(
    author_id: &str,
    post_ids: &[String],
    after_id: &str,
    limit: usize,
) -> Query {
    Query::new(
        "get_post_children_page",
        "
        MATCH (u:User {id: $author_id})
        UNWIND $post_ids AS post_id
        MATCH (p:Post {id: post_id})
        WITH u, p WHERE (u)-[:AUTHORED]->(p)
        MATCH (p)<-[:REPLIED|REPOSTED]-(child:Post)
        WHERE child.id > $after_id
        WITH DISTINCT child
        MATCH (child)<-[:AUTHORED]-(child_author:User)
        RETURN child_author.id AS author_id, child.id AS post_id
        ORDER BY child.id ASC
        LIMIT $limit
        ",
    )
    .param("author_id", author_id)
    .param("post_ids", post_ids.to_vec())
    .param("after_id", after_id.to_string())
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
}

/// Up to `limit` of the `TAGGED` edges `tagger_id` wrote whose address still starts with
/// `prefix`, in no particular order and with no offset: the internal migration moves each
/// edge it rewrites out of this set, so the next read starts over. Each row carries the edge's
/// element id, its `id` and `label`, the target's `kind` (`post`, `user`, `resource`, or
/// `other` for anything else) and the target's keys: `target_id` for a post or a user,
/// `author_id` for a post (null when it has no `AUTHORED` author), `resource_uri` for a
/// resource, and the edge's address before the rewrite, `tag_uri`, which the resource changes
/// feed published it under.
pub fn get_tagger_edges_under(tagger_id: &str, prefix: &str, limit: usize) -> Query {
    Query::new(
        "get_tagger_edges_under",
        "
        MATCH (:User {id: $tagger_id})-[t:TAGGED]->(target)
        WHERE t.uri STARTS WITH $prefix
        WITH t, target LIMIT $limit
        OPTIONAL MATCH (target)<-[:AUTHORED]-(author:User)
        RETURN elementId(t) AS edge,
            t.id AS id,
            t.label AS label,
            CASE
                WHEN target:Post THEN 'post'
                WHEN target:User THEN 'user'
                WHEN target:Resource THEN 'resource'
                ELSE 'other'
            END AS kind,
            target.id AS target_id,
            author.id AS author_id,
            target.uri AS resource_uri,
            t.uri AS tag_uri
        ",
    )
    .param("tagger_id", tagger_id)
    .param("prefix", prefix)
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
}

/// The `Resource` tags `tagger_id` wrote whose address is one of `tag_uris`, as `(tag_uri,
/// label, indexed_at, resource_id, resource_uri)` rows: the edges the tags step moved, read
/// back for the resource changes feed. An address with no such edge has no row.
pub fn get_tagger_resource_tags_at(tagger_id: &str, tag_uris: &[String]) -> Query {
    Query::new(
        "get_tagger_resource_tags_at",
        "
        MATCH (:User {id: $tagger_id})-[t:TAGGED]->(r:Resource)
        WHERE t.uri IN $tag_uris
        RETURN t.uri AS tag_uri,
            t.label AS label,
            t.indexed_at AS indexed_at,
            r.id AS resource_id,
            r.uri AS resource_uri
        ",
    )
    .param("tagger_id", tagger_id)
    .param("tag_uris", tag_uris.to_vec())
}

/// Writes the stored address and the attachments of each of `author_id`'s posts in `posts`,
/// `(post_id, uri, attachments)` rows, in one batch. A post stored with no attachments
/// (`None`) keeps none, whatever the row carries, so a rewrite never turns a missing list
/// into an empty one. A post that is not theirs is left alone.
///
/// The `WITH` barrier is load-bearing, as in `get_post_by_id`: it pins the seek on `Post(id)`
/// per row, where the planner may otherwise expand every post of the author for each row.
pub fn set_post_addresses(
    author_id: &str,
    posts: &[(String, String, Option<Vec<String>>)],
) -> Query {
    let addresses: Vec<Vec<String>> = posts
        .iter()
        .map(|(post_id, uri, _)| vec![post_id.clone(), uri.clone()])
        .collect();
    let attachments: Vec<Vec<String>> = posts
        .iter()
        .map(|(_, _, attachments)| attachments.clone().unwrap_or_default())
        .collect();
    Query::new(
        "set_post_addresses",
        "
        MATCH (u:User {id: $author_id})
        UNWIND range(0, size($posts) - 1) AS i
        WITH u, $posts[i] AS post, $attachments[i] AS attachments
        MATCH (p:Post {id: post[0]})
        WITH u, p, post, attachments WHERE (u)-[:AUTHORED]->(p)
        SET p.uri = post[1],
            p.attachments = CASE WHEN p.attachments IS NULL THEN null ELSE attachments END
        ",
    )
    .param("author_id", author_id)
    .param("posts", addresses)
    .param("attachments", attachments)
}

/// Writes the `id` and the `uri` of each of `tagger_id`'s `TAGGED` edges in `tags`, `(edge,
/// id, uri)` rows keyed by the edge's element id, one element-id seek per row, and returns how
/// many it wrote as `changed`. An edge that is gone, or not the tagger's, is left alone and not
/// counted.
///
/// The anonymous pattern is load-bearing: anchored on the tagger, the planner expands every
/// edge they wrote for each row; anchored on nothing, it seeks the edge by its element id
/// (`DirectedRelationshipByElementIdSeek`) and checks the tagger on the start node.
pub fn set_tag_addresses(tagger_id: &str, tags: &[(String, String, String)]) -> Query {
    let tags: Vec<Vec<String>> = tags
        .iter()
        .map(|(edge, id, uri)| vec![edge.clone(), id.clone(), uri.clone()])
        .collect();
    Query::new(
        "set_tag_addresses",
        "
        UNWIND $tags AS tag
        MATCH ()-[t:TAGGED]->()
        WHERE elementId(t) = tag[0] AND startNode(t).id = $tagger_id
        SET t.id = tag[1], t.uri = tag[2]
        RETURN count(t) AS changed
        ",
    )
    .param("tagger_id", tagger_id)
    .param("tags", tags)
}
