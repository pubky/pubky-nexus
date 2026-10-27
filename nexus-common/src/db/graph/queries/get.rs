use crate::db::graph::error::{GraphError, GraphResult};
use crate::db::graph::Query;
use crate::db::kv::SortOrder;
use crate::models::post::{KindFilter, StreamSource};
use crate::models::resource::stream::ResourceSorting;
use crate::types::routes::HotTagsInputDTO;
use crate::types::CacheTimeframe;
use crate::types::DomainTrust;
use crate::types::Pagination;
use crate::types::StreamReach;
use crate::types::StreamSorting;
use crate::types::Timeframe;
use crate::types::WotDepth;

// Defense-in-depth: cap SKIP and LIMIT before splicing into Cypher so a future
// route regression can't produce runaway result sets or excessive skip cost.
const MAX_QUERY_SKIP: usize = 10_000;
const MAX_QUERY_LIMIT: usize = 1_000;

// Retrieve post node by post id and author id
pub fn get_post_by_id(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "get_post_by_id",
        "
            // The WITH barrier is load-bearing: without it the planner anchors on the
            // author and expands all their posts instead of seeking the post by id.
            MATCH (p:Post {id: $post_id})
            WITH p
            MATCH (u:User {id: $author_id}) WHERE (u)-[:AUTHORED]->(p)
            OPTIONAL MATCH (p)-[replied:REPLIED]->(parent_post:Post)<-[:AUTHORED]-(author:User)
            WITH u, p, parent_post, author
            RETURN {
                uri: p.uri,
                content: p.content,
                id: p.id,
                indexed_at: p.indexed_at,
                author: u.id,
                // default value when the specified property is null
                // Avoids enum deserialization ERROR
                kind: COALESCE(p.kind, 'short'),
                attachments: p.attachments,
                lock: p.lock,
                // Posts written before the flag lack the property and are live
                deleted: COALESCE(p.deleted, false)
            } as details,
            COLLECT([author.id, parent_post.id]) AS reply

        ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn post_counts(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "post_counts",
        "
        // Anchor on the post's unique id: matching via the author would make the
        // planner expand every post they wrote.
        MATCH (p:Post {id: $post_id})
        WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
        WITH p
        OPTIONAL MATCH (p)<-[t:TAGGED]-()
        WITH p, COUNT (t) AS tags_count, COUNT(DISTINCT t.label) AS unique_tags_count
        RETURN p IS NOT NULL AS exists,
            {
                tags: tags_count,
                unique_tags: unique_tags_count,
                replies: COUNT { (p)<-[:REPLIED]-() },
                reposts: COUNT { (p)<-[:REPOSTED]-() },
                collections: COUNT { (p)<-[:COLLECTED]-() }
            } AS counts,
            EXISTS { (p)-[:REPLIED]->(:Post) } AS is_reply
    ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Check if the viewer_id has a bookmark in the post
pub fn post_bookmark(author_id: &str, post_id: &str, viewer_id: &str) -> Query {
    Query::new(
        "post_bookmark",
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         MATCH (viewer:User {id: $viewer_id})-[b:BOOKMARKED]->(p)
         RETURN b",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("viewer_id", viewer_id)
}

// Keyset pagination on p.id (indexed).
pub fn user_bookmarks(user_id: &str, cursor: &str, limit: i64) -> Query {
    Query::new(
        "user_bookmarks",
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED]->(p:Post)<-[:AUTHORED]-(author:User)
         WHERE p.id > $cursor
         RETURN b, p.id AS post_id, author.id AS author_id
         ORDER BY p.id LIMIT $limit",
    )
    .param("user_id", user_id)
    .param("cursor", cursor)
    .param("limit", limit)
}

// b.id collides across bookmarkers (hashes only the post uri), but BOOKMARKED is MERGEd
// per (user, post), so bookmarker.id is unique per row and keyset pagination uses it.
pub fn get_post_bookmarks(author_id: &str, post_id: &str, cursor: &[String], limit: i64) -> Query {
    Query::new(
        "get_post_bookmarks",
        "MATCH (bookmarker:User)-[b:BOOKMARKED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         WHERE [bookmarker.id] > $cursor AND bookmarker.id IS NOT NULL
         RETURN b.id AS bookmark_id, bookmarker.id AS bookmarker_id, [bookmarker.id] AS cursor
         ORDER BY bookmarker.id
         LIMIT $limit",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("cursor", cursor.to_vec())
    .param("limit", limit)
}

// Read the target (post_id, author_id) for a bookmark without deleting the edge.
// Used in sync_del to read before graph-last deletion.
pub fn get_bookmark_target(user_id: &str, bookmark_id: &str) -> Query {
    Query::new(
        "get_bookmark_target",
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED {id: $bookmark_id}]->(post:Post)<-[:AUTHORED]-(author:User)
         RETURN post.id AS post_id, author.id AS author_id",
    )
    .param("user_id", user_id)
    .param("bookmark_id", bookmark_id)
}

// Get all the reposts that a post has received (used for edit/delete notifications)
// Keyset pagination on the repost Post id (globally unique + indexed).
pub fn get_post_reposts(author_id: &str, post_id: &str, cursor: &[String], limit: i64) -> Query {
    Query::new(
        "get_post_reposts",
        "MATCH (reposter:User)-[:AUTHORED]->(repost:Post)-[:REPOSTED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         WHERE [repost.id] > $cursor AND repost.id IS NOT NULL
         RETURN reposter.id AS reposter_id, repost.uri AS repost_uri, [repost.id] AS cursor
         ORDER BY repost.id
         LIMIT $limit",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("cursor", cursor.to_vec())
    .param("limit", limit)
}

// Get all the replies that a post has received (used for edit/delete notifications)
// Keyset pagination on the reply Post id (globally unique + indexed).
pub fn get_post_replies(author_id: &str, post_id: &str, cursor: &[String], limit: i64) -> Query {
    Query::new(
        "get_post_replies",
        "MATCH (replier:User)-[:AUTHORED]->(reply:Post)-[:REPLIED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         WHERE [reply.id] > $cursor AND reply.id IS NOT NULL
         RETURN replier.id AS replier_id, reply.uri AS reply_uri, [reply.id] AS cursor
         ORDER BY reply.id
         LIMIT $limit",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("cursor", cursor.to_vec())
    .param("limit", limit)
}

// Read the target details for a tag without deleting the TAGGED edge.
// Used in tag del to read before graph-last deletion.
pub fn get_tag_target(user_id: &str, tag_id: &str, app: Option<&str>) -> Query {
    let app_filter = match app {
        Some(_) => "\n    WHERE tag.app = $app",
        None => "\n    WHERE tag.app IS NULL",
    };

    let cypher = format!(
        "MATCH (user:User {{id: $user_id}})-[tag:TAGGED {{id: $tag_id}}]->(target){app_filter}
         OPTIONAL MATCH (target)<-[:AUTHORED]-(author:User)
         WITH CASE WHEN target:User THEN target.id ELSE null END AS user_id,
              CASE WHEN target:Post THEN target.id ELSE null END AS post_id,
              CASE WHEN target:Post THEN author.id ELSE null END AS author_id,
              CASE WHEN target:Post THEN target.uri ELSE null END AS post_uri,
              CASE WHEN target:Resource THEN target.id ELSE null END AS resource_id,
              CASE WHEN target:Resource THEN target.uri ELSE null END AS resource_uri,
              tag.indexed_at AS tag_indexed_at,
              tag.label AS label,
              tag.app AS app
         RETURN user_id, post_id, author_id, post_uri, resource_id, resource_uri, tag_indexed_at, label, app"
    );

    let mut query = Query::new("get_tag_target", &cypher)
        .param("user_id", user_id)
        .param("tag_id", tag_id);

    if let Some(a) = app {
        query = query.param("app", a);
    }

    query
}

/// Stored time of a resource tag, including a normalization-equivalent replay
/// whose raw URI produced a different object ID.
pub fn get_resource_tag_indexed_at(
    author: &str,
    resource_id: &str,
    label: &str,
    app: &str,
) -> Query {
    Query::new("get_resource_tag_indexed_at", "MATCH (:User {id: $author})-[t:TAGGED {label: $label, app: $app}]->(:Resource {id: $resource_id}) RETURN t.indexed_at AS tag_indexed_at")
        .param("author", author).param("resource_id", resource_id).param("label", label).param("app", app)
}

// t.id collides across taggers (hashes only {target_uri, label}), so the keyset is the row value
// [tagger.id, t.id]: TAGGED is MERGEd per (tagger, post, label) and t.id maps 1:1 to label within
// a post, which makes the pair unique per row.
// Cursor columns are guarded IS NOT NULL: `['a', null] > []` is true, so a null column would come
// back and fail to deserialize into the cursor, aborting the scan. create_post_tag always sets
// t.id, so the guard only ever excludes corrupt edges.
pub fn get_post_tags(author_id: &str, post_id: &str, cursor: &[String], limit: i64) -> Query {
    Query::new(
        "get_post_tags",
        "MATCH (p:Post {id: $post_id})
         WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
         MATCH (tagger:User)-[t:TAGGED]->(p)
         WHERE [tagger.id, t.id] > $cursor AND tagger.id IS NOT NULL AND t.id IS NOT NULL
         RETURN tagger.id AS tagger_id, t.uri AS tag_uri, [tagger.id, t.id] AS cursor
         ORDER BY tagger.id, t.id
         LIMIT $limit",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("cursor", cursor.to_vec())
    .param("limit", limit)
}

pub fn post_relationships(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "post_relationships",
        "MATCH (p:Post {id: $post_id})
        WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
        OPTIONAL MATCH (p)-[:REPLIED]->(replied_post:Post)<-[:AUTHORED]-(replied_author:User)
        OPTIONAL MATCH (p)-[:REPOSTED]->(reposted_post:Post)<-[:AUTHORED]-(reposted_author:User)
        OPTIONAL MATCH (p)-[:MENTIONED]->(mentioned_user:User)
        RETURN
          replied_post.uri AS replied_uri,
          reposted_post.uri AS reposted_uri,
          COLLECT(mentioned_user.id) AS mentioned_user_ids",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

/// The posts a post replies to and reposts: each one's stored address and key.
pub fn post_parents(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "post_parents",
        "MATCH (p:Post {id: $post_id})
        WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
        OPTIONAL MATCH (p)-[:REPLIED]->(replied_post:Post)<-[:AUTHORED]-(replied_author:User)
        OPTIONAL MATCH (p)-[:REPOSTED]->(reposted_post:Post)<-[:AUTHORED]-(reposted_author:User)
        RETURN
          replied_post.uri AS replied_uri,
          replied_author.id AS replied_author_id,
          replied_post.id AS replied_post_id,
          reposted_post.uri AS reposted_uri,
          reposted_author.id AS reposted_author_id,
          reposted_post.id AS reposted_post_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Retrieve many users by id
// We return also id if not we will not get not found users
pub fn get_users_details_by_ids(user_ids: &[&str]) -> Query {
    Query::new(
        "get_users_details_by_ids",
        "
        UNWIND $ids AS id
        OPTIONAL MATCH (record:User {id: id})
        RETURN
            id,
            CASE
                WHEN record IS NOT NULL
                    THEN record
                    ELSE null
                END AS record
        ",
    )
    .param("ids", user_ids)
}

/// Retrieves unique global tags for posts, returning a list of `post_ids` and `timestamp` pairs for each tag label.
pub fn global_tags_by_post() -> Query {
    Query::new(
        "global_tags_by_post",
        "
        MATCH (tagger:User)-[t:TAGGED]->(post:Post)<-[:AUTHORED]-(author:User)
        WITH t.label AS label, author.id + ':' + post.id AS post_id, post.indexed_at AS score
        WITH DISTINCT post_id, label, score
        WITH label, COLLECT([toFloat(score), post_id ]) AS sorted_set
        RETURN label, sorted_set
        ",
    )
}

// TODO: Do not traverse all the graph again to get the engagement score. Rethink how to share that info in the indexer
/// Retrieves unique global tags for posts, calculating an engagement score based on tag counts,
/// replies, reposts and mentions. The query returns a `key` by combining author's ID
/// and post's ID, along with a sorted set of engagement scores for each tag label.
pub fn global_tags_by_post_engagement() -> Query {
    Query::new(
        "global_tags_by_post_engagement",
        "
        MATCH (author:User)-[:AUTHORED]->(post:Post)<-[tag:TAGGED]-(tagger:User)
        WITH post, COUNT(tag) AS tags_count, tag.label AS label, author.id + ':' + post.id AS key
        WITH DISTINCT key, label, post, tags_count
        WHERE tags_count > 0
        // Each engagement count is its own COUNT{} subquery, so they don't
        // multiply into a cartesian product per post.
        WITH key, label,
             COUNT { (post)<-[:TAGGED]-() } AS taggers,
             COUNT { (post)<-[:REPLIED]-() } AS replies_count,
             COUNT { (post)<-[:REPOSTED]-() } AS reposts_count,
             COUNT { (post)-[:MENTIONED]->() } AS mention_count
        WITH label, COLLECT([toFloat(taggers + replies_count + reposts_count + mention_count), key ]) AS sorted_set
        RETURN label, sorted_set
        order by label
        ",
    )
}

/// Retrieves unique global tags on user profiles, returning per label the tagged
/// user ids paired with their distinct tagger counts.
pub fn global_tags_by_user() -> Query {
    Query::new(
        "global_tags_by_user",
        "
        // create_user_tag MERGEs one TAGGED edge per (tagger, tagged, label),
        // so COUNT(t) is the distinct tagger count.
        MATCH (tagger:User)-[t:TAGGED]->(u:User)
        WHERE NOT coalesce(u.deleted, false)
        WITH t.label AS label, u.id AS user_id, COUNT(t) AS score
        WITH label, COLLECT([toFloat(score), user_id]) AS sorted_set
        RETURN label, sorted_set
        ",
    )
}

/// Enumerates the distinct (tagged user, label) pairs carried by user
/// profiles, without counts: backfills derive each score from the live
/// taggers set instead of trusting a snapshot value.
pub fn get_user_tag_pairs() -> Query {
    Query::new(
        "get_user_tag_pairs",
        "
        MATCH (:User)-[t:TAGGED]->(u:User)
        WHERE NOT coalesce(u.deleted, false)
        RETURN DISTINCT u.id AS user_id, t.label AS label
        ",
    )
}

/// Users carrying positive trust, highest first: the ranked population behind
/// the social graph badge.
///
/// A node unreachable from the seed set may carry `0.0` or no `trust` property
/// at all, and `null > 0` is null, so both drop out here. Only positive scores
/// are ranked: everyone at zero shares one value, so their order would be
/// arbitrary and would reshuffle every run. Absence from the ranking is what
/// marks them new.
///
/// `user_id ASC` breaks score ties deterministically, so two runs over the same
/// graph agree.
pub fn get_trust_ranked_user_ids() -> Query {
    Query::new(
        "get_trust_ranked_user_ids",
        format!(
            "
        MATCH (u:User)
        WHERE {}
        RETURN u.id AS user_id
        ORDER BY u.trust DESC, user_id ASC
        ",
            ranked_user("u")
        ),
    )
}

/// Who counts as ranked, for the user bound to `var`: the ranking and the
/// `source=all` stream rule share it, so both hide the same authors.
fn ranked_user(var: &str) -> String {
    format!("{var}.trust > 0 AND NOT coalesce({var}.deleted, false)")
}

/// One keyset page of the most-followed ranking, ordered by `id`, strictly after
/// `after_id` (`""` for the first page). It holds every user who is neither
/// tombstoned nor a discovery stub, scored by the [`user_counts_by_ids`] follower count,
/// so a user with no followers ranks with 0.
pub fn get_most_followed_users(after_id: &str, limit: usize) -> Query {
    let cypher = format!(
        "
        MATCH (u:User)
        WHERE u.id > $after_id
          AND NOT coalesce(u.deleted, false)
          // A discovery stub is named after its id, which no profile name can be.
          AND u.name <> u.id
        RETURN u.id AS id, {FOLLOWERS_COUNT} AS followers
        ORDER BY u.id
        LIMIT $limit
        "
    );
    Query::new("get_most_followed_users", &cypher)
        .param("after_id", after_id)
        .param("limit", limit as i64)
}

/// Ids of the users who wrote at least one post.
pub fn post_author_ids() -> Query {
    Query::new(
        "post_author_ids",
        "
        MATCH (u:User)
        WHERE EXISTS { (u)-[:AUTHORED]->(:Post) }
        RETURN collect(u.id) AS user_ids
        ",
    )
}

/// Up to `limit` posts `author_id` wrote after the `(indexed_at, id)` cursor
/// `after`, oldest first, with what the shared sorted sets index them by,
/// scored as a full reindex scores them.
pub fn author_post_entries(author_id: &str, after: (i64, &str), limit: usize) -> Query {
    Query::new(
        "author_post_entries",
        "
        MATCH (author:User {id: $author_id})-[:AUTHORED]->(p:Post)
        WHERE p.indexed_at > $after_indexed_at
           OR (p.indexed_at = $after_indexed_at AND p.id > $after_id)
        WITH author, p
        ORDER BY p.indexed_at, p.id
        LIMIT $limit
        RETURN author.id AS author_id,
            p.id AS post_id,
            p.indexed_at AS indexed_at,
            EXISTS { (p)-[:REPLIED]->(:Post) } AS is_reply,
            COLLECT { MATCH (p)<-[tag:TAGGED]-(:User) RETURN DISTINCT tag.label } AS labels,
            COUNT { (p)<-[:TAGGED]-() }
                + COUNT { (p)<-[:REPLIED]-() }
                + COUNT { (p)<-[:REPOSTED]-() } AS engagement,
            COUNT { (p)-[:MENTIONED]->() } AS mentions
        ",
    )
    .param("author_id", author_id)
    .param("after_indexed_at", after.0)
    .param("after_id", after.1)
    .param("limit", limit as i64)
}

/// One keyset page of user ids, ordered by `id`, strictly after `after_id`
/// (`""` for the first page). Feed the last id of a page back as `after_id`
/// for the next; an empty page means the end.
///
/// Tombstoned users and stubs are included: every `:User` node counts.
pub fn get_user_ids_page(after_id: &str, limit: usize) -> Query {
    Query::new(
        "get_user_ids_page",
        "
        MATCH (u:User)
        WHERE u.id > $after_id
        RETURN u.id AS id
        ORDER BY u.id ASC
        LIMIT $limit
        ",
    )
    .param("after_id", after_id.to_string())
    .param("limit", limit.min(MAX_QUERY_LIMIT) as i64)
}

/// Users whose profile carries any of the given tag labels, scored by distinct
/// tagger count summed across the searched labels.
pub fn search_users_by_tags(labels: &[String], skip: Option<usize>, limit: Option<usize>) -> Query {
    Query::new(
        "search_users_by_tags",
        users_by_tags_cypher("", skip, limit),
    )
    .param("labels", labels.to_vec())
}

/// [`search_users_by_tags`] restricted to the users in `user_id`'s `reach`,
/// excluding `user_id` itself. Pages inside the graph, so only the requested
/// window leaves Neo4j.
pub fn search_users_by_tags_with_reach(
    labels: &[String],
    user_id: &str,
    reach: &StreamReach,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Query {
    let reach_prefix = format!(
        "MATCH (user:User {{id: $user_id}})
        {}
        WHERE reach.id <> $user_id
        // A WoT expand yields one row per path, so dedupe the reach before the
        // tag join, or a user's tags count once per path. DISTINCT right after
        // the variable-length expand also lets the planner use a pruning BFS.
        // Starting from the tag with an EXISTS {{ (user)-[:FOLLOWS*1..d]->(u) }}
        // check was slower: it runs an unpruned expand per tagged user.
        WITH DISTINCT reach AS u",
        stream_reach_to_graph_subquery(reach)
    );

    reach_attrs(
        Query::new(
            "search_users_by_tags_with_reach",
            users_by_tags_cypher(&reach_prefix, skip, limit),
        ),
        reach,
    )
    .param("labels", labels.to_vec())
    .param("user_id", user_id)
}

/// Shared scoring for the user tag searches. `prefix` may bind `u` to narrow
/// the tagged users; empty searches every user.
fn users_by_tags_cypher(prefix: &str, skip: Option<usize>, limit: Option<usize>) -> String {
    let mut cypher = format!(
        "
        {prefix}
        MATCH (tagger:User)-[tag:TAGGED]->(u:User)
        WHERE tag.label IN $labels AND NOT coalesce(u.deleted, false)
        WITH u, COUNT(tag) AS score
        RETURN u.id AS user_id, score
        // id DESC matches how Redis breaks equal scores (reverse-lex member
        // order), keeping pagination windows identical across both paths
        ORDER BY score DESC, u.id DESC
        "
    );

    if let Some(skip) = skip {
        cypher.push_str(&format!("SKIP {}\n", skip.min(MAX_QUERY_SKIP)));
    }
    if let Some(limit) = limit {
        cypher.push_str(&format!("LIMIT {}\n", limit.min(MAX_QUERY_LIMIT)));
    }
    cypher
}

// Retrieve all the tags of the post
// `tag_uri` is the viewer's tag address per label, null without a viewer
pub fn post_tags(user_id: &str, post_id: &str, viewer_id: Option<&str>) -> Query {
    Query::new(
        "post_tags",
        "
        // Anchor on the post's unique id: matching via the author would make the
        // planner expand every post they wrote.
        MATCH (p:Post {id: $post_id})
        WITH p
        MATCH (u:User {id: $user_id}) WHERE (u)-[:AUTHORED]->(p)
        CALL {
            WITH p
            MATCH (tagger:User)-[tag:TAGGED]->(p)
            WITH tag.label AS name, collect(DISTINCT tagger.id) AS tagger_ids,
                 min(CASE WHEN tagger.id = $viewer_id THEN tag.uri END) AS viewer_uri
            RETURN collect({
                label: name,
                taggers: tagger_ids,
                taggers_count: SIZE(tagger_ids),
                tag_uri: viewer_uri
            }) AS tags
        }
        RETURN
            u IS NOT NULL AS exists,
            tags
    ",
    )
    .param("user_id", user_id)
    .param("post_id", post_id)
    .param("viewer_id", viewer_id.map(str::to_string))
}

// Retrieve all the tags of the user
// `tag_uri` is the viewer's tag address per label, null without a viewer
pub fn user_tags(user_id: &str, viewer_id: Option<&str>) -> Query {
    Query::new(
        "user_tags",
        "
        MATCH (u:User {id: $user_id})
        CALL {
            WITH u
            MATCH (p:User)-[t:TAGGED]->(u)
            WITH t.label AS name, collect(DISTINCT p.id) AS tagger_ids,
                 min(CASE WHEN p.id = $viewer_id THEN t.uri END) AS viewer_uri
            RETURN collect({
                label: name,
                taggers: tagger_ids,
                taggers_count: SIZE(tagger_ids),
                tag_uri: viewer_uri
            }) AS tags
        }
        RETURN
            u IS NOT NULL AS exists,
            tags
    ",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id.map(str::to_string))
}

/// The viewer's tag addresses (`t.uri`) on a post, for the given labels
pub fn viewer_post_tag_uris(
    author_id: &str,
    post_id: &str,
    viewer_id: &str,
    labels: Vec<String>,
) -> Query {
    Query::new(
        "viewer_post_tag_uris",
        "
        MATCH (p:Post {id: $post_id})
        WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
        MATCH (:User {id: $viewer_id})-[t:TAGGED]->(p)
        WHERE t.label IN $labels AND t.uri IS NOT NULL
        RETURN t.label AS label, min(t.uri) AS uri
    ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("viewer_id", viewer_id)
    .param("labels", labels)
}

/// The viewer's tag addresses (`t.uri`) on a user, for the given labels
pub fn viewer_user_tag_uris(user_id: &str, viewer_id: &str, labels: Vec<String>) -> Query {
    Query::new(
        "viewer_user_tag_uris",
        "
        MATCH (:User {id: $viewer_id})-[t:TAGGED]->(:User {id: $user_id})
        WHERE t.label IN $labels AND t.uri IS NOT NULL
        RETURN t.label AS label, min(t.uri) AS uri
    ",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id)
    .param("labels", labels)
}

/// A Resource node's details and a page of its tags, in one read. Labels are
/// ordered by tagger count, then label, and paginated with `skip_tags`/`limit_tags`.
/// Each label carries its full `taggers_count`, at most `limit_taggers` tagger ids,
/// and `tag_uri`, the viewer's tag address, looked up among all of its taggers, not
/// only the listed ones; null without a viewer. With one edge per app, `min` picks
/// the same address as `resource_label_taggers`. A missing Resource returns
/// `details = null`; an existing one without tags returns `tags = []`.
pub fn resource_view(
    resource_id: &str,
    viewer_id: Option<&str>,
    skip_tags: usize,
    limit_tags: usize,
    limit_taggers: usize,
) -> Query {
    Query::new(
        "resource_view",
        "
        OPTIONAL MATCH (r:Resource {id: $resource_id})
        OPTIONAL MATCH (viewer:User {id: $viewer_id})
        CALL {
            WITH r, viewer
            MATCH (tagger:User)-[tag:TAGGED]->(r)
            // Collect nodes, not ids: only the listed taggers' ids are read,
            // and the viewer's edges are found by comparing nodes
            WITH tag.label AS name, collect(DISTINCT tagger) AS taggers,
                 min(CASE WHEN tagger = viewer THEN tag.uri END) AS viewer_uri
            WITH name, taggers, viewer_uri, SIZE(taggers) AS taggers_count
            ORDER BY taggers_count DESC, name ASC
            SKIP $skip_tags
            LIMIT $limit_tags
            RETURN collect({
                label: name,
                taggers: [tagger IN taggers[0..$limit_taggers] | tagger.id],
                taggers_count: taggers_count,
                tag_uri: viewer_uri
            }) AS tags
        }
        RETURN r {.id, .uri, .scheme, .indexed_at} AS details, tags
    ",
    )
    .param("resource_id", resource_id)
    .param("viewer_id", viewer_id.map(str::to_string))
    .param("skip_tags", skip_tags as i64)
    .param("limit_tags", limit_tags as i64)
    .param("limit_taggers", limit_taggers as i64)
}

/// Taggers of `label` on a Resource node, sorted by user id and paged with
/// `skip`/`limit`. A user who tagged the label from two apps is listed once.
/// `tag_uri` is the viewer's tag address on the label, null without a viewer or
/// their tag; with one edge per app, `min` picks the same address as
/// `resource_view`. A missing Resource has no taggers.
///
/// The index hint makes both matches start from the Resource, so the cost follows
/// the resource's own tags. Left to itself, the planner can seek every TAGGED edge
/// with the label through `taggedLabelIndex`, across posts and users too.
pub fn resource_label_taggers(
    resource_id: &str,
    label: &str,
    viewer_id: Option<&str>,
    skip: usize,
    limit: usize,
) -> Query {
    Query::new(
        "resource_label_taggers",
        "
        OPTIONAL MATCH (viewer:User {id: $viewer_id})
        CALL {
            MATCH (tagger:User)-[tag:TAGGED]->(r:Resource {id: $resource_id})
            USING INDEX r:Resource(id)
            WHERE tag.label = $label
            WITH DISTINCT tagger.id AS tagger_id
            ORDER BY tagger_id
            SKIP $skip
            LIMIT $limit
            RETURN collect(tagger_id) AS taggers
        }
        CALL {
            WITH viewer
            OPTIONAL MATCH (viewer)-[tag:TAGGED]->(r:Resource {id: $resource_id})
            USING INDEX r:Resource(id)
            WHERE tag.label = $label
            RETURN min(tag.uri) AS tag_uri
        }
        RETURN taggers, tag_uri
    ",
    )
    .param("resource_id", resource_id)
    .param("label", label)
    .param("viewer_id", viewer_id.map(str::to_string))
    .param("skip", skip as i64)
    .param("limit", limit as i64)
}

/// Query a stream of Resources with optional app and tag filters.
///
/// Ordering:
/// - `Timeline` sorts by the newest matching TAGGED edge (`MAX(t.indexed_at)`),
///   not by when the Resource node was created. Adding a newer tag moves the
///   resource up; removing it drops the resource back to its latest remaining
///   tag. With an app or label filter, only the matching edges count.
///
/// `score` is the sort value (the timestamp), so callers can hand it
/// back as a cursor. `start` is the resume cursor and `end` the hard limit,
/// both inclusive and following the sort direction (`post_stream` uses the
/// same rule). `r.id` breaks ties deterministically within a response, but
/// paging across ties is still best-effort: the cursor carries only the score,
/// not the id, so resuming from inside a tie group takes a `skip` covering
/// every already-served row with that score.
pub fn resource_stream(
    app: Option<&str>,
    labels: Option<&[String]>,
    sorting: &ResourceSorting,
    order: &SortOrder,
    pagination: &Pagination,
) -> Query {
    // Map enums to safe Cypher literals — prevents injection
    let order_direction = match order {
        SortOrder::Ascending => "ASC",
        SortOrder::Descending => "DESC",
    };
    let (start_op, end_op) = match order {
        SortOrder::Descending => ("<=", ">="),
        SortOrder::Ascending => (">=", "<="),
    };

    let mut cypher = String::from("MATCH (tagger:User)-[t:TAGGED]->(r:Resource)\n");

    let mut where_clauses = Vec::new();
    if app.is_some() {
        where_clauses.push("t.app = $app");
    }
    if labels.is_some() {
        where_clauses.push("t.label IN $labels");
    }
    if !where_clauses.is_empty() {
        cypher.push_str("WHERE ");
        cypher.push_str(&where_clauses.join(" AND "));
        cypher.push('\n');
    }

    let score_expr = match sorting {
        ResourceSorting::Timeline => "MAX(t.indexed_at)",
    };
    cypher.push_str(&format!("WITH r, {score_expr} AS score\n"));

    // Cursor bounds apply to the aggregated score, so they live after the WITH.
    let mut cursor_clauses = Vec::new();
    if pagination.start.is_some() {
        cursor_clauses.push(format!("score {start_op} $start"));
    }
    if pagination.end.is_some() {
        cursor_clauses.push(format!("score {end_op} $end"));
    }
    if !cursor_clauses.is_empty() {
        cypher.push_str("WHERE ");
        cypher.push_str(&cursor_clauses.join(" AND "));
        cypher.push('\n');
    }

    cypher.push_str(&format!(
        "RETURN r.id AS resource_id, score\nORDER BY score {order_direction}, r.id {order_direction}\n"
    ));
    cypher.push_str(&format!(
        "SKIP {}\nLIMIT {}\n",
        pagination.skip.unwrap_or(0).min(MAX_QUERY_SKIP),
        pagination.limit.unwrap_or(10).min(MAX_QUERY_LIMIT)
    ));

    let mut query = Query::new("resource_stream", &cypher);

    if let Some(a) = app {
        query = query.param("app", a);
    }
    if let Some(l) = labels {
        let label_strings: Vec<String> = l.iter().map(|s| s.to_string()).collect();
        query = query.param("labels", label_strings);
    }
    if let Some(start) = pagination.start {
        query = query.param("start", start);
    }
    if let Some(end) = pagination.end {
        query = query.param("end", end);
    }

    query
}

/// Retrieve a homeserver by ID
pub fn get_homeserver_by_id(id: &str) -> Query {
    Query::new(
        "get_homeserver_by_id",
        "MATCH (hs:Homeserver {id: $id})
        WITH hs.id AS id
        RETURN id",
    )
    .param("id", id)
}

/// Retrieves all homeserver IDs that have at least one active user
/// (incoming `HOSTED_BY` relationships from `User` nodes).
///
/// Sorted by aggregate hosted trust descending, then by active user count.
/// The caller truncates this list, so the order decides which homeservers get
/// polled at all: ranking by hosted trust spends a bounded polling budget on
/// the homeservers whose users are expensive to fake, rather than on whichever
/// one registered the most keys.
///
/// Homeservers with equal trust and equal user count are deliberately left in
/// no particular order: with no tiebreak, which of them lands past the cut can
/// vary from run to run, so a tie straddling the limit is shared out rather
/// than always falling on the same homeserver.
///
/// `coalesce(u.trust, 0.0)` matters for the default install: `[trust_rank] seed`
/// ships empty, so no user carries trust, every sum is 0.0, and the ordering
/// falls through to `active_users` — exactly today's behaviour. The trust term
/// only starts doing work once an operator configures a seed set.
///
/// Returns a single `homeservers_list` column containing the collected IDs.
pub fn get_all_homeservers_with_active_users() -> Query {
    Query::new(
        "get_all_homeservers_with_active_users",
        "MATCH (u:User)-[r:HOSTED_BY]->(hs:Homeserver)
        WHERE NOT coalesce(u.deleted, false) AND NOT coalesce(r.stale, false)
        WITH hs.id AS id,
             sum(coalesce(u.trust, 0.0)) AS hosted_trust,
             count(u) AS active_users
        ORDER BY hosted_trust DESC, active_users DESC
        RETURN collect(id) AS homeservers_list",
    )
}

/// Retrieves user IDs whose homeserver mapping is stale
/// (`resolved_at` is older than `ttl_ms`) or missing (no `HOSTED_BY` edge).
pub fn get_users_needing_hs_resolution(ttl_ms: u64) -> Query {
    Query::new(
        "get_users_needing_hs_resolution",
        "MATCH (u:User)
         WHERE NOT coalesce(u.deleted, false)
         OPTIONAL MATCH (u)-[r:HOSTED_BY]->(:Homeserver)
         WITH u, r
         WHERE r IS NULL
            OR r.resolved_at IS NULL
            OR r.resolved_at < (timestamp() - $ttl_ms)
         RETURN collect(u.id) AS user_ids",
    )
    .param("ttl_ms", ttl_ms as i64)
}

/// Retrieves the homeserver ID a user is currently hosted on, if any, along with
/// whether that `HOSTED_BY` mapping is marked `stale`.
pub fn get_user_homeserver(user_id: &str) -> Query {
    Query::new(
        "get_user_homeserver",
        "MATCH (u:User {id: $user_id})-[r:HOSTED_BY]->(hs:Homeserver)
         RETURN hs.id AS homeserver_id, coalesce(r.stale, false) AS stale",
    )
    .param("user_id", user_id.to_string())
}

/// Counts users with a `HOSTED_BY` mapping, and how many of those mappings are
/// marked `stale`. Deleted users are excluded from both counts.
pub fn count_user_homeserver_mappings() -> Query {
    Query::new(
        "count_user_homeserver_mappings",
        "MATCH (u:User)-[r:HOSTED_BY]->(:Homeserver)
         WHERE NOT coalesce(u.deleted, false)
         RETURN count(r) AS mapped_users,
                count(CASE WHEN r.stale = true THEN 1 END) AS stale_users",
    )
}

/// Retrieves all user IDs actively hosted on a given homeserver.
///
/// Excludes users whose mapping is marked `stale` — i.e. whose published
/// homeserver has diverged from the stored one — so the watcher stops
/// indexing them until the mapping realigns.
pub fn get_active_users_by_homeserver(hs_id: &str) -> Query {
    Query::new(
        "get_active_users_by_homeserver",
        "MATCH (u:User)-[r:HOSTED_BY]->(:Homeserver {id: $hs_id})
         WHERE NOT coalesce(u.deleted, false) AND NOT coalesce(r.stale, false)
         RETURN collect(u.id) AS user_ids",
    )
    .param("hs_id", hs_id.to_string())
}

/// Tags on a user applied by users in the viewer's Web of Trust (transitive
/// FOLLOWS, 1..=depth). A follow cycle that reaches the viewer again does not add
/// the viewer to the trusted tagger set. User existence is the anchor: the user
/// is matched first and the viewer with `OPTIONAL MATCH`, so an existing user
/// always returns one row with `tags` (`[]` when no trusted tagger tagged them,
/// or the viewer is unknown) for an empty/normal 200; only a missing user returns
/// zero rows (404). `tag_uri` is the viewer's tag address per label, null where they haven't tagged it.
/// Mirrors `get_viewer_trusted_network_post_tags`.
pub fn get_viewer_trusted_network_tags(user_id: &str, viewer_id: &str, depth: WotDepth) -> Query {
    let graph_query = format!(
        "
        MATCH (tagged:User {{id: $user_id}})
        // Viewer is optional: an unknown viewer is NULL, so the CALL's expand finds
        // no taggers and `tags` collects to [] (existing user with no trusted tags
        // -> 200 []), rather than dropping the row and 404-ing.
        OPTIONAL MATCH (viewer:User {{id: $viewer_id}})
        CALL {{
            WITH viewer, tagged
            MATCH (viewer)-[:FOLLOWS*1..{depth}]->(tagger:User)-[tag:TAGGED]->(tagged)
            WHERE tagger.id <> viewer.id
            WITH viewer, tagged, tag.label AS label, collect(DISTINCT tagger.id) AS taggerIds
            RETURN collect({{
                label: label,
                taggers: taggerIds,
                taggers_count: SIZE(taggerIds),
                tag_uri: head([(viewer)-[vt:TAGGED]->(tagged)
                    WHERE vt.label = label AND vt.uri IS NOT NULL | vt.uri])
            }}) AS tags
        }}
        RETURN tagged IS NOT NULL AS exists, tags
        "
    );

    // Add to the query the params
    Query::new("get_viewer_trusted_network_tags", graph_query.as_str())
        .param("user_id", user_id)
        .param("viewer_id", viewer_id)
}

/// Tags on a single post applied by users in the viewer's Web of Trust
/// (transitive FOLLOWS, 1..=depth). A follow cycle that reaches the viewer again
/// does not add the viewer to the trusted tagger set. Post existence is the
/// anchor: the post is matched first and the viewer with `OPTIONAL MATCH`, so an
/// existing post always returns one row with `tags` (`[]` when no trusted tagger
/// tagged it, or when the viewer is unknown) for an empty/normal 200; only a
/// missing post returns zero rows (404). Labels are ordered by tagger count and
/// paginated with `skip_tags`/`limit_tags`; each label's taggers are capped at
/// `limit_taggers`, mirroring the global tag endpoint so the response stays bounded.
/// `tag_uri` is the viewer's tag address per label, null where they haven't tagged it.
pub fn get_viewer_trusted_network_post_tags(
    author_id: &str,
    post_id: &str,
    viewer_id: &str,
    depth: WotDepth,
    skip_tags: usize,
    limit_tags: usize,
    limit_taggers: usize,
) -> Query {
    let graph_query = format!(
        "
        MATCH (:User {{id: $author_id}})-[:AUTHORED]->(p:Post {{id: $post_id}})
        OPTIONAL MATCH (viewer:User {{id: $viewer_id}})
        CALL {{
            WITH viewer, p
            MATCH (viewer)-[:FOLLOWS*1..{depth}]->(tagger:User)-[tag:TAGGED]->(p)
            WHERE tagger.id <> viewer.id
            WITH viewer, p, tag.label AS label, collect(DISTINCT tagger.id) AS taggerIds
            WITH viewer, p, label, taggerIds, SIZE(taggerIds) AS taggersCount
            ORDER BY taggersCount DESC, label ASC
            SKIP $skip_tags
            LIMIT $limit_tags
            RETURN collect({{
                label: label,
                taggers: taggerIds[0..$limit_taggers],
                taggers_count: taggersCount,
                tag_uri: head([(viewer)-[vt:TAGGED]->(p)
                    WHERE vt.label = label AND vt.uri IS NOT NULL | vt.uri])
            }}) AS tags
        }}
        RETURN p IS NOT NULL AS exists, tags
        "
    );

    Query::new("get_viewer_trusted_network_post_tags", graph_query.as_str())
        .param("author_id", author_id)
        .param("post_id", post_id)
        .param("viewer_id", viewer_id)
        .param("skip_tags", skip_tags as i64)
        .param("limit_tags", limit_tags as i64)
        .param("limit_taggers", limit_taggers as i64)
}

// The counts the all-time influencer and most-followed scores are built from, so they
// can't drift from `UserCounts`.
const FOLLOWERS_COUNT: &str = "COUNT { (:User)-[:FOLLOWS]->(u) }";
const POSTS_COUNT: &str = "COUNT { (u)-[:AUTHORED]->(:Post) }";
// tagged = tags this user assigned to users + to posts
const TAGGED_COUNT: &str = "COUNT { (u)-[:TAGGED]->(:User) } + COUNT { (u)-[:TAGGED]->(:Post) }";

/// Per-user count fields, as `(alias, COUNT {} subquery)` pairs bound to the
/// node variable `u`. Single source of truth shared by [`user_counts_by_ids`] (map
/// form) and the trust report (column form), so their filters — especially the
/// collection/bookmark rules — can't drift between the two queries.
const USER_COUNT_FIELDS: &[(&str, &str)] = &[
    ("following", "COUNT { (u)-[:FOLLOWS]->(:User) }"),
    ("followers", FOLLOWERS_COUNT),
    (
        "friends",
        "COUNT { (u)-[:FOLLOWS]->(friend:User) WHERE (friend)-[:FOLLOWS]->(u) }",
    ),
    ("posts", POSTS_COUNT),
    (
        "replies",
        "COUNT { (u)-[:AUTHORED]->(:Post)-[:REPLIED]->(:Post) }",
    ),
    (
        "collections",
        "COUNT { (u)-[:AUTHORED]->(p:Post) WHERE p.kind = 'collection' }",
    ),
    // A collection-follow is stored as a bookmark; keep it out of the count.
    (
        "bookmarks",
        "COUNT { (u)-[:BOOKMARKED]->(bp:Post) WHERE (bp.kind IS NULL OR bp.kind <> 'collection') }",
    ),
    ("tagged", TAGGED_COUNT),
    ("tags", "COUNT { (:User)-[:TAGGED]->(u) }"),
    (
        "unique_tags",
        "COUNT { MATCH (u)<-[t:TAGGED]-(:User) RETURN DISTINCT t.label }",
    ),
];

/// Renders [`USER_COUNT_FIELDS`] as `field: COUNT {…}, …` for a Cypher map literal.
fn user_count_fields_map() -> String {
    USER_COUNT_FIELDS
        .iter()
        .map(|(name, expr)| format!("{name}: {expr}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Renders [`USER_COUNT_FIELDS`] as `COUNT {…} AS field, …` for a flat RETURN.
pub fn user_count_fields_columns() -> String {
    USER_COUNT_FIELDS
        .iter()
        .map(|(name, expr)| format!("{expr} AS {name}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One row per distinct id, in no guaranteed order: `exists` is false for ids
/// with no `User` node and `counts` holds every [`USER_COUNT_FIELDS`] entry.
pub fn user_counts_by_ids(user_ids: &[&str]) -> Query {
    // Each field is an independent COUNT { } subquery off the single (u) row.
    // Do NOT precede this with a row-multiplying OPTIONAL MATCH (e.g. over
    // received tags): that makes every subquery a per-row grouping key, so the
    // whole block runs once per received tag, i.e. O(received_tags x authored_posts)
    // and hangs on heavy users. See issue #935. A repeated id multiplies rows the
    // same way, so DISTINCT drops the copies before any subquery runs.
    let cypher = format!(
        "UNWIND $user_ids AS id
         WITH DISTINCT id
         OPTIONAL MATCH (u:User {{id: id}})
         RETURN
             id,
             u IS NOT NULL AS exists,
             {{ {fields} }} AS counts;",
        fields = user_count_fields_map()
    );
    Query::new("user_counts_by_ids", cypher).param("user_ids", user_ids)
}

/// `all_exist` is true only when every id has a `User` node. No DISTINCT here:
/// `SIZE($user_ids)` counts every copy, so a self-relationship's `[a, a]` would fail.
pub fn users_exist(user_ids: &[&str]) -> Query {
    Query::new(
        "users_exist",
        "UNWIND $user_ids AS id
         OPTIONAL MATCH (u:User {id: id})
         RETURN COUNT(u) = SIZE($user_ids) AS all_exist",
    )
    .param("user_ids", user_ids)
}

// These queries aggregate into a single row, so SKIP/LIMIT cannot paginate the
// collected list; callers slice in Rust instead.
pub fn get_user_followers(user_id: &str) -> Query {
    Query::new(
        "get_user_followers",
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (u)<-[:FOLLOWS]-(follower:User)
         RETURN COUNT(u) > 0 AS user_exists,
                COLLECT(follower.id) AS follower_ids",
    )
    .param("user_id", user_id)
}

pub fn get_user_following(user_id: &str) -> Query {
    Query::new(
        "get_user_following",
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
         RETURN COUNT(u) > 0 AS user_exists,
                COLLECT(following.id) AS following_ids",
    )
    .param("user_id", user_id)
}

fn stream_reach_to_graph_subquery(reach: &StreamReach) -> String {
    match reach {
        StreamReach::Followers => "MATCH (user:User)<-[:FOLLOWS]-(reach:User)".to_string(),
        StreamReach::Following => "MATCH (user:User)-[:FOLLOWS]->(reach:User)".to_string(),
        StreamReach::Friends => {
            "MATCH (user:User)-[:FOLLOWS]->(reach:User), (user)<-[:FOLLOWS]-(reach)".to_string()
        }
        StreamReach::Wot(depth) => {
            format!("MATCH (user:User)-[:FOLLOWS*1..{depth}]->(reach:User)")
        }
    }
}

/// Up to `limit` distinct users in `user_id`'s `reach` who authored at least
/// one post, excluding `user_id`, the most prolific first. Users without posts
/// are left out: they cannot match a post search, so keeping them would spend
/// `limit` on authors that match nothing. The post count matches
/// `UserCounts::posts` (every authored post, replies included); equal counts
/// break ties by id descending.
pub fn get_reach_authors_by_posts(user_id: &str, reach: &StreamReach, limit: usize) -> Query {
    let cypher = format!(
        "
        MATCH (user:User {{id: $user_id}})
        {}
        WHERE reach.id <> $user_id
        WITH DISTINCT reach
        // AUTHORED only ever points at posts, so the unlabelled pattern is a
        // degree lookup rather than an expansion
        WITH reach, COUNT {{ (reach)-[:AUTHORED]->() }} AS posts
        WHERE posts > 0
        RETURN reach.id AS author_id
        ORDER BY posts DESC, author_id DESC
        LIMIT $limit
        ",
        stream_reach_to_graph_subquery(reach)
    );
    reach_attrs(Query::new("get_reach_authors_by_posts", &cypher), reach)
        .param("user_id", user_id)
        .param("limit", i64::try_from(limit).unwrap_or(i64::MAX))
}

/// Whether `target_id` is in `user_id`'s `reach`. `false` for the user itself
/// and for unknown users.
pub fn reach_contains_user(user_id: &str, target_id: &str, reach: &StreamReach) -> Query {
    let check = match reach {
        StreamReach::Following => "EXISTS { (user)-[:FOLLOWS]->(target) }".to_string(),
        StreamReach::Followers => "EXISTS { (target)-[:FOLLOWS]->(user) }".to_string(),
        StreamReach::Friends => {
            "EXISTS { (user)-[:FOLLOWS]->(target) } AND EXISTS { (target)-[:FOLLOWS]->(user) }"
                .to_string()
        }
        // shortestPath searches from both ends; a variable-length EXISTS runs
        // as an unpruned expand
        StreamReach::Wot(depth) => {
            format!("EXISTS {{ MATCH shortestPath((user)-[:FOLLOWS*1..{depth}]->(target)) }}")
        }
    };
    let cypher = format!(
        "
        MATCH (user:User {{id: $user_id}}), (target:User {{id: $target_id}})
        WHERE target.id <> $user_id
        RETURN {check} AS reached
        "
    );
    reach_attrs(Query::new("reach_contains_user", &cypher), reach)
        .param("user_id", user_id)
        .param("target_id", target_id)
}

pub fn get_tags_by_label_prefix(label_prefix: &str) -> Query {
    Query::new(
        "get_tags_by_label_prefix",
        "
        MATCH ()-[t:TAGGED]->()
        WHERE t.label STARTS WITH $label_prefix
        RETURN COLLECT(DISTINCT t.label) AS tag_labels
        ",
    )
    .param("label_prefix", label_prefix)
}

pub fn get_tags() -> Query {
    Query::new(
        "get_tags",
        "
        MATCH ()-[t:TAGGED]->()
        RETURN COLLECT(DISTINCT t.label) AS tag_labels
        ",
    )
}

fn reach_attrs(query: Query, reach: &StreamReach) -> Query {
    let (name, depth) = reach.telemetry_dimensions();
    query
        .telemetry_attr("reach", name)
        .telemetry_attr_opt("depth", depth)
}

pub fn get_tag_taggers_by_reach(
    label: &str,
    user_id: &str,
    reach: StreamReach,
    skip: usize,
    limit: usize,
) -> Query {
    let cypher = format!(
        "
            {}
            WHERE user.id = $user_id
            // One list of reached users: a WoT reach is walked once per user instead
            // of once per follow path, and the label index below is read once.
            // Deduplicating into rows instead makes the planner re-read the label
            // index for every reached user
            WITH COLLECT(DISTINCT reach) AS reached
            // The tagged node can be generic, representing either a Post, a User, or both.
            // For now, it will be a Post to align with UX requirements.
            MATCH (reach:User)-[tag:TAGGED]->(tagged:Post)
            WHERE tag.label = $label AND reach IN reached

            // Get the latest tagged timestamp per `reach` user
            WITH DISTINCT reach, MAX(tag.indexed_at) AS latest_tag_time
            // Taggers with the same timestamp are ordered by id so the pages are stable
            ORDER BY latest_tag_time DESC, reach.id ASC

            // Use slice notation instead of SKIP and LIMIT
            WITH COLLECT({{ reach_id: reach.id }})[$skip..$skip + $limit] AS paginated
            UNWIND paginated AS row

            RETURN COLLECT(row.reach_id) AS tagger_ids
            ",
        stream_reach_to_graph_subquery(&reach)
    );
    reach_attrs(Query::new("get_tag_taggers_by_reach", &cypher), &reach)
        .param("label", label)
        .param("user_id", user_id)
        .param("skip", skip as i64)
        .param("limit", limit as i64)
}

pub fn get_hot_tags_by_reach(
    user_id: &str,
    reach: StreamReach,
    tags_query: &HotTagsInputDTO,
) -> Query {
    let input_tagged_type = match &tags_query.tagged_type {
        Some(tagged_type) => tagged_type.to_string(),
        None => String::from("Post|User"),
    };

    let (from, to) = tags_query.timeframe.to_timestamp_range();
    let cypher = format!(
        "
        {}
        WHERE user.id = $user_id
        WITH DISTINCT reach
        MATCH (reach)-[tag:TAGGED]->(tagged:{})
        WHERE tag.indexed_at >= $from AND tag.indexed_at < $to
        // COLLECT keeps its input order, so the cut keeps the lowest tagger ids
        WITH tag.label AS label, reach.id AS tagger_id, tagged
        ORDER BY tagger_id ASC
        WITH
            label,
            COLLECT(DISTINCT tagger_id)[..{}] AS taggers,
            COUNT(DISTINCT tagged) AS uniqueTaggedCount,
            COUNT(DISTINCT tagger_id) AS taggers_count
        WITH {{
            label: label,
            taggers_id: taggers,
            tagged_count: uniqueTaggedCount,
            taggers_count: taggers_count
        }} AS hot_tag
        ORDER BY hot_tag.tagged_count DESC, hot_tag.label ASC
        SKIP $skip LIMIT $limit
        RETURN COLLECT(hot_tag) as hot_tags
    ",
        stream_reach_to_graph_subquery(&reach),
        input_tagged_type,
        tags_query.taggers_limit
    );
    reach_attrs(Query::new("get_hot_tags_by_reach", &cypher), &reach)
        .param("user_id", user_id)
        .param("skip", tags_query.skip as i64)
        .param("limit", tags_query.limit as i64)
        .param("from", from)
        .param("to", to)
}

pub fn get_global_hot_tags(tags_query: &HotTagsInputDTO) -> Query {
    let input_tagged_type = match &tags_query.tagged_type {
        Some(tagged_type) => tagged_type.to_string(),
        None => String::from("Post|User"),
    };
    let (from, to) = tags_query.timeframe.to_timestamp_range();
    let cypher = format!(
        "
        MATCH (user: User)-[tag:TAGGED]->(tagged:{})
        WHERE tag.indexed_at >= $from AND tag.indexed_at < $to
        // COLLECT keeps its input order, so the cut keeps the lowest tagger ids
        WITH tag.label AS label, user.id AS tagger_id, tagged
        ORDER BY tagger_id ASC
        WITH
            label,
            COLLECT(DISTINCT tagger_id)[..{}] AS taggers,
            COUNT(DISTINCT tagged) AS uniqueTaggedCount,
            COUNT(DISTINCT tagger_id) AS taggers_count
        WITH {{
            label: label,
            taggers_id: taggers,
            tagged_count: uniqueTaggedCount,
            taggers_count: taggers_count
        }} AS hot_tag
        ORDER BY hot_tag.tagged_count DESC, hot_tag.label ASC
        SKIP $skip LIMIT $limit
        RETURN COLLECT(hot_tag) as hot_tags
    ",
        input_tagged_type, tags_query.taggers_limit
    );
    Query::new("get_global_hot_tags", &cypher)
        .param("skip", tags_query.skip as i64)
        .param("limit", tags_query.limit as i64)
        .param("from", from)
        .param("to", to)
}

pub fn get_influencers_by_reach(
    user_id: &str,
    reach: StreamReach,
    skip: usize,
    limit: usize,
    timeframe: &Timeframe,
) -> Query {
    let (from, to) = timeframe.to_timestamp_range();
    let cypher = format!(
        "
        {}
        WHERE user.id = $user_id
        WITH DISTINCT reach
        WHERE NOT coalesce(reach.deleted, false)

        CALL (reach) {{
            MATCH (others:User)-[follow:FOLLOWS]->(reach)
            RETURN count(DISTINCT follow) as followers_count
        }}
        CALL (reach) {{
            MATCH (reach)-[tag:TAGGED]->(:Post)
            WHERE tag.indexed_at >= $from AND tag.indexed_at < $to
            RETURN count(DISTINCT tag) as tags_count
        }}
        CALL (reach) {{
            MATCH (reach)-[:AUTHORED]->(post:Post)
            WHERE post.indexed_at >= $from AND post.indexed_at < $to
            RETURN count(DISTINCT post) as posts_count
        }}

        WITH reach, followers_count, tags_count, posts_count
        WITH {{
            id: reach.id,
            score: (tags_count + posts_count) * sqrt(followers_count)
        }} AS influencer
        ORDER BY influencer.score DESC
        SKIP $skip
        LIMIT $limit
        RETURN COLLECT([influencer.id, influencer.score]) as influencers
    ",
        stream_reach_to_graph_subquery(&reach),
    );
    reach_attrs(Query::new("get_influencers_by_reach", &cypher), &reach)
        .param("user_id", user_id)
        .param("skip", skip as i64)
        .param("limit", limit as i64)
        .param("from", from)
        .param("to", to)
}

/// One keyset page of the all-time influencer ranking, ordered by `id`, strictly
/// after `after_id` (`""` for the first page). It holds every user who is neither
/// tombstoned nor a discovery stub, scored `(tagged + posts) * sqrt(followers)`
/// from the [`user_counts_by_ids`] counts, so a user with no followers ranks with 0.
pub fn get_all_time_influencers(after_id: &str, limit: usize) -> Query {
    let cypher = format!(
        "
        MATCH (u:User)
        WHERE u.id > $after_id
          AND NOT coalesce(u.deleted, false)
          // A discovery stub is named after its id, which no profile name can be.
          AND u.name <> u.id
        RETURN u.id AS id, ({TAGGED_COUNT} + {POSTS_COUNT}) * sqrt({FOLLOWERS_COUNT}) AS score
        ORDER BY u.id
        LIMIT $limit
        "
    );
    Query::new("get_all_time_influencers", &cypher)
        .param("after_id", after_id)
        .param("limit", limit as i64)
}

/// The top `limit` users after `skip` for a ranged timeframe, which only counts
/// tags on posts. AllTime is ranked in full by [`get_all_time_influencers`].
pub fn get_global_influencers(skip: usize, limit: usize, timeframe: CacheTimeframe) -> Query {
    let (from, to) = timeframe.timeframe().to_timestamp_range();
    Query::new(
        "get_global_influencers",
        "
        MATCH (user:User)
        WHERE NOT coalesce(user.deleted, false)
        WITH DISTINCT user

        // Each count is a scoped CALL(user){} subquery so it stays per-user
        // instead of multiplying into a cartesian product. Mirrors
        // get_influencers_by_reach.
        CALL (user) {
            MATCH (others:User)-[follow:FOLLOWS]->(user)
            WHERE follow.indexed_at >= $from AND follow.indexed_at < $to
            RETURN count(DISTINCT follow) AS followers_count
        }
        CALL (user) {
            MATCH (user)-[tag:TAGGED]->(:Post)
            WHERE tag.indexed_at >= $from AND tag.indexed_at < $to
            RETURN count(DISTINCT tag) AS tags_count
        }
        CALL (user) {
            // Filter on the Post node's indexed_at, like get_influencers_by_reach does.
            // The AUTHORED edge carries no indexed_at property (create_post never sets
            // one), so filtering on the edge would always yield posts_count = 0.
            MATCH (user)-[:AUTHORED]->(post:Post)
            WHERE post.indexed_at >= $from AND post.indexed_at < $to
            RETURN count(DISTINCT post) AS posts_count
        }
        WITH {
            id: user.id,
            score: (tags_count + posts_count) * sqrt(followers_count)
        } AS influencer
        WHERE influencer.id IS NOT NULL

        ORDER BY influencer.score DESC, influencer.id ASC
        SKIP $skip
        LIMIT $limit
        RETURN COLLECT([influencer.id, influencer.score]) as influencers
    ",
    )
    .param("skip", skip as i64)
    .param("limit", limit as i64)
    .param("from", from)
    .param("to", to)
}

/// Every Collection post key, for backfills.
pub fn get_collection_posts() -> Query {
    Query::new(
        "get_collection_posts",
        "
        MATCH (u:User)-[:AUTHORED]->(c:Post {kind: 'collection'})
        RETURN u.id AS author_id, c.id AS post_id
        ",
    )
}

pub fn get_files_by_ids(key_pair: &[&[&str]]) -> Query {
    Query::new(
        "get_files_by_ids",
        "
        UNWIND $pairs AS pair
        OPTIONAL MATCH (record:File {owner_id: pair[0], id: pair[1]})
        RETURN record
        ",
    )
    .param("pairs", key_pair)
}

/// Which authors a `source=all` stream served from the graph keeps.
#[derive(Debug, Clone, Copy)]
pub enum TrustRule<'a> {
    Ranked,
    /// Ranked authors and the viewer: their own posts stay on their timeline.
    RankedAndViewer(&'a str),
}

impl TrustRule<'_> {
    fn condition(self) -> String {
        match self {
            TrustRule::Ranked => ranked_user("author"),
            TrustRule::RankedAndViewer(_) => {
                format!("({}) OR author.id = $viewer_id", ranked_user("author"))
            }
        }
    }
}

// Build the graph query based on parameters
pub fn post_stream(
    source: StreamSource,
    sorting: StreamSorting,
    order: SortOrder,
    tags: &Option<Vec<String>>,
    pagination: Pagination,
    kind: Option<KindFilter>,
    trust_rule: Option<TrustRule>,
) -> GraphResult<Query> {
    // Initialize the cypher query
    let mut cypher = String::new();

    // Initialize where_clause_applied to false
    let mut where_clause_applied = false;

    // Start with the observer node if needed
    // Needed that one for source pattern matching
    if source.get_observer().is_some() {
        cypher.push_str("MATCH (observer:User {id: $observer_id})\n");
    }

    // Apply source MATCH clause. Must precede the posts MATCH: a CALL subquery
    // executes once per incoming row and the planner cannot hoist it, so emitting
    // it after the posts MATCH re-runs the reach traversal for every post in the
    // graph.
    //
    // The WoT arms bind, filter, and dedupe authors before the posts MATCH.
    // Variable-length traversals yield one row per path, so expanding posts first
    // would multiply work that a later DISTINCT could only remove afterward.
    // Keep each source's filters in its arm; WITH DISTINCT drops path-specific
    // variables from scope and carries only the qualified authors forward.
    match &source {
        StreamSource::Following { .. } => cypher.push_str(
            "MATCH (observer)-[:FOLLOWS]->(author)\n\
             WHERE author.id <> $observer_id\n",
        ),
        StreamSource::Followers { .. } => cypher.push_str(
            "MATCH (observer)<-[:FOLLOWS]-(author)\n\
             WHERE author.id <> $observer_id\n",
        ),
        StreamSource::Friends { .. } => cypher.push_str(
            "MATCH (observer)-[:FOLLOWS]->(author)-[:FOLLOWS]->(observer)\n\
             WHERE author.id <> $observer_id\n",
        ),
        StreamSource::Bookmarks { .. } => cypher.push_str("MATCH (observer)-[:BOOKMARKED]->(p)\n"),
        StreamSource::Wot { depth, .. } => cypher.push_str(&format!(
            "MATCH (observer)-[:FOLLOWS*1..{depth}]->(author:User)\n\
             WHERE author.id <> $observer_id\n\
             WITH DISTINCT author\n"
        )),
        // Me (depth-0): the observer is the sole tagger, so match their TAGGED
        // edge directly. A matching self-endorsement may qualify the observer as
        // an author; no traversal or CALL is needed.
        StreamSource::WotDomain {
            trust: DomainTrust::Me,
            ..
        } => cypher.push_str(
            "MATCH (observer)-[endorsement:TAGGED]->(author:User)\n\
             WHERE endorsement.label IN $domain_tags\n\
             WITH DISTINCT author\n",
        ),
        // Network: collapse the trust reach to distinct taggers before the tag
        // join. A follow cycle must not make the observer a network tagger, but
        // a trusted tagger may still qualify the observer as an author.
        StreamSource::WotDomain {
            trust: DomainTrust::Network(depth),
            ..
        } => cypher.push_str(&format!(
            "CALL {{ WITH observer MATCH (observer)-[:FOLLOWS*1..{depth}]->(tagger:User) WHERE tagger.id <> observer.id RETURN DISTINCT tagger }}\n\
             MATCH (tagger)-[endorsement:TAGGED]->(author:User)\n\
             WHERE endorsement.label IN $domain_tags\n\
             WITH DISTINCT author\n"
        )),
        // Anchor on the item's unique id (see `post_counts`), then bind `p` to
        // the collections that curate it; the posts MATCH below adds the curator.
        StreamSource::PostCollections { .. } => cypher.push_str(
            "MATCH (item:Post {id: $post_id})\n\
             WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(item) }\n\
             MATCH (item)<-[:COLLECTED]-(p:Post)\n",
        ),
        _ => {}
    }

    // Base match for posts and authors. For observer-anchored sources `author`
    // is already bound, so this expands their posts instead of enumerating all
    // posts.
    let author_rule = trust_rule
        .filter(|_| matches!(source, StreamSource::All))
        .map(TrustRule::condition);
    match author_rule {
        // With the trust rule the author is matched after sorting, below.
        Some(_) => cypher.push_str("MATCH (p:Post)\n"),
        None => cypher.push_str("MATCH (p:Post)<-[:AUTHORED]-(author:User)\n"),
    }

    // Apply tags
    if tags.is_some() {
        cypher.push_str("MATCH (:User)-[tag:TAGGED]->(p)\n");
        append_condition(
            &mut cypher,
            "tag.label IN $labels",
            &mut where_clause_applied,
        );
    }

    // If source has an author, add where clause. It is related with source pattern matching
    // If the source is Author, it is enough adding where clause. Not need to relate nodes
    if source.get_author().is_some() {
        append_condition(
            &mut cypher,
            "author.id = $author_id",
            &mut where_clause_applied,
        );
    }

    // If a post kind filter is provided, add the corresponding condition.
    match &kind {
        Some(KindFilter::Kind(_)) => {
            append_condition(&mut cypher, "p.kind = $kind", &mut where_clause_applied);
        }
        // The IS NULL guard is load-bearing: with Cypher's three-valued logic,
        // `NOT p.kind IN [...]` evaluates to NULL (row dropped) for legacy
        // posts whose kind property was never set.
        Some(KindFilter::Exclude(_)) => {
            append_condition(
                &mut cypher,
                "(p.kind IS NULL OR NOT p.kind IN $exclude_kinds)",
                &mut where_clause_applied,
            );
        }
        None => {}
    }

    // Filter just the parent posts. StreamSource::PostReplies and
    // StreamSource::AuthorReplies must never reach this query: it has no reply
    // MATCH arm, so this parents-only condition would invert their semantics.
    // The route layer rejects kind filters for those sources
    // (validate_source_compat).
    //
    // Bookmarks are exempt: they target specific posts the user picked, replies
    // included, and the Redis bookmarks sorted set carries them all. Applying
    // the parents-only filter here would silently drop bookmarked replies on the
    // graph path (reachable via kind/exclude_kinds or engagement sort), diverging
    // from the index path. Every other source that routes here wants parents only.
    if !matches!(source, StreamSource::Bookmarks { .. }) {
        append_condition(
            &mut cypher,
            "NOT ( (p)-[:REPLIED]->(:Post) )",
            &mut where_clause_applied,
        );
    }

    // Cursor bounds follow the sort direction. `start` is always the resume
    // cursor (the last row's `last_post_score`) and `end` the hard limit:
    // descending pages downward from `start` (<=) to the `end` floor (>=);
    // ascending pages upward from `start` (>=) to the `end` ceiling (<=).
    let (start_op, end_op) = match order {
        SortOrder::Descending => ("<=", ">="),
        SortOrder::Ascending => (">=", "<="),
    };

    // Apply time interval conditions. Only can be applied with timeline sorting
    // The engagement score has to be computed
    if sorting == StreamSorting::Timeline {
        if pagination.start.is_some() {
            append_condition(
                &mut cypher,
                &format!("p.indexed_at {start_op} $start"),
                &mut where_clause_applied,
            );
        }

        if pagination.end.is_some() {
            append_condition(
                &mut cypher,
                &format!("p.indexed_at {end_op} $end"),
                &mut where_clause_applied,
            );
        }
    }

    let order_dir = match order {
        SortOrder::Ascending => "ASC",
        SortOrder::Descending => "DESC",
    };

    // Make unique the posts, cannot be repeated. With the trust rule, sorting the
    // posts before matching their authors lets the planner stop at LIMIT (37-49%
    // fewer DB hits on `exclude_kinds` and multi-tag streams).
    match (&author_rule, &sorting) {
        (Some(rule), StreamSorting::Timeline) => cypher.push_str(&format!(
            "WITH DISTINCT p\nORDER BY p.indexed_at {order_dir}, p.id {order_dir}\nMATCH (p)<-[:AUTHORED]-(author:User)\nWHERE {rule}\n"
        )),
        (Some(rule), StreamSorting::TotalEngagement) => cypher.push_str(&format!(
            "WITH DISTINCT p\nMATCH (p)<-[:AUTHORED]-(author:User)\nWHERE {rule}\n"
        )),
        (None, _) => cypher.push_str("WITH DISTINCT p, author\n"),
    }

    // Apply StreamSorting. `score` is the value the cursor (`last_post_score`) pages
    // on: the post timestamp for Timeline, the engagement count for TotalEngagement.
    // `p.id` is a deterministic secondary key so equal scores keep a stable order
    // within a response (pagination across ties is still best-effort: the cursor
    // carries only the score, not the id).
    let (score_expr, order_clause) = match sorting {
        StreamSorting::Timeline => (
            "p.indexed_at",
            format!("ORDER BY p.indexed_at {order_dir}, p.id {order_dir}"),
        ),
        StreamSorting::TotalEngagement => {
            // Each engagement count is its own COUNT{} subquery, so they don't
            // multiply into a cartesian product per post.
            cypher.push_str(
                "
                WITH p, author,
                    COUNT { (p)<-[:TAGGED]-(:User) }
                    + COUNT { (p)<-[:REPLIED]-(:Post) }
                    + COUNT { (p)<-[:REPOSTED]-(:Post) } AS total_engagement
                ",
            );

            // Initialise again
            where_clause_applied = false;

            // Add total_engagement to filter by engagement the post
            if pagination.start.is_some() {
                append_condition(
                    &mut cypher,
                    &format!("total_engagement {start_op} $start"),
                    &mut where_clause_applied,
                );
            }

            if pagination.end.is_some() {
                append_condition(
                    &mut cypher,
                    &format!("total_engagement {end_op} $end"),
                    &mut where_clause_applied,
                );
            }

            (
                "total_engagement",
                format!("ORDER BY total_engagement {order_dir}, p.id {order_dir}"),
            )
        }
    };

    cypher.push_str(&format!(
        "RETURN author.id AS author_id, p.id AS post_id, {score_expr} AS score\n{order_clause}\n"
    ));

    // Apply skip and limit
    if let Some(skip) = pagination.skip {
        cypher.push_str(&format!("SKIP {}\n", skip.min(MAX_QUERY_SKIP)));
    }
    if let Some(limit) = pagination.limit {
        cypher.push_str(&format!("LIMIT {}\n", limit.min(MAX_QUERY_LIMIT)));
    }

    // Short-circuited upstream in collect_post_keys.
    if matches!(&source, StreamSource::Collection { .. }) {
        return Err(GraphError::QueryBuildError(
            "StreamSource::Collection must be served by collect_post_keys".to_string(),
        ));
    }

    let (source_attr, depth) = source.telemetry_dimensions();
    let query = Query::new("post_stream", &cypher)
        .telemetry_attr("source", source_attr)
        .telemetry_attr_opt("depth", depth);
    let query = match trust_rule {
        Some(TrustRule::RankedAndViewer(viewer_id)) => query.param("viewer_id", viewer_id),
        _ => query,
    };
    Ok(build_query_with_params(
        query,
        &source,
        tags,
        kind,
        &pagination,
    ))
}

/// Appends a condition to the Cypher query, using `WHERE` if no `WHERE` clause
/// has been applied yet, or `AND` if a `WHERE` clause is already present.
///
/// # Arguments
///
/// * `cypher` - A mutable reference to the Cypher query string to which the condition will be appended
/// * `condition` - The condition to be added to the query
/// * `where_clause_applied` - A mutable reference to a boolean flag indicating whether a `WHERE` clause
///   has already been applied to the query.
fn append_condition(cypher: &mut String, condition: &str, where_clause_applied: &mut bool) {
    if *where_clause_applied {
        cypher.push_str(&format!("AND {condition}\n"));
    } else {
        cypher.push_str(&format!("WHERE {condition}\n"));
        *where_clause_applied = true;
    }
}

/// Applies the necessary parameters to an already-constructed `Query`.
///
/// # Arguments
///
/// * `query` - A `Query` already constructed with its label and cypher string.
/// * `source` - The `StreamSource` specifying the origin of the posts (e.g., Following, Followers).
/// * `tags` - An optional list of tag labels to filter the posts.
/// * `kind` - An optional `KindFilter` to include a single kind or exclude a list of kinds.
/// * `pagination` - The `Pagination` object containing pagination parameters like `start`, `end`, `skip`, and `limit`.
fn build_query_with_params(
    mut query: Query,
    source: &StreamSource,
    tags: &Option<Vec<String>>,
    kind: Option<KindFilter>,
    pagination: &Pagination,
) -> Query {
    if let Some(observer_id) = source.get_observer() {
        query = query.param("observer_id", observer_id.to_string());
    }
    if let Some(domain_tags) = source.get_domain_tags() {
        query = query.param("domain_tags", domain_tags.to_vec());
    }
    if let Some(labels) = tags.clone() {
        query = query.param("labels", labels);
    }
    if let Some(author_id) = source.get_author() {
        query = query.param("author_id", author_id.to_string());
    }
    if let Some((author_id, post_id)) = source.get_anchor_post() {
        query = query
            .param("author_id", author_id.to_string())
            .param("post_id", post_id.to_string());
    }
    match kind {
        Some(KindFilter::Kind(post_kind)) => {
            query = query.param("kind", post_kind.to_string());
        }
        Some(KindFilter::Exclude(kinds)) => {
            // Bare lowercase names, matching how `p.kind` is stored (see put.rs).
            query = query.param(
                "exclude_kinds",
                kinds.iter().map(ToString::to_string).collect::<Vec<_>>(),
            );
        }
        None => {}
    }
    if let Some(start_interval) = pagination.start {
        query = query.param("start", start_interval);
    }
    if let Some(end_interval) = pagination.end {
        query = query.param("end", end_interval);
    }

    query
}

/// Determines whether a user has any relationships
/// # Arguments
/// * `user_id` - The unique identifier of the user
pub fn user_is_safe_to_delete(user_id: &str) -> Query {
    Query::new(
        "user_is_safe_to_delete",
        "
        MATCH (u:User {id: $user_id})
        // EXISTS stops at the first relationship, so this is O(1) on high-degree
        // users rather than scanning every edge.
        RETURN EXISTS { (u)-[]-() } AS flag
",
    )
    .param("user_id", user_id)
}

/// Checks if a post has any relationships that aren't in the set of allowed
/// relationships for post deletion. If the post has such relationships,
/// the query returns `true`; otherwise, `false`
/// If the user or post does not exist, the query returns no rows.
/// # Arguments
/// * `author_id` - The unique identifier of the user who authored the post
/// * `post_id` - The unique identifier of the post
pub fn post_is_safe_to_delete(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "post_is_safe_to_delete",
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        // EXISTS stops at the first disallowed relationship, so this is O(1) on
        // high-degree posts rather than scanning every edge.
        RETURN EXISTS {
            MATCH (p)-[r]-()
            WHERE NOT (
                // Allowed relationships:
                // 1. Incoming AUTHORED relationship from the specified user
                (type(r) = 'AUTHORED' AND startNode(r).id = $author_id AND endNode(r) = p)
                OR
                // 2. Outgoing REPOSTED relationship to another post
                (type(r) = 'REPOSTED' AND startNode(r) = p)
                OR
                // 3. Outgoing REPLIED relationship to another post
                (type(r) = 'REPLIED' AND startNode(r) = p)
                OR
                // 4. COLLECTED either way: a curated post drops out of its
                // collections on deletion, and a collection may hold items.
                type(r) = 'COLLECTED'
            )
        } AS flag
",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

/// Find user recommendations: active users (with 5+ posts) who are 2-3 degrees of separation away
/// from the given user, but not directly followed by them.
/// Deleted users are filtered in Cypher; only the user ID is projected (no name column).
pub fn recommend_users(user_id: &str, limit: usize) -> Query {
    Query::new(
        "recommend_users",
        "
        MATCH (user:User {id: $user_id})
        // Depth 1 is always directly followed, hence excluded below: start at 2.
        // DISTINCT right after the expand lets the planner prune (one row per reached
        // node, not per path), so the filters below run once per candidate.
        MATCH (user)-[:FOLLOWS*2..3]->(potential:User)
        WITH DISTINCT user, potential
        WHERE potential <> user
          AND NOT coalesce(potential.deleted, false)
          AND NOT (user)-[:FOLLOWS]->(potential)
          // Degree lookup instead of expand + aggregate; keeps LIMIT lazy.
          AND COUNT { (potential)-[:AUTHORED]->() } >= 5
        RETURN potential.id AS recommended_user_id
        LIMIT $limit
    ",
    )
    .param("user_id", user_id.to_string())
    .param("limit", limit as i64)
}

/// Ranks the users the network associates with each label, one list per label.
///
/// Both arms are needed: profile tags skew to identity, so most interests only appear on posts.
/// Summing endorser trust rather than counting keeps a few prolific taggers from minting
/// reputation. `since` is a liveness threshold, not a measurement window, so it has no upper
/// bound. It reads `Post.indexed_at`, since the `AUTHORED` edge carries none; pass 0 for "has
/// ever posted".
pub fn starter_pack_users(
    labels: &[String],
    user_id: Option<&str>,
    since: i64,
    per_label: usize,
) -> Query {
    Query::new(
        "starter_pack_users",
        "
        // Bound once, so the follows check stays a lookup.
        OPTIONAL MATCH (user:User {id: $user_id})
        UNWIND $labels AS label
        WITH DISTINCT user, label
        // Ranking per label in its own subquery so ORDER BY + LIMIT plans as Top. Collecting
        // first and slicing after sorts every candidate of every label instead.
        CALL (user, label) {
            CALL (label) {
                MATCH (tagger:User)-[t:TAGGED]->(candidate:User)
                WHERE t.label = label
                RETURN candidate, tagger
              UNION
                MATCH (tagger:User)-[t:TAGGED]->(:Post)<-[:AUTHORED]-(candidate:User)
                WHERE t.label = label
                RETURN candidate, tagger
            }
            // UNION, not UNION ALL: it is what makes this one vote per endorser.
            WITH user, candidate,
                 sum(coalesce(tagger.trust, 0.0)) AS trust_score,
                 count(DISTINCT tagger) AS endorsers
            // Cheaper here than against every endorsement row.
            WHERE NOT coalesce(candidate.deleted, false)
              AND (user IS NULL OR (candidate <> user AND NOT (user)-[:FOLLOWS]->(candidate)))
              AND EXISTS { MATCH (candidate)-[:AUTHORED]->(p:Post) WHERE p.indexed_at >= $since }
            WITH candidate.id AS id, trust_score, endorsers
            ORDER BY trust_score DESC, endorsers DESC, id ASC
            LIMIT $per_label
            RETURN collect(id) AS candidates
        }
        // A label with no candidates stays absent, as the caller expects.
        WITH label, candidates
        WHERE candidates <> []
        RETURN label, candidates
        ",
    )
    .param("labels", labels.to_vec())
    .param("user_id", user_id.map(str::to_string))
    .param("since", since)
    .param("per_label", per_label.min(MAX_QUERY_LIMIT) as i64)
}

/// Retrieve specific tag created by the user
pub fn get_tag_by_tagger_and_id(tagger_id: &str, tag_id: &str) -> Query {
    Query::new(
        "get_tag_by_tagger_and_id",
        "
        MATCH (tagger:User { id: $tagger_id})-[tag:TAGGED {id: $tag_id }]->(tagged)
        RETURN
            labels(tagged) as tagged_labels,
            CASE WHEN tagged:Post OR tagged:User THEN tagged.uri END as tagged_uri,
            tag.id as id,
            tag.indexed_at as indexed_at,
            tag.label as label
        ",
    )
    .param("tagger_id", tagger_id)
    .param("tag_id", tag_id)
}

/// Whether the moderator took down an address; the `:Takedown(uri)` constraint indexes it.
pub fn is_taken_down(uri: &str) -> Query {
    Query::new(
        "is_taken_down",
        "RETURN EXISTS { MATCH (:Takedown {uri: $uri}) } AS taken_down",
    )
    .param("uri", uri.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::graph::query::TelemetryValue;
    use crate::types::WotDepth;
    use TelemetryValue::{Int, Str};

    fn build_query(source: StreamSource) -> Query {
        post_stream(
            source,
            StreamSorting::Timeline,
            SortOrder::Descending,
            &None,
            Pagination {
                limit: Some(10),
                ..Default::default()
            },
            None,
            None,
        )
        .unwrap()
    }

    fn all_stream(
        sorting: StreamSorting,
        tags: &Option<Vec<String>>,
        kind: Option<KindFilter>,
        rule: Option<TrustRule>,
    ) -> String {
        let pagination = Pagination {
            limit: Some(10),
            ..Default::default()
        };
        let order = SortOrder::Descending;
        post_stream(
            StreamSource::All,
            sorting,
            order,
            tags,
            pagination,
            kind,
            rule,
        )
        .unwrap()
        .to_cypher_populated()
    }

    fn assert_in_order(cypher: &str, clauses: &[&str]) {
        let positions: Vec<Option<usize>> = clauses.iter().map(|c| cypher.find(c)).collect();
        assert!(positions.iter().all(Option::is_some), "{cypher}");
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{cypher}");
    }

    /// The posts are filtered and sorted before the author is matched, so the
    /// planner checks authors lazily in sort order and stops at LIMIT.
    #[test]
    fn post_stream_trust_rule_matches_authors_after_sorting_posts() {
        let tags = Some(vec!["a".to_string(), "b".to_string()]);
        let kind = Some(KindFilter::Kind(
            pubky_social_specs::legacy_v0::PubkyAppPostKind::Short,
        ));
        let cypher = all_stream(
            StreamSorting::Timeline,
            &tags,
            kind,
            Some(TrustRule::Ranked),
        );
        assert!(cypher.starts_with("MATCH (p:Post)\n"), "{cypher}");
        let rule = format!("WHERE {}", ranked_user("author"));
        let clauses = [
            "MATCH (:User)-[tag:TAGGED]->(p)",
            "p.kind = 'short'",
            "NOT ( (p)-[:REPLIED]->(:Post) )",
            "WITH DISTINCT p\nORDER BY p.indexed_at DESC, p.id DESC",
            "MATCH (p)<-[:AUTHORED]-(author:User)",
            &rule,
            "RETURN author.id AS author_id",
        ];
        assert_in_order(&cypher, &clauses);
    }

    /// Engagement sorting drops hidden authors before counting engagement.
    #[test]
    fn post_stream_trust_rule_filters_engagement_streams() {
        let kind = Some(KindFilter::Kind(
            pubky_social_specs::legacy_v0::PubkyAppPostKind::Short,
        ));
        let ranked = Some(TrustRule::Ranked);
        let cypher = all_stream(StreamSorting::TotalEngagement, &None, kind, ranked);
        let rule = format!("WHERE {}", ranked_user("author"));
        let clauses = [
            "WITH DISTINCT p\nMATCH (p)<-[:AUTHORED]-(author:User)",
            &rule,
            "AS total_engagement",
            "ORDER BY total_engagement DESC",
        ];
        assert_in_order(&cypher, &clauses);
    }

    fn build(source: StreamSource) -> String {
        build_query(source).to_cypher_populated()
    }

    #[test]
    fn reach_queries_use_base_label_with_reach_and_depth_attrs() {
        let cases = [
            (StreamReach::Followers, vec![("reach", Str("followers"))]),
            (StreamReach::Following, vec![("reach", Str("following"))]),
            (StreamReach::Friends, vec![("reach", Str("friends"))]),
            (
                StreamReach::Wot(WotDepth::new(1).unwrap()),
                vec![("reach", Str("wot")), ("depth", Int(1))],
            ),
            (
                StreamReach::Wot(WotDepth::new(2).unwrap()),
                vec![("reach", Str("wot")), ("depth", Int(2))],
            ),
            (
                StreamReach::Wot(WotDepth::new(3).unwrap()),
                vec![("reach", Str("wot")), ("depth", Int(3))],
            ),
        ];
        for (reach, expected) in cases {
            let influencers =
                get_influencers_by_reach("user", reach.clone(), 0, 10, &Timeframe::AllTime);
            assert_eq!(influencers.label(), "get_influencers_by_reach");
            assert_eq!(influencers.telemetry_attrs(), expected.as_slice());

            let taggers = get_tag_taggers_by_reach("tag", "user", reach.clone(), 0, 10);
            assert_eq!(taggers.label(), "get_tag_taggers_by_reach");
            assert_eq!(taggers.telemetry_attrs(), expected.as_slice());

            let user_tag_search =
                search_users_by_tags_with_reach(&["label".into()], "user", &reach, None, None);
            assert_eq!(user_tag_search.label(), "search_users_by_tags_with_reach");
            assert_eq!(user_tag_search.telemetry_attrs(), expected.as_slice());

            let reach_authors = get_reach_authors_by_posts("user", &reach, 10);
            assert_eq!(reach_authors.label(), "get_reach_authors_by_posts");
            assert_eq!(reach_authors.telemetry_attrs(), expected.as_slice());

            let reach_contains = reach_contains_user("user", "target", &reach);
            assert_eq!(reach_contains.label(), "reach_contains_user");
            assert_eq!(reach_contains.telemetry_attrs(), expected.as_slice());

            let hot_tags_input = HotTagsInputDTO::new(Timeframe::AllTime, 10, 0, 5, None);
            let hot_tags = get_hot_tags_by_reach("user", reach, &hot_tags_input);
            assert_eq!(hot_tags.label(), "get_hot_tags_by_reach");
            assert_eq!(hot_tags.telemetry_attrs(), expected.as_slice());
        }
    }

    #[test]
    fn post_stream_uses_base_label_with_source_and_depth_attrs() {
        let observer = || "obs".to_string();
        let cases: Vec<(StreamSource, Vec<(&'static str, TelemetryValue)>)> = vec![
            (StreamSource::All, vec![("source", Str("all"))]),
            (
                StreamSource::Following {
                    observer_id: observer(),
                },
                vec![("source", Str("following"))],
            ),
            (
                StreamSource::Followers {
                    observer_id: observer(),
                },
                vec![("source", Str("followers"))],
            ),
            (
                StreamSource::Friends {
                    observer_id: observer(),
                },
                vec![("source", Str("friends"))],
            ),
            (
                StreamSource::Bookmarks {
                    observer_id: observer(),
                },
                vec![("source", Str("bookmarks"))],
            ),
            (
                StreamSource::Author {
                    author_id: observer(),
                },
                vec![("source", Str("author"))],
            ),
            (
                StreamSource::AuthorReplies {
                    author_id: observer(),
                },
                vec![("source", Str("author_replies"))],
            ),
            (
                StreamSource::PostReplies {
                    post_id: "post".to_string(),
                    author_id: observer(),
                },
                vec![("source", Str("post_replies"))],
            ),
            (
                StreamSource::Wot {
                    observer_id: observer(),
                    depth: WotDepth::new(1).unwrap(),
                },
                vec![("source", Str("wot")), ("depth", Int(1))],
            ),
            (
                StreamSource::Wot {
                    observer_id: observer(),
                    depth: WotDepth::new(3).unwrap(),
                },
                vec![("source", Str("wot")), ("depth", Int(3))],
            ),
            (
                StreamSource::WotDomain {
                    observer_id: observer(),
                    trust: DomainTrust::Me,
                    domain_tags: vec!["bitcoin".to_string()],
                },
                vec![("source", Str("wot_domain")), ("depth", Int(0))],
            ),
            (
                StreamSource::WotDomain {
                    observer_id: observer(),
                    trust: DomainTrust::Network(WotDepth::new(2).unwrap()),
                    domain_tags: vec!["bitcoin".to_string()],
                },
                vec![("source", Str("wot_domain")), ("depth", Int(2))],
            ),
        ];
        for (source, expected) in cases {
            let query = build_query(source);
            assert_eq!(query.label(), "post_stream");
            assert_eq!(query.telemetry_attrs(), expected.as_slice());
        }
    }

    /// The trust traversal must bind and dedupe authors before the posts MATCH.
    /// A CALL subquery runs once per incoming row (the planner cannot hoist it)
    /// and a variable-length traversal yields one row per path, so either one
    /// placed after the posts MATCH multiplies into posts-times-reach work.
    #[test]
    fn wot_sources_bind_authors_before_posts() {
        let observer = || "obs".to_string();
        let tags = || vec!["bitcoin".to_string()];
        for source in [
            StreamSource::Wot {
                observer_id: observer(),
                depth: WotDepth::default(),
            },
            StreamSource::WotDomain {
                observer_id: observer(),
                trust: DomainTrust::Me,
                domain_tags: tags(),
            },
            StreamSource::WotDomain {
                observer_id: observer(),
                trust: DomainTrust::Network(WotDepth::default()),
                domain_tags: tags(),
            },
        ] {
            let cypher = build(source);
            let dedup = cypher
                .find("WITH DISTINCT author")
                .expect("wot sources dedupe authors");
            let posts = cypher.find("MATCH (p:Post)").expect("posts match");
            assert!(
                dedup < posts,
                "author dedup must precede the posts MATCH:\n{cypher}"
            );
        }
    }

    #[test]
    fn user_tag_search_dedupes_reach_before_tag_join() {
        let labels = ["label".to_string()];
        let reaches = [
            StreamReach::Followers,
            StreamReach::Following,
            StreamReach::Friends,
            StreamReach::Wot(WotDepth::new(1).unwrap()),
            StreamReach::Wot(WotDepth::new(2).unwrap()),
            StreamReach::Wot(WotDepth::new(3).unwrap()),
        ];
        for reach in reaches {
            let cypher = search_users_by_tags_with_reach(&labels, "user", &reach, None, None)
                .to_cypher_populated();
            let dedupe = cypher
                .find("WITH DISTINCT reach AS u")
                .unwrap_or_else(|| panic!("{reach:?} must dedupe the reach:\n{cypher}"));
            let tag_join = cypher
                .find("MATCH (tagger:User)-[tag:TAGGED]->(u:User)")
                .unwrap_or_else(|| panic!("{reach:?} must join tags on u:\n{cypher}"));
            assert!(
                dedupe < tag_join,
                "{reach:?} must dedupe before the tag join, or tags count once per path:\n{cypher}"
            );
        }
    }

    #[test]
    fn reach_membership_uses_shortest_path_for_wot() {
        let wot = reach_contains_user(
            "user",
            "target",
            &StreamReach::Wot(WotDepth::new(3).unwrap()),
        )
        .to_cypher_populated();
        assert!(
            wot.contains("EXISTS { MATCH shortestPath((user)-[:FOLLOWS*1..3]->(target)) }"),
            "WoT membership must use shortestPath:\n{wot}"
        );
        let friends =
            reach_contains_user("user", "target", &StreamReach::Friends).to_cypher_populated();
        assert!(
            friends.contains("(user)-[:FOLLOWS]->(target)")
                && friends.contains("(target)-[:FOLLOWS]->(user)"),
            "friends must check both directions:\n{friends}"
        );
    }

    #[test]
    fn trusted_network_tag_queries_exclude_viewer_reached_through_cycle() {
        let user_tags = get_viewer_trusted_network_tags("user", "viewer", WotDepth::default())
            .to_cypher_populated();
        let post_tags = get_viewer_trusted_network_post_tags(
            "author",
            "post",
            "viewer",
            WotDepth::default(),
            0,
            10,
            10,
        )
        .to_cypher_populated();

        for cypher in [user_tags, post_tags] {
            assert!(
                cypher.contains("WHERE tagger.id <> viewer.id"),
                "trusted-network tag queries must exclude a viewer reached through a follow cycle:\n{cypher}"
            );
        }
    }

    fn build_resource_stream(
        app: Option<&str>,
        labels: Option<&[String]>,
        sorting: ResourceSorting,
        order: SortOrder,
        pagination: Pagination,
    ) -> String {
        resource_stream(app, labels, &sorting, &order, &pagination).to_cypher_populated()
    }

    #[test]
    fn resource_stream_timeline_orders_by_latest_matching_tag() {
        let cypher = build_resource_stream(
            None,
            None,
            ResourceSorting::Timeline,
            SortOrder::Descending,
            Pagination::default(),
        );
        assert!(cypher.contains("WITH r, MAX(t.indexed_at) AS score"));
        assert!(
            !cypher.contains("r.indexed_at"),
            "the resource node timestamp must not decide the timeline: {cypher}"
        );
        assert!(cypher.contains("ORDER BY score DESC, r.id DESC"));
        assert!(
            !cypher.contains("WHERE"),
            "no filter without app/labels: {cypher}"
        );
        assert!(cypher.contains("SKIP 0\nLIMIT 10"));
    }

    #[test]
    fn resource_stream_applies_app_and_label_filters_to_the_edges() {
        let labels = vec!["bitcoin".to_string(), "nostr".to_string()];
        let cypher = build_resource_stream(
            Some("mapky"),
            Some(&labels),
            ResourceSorting::Timeline,
            SortOrder::Descending,
            Pagination::default(),
        );
        assert!(cypher.contains("MATCH (tagger:User)-[t:TAGGED]->(r:Resource)\nWHERE t.app = 'mapky' AND t.label IN ['bitcoin', 'nostr']\n"));
    }

    #[test]
    fn resource_stream_cursor_bounds_follow_the_sort_direction() {
        let pagination = Pagination {
            skip: Some(5),
            limit: Some(7),
            start: Some(200.0),
            end: Some(100.0),
        };
        let desc = build_resource_stream(
            None,
            None,
            ResourceSorting::Timeline,
            SortOrder::Descending,
            pagination,
        );
        assert!(desc.contains("AS score\nWHERE score <= 200"));
        assert!(desc.contains("AND score >= 100\n"));
        assert!(desc.contains("ORDER BY score DESC, r.id DESC\nSKIP 5\nLIMIT 7"));

        let asc = build_resource_stream(
            None,
            None,
            ResourceSorting::Timeline,
            SortOrder::Ascending,
            pagination,
        );
        assert!(asc.contains("AS score\nWHERE score >= 200"));
        assert!(asc.contains("AND score <= 100\n"));
        assert!(asc.contains("ORDER BY score ASC, r.id ASC\nSKIP 5\nLIMIT 7"));
    }

    #[test]
    fn resource_stream_caps_skip_and_limit() {
        let cypher = build_resource_stream(
            None,
            None,
            ResourceSorting::Timeline,
            SortOrder::Descending,
            Pagination {
                skip: Some(MAX_QUERY_SKIP + 1),
                limit: Some(MAX_QUERY_LIMIT + 1),
                ..Default::default()
            },
        );
        assert!(cypher.contains(&format!("SKIP {MAX_QUERY_SKIP}\nLIMIT {MAX_QUERY_LIMIT}")));
    }

    #[test]
    fn tag_taggers_by_reach_dedups_reach_before_matching_tags() {
        let reaches = [
            StreamReach::Followers,
            StreamReach::Following,
            StreamReach::Friends,
            StreamReach::Wot(WotDepth::new(1).unwrap()),
            StreamReach::Wot(WotDepth::new(2).unwrap()),
            StreamReach::Wot(WotDepth::new(3).unwrap()),
        ];
        for reach in reaches {
            let cypher = get_tag_taggers_by_reach("bitcoin", "user_id_x", reach, 0, 20)
                .to_cypher_populated();
            let tag_match = cypher
                .find("MATCH (reach:User)-[tag:TAGGED]")
                .unwrap_or_else(|| panic!("no TAGGED match: {cypher}"));
            let distinct = cypher
                .find("WITH COLLECT(DISTINCT reach) AS reached")
                .unwrap_or_else(|| panic!("no collected reach: {cypher}"));
            let user_filter = cypher
                .find("WHERE user.id")
                .unwrap_or_else(|| panic!("no user filter: {cypher}"));
            assert!(
                distinct < tag_match,
                "reach must be deduplicated before matching tags: {cypher}"
            );
            assert!(
                user_filter < tag_match,
                "the user filter must apply before matching tags: {cypher}"
            );
            assert!(
                cypher.contains("reach IN reached"),
                "tags must be limited to the collected reach: {cypher}"
            );
            assert!(
                cypher.contains("ORDER BY latest_tag_time DESC, reach.id ASC"),
                "pages need a stable tie-break: {cypher}"
            );
        }
    }
}
