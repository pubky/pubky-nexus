use crate::models::post::StreamSource;
use crate::types::Pagination;
use crate::types::StreamSorting;
use neo4rs::{query, Query};
use pubky_app_specs::PostKind;

// Retrieve post node by post id and author id
pub fn get_post_by_id(author_id: &str, post_id: &str) -> Query {
    query(
        "
            MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
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
                attachments: p.attachments
            } as details,
            COLLECT([author.id, parent_post.id]) AS reply

        ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn post_counts(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         OPTIONAL MATCH (p)<-[tag:TAGGED]-()
         OPTIONAL MATCH (p)<-[reply:REPLIED]-()
         OPTIONAL MATCH (p)<-[repost:REPOSTED]-()
         OPTIONAL MATCH (p)-[replied:REPLIED]->(Post)
         WITH p, COUNT(DISTINCT tag) AS tags_count,
                 COUNT(DISTINCT reply) AS replies_count,
                 COUNT(DISTINCT repost) AS reposts_count,
                 (replied IS NOT NULL) AS is_reply
         RETURN 
            p IS NOT NULL AS exists,
            {
                tags: tags_count,
                replies: replies_count,
                reposts: reposts_count
            } AS counts,
            is_reply
        ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Check if the viewer_id has a bookmark in the post
pub fn post_bookmark(author_id: &str, post_id: &str, viewer_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         OPTIONAL MATCH (viewer:User {id: $viewer_id})-[b:BOOKMARKED]->(p)
         RETURN b",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("viewer_id", viewer_id)
}

// Check all the bookmarks that user creates
pub fn user_bookmarks(user_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $user_id})-[b:BOOKMARKED]->(p:Post)<-[:AUTHORED]-(author:User)
         RETURN b, p.id AS post_id, author.id AS author_id",
    )
    .param("user_id", user_id)
}

// Get all the bookmarks that a post has received (used for edit/delete notifications)
pub fn get_post_bookmarks(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (bookmarker:User)-[b:BOOKMARKED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN b.id AS bookmark_id, bookmarker.id AS bookmarker_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Get all the reposts that a post has received (used for edit/delete notifications)
pub fn get_post_reposts(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (reposter:User)-[:AUTHORED]->(repost:Post)-[:REPOSTED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN reposter.id AS reposter_id, repost.id AS repost_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Get all the replies that a post has received (used for edit/delete notifications)
pub fn get_post_replies(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (replier:User)-[:AUTHORED]->(reply:Post)-[:REPLIED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN replier.id AS replier_id, reply.id AS reply_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Get all the tags/taggers that a post has received (used for edit/delete notifications)
pub fn get_post_tags(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (tagger:User)-[t:TAGGED]->(p:Post {id: $post_id})<-[:AUTHORED]-(author:User {id: $author_id})
         RETURN tagger.id AS tagger_id, t.id AS tag_id",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn post_relationships(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
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
    query(
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
pub fn global_tags_by_post() -> neo4rs::Query {
    query(
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
pub fn global_tags_by_post_engagement() -> neo4rs::Query {
    query(
        "
        MATCH (author:User)-[:AUTHORED]->(post:Post)<-[tag:TAGGED]-(tagger:User)
        WITH post, COUNT(tag) AS tags_count, tag.label AS label, author.id + ':' + post.id AS key
        WITH DISTINCT key, label, post, tags_count
        WHERE tags_count > 0
        OPTIONAL MATCH (post)<-[reply:REPLIED]-()
        OPTIONAL MATCH (post)<-[repost:REPOSTED]-()
        OPTIONAL MATCH (post)-[mention:MENTIONED]->()
        OPTIONAL MATCH (post)<-[tagged:TAGGED]-()
        WITH COUNT(DISTINCT tagged) AS taggers, COUNT(DISTINCT reply) AS replies_count, COUNT(DISTINCT repost) AS reposts_count, COUNT(DISTINCT mention) AS mention_count, key, label
        WITH label, COLLECT([toFloat(taggers + replies_count + reposts_count + mention_count), key ]) AS sorted_set
        RETURN label, sorted_set
        order by label
        "
    )
}

// Retrieve all the tags of the post
pub fn post_tags(user_id: &str, post_id: &str) -> neo4rs::Query {
    query(
        "
        MATCH (u:User {id: $user_id})-[:AUTHORED]->(p:Post {id: $post_id})
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
pub fn user_tags(user_id: &str) -> neo4rs::Query {
    query(
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

pub fn user_counts(user_id: &str) -> neo4rs::Query {
    query(
        "
        MATCH (u:User {id: $user_id})

        // Collect outgoing relationships to Users
        OPTIONAL MATCH (u)-[rel_u_user:FOLLOWS|TAGGED]->(other_user:User)
        WITH u, collect(rel_u_user) AS rels_u_user

        // Collect incoming relationships from Users
        OPTIONAL MATCH (user_to_u:User)-[rel_user_u:FOLLOWS|TAGGED]->(u)
        WITH u, rels_u_user, collect(rel_user_u) AS rels_user_u

        // Find friends
        OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
        WITH u, rels_u_user, rels_user_u, count(friend) AS friends

        // Collect relationships to Posts
        OPTIONAL MATCH (u)-[rel_u_post:BOOKMARKED|AUTHORED|TAGGED]->(p:Post)
        WITH u, rels_u_user, rels_user_u, friends, collect(rel_u_post) AS rels_u_post

        // Count replies authored by the user
        OPTIONAL MATCH (u)-[:AUTHORED]->(reply:Post)-[:REPLIED]->(:Post)
        WITH u, rels_u_user, rels_user_u, friends, rels_u_post, count(reply) AS replies

        // Calculate counts
        WITH u,
            size([rel IN rels_u_user WHERE type(rel) = 'FOLLOWS']) AS following,
            size([rel IN rels_user_u WHERE type(rel) = 'FOLLOWS']) AS followers,
            friends,
            size([rel IN rels_u_post WHERE type(rel) = 'AUTHORED']) AS posts,
            size([rel IN rels_u_post WHERE type(rel) = 'TAGGED']) AS post_tags,
            size([rel IN rels_u_user WHERE type(rel) = 'TAGGED']) AS user_tags,
            size([rel IN rels_u_post WHERE type(rel) = 'BOOKMARKED']) AS bookmarks,
            size([rel IN rels_user_u WHERE type(rel) = 'TAGGED']) AS tagged,
            replies
        RETURN 
            u IS NOT NULL AS exists,
            {
                following: following,
                followers: followers,
                friends: friends,
                posts: posts,
                tags: user_tags + post_tags,
                bookmarks: bookmarks,
                tagged: tagged,
                replies: replies
            } AS counts
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
        query_string.push_str(&format!(" SKIP {}", skip_value));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value));
    }
    query(&query_string).param("user_id", user_id)
}

pub fn get_user_following(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id}) 
         OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
         RETURN COUNT(u) > 0 AS user_exists, 
                COLLECT(following.id) AS following_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value));
    }
    query(&query_string).param("user_id", user_id)
}

pub fn get_user_muted(user_id: &str, skip: Option<usize>, limit: Option<usize>) -> Query {
    let mut query_string = String::from(
        "MATCH (u:User {id: $user_id}) 
         OPTIONAL MATCH (u)-[:MUTED]->(muted:User)
         RETURN COUNT(u) > 0 AS user_exists, 
                COLLECT(muted.id) AS muted_ids",
    );
    if let Some(skip_value) = skip {
        query_string.push_str(&format!(" SKIP {}", skip_value));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {}", limit_value));
    }
    query(&query_string).param("user_id", user_id)
}

// Retrieves posts popular tags and its taggers across the entire network
pub fn get_global_hot_tags_scores() -> Query {
    query(
        "
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WITH tag.label AS label, COUNT(DISTINCT p) AS uniquePosts, COLLECT(DISTINCT u.id) AS user_ids
        RETURN COLLECT([toFloat(uniquePosts), label]) AS hot_tags_score, COLLECT([label, user_ids]) AS hot_tags_users
    ",
    )
}

// Retrieves popular hot tags taggers across the entire network
pub fn get_global_hot_tags_taggers(tag_list: &[&str]) -> Query {
    query(
        "
        UNWIND $labels AS tag_name
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WHERE tag.label = tag_name
        WITH tag.label AS label, COLLECT(DISTINCT u.id) AS userIds
        RETURN COLLECT(userIds) AS tag_user_ids
    ",
    )
    .param("labels", tag_list)
}

// Analyzes tag usage for a specific list of user IDs. Groups tags by name,
// showing for each: label, post count and list of user IDs
// Orders by post_count (descending).
// Note: Only considers users from the provided users_id list.
pub fn get_tags_by_user_ids(users_id: &[&str]) -> Query {
    query(
        "
        UNWIND $ids AS id
        MATCH (u:User)-[tag:TAGGED]->(p:Post)
        WHERE u.id = id
        WITH tag.label AS label, COLLECT(DISTINCT u.id) AS taggers, COUNT(DISTINCT p) AS uniquePosts
        WITH {
            label: label,
            taggers_id: taggers,
            post_count: uniquePosts,
            taggers_count: SIZE(taggers)
        } AS hot_tag
        ORDER BY hot_tag.post_count DESC
        RETURN COLLECT(hot_tag) AS hot_tags
    ",
    )
    .param("ids", users_id)
}

pub fn get_files_by_ids(key_pair: &[&[&str]]) -> Query {
    query(
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
    tags: &Option<Vec<String>>,
    pagination: Pagination,
    kind: Option<PostKind>,
) -> Query {
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
    if let Some(query) = match source {
        StreamSource::Following { .. } => Some("MATCH (observer)-[:FOLLOWS]->(author)\n"),
        StreamSource::Followers { .. } => Some("MATCH (observer)<-[:FOLLOWS]-(author)\n"),
        StreamSource::Friends { .. } => {
            Some("MATCH (observer)-[:FOLLOWS]->(author)-[:FOLLOWS]->(observer)\n")
        }
        StreamSource::Bookmarks { .. } => Some("MATCH (observer)-[:BOOKMARKED]->(p)\n"),
        _ => None,
    } {
        cypher.push_str(query);
    }

    // Apply tags
    if tags.is_some() {
        cypher.push_str("MATCH (User)-[tag:TAGGED]->(p)\n");
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

    // If post kind is provided, add the corresponding condition
    if kind.is_some() {
        append_condition(&mut cypher, "p.kind = $kind", &mut where_clause_applied);
    }

    // Apply time interval conditions. Only can be applied with timeline sorting
    // The engagament score has to be computed
    if sorting == StreamSorting::Timeline {
        if pagination.start.is_some() {
            append_condition(
                &mut cypher,
                "p.indexed_at <= $start",
                &mut where_clause_applied,
            );
        }

        if pagination.end.is_some() {
            append_condition(
                &mut cypher,
                "p.indexed_at >= $end",
                &mut where_clause_applied,
            );
        }
    }

    // Make unique the posts, cannot be repeated
    cypher.push_str("WITH DISTINCT p, author\n");

    // Apply StreamSorting
    // Conditionally compute engagement counts only for TotalEngagement sorting
    let order_clause = match sorting {
        StreamSorting::Timeline => "ORDER BY p.indexed_at DESC".to_string(),
        StreamSorting::TotalEngagement => {
            // TODO: These optional matches could potentially be combined/collected to improve performance
            cypher.push_str(
                "
                // Count tags
                OPTIONAL MATCH (p)<-[tag:TAGGED]-(:User)  
                // Count replies
                OPTIONAL MATCH (p)<-[reply:REPLIED]-(:Post)
                // Count reposts
                OPTIONAL MATCH (p)<-[repost:REPOSTED]-(:Post)

                WITH p, author, 
                    COUNT(DISTINCT tag) AS tags_count,
                    COUNT(DISTINCT reply) AS replies_count,
                    COUNT(DISTINCT repost) AS reposts_count,
                    (COUNT(DISTINCT tag) + COUNT(DISTINCT reply) + COUNT(DISTINCT repost)) AS total_engagement
                ",
            );

            // Initialise again
            where_clause_applied = false;

            // Add total_engagement to filter by engagement the post
            if pagination.start.is_some() {
                append_condition(
                    &mut cypher,
                    "total_engagement <= $start",
                    &mut where_clause_applied,
                );
            }

            if pagination.end.is_some() {
                append_condition(
                    &mut cypher,
                    "total_engagement >= $end",
                    &mut where_clause_applied,
                );
            }

            "ORDER BY total_engagement DESC".to_string()
        }
    };

    // Final return statement
    cypher.push_str(&format!(
        "RETURN author.id AS author_id, p.id AS post_id\n{}\n",
        order_clause
    ));

    // Apply skip and limit
    if let Some(skip) = pagination.skip {
        cypher.push_str(&format!("SKIP {}\n", skip));
    }
    if let Some(limit) = pagination.limit {
        cypher.push_str(&format!("LIMIT {}\n", limit));
    }

    println!("{:?}", cypher);

    // Build the query and apply parameters using `param` method
    build_query_with_params(&cypher, &source, tags, kind, &pagination)
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

/// Builds a `Query` object by applying the necessary parameters to the Cypher query string.
///
/// This function takes the constructed Cypher query string and applies all the relevant parameters
/// based on the provided `source`, `tags`, `kind`, and `pagination`. It ensures that all parameters
/// used in the query are properly set with their corresponding values.
///
/// # Arguments
///
/// * `cypher` - The Cypher query string that has been constructed.
/// * `source` - The `StreamSource` specifying the origin of the posts (e.g., Following, Followers).
/// * `tags` - An optional list of tag labels to filter the posts.
/// * `kind` - An optional `PostKind` to filter the posts by their kind.
/// * `pagination` - The `Pagination` object containing pagination parameters like `start`, `end`, `skip`, and `limit`.
fn build_query_with_params(
    cypher: &str,
    source: &StreamSource,
    tags: &Option<Vec<String>>,
    kind: Option<PostKind>,
    pagination: &Pagination,
) -> Query {
    let mut query = query(cypher);

    if let Some(observer_id) = source.get_observer() {
        query = query.param("observer_id", observer_id.to_string());
    }
    if let Some(labels) = tags.clone() {
        query = query.param("labels", labels);
    }
    if let Some(author_id) = source.get_author() {
        query = query.param("author_id", author_id.to_string());
    }
    if let Some(post_kind) = kind {
        query = query.param("kind", post_kind.to_string());
    }
    if let Some(start_interval) = pagination.start {
        query = query.param("start", start_interval);
    }
    if let Some(end_interval) = pagination.end {
        query = query.param("end", end_interval);
    }

    query
}

// User has any existing relationship. Used to determine
// the delete behaviour of a User.
pub fn user_is_safe_to_delete(user_id: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $user_id})-[r]-()
        RETURN COUNT(r) = 0 AS boolean
        ",
    )
    .param("user_id", user_id)
}

// Post has any existing relationship. Used to determine
// the delete behaviour of a Post.
pub fn post_is_safe_to_delete(author_id: &str, post_id: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
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
        RETURN COUNT(r) = 0 AS boolean
        ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn recommend_users(user_id: &str, limit: usize) -> neo4rs::Query {
    query(
        "
        MATCH (user:User {id: $user_id})
        MATCH (user)-[:FOLLOWS*1..3]->(potential:User)
        WHERE NOT (user)-[:FOLLOWS]->(potential)
        AND potential.id <> $user_id
        WITH DISTINCT potential
        MATCH (potential)-[:AUTHORED]->(post:Post)
        WITH potential, COUNT(post) AS post_count
        WHERE post_count >= 5
        RETURN potential.id AS recommended_user_id
        LIMIT $limit
    ",
    )
    .param("user_id", user_id.to_string())
    .param("limit", limit as i64)
}
