use neo4rs::{query, Query};

// Retrieve post node by post id and author id
pub fn get_post_by_id(author_id: &str, post_id: &str) -> Query {
    query("MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id}) RETURN p")
        .param("author_id", author_id)
        .param("post_id", post_id)
}

pub fn post_counts(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         OPTIONAL MATCH (p)<-[tag:TAGGED]-()
         OPTIONAL MATCH (p)<-[reply:REPLIED]-()
         OPTIONAL MATCH (p)<-[repost:REPOSTED]-()
         RETURN COUNT(p) > 0 AS post_exists,
                COUNT(DISTINCT tag) AS tags_count,
                COUNT(DISTINCT reply) AS replies_count,
                COUNT(DISTINCT repost) AS reposts_count",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

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
        OPTIONAL MATCH (user:User {id: id})
        RETURN 
            id,
            CASE 
                WHEN user IS NOT NULL 
                    THEN user { .id, .bio, .status, .name, .indexed_at, .links }
                    ELSE null
                END AS record
        ",
    )
    .param("ids", user_ids)
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
            }) AS post_tags
        }
        RETURN 
            u IS NOT NULL AS post_exists,
            post_tags
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
                taggers: tagger_ids
            }) AS user_tags
        }
        RETURN 
            u IS NOT NULL AS user_exists,
            user_tags
    ",
    )
    .param("user_id", user_id)
}

pub fn user_counts(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})
           OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
           OPTIONAL MATCH (follower:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:AUTHORED]->(post:Post)
           OPTIONAL MATCH (u)-[tag:TAGGED]->(:Post)
           RETURN COUNT(u) > 0 AS user_exists,
                  COUNT(DISTINCT following) AS following_count,
                  COUNT(DISTINCT follower) AS followers_count,
                  COUNT(DISTINCT friend) AS friends_count,
                  COUNT(DISTINCT post) AS posts_count,
                  COUNT(DISTINCT tag) AS tags_count",
    )
    .param("id", user_id)
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

// Retrieves popular tags and its taggers across the entire network
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

pub fn get_thread(author_id: &str, post_id: &str, skip: usize, limit: usize) -> Query {
    query(
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        CALL {
            WITH p
            // Recursively get all replies and their authors
            MATCH (reply_author:User)-[:AUTHORED]->(reply:Post)-[:REPLIED*]->(p)
            RETURN reply, reply_author
            ORDER BY reply.indexed_at ASC
            SKIP $skip LIMIT $limit
        }
        RETURN p AS root_post, collect({reply_id: reply.id, author_id: reply_author.id}) AS replies
        ",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
    .param("skip", skip as i64)
    .param("limit", limit as i64)
}
