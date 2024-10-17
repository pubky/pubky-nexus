use neo4rs::{query, Query};

use crate::models::post::{PostStreamSorting, ViewerStreamSource};

// Retrieve post node by post id and author id
pub fn get_post_by_id(author_id: &str, post_id: &str) -> Query {
    query(
        "
            MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
            OPTIONAL MATCH (p)-[replied:REPLIED]->(Post)
            WITH u, p, (replied IS NOT NULL) AS is_reply
            RETURN {
                uri: 'pubky://' + u.id + '/pub/pubky.app/posts/' + p.id,
                content: p.content,
                id: p.id,
                indexed_at: p.indexed_at,
                author: u.id,
                // default value when the specified property is null
                // Avoids enum deserialization ERROR
                kind: COALESCE(p.kind, 'Short'),
                attachments: p.attachments
            } as details,
            is_reply
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
        WITH u, rels_u_user, rels_user_u, collect(friend) AS friends

        // Collect relationships to Posts
        OPTIONAL MATCH (u)-[rel_u_post:BOOKMARKED|AUTHORED|TAGGED]->(p:Post)
        WITH u, rels_u_user, rels_user_u, friends, collect(rel_u_post) AS rels_u_post

        // Calculate counts
        WITH u,
            size([rel IN rels_u_user WHERE type(rel) = 'FOLLOWS']) AS following,
            size([rel IN rels_user_u WHERE type(rel) = 'FOLLOWS']) AS followers,
            size(friends) AS friends_count,
            size([rel IN rels_u_post WHERE type(rel) = 'AUTHORED']) AS posts,
            size([rel IN rels_u_post WHERE type(rel) = 'TAGGED']) AS post_tags,
            size([rel IN rels_u_user WHERE type(rel) = 'TAGGED']) AS user_tags,
            size([rel IN rels_u_post WHERE type(rel) = 'BOOKMARKED']) AS bookmarks,
            size([rel IN rels_user_u WHERE type(rel) = 'TAGGED']) AS tagged
        RETURN 
            u IS NOT NULL AS exists,
            {
                following: following,
                followers: followers,
                friends: friends_count,
                posts: posts,
                tags: user_tags + post_tags,
                bookmarks: bookmarks,
                tagged: tagged
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

pub fn get_thread(
    author_id: &str,
    post_id: &str,
    depth: usize,
    skip: usize,
    limit: usize,
) -> Query {
    let query_string = format!(
        "
        MATCH (u:User {{id: $author_id}})-[:AUTHORED]->(p:Post {{id: $post_id}})
        CALL {{
            WITH p
            // Recursively get all replies and their authors
            MATCH (reply_author:User)-[:AUTHORED]->(reply:Post)-[:REPLIED*1..{}]->(p)
            RETURN reply, reply_author
            ORDER BY reply.indexed_at ASC
            SKIP $skip 
            LIMIT $limit
        }}
        RETURN collect({{reply_id: reply.id, author_id: reply_author.id}}) AS replies
        ",
        depth
    );
    query(&query_string)
        .param("author_id", author_id)
        .param("post_id", post_id)
        .param("skip", skip as i64)
        .param("limit", limit as i64)
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
    viewer_id: Option<String>,
    author_id: Option<String>,
    source: ViewerStreamSource,
    tags: Option<Vec<String>>,
    sorting: PostStreamSorting,
    skip: Option<usize>,
    limit: Option<usize>,
) -> Query {
    let mut cypher = String::new();

    // Start with the viewer node if needed
    if viewer_id.is_some() {
        cypher.push_str("MATCH (viewer:User {id: $viewer_id})\n");
    }

    // Base match for posts and authors
    cypher.push_str("MATCH (p:Post)<-[:AUTHORED]-(author:User)\n");

    // Apply author filter if provided
    if author_id.is_some() {
        cypher.push_str("WHERE author.id = $author_id\n");
    }

    // Apply source
    if viewer_id.is_some() {
        match source {
            ViewerStreamSource::Following => {
                cypher.push_str("MATCH (viewer)-[:FOLLOWS]->(author)\n");
            }
            ViewerStreamSource::Followers => {
                cypher.push_str("MATCH (viewer)<-[:FOLLOWS]-(author)\n");
            }
            ViewerStreamSource::Friends => {
                cypher.push_str("MATCH (viewer)-[:FOLLOWS]->(author)-[:FOLLOWS]->(viewer)\n");
            }
            ViewerStreamSource::Bookmarks => {
                cypher.push_str("MATCH (viewer)-[:BOOKMARKED]->(p)\n");
            }
            ViewerStreamSource::All => {
                // No additional match needed
            }
            ViewerStreamSource::Replies => {
                // No additional match needed
            }
        }
    }

    // Apply tags
    if tags.is_some() {
        cypher.push_str("MATCH (User)-[tag:TAGGED]->(p)\n");
        cypher.push_str("WHERE tag.label IN $labels\n");
    }

    // Apply Sorting
    // Conditionally compute engagement counts only for TotalEngagement sorting
    let order_clause = match sorting {
        PostStreamSorting::Timeline => "ORDER BY p.indexed_at DESC".to_string(),
        PostStreamSorting::TotalEngagement => {
            // TODO: These optional matches could potentially be combined/collected to improve perf
            cypher.push_str(
                "
                // Count tags
                OPTIONAL MATCH (p)<-[tag:TAGGED]-(:User)  
                WITH p, author, COUNT(DISTINCT tag) AS tags_count

                // Count replies
                OPTIONAL MATCH (p)<-[reply:REPLIED]-(:Post)  // Count replies
                WITH p, author, tags_count, COUNT(DISTINCT reply) AS replies_count

                // Count reposts
                OPTIONAL MATCH (p)<-[repost:REPOSTED]-(:Post)  
                WITH p, author, tags_count, replies_count, COUNT(DISTINCT repost) AS reposts_count
                ",
            );
            "ORDER BY (tags_count + replies_count + reposts_count) DESC".to_string()
        }
    };

    // Final return statement
    cypher.push_str(&format!(
        "RETURN author.id AS author_id, p.id AS post_id\n{}\n",
        order_clause
    ));

    // Apply skip and limit
    if let Some(skip) = skip {
        cypher.push_str(&format!("SKIP {}\n", skip));
    }
    if let Some(limit) = limit {
        cypher.push_str(&format!("LIMIT {}\n", limit));
    }

    // Build the query and apply parameters using `param` method
    let mut query = query(&cypher);

    // Insert parameters
    if let Some(viewer_id) = viewer_id {
        query = query.param("viewer_id", viewer_id);
    }
    if let Some(labels) = tags {
        query = query.param("labels", labels);
    }
    if let Some(author_id) = author_id {
        query = query.param("author_id", author_id);
    }

    query
}
