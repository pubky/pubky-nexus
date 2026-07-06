use crate::db::graph::error::{GraphError, GraphResult};
use crate::db::graph::Query;
use crate::db::kv::SortOrder;
use crate::models::post::{KindFilter, StreamSource};
use crate::models::resource::stream::ResourceSorting;
use crate::types::routes::HotTagsInputDTO;
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
                uri: 'pubky://' + u.id + '/pub/pubky.app/posts/' + p.id,
                content: p.content,
                id: p.id,
                indexed_at: p.indexed_at,
                author: u.id,
                // default value when the specified property is null
                // Avoids enum deserialization ERROR
                kind: COALESCE(p.kind, 'short'),
                attachments: p.attachments,
                lock: p.lock
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
                reposts: COUNT { (p)<-[:REPOSTED]-() }
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

// Check all the bookmarks that user creates
pub fn user_bookmarks(user_id: &str) -> Query {
    Query::new(
        "user_bookmarks",
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED]->(p:Post)<-[:AUTHORED]-(author:User)
         RETURN b, p.id AS post_id, author.id AS author_id",
    )
    .param("user_id", user_id)
}

// Get all the bookmarks that a post has received (used for edit/delete notifications)
pub fn get_post_bookmarks(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "get_post_bookmarks",
        "MATCH (bookmarker:User)-[b:BOOKMARKED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN b.id AS bookmark_id, bookmarker.id AS bookmarker_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
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
pub fn get_post_reposts(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "get_post_reposts",
        "MATCH (reposter:User)-[:AUTHORED]->(repost:Post)-[:REPOSTED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN reposter.id AS reposter_id, repost.id AS repost_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Get all the replies that a post has received (used for edit/delete notifications)
pub fn get_post_replies(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "get_post_replies",
        "MATCH (replier:User)-[:AUTHORED]->(reply:Post)-[:REPLIED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN replier.id AS replier_id, reply.id AS reply_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
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
              CASE WHEN target:Resource THEN target.id ELSE null END AS resource_id,
              tag.label AS label,
              tag.app AS app
         RETURN user_id, post_id, author_id, resource_id, label, app"
    );

    let mut query = Query::new("get_tag_target", &cypher)
        .param("user_id", user_id)
        .param("tag_id", tag_id);

    if let Some(a) = app {
        query = query.param("app", a);
    }

    query
}

// Get all the tags/taggers that a post has received (used for edit/delete notifications)
pub fn get_post_tags(author_id: &str, post_id: &str) -> Query {
    Query::new(
        "get_post_tags",
        "MATCH (p:Post {id: $post_id})
         WHERE EXISTS { (:User {id: $author_id})-[:AUTHORED]->(p) }
         MATCH (tagger:User)-[t:TAGGED]->(p)
         RETURN tagger.id AS tagger_id, t.id AS tag_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
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
          replied_post.id AS replied_post_id,
          replied_author.id AS replied_author_id,
          reposted_post.id AS reposted_post_id,
          reposted_author.id AS reposted_author_id,
          COLLECT(mentioned_user.id) AS mentioned_user_ids",
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

// Retrieve all the tags of the post
pub fn post_tags(user_id: &str, post_id: &str) -> Query {
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
            WITH tag.label AS name, collect(DISTINCT tagger.id) AS tagger_ids
            RETURN collect({
                label: name,
                taggers: tagger_ids,
                taggers_count: SIZE(tagger_ids)
            }) AS tags
        }
        RETURN
            u IS NOT NULL AS exists,
            tags
    ",
    )
    .param("user_id", user_id)
    .param("post_id", post_id)
}

// Retrieve all the tags of the user
pub fn user_tags(user_id: &str) -> Query {
    Query::new(
        "user_tags",
        "
        MATCH (u:User {id: $user_id})
        CALL {
            WITH u
            MATCH (p:User)-[t:TAGGED]->(u)
            WITH t.label AS name, collect(DISTINCT p.id) AS tagger_ids
            RETURN collect({
                label: name,
                taggers: tagger_ids,
                taggers_count: SIZE(tagger_ids)
            }) AS tags
        }
        RETURN
            u IS NOT NULL AS exists,
            tags
    ",
    )
    .param("user_id", user_id)
}

/// Retrieve Resource node details by ID
pub fn get_resource_by_id(resource_id: &str) -> Query {
    Query::new(
        "get_resource_by_id",
        "
        MATCH (r:Resource {id: $resource_id})
        RETURN r.id AS id, r.uri AS uri, r.scheme AS scheme, r.indexed_at AS indexed_at
    ",
    )
    .param("resource_id", resource_id)
}

/// Retrieve all tags on a Resource node
pub fn resource_tags(resource_id: &str) -> Query {
    Query::new(
        "resource_tags",
        "
        OPTIONAL MATCH (r:Resource {id: $resource_id})
        CALL {
            WITH r
            MATCH (tagger:User)-[tag:TAGGED]->(r)
            WITH tag.label AS name, collect(DISTINCT tagger.id) AS tagger_ids
            RETURN collect({
                label: name,
                taggers: tagger_ids,
                taggers_count: SIZE(tagger_ids)
            }) AS tags
        }
        RETURN
            r IS NOT NULL AS exists,
            tags
    ",
    )
    .param("resource_id", resource_id)
}

/// Query a stream of Resources with optional app and tag filters.
/// Falls back to this when Redis sorted sets can't satisfy the query.
pub fn resource_stream(
    app: Option<&str>,
    labels: Option<&[String]>,
    sorting: &ResourceSorting,
    order: &SortOrder,
    skip: usize,
    limit: usize,
) -> Query {
    // Map enums to safe Cypher literals — prevents injection
    let sorting_field = match sorting {
        ResourceSorting::Timeline => "r.indexed_at",
        ResourceSorting::TaggersCount => "taggers_count",
    };
    let order_direction = match order {
        SortOrder::Ascending => "ASC",
        SortOrder::Descending => "DESC",
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

    cypher.push_str(&format!(
        "WITH DISTINCT r, COUNT(DISTINCT tagger) AS taggers_count
         ORDER BY {sorting_field} {order_direction}
         SKIP $skip LIMIT $limit
         RETURN r.id AS resource_id, r.indexed_at AS indexed_at, taggers_count"
    ));

    let mut query = Query::new("resource_stream", &cypher)
        .param("skip", skip as i64)
        .param("limit", limit as i64);

    if let Some(a) = app {
        query = query.param("app", a);
    }
    if let Some(l) = labels {
        let label_strings: Vec<String> = l.iter().map(|s| s.to_string()).collect();
        query = query.param("labels", label_strings);
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
/// The results are sorted by the number of active users in descending order.
/// Returns a single `homeservers_list` column containing the collected IDs.
pub fn get_all_homeservers_with_active_users() -> Query {
    Query::new(
        "get_all_homeservers_with_active_users",
        "MATCH (u:User)-[r:HOSTED_BY]->(hs:Homeserver)
        WHERE u.name <> '[DELETED]' AND NOT coalesce(r.stale, false)
        WITH hs.id AS id, count(u) AS active_users
        ORDER BY active_users DESC
        RETURN collect(id) AS homeservers_list",
    )
}

/// Retrieves user IDs whose homeserver mapping is stale
/// (`resolved_at` is older than `ttl_ms`) or missing (no `HOSTED_BY` edge).
pub fn get_users_needing_hs_resolution(ttl_ms: u64) -> Query {
    Query::new(
        "get_users_needing_hs_resolution",
        "MATCH (u:User)
         WHERE u.name <> '[DELETED]'
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

/// Retrieves all user IDs actively hosted on a given homeserver.
///
/// Excludes users whose mapping is marked `stale` — i.e. whose published
/// homeserver has diverged from the stored one — so the watcher stops
/// indexing them until the mapping realigns.
pub fn get_active_users_by_homeserver(hs_id: &str) -> Query {
    Query::new(
        "get_active_users_by_homeserver",
        "MATCH (u:User)-[r:HOSTED_BY]->(:Homeserver {id: $hs_id})
         WHERE u.name <> '[DELETED]' AND NOT coalesce(r.stale, false)
         RETURN collect(u.id) AS user_ids",
    )
    .param("hs_id", hs_id.to_string())
}

/// Tags on a user applied by users in the viewer's Web of Trust (transitive
/// FOLLOWS, 1..=depth). User existence is the anchor: the user is matched first
/// and the viewer with `OPTIONAL MATCH`, so an existing user always returns one
/// row with `tags` (`[]` when no trusted tagger tagged them, or the viewer is
/// unknown) for an empty/normal 200; only a missing user returns zero rows (404).
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
            WITH tag.label AS label, collect(DISTINCT tagger.id) AS taggerIds
            RETURN collect({{
                label: label,
                taggers: taggerIds,
                taggers_count: SIZE(taggerIds)
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
/// (transitive FOLLOWS, 1..=depth). Post existence is the anchor: the post is
/// matched first and the viewer with `OPTIONAL MATCH`, so an existing post always
/// returns one row with `tags` (`[]` when no trusted tagger tagged it, or when the
/// viewer is unknown) for an empty/normal 200; only a missing post returns zero
/// rows (404). Labels are ordered by tagger count and paginated with
/// `skip_tags`/`limit_tags`; each label's taggers are capped at `limit_taggers`,
/// mirroring the global tag endpoint so the response stays bounded.
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
            WITH tag.label AS label, collect(DISTINCT tagger.id) AS taggerIds
            WITH label, taggerIds, SIZE(taggerIds) AS taggersCount
            ORDER BY taggersCount DESC, label ASC
            SKIP $skip_tags
            LIMIT $limit_tags
            RETURN collect({{
                label: label,
                taggers: taggerIds[0..$limit_taggers],
                taggers_count: taggersCount
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

pub fn user_counts(user_id: &str) -> Query {
    Query::new(
        "user_counts",
        "
        MATCH (u:User {id: $user_id})
        // Each field is an independent COUNT { } subquery off the single (u) row.
        // Do NOT precede this with a row-multiplying OPTIONAL MATCH (e.g. over
        // received tags): that makes every subquery a per-row grouping key, so the
        // whole block runs once per received tag, i.e. O(received_tags x authored_posts)
        // and hangs on heavy users. See issue #935.
        RETURN
            u IS NOT NULL AS exists,
            {
                following: COUNT { (u)-[:FOLLOWS]->(:User) },
                followers: COUNT { (:User)-[:FOLLOWS]->(u) },
                friends: COUNT { (u)-[:FOLLOWS]->(friend:User) WHERE (friend)-[:FOLLOWS]->(u) },
                posts: COUNT { (u)-[:AUTHORED]->(:Post) },
                replies: COUNT { (u)-[:AUTHORED]->(:Post)-[:REPLIED]->(:Post) },
                collections: COUNT { (u)-[:AUTHORED]->(p:Post) WHERE p.kind = 'collection' },
                // A collection-follow is stored as a bookmark; keep it out of the count.
                bookmarks: COUNT { (u)-[:BOOKMARKED]->(bp:Post) WHERE (bp.kind IS NULL OR bp.kind <> 'collection') },
                // tagged = tags this user assigned to users + to posts
                tagged: COUNT { (u)-[:TAGGED]->(:User) } + COUNT { (u)-[:TAGGED]->(:Post) },
                tags: COUNT { (:User)-[:TAGGED]->(u) },
                unique_tags: COUNT { MATCH (u)<-[t:TAGGED]-(:User) RETURN DISTINCT t.label }
            } AS counts;
        ",
    )
    .param("user_id", user_id)
}

pub fn get_user_followers(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (u)<-[:FOLLOWS]-(follower:User)
         RETURN COUNT(u) > 0 AS user_exists,
                COLLECT(follower.id) AS follower_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value.min(MAX_QUERY_SKIP)));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value.min(MAX_QUERY_LIMIT)));
    }
    Query::new("get_user_followers", &query_string).param("user_id", user_id)
}

pub fn get_user_following(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
         RETURN COUNT(u) > 0 AS user_exists,
                COLLECT(following.id) AS following_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value.min(MAX_QUERY_SKIP)));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value.min(MAX_QUERY_LIMIT)));
    }
    Query::new("get_user_following", &query_string).param("user_id", user_id)
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
            // The tagged node can be generic, representing either a Post, a User, or both.
            // For now, it will be a Post to align with UX requirements.
            MATCH (reach)-[tag:TAGGED]->(tagged:Post)
            WHERE user.id = $user_id AND tag.label = $label

            // Get the latest tagged timestamp per `reach` user
            WITH DISTINCT reach, MAX(tag.indexed_at) AS latest_tag_time
            ORDER BY latest_tag_time DESC

            // Use slice notation instead of SKIP and LIMIT
            WITH COLLECT({{ reach_id: reach.id }})[$skip..$skip + $limit] AS paginated
            UNWIND paginated AS row

            RETURN COLLECT(row.reach_id) AS tagger_ids
            ",
        stream_reach_to_graph_subquery(&reach)
    );
    Query::new("get_tag_taggers_by_reach", &cypher)
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
        MATCH (reach)-[tag:TAGGED]->(tagged:{})
        WHERE user.id = $user_id AND tag.indexed_at >= $from AND tag.indexed_at < $to
        WITH
            tag.label AS label,
            COLLECT(DISTINCT reach.id)[..{}] AS taggers,
            COUNT(DISTINCT tagged) AS uniqueTaggedCount,
            COUNT(DISTINCT reach.id) AS taggers_count
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
    Query::new("get_hot_tags_by_reach", &cypher)
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
        WITH
            tag.label AS label,
            COLLECT(DISTINCT user.id)[..{}] AS taggers,
            COUNT(DISTINCT tagged) AS uniqueTaggedCount,
            COUNT(DISTINCT user.id) AS taggers_count
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
        WHERE reach.name <> '[DELETED]'

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
    Query::new("get_influencers_by_reach", &cypher)
        .param("user_id", user_id)
        .param("skip", skip as i64)
        .param("limit", limit as i64)
        .param("from", from)
        .param("to", to)
}

pub fn get_global_influencers(skip: usize, limit: usize, timeframe: &Timeframe) -> Query {
    let (from, to) = timeframe.to_timestamp_range();
    Query::new(
        "get_global_influencers",
        "
        MATCH (user:User)
        WHERE user.name <> '[DELETED]'
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
            MATCH (user)-[authored:AUTHORED]->(post:Post)
            WHERE authored.indexed_at >= $from AND authored.indexed_at < $to
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

// Build the graph query based on parameters
pub fn post_stream(
    source: StreamSource,
    sorting: StreamSorting,
    order: SortOrder,
    tags: &Option<Vec<String>>,
    pagination: Pagination,
    kind: Option<KindFilter>,
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

    // Base match for posts and authors
    cypher.push_str("MATCH (p:Post)<-[:AUTHORED]-(author:User)\n");

    // Apply source MATCH clause
    match &source {
        StreamSource::Following { .. } => {
            cypher.push_str("MATCH (observer)-[:FOLLOWS]->(author)\n")
        }
        StreamSource::Followers { .. } => {
            cypher.push_str("MATCH (observer)<-[:FOLLOWS]-(author)\n")
        }
        StreamSource::Friends { .. } => {
            cypher.push_str("MATCH (observer)-[:FOLLOWS]->(author)-[:FOLLOWS]->(observer)\n")
        }
        StreamSource::Bookmarks { .. } => cypher.push_str("MATCH (observer)-[:BOOKMARKED]->(p)\n"),
        // Inline: `author` is already bound by the base AUTHORED match, so a
        // subquery returning `author` would conflict. Terminal WITH DISTINCT dedupes.
        StreamSource::Wot { depth, .. } => {
            cypher.push_str(&format!("MATCH (observer)-[:FOLLOWS*1..{depth}]->(author)\n"))
        }
        // Me (depth-0): the observer is the sole tagger, so match their TAGGED
        // edge directly, no traversal and no CALL. Cheaper than the network path.
        StreamSource::WotDomain {
            trust: DomainTrust::Me,
            ..
        } => cypher.push_str("MATCH (observer)-[endorsement:TAGGED]->(author)\n"),
        // Network: CALL collapses the trust reach to distinct taggers before the tag join.
        StreamSource::WotDomain {
            trust: DomainTrust::Network(depth),
            ..
        } => cypher.push_str(&format!(
            "CALL {{ WITH observer MATCH (observer)-[:FOLLOWS*1..{depth}]->(tagger:User) RETURN DISTINCT tagger }}\n\
             MATCH (tagger)-[endorsement:TAGGED]->(author)\n"
        )),
        _ => {}
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

    // Web of Trust must not surface the observer's own posts: a trusted path can
    // loop back to them (a mutual follow reaches the observer at depth >= 2 for
    // `wot`; a WoT member tagging the observer reaches them for `wot_domain`).
    // After the tags MATCH so all WHERE/AND conditions stay contiguous.
    if matches!(
        source,
        StreamSource::Wot { .. } | StreamSource::WotDomain { .. }
    ) {
        append_condition(
            &mut cypher,
            "author.id <> $observer_id",
            &mut where_clause_applied,
        );
    }

    // Domain-trust filter: keep authors endorsed (tagged) by a WoT tagger with a
    // label in $domain_tags. Placed after the tags block so all WHERE/AND
    // conditions stay contiguous; `endorsement` is in scope until WITH DISTINCT.
    if matches!(source, StreamSource::WotDomain { .. }) {
        append_condition(
            &mut cypher,
            "endorsement.label IN $domain_tags",
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

    // Make unique the posts, cannot be repeated
    cypher.push_str("WITH DISTINCT p, author\n");

    let order_dir = match order {
        SortOrder::Ascending => "ASC",
        SortOrder::Descending => "DESC",
    };

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

    let query_name = match &source {
        StreamSource::Following { .. } => "post_stream_following",
        StreamSource::Followers { .. } => "post_stream_followers",
        StreamSource::Friends { .. } => "post_stream_friends",
        StreamSource::Bookmarks { .. } => "post_stream_bookmarks",
        StreamSource::Author { .. } => "post_stream_author",
        StreamSource::AuthorReplies { .. } => "post_stream_author_replies",
        StreamSource::PostReplies { .. } => "post_stream_post_replies",
        // Short-circuited upstream in collect_post_keys.
        StreamSource::Collection { .. } => {
            return Err(GraphError::QueryBuildError(
                "StreamSource::Collection must be served by collect_post_keys".to_string(),
            ));
        }
        StreamSource::Wot { .. } => "post_stream_wot",
        StreamSource::WotDomain { .. } => "post_stream_wot_domain",
        StreamSource::All => "post_stream_all",
    };
    let query = Query::new(query_name, &cypher);
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
            )
        } AS flag
",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

/// Find user recommendations: active users (with 5+ posts) who are 1-3 degrees of separation away
/// from the given user, but not directly followed by them
pub fn recommend_users(user_id: &str, limit: usize) -> Query {
    Query::new(
        "recommend_users",
        "
        MATCH (user:User {id: $user_id})
        MATCH (user)-[:FOLLOWS*1..3]->(potential:User)
        WHERE NOT (user)-[:FOLLOWS]->(potential)
        AND potential.id <> $user_id
        WITH DISTINCT potential
        MATCH (potential)-[:AUTHORED]->(post:Post)
        WITH potential, COUNT(post) AS post_count
        WHERE post_count >= 5
        RETURN potential.id AS recommended_user_id, potential.name AS recommended_user_name
        LIMIT $limit
    ",
    )
    .param("user_id", user_id.to_string())
    .param("limit", limit as i64)
}

/// Retrieve specific tag created by the user
pub fn get_tag_by_tagger_and_id(tagger_id: &str, tag_id: &str) -> Query {
    Query::new(
        "get_tag_by_tagger_and_id",
        "
        MATCH (tagger:User { id: $tagger_id})-[tag:TAGGED {id: $tag_id }]->(tagged)
        OPTIONAL MATCH (author:User)-[:AUTHORED]->(tagged)
        RETURN
            labels(tagged) as tagged_labels,
            tagged.id as tagged_id,
            author.id as author_id,
            tag.id as id,
            tag.indexed_at as indexed_at,
            tag.label as label
        ",
    )
    .param("tagger_id", tagger_id)
    .param("tag_id", tag_id)
}
