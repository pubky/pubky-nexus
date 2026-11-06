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
/// resource.
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
            target.uri AS resource_uri
        ",
    )
    .param("tagger_id", tagger_id)
    .param("prefix", prefix)
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
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
/// counted. An edge whose id changes keeps its old one as `v0_id`, which a delete of its
/// `pubky.app` file still finds it by; an edge whose id stays keeps the `v0_id` it has.
///
/// A `Resource` edge whose address changes is marked as moved, for the resource changes feed,
/// which published it under the old address: `v0_uri` keeps that address as stored, and
/// `moved_at` is `moved_at`, until the move is published (see [`get_moved_resource_tags`]). An
/// edge already at its new address, as on a rerun, keeps both as they are, so a rerun never
/// overwrites the address the feed knows.
///
/// The anonymous start node is load-bearing: anchored on the tagger, the planner expands every
/// edge they wrote for each row; anchored on nothing, it seeks the edge by its element id
/// (`DirectedRelationshipByElementIdSeek`) and checks the tagger on the start node.
pub fn set_tag_addresses(
    tagger_id: &str,
    tags: &[(String, String, String)],
    moved_at: i64,
) -> Query {
    let tags: Vec<Vec<String>> = tags
        .iter()
        .map(|(edge, id, uri)| vec![edge.clone(), id.clone(), uri.clone()])
        .collect();
    Query::new(
        "set_tag_addresses",
        "
        UNWIND $tags AS tag
        MATCH ()-[t:TAGGED]->(target)
        WHERE elementId(t) = tag[0] AND startNode(t).id = $tagger_id
        WITH t, tag, coalesce(target:Resource AND t.uri <> tag[2], false) AS moves
        SET t.v0_id = CASE WHEN t.id = tag[1] THEN t.v0_id ELSE t.id END,
            t.v0_uri = CASE WHEN moves THEN t.uri ELSE t.v0_uri END,
            t.moved_at = CASE WHEN moves THEN $moved_at ELSE t.moved_at END
        SET t.id = tag[1], t.uri = tag[2]
        RETURN count(t) AS changed
        ",
    )
    .param("tagger_id", tagger_id)
    .param("tags", tags)
    .param("moved_at", moved_at)
}

/// One keyset page of the `Resource` tag edges whose move the resource changes feed has not
/// published yet (`moved_at` set, see [`set_tag_addresses`]), of `tagger_id` or, with `None`,
/// of every tagger: `(edge, tagger_id, tag_id, v0_id, moved_at)` rows ordered by `moved_at`,
/// then the edge's element id, strictly after `after` (`(i64::MIN, "")` for the first page),
/// up to `limit`. The keyset leads with a range on `moved_at`, so without a tagger the page is
/// a seek on the `TAGGED(moved_at)` index, which holds only the edges still to publish.
pub fn get_moved_resource_tags(tagger_id: Option<&str>, after: (i64, &str), limit: usize) -> Query {
    let (name, pattern) = match tagger_id {
        Some(_) => (
            "get_moved_resource_tags_of",
            "MATCH (u:User {id: $tagger_id})-[t:TAGGED]->(:Resource)",
        ),
        None => (
            "get_moved_resource_tags",
            "MATCH (u:User)-[t:TAGGED]->(:Resource)",
        ),
    };
    Query::new(
        name,
        format!(
            "
        {pattern}
        WHERE t.moved_at >= $after_at
            AND (t.moved_at > $after_at OR elementId(t) > $after_edge)
        WITH u, t
        ORDER BY t.moved_at, elementId(t)
        LIMIT $limit
        RETURN elementId(t) AS edge,
            u.id AS tagger_id,
            t.id AS tag_id,
            t.v0_id AS v0_id,
            t.moved_at AS moved_at
        "
        ),
    )
    .param("tagger_id", tagger_id.unwrap_or_default())
    .param("after_at", after.0)
    .param("after_edge", after.1)
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
}

/// The `Resource` tag edges of `edges`, `(edge, tagger_id, tag_id)` triples, whose move is still
/// to publish, as `(edge, tagger_id, tag_id, v0_id, tag_uri, v0_uri, moved_at, label,
/// indexed_at, resource_id, resource_uri)` rows. An edge that is gone, has another id or
/// tagger, or whose move was published has no row. Seeks each edge by its element id, as
/// [`set_tag_addresses`] does.
pub fn get_moved_resource_tags_at(edges: &[(String, String, String)]) -> Query {
    let (ids, taggers, tag_ids) = unzip3(edges);
    Query::new(
        "get_moved_resource_tags_at",
        "
        UNWIND range(0, size($edges) - 1) AS i
        MATCH ()-[t:TAGGED]->(r:Resource)
        WHERE elementId(t) = $edges[i] AND startNode(t).id = $taggers[i] AND t.id = $tag_ids[i]
            AND t.moved_at IS NOT NULL
        RETURN $edges[i] AS edge,
            $taggers[i] AS tagger_id,
            t.id AS tag_id,
            t.v0_id AS v0_id,
            t.uri AS tag_uri,
            t.v0_uri AS v0_uri,
            t.moved_at AS moved_at,
            t.label AS label,
            t.indexed_at AS indexed_at,
            r.id AS resource_id,
            r.uri AS resource_uri
        ",
    )
    .param("edges", ids)
    .param("taggers", taggers)
    .param("tag_ids", tag_ids)
}

/// Marks the moves of the `Resource` tag edges `edges`, `(edge, tagger_id)` pairs, as
/// published, each only if its `moved_at` is still the one in `moved_at` (same order), and
/// returns how many it changed as `cleared`.
pub fn clear_moved_resource_tags(edges: &[(String, String)], moved_at: &[i64]) -> Query {
    let (ids, taggers): (Vec<String>, Vec<String>) = edges.iter().cloned().unzip();
    Query::new(
        "clear_moved_resource_tags",
        "
        UNWIND range(0, size($edges) - 1) AS i
        MATCH ()-[t:TAGGED]->(:Resource)
        WHERE elementId(t) = $edges[i] AND startNode(t).id = $taggers[i]
            AND t.moved_at = $moved_at[i]
        SET t.moved_at = null
        RETURN count(t) AS cleared
        ",
    )
    .param("edges", ids)
    .param("taggers", taggers)
    .param("moved_at", moved_at.to_vec())
}

fn unzip3(rows: &[(String, String, String)]) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut out = (Vec::new(), Vec::new(), Vec::new());
    for (a, b, c) in rows {
        out.0.push(a.clone());
        out.1.push(b.clone());
        out.2.push(c.clone());
    }
    out
}
