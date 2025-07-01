use crate::models::post::StreamSource;
use crate::types::routes::HotTagsInputDTO;
use crate::types::Pagination;
use crate::types::StreamReach;
use crate::types::StreamSorting;
use crate::types::Timeframe;
use neo4rs::{query, Query};
use pubky_app_specs::PubkyAppPostKind;

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
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})  
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

/// Retrieve tags for a user within the viewer's trusted network
/// # Arguments
///
/// - `user_id` - A string slice representing the ID of the user whose tags are being queried.
/// - `viewer_id` - A string slice representing the ID of the viewer whose trusted network is used as a filter.
/// - `depth` - A `u8` value specifying the depth of the viewer's trusted network (e.g., 1 for direct connections,
///   2 for connections of connections, and so on).
///
/// # Cypher Query Behavior
///
/// - **Nodes and Relationships**:
///   - Finds the `viewer` node with the given `viewer_id`.
///   - Finds the `tagged` user node with the given `user_id`.
///   - Traverses the `FOLLOWS` relationships up to the specified `depth` from the viewer to find trusted `tagger` users.
///   - Matches `TAGGED` relationships between taggers and the tagged user.
/// - **Return Values**:
///   - `exists`: A boolean indicating whether any taggers were found.
///   - `tags`: A collection of objects, each containing:
///       - `label`: The tag label.
///       - `taggers`: A list of tagger user IDs who applied the tag.
///       - `taggers_count`: The number of taggers who applied the tag.
pub fn get_viewer_trusted_network_tags(user_id: &str, viewer_id: &str, depth: u8) -> neo4rs::Query {
    let graph_query = format!(
        "
        MATCH (viewer:User {{id: $viewer_id}})
        MATCH (tagged:User {{id: $user_id}})
        CALL {{
            WITH viewer
            MATCH (viewer)-[:FOLLOWS*1..{depth}]->(tagger:User)
            RETURN DISTINCT tagger
        }}
        MATCH (tagger)-[tag:TAGGED]->(tagged)
        WITH tag.label AS label, collect(tagger.id) AS taggerIds
        RETURN 
            taggerIds IS NOT NULL AS exists,
            collect({{
                label: label,
                taggers: taggerIds,
                taggers_count: SIZE(taggerIds)
        }}) AS tags
        "
    );

    // Add to the query the params
    query(graph_query.as_str())
        .param("user_id", user_id)
        .param("viewer_id", viewer_id)
}

pub fn user_counts(user_id: &str) -> neo4rs::Query {
    query(
        "
        MATCH (u:User {id: $user_id})        
        // tags that reference this user
        OPTIONAL MATCH (u)<-[t:TAGGED]-(:User)
        WITH u, COUNT(DISTINCT t.label) AS unique_tags,

        // Count relationships to users
        COUNT { (u)-[:FOLLOWS]->(:User) } AS following,
        COUNT { (:User)-[:FOLLOWS]->(u) } AS followers,
        COUNT { (u)-[:FOLLOWS]->(friend:User) WHERE (friend)-[:FOLLOWS]->(u) } AS friends,

        // Count relationships to posts
        COUNT { (u)-[:AUTHORED]->(:Post) } AS posts,
        COUNT { (u)-[:AUTHORED]->(:Post)-[:REPLIED]->(:Post) } AS replies,
        COUNT { (u)-[:BOOKMARKED]->(:Post) } AS bookmarks,

        // Count user and post tagging
        COUNT { (u)-[:TAGGED]->(:User) } AS user_tags,
        COUNT { (u)-[:TAGGED]->(:Post) } AS post_tags,
        COUNT { (:User)-[:TAGGED]->(u) } AS tags

        RETURN 
            u IS NOT NULL AS exists,
            {
                following: following,
                followers: followers,
                friends: friends,
                posts: posts,
                replies: replies,
                tagged: user_tags + post_tags,
                tags: tags,
                unique_tags: unique_tags,
                bookmarks: bookmarks
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
        query_string.push_str(&format!(" SKIP {skip_value}"));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {limit_value}"));
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
        query_string.push_str(&format!(" SKIP {skip_value}"));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {limit_value}"));
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
        query_string.push_str(&format!(" SKIP {skip_value}"));
    }
    if let Some(limit_value) = limit {
        query_string.push_str(&format!(" LIMIT {limit_value}"));
    }
    query(&query_string).param("user_id", user_id)
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
    query(
        "
        MATCH ()-[t:TAGGED]->()
        WHERE t.label STARTS WITH $label_prefix
        RETURN COLLECT(DISTINCT t.label) AS tag_labels
        ",
    )
    .param("label_prefix", label_prefix)
}

pub fn get_tags() -> Query {
    query(
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
    query(
        format!(
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
        )
        .as_str(),
    )
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
    query(
        format!(
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
        )
        .as_str(),
    )
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
    query(
        format!(
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
        )
        .as_str(),
    )
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
    query(
        format!(
            "
        {}
        WHERE user.id = $user_id  
        WITH DISTINCT reach

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
        )
        .as_str(),
    )
    .param("user_id", user_id)
    .param("skip", skip as i64)
    .param("limit", limit as i64)
    .param("from", from)
    .param("to", to)
}

pub fn get_global_influencers(skip: usize, limit: usize, timeframe: &Timeframe) -> Query {
    let (from, to) = timeframe.to_timestamp_range();
    query(
        "
        MATCH (user:User)
        WITH DISTINCT user

        OPTIONAL MATCH (others:User)-[follow:FOLLOWS]->(user)
        WHERE follow.indexed_at >= $from AND follow.indexed_at < $to

        OPTIONAL MATCH (user)-[tag:TAGGED]->(tagged:Post)
        WHERE tag.indexed_at >= $from AND tag.indexed_at < $to
        
        OPTIONAL MATCH (user)-[authored:AUTHORED]->(post:Post)
        WHERE authored.indexed_at >= $from AND authored.indexed_at < $to

        WITH user, COUNT(DISTINCT follow) AS followers_count, COUNT(DISTINCT tag) AS tags_count,
             COUNT(DISTINCT post) AS posts_count
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
    kind: Option<PubkyAppPostKind>,
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

    // Filter just the parent posts: StreamSource:PostReplies and StreamSource:AuthorReplies do not reach that query
    // so we do not need any condition to filter just parent nodes
    append_condition(
        &mut cypher,
        "NOT ( (p)-[:REPLIED]->(:Post) )",
        &mut where_clause_applied,
    );

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
        "RETURN author.id AS author_id, p.id AS post_id\n{order_clause}\n"
    ));

    // Apply skip and limit
    if let Some(skip) = pagination.skip {
        cypher.push_str(&format!("SKIP {skip}\n"));
    }
    if let Some(limit) = pagination.limit {
        cypher.push_str(&format!("LIMIT {limit}\n"));
    }

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
/// * `kind` - An optional `PubkyAppPostKind` to filter the posts by their kind.
/// * `pagination` - The `Pagination` object containing pagination parameters like `start`, `end`, `skip`, and `limit`.
fn build_query_with_params(
    cypher: &str,
    source: &StreamSource,
    tags: &Option<Vec<String>>,
    kind: Option<PubkyAppPostKind>,
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

/// Determines whether a user has any relationships
/// # Arguments
/// * `user_id` - The unique identifier of the user
pub fn user_is_safe_to_delete(user_id: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $user_id})
        // Ensures all relationships to the user (u) are checked, counting as 0 if none exist
        OPTIONAL MATCH (u)-[r]-()
        // Checks if the user has any relationships
        WITH u, NOT (COUNT(r) = 0) AS flag
        RETURN flag
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
    query(
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        // Ensures all relationships to the post (p) are checked, counting as 0 if none exist
        OPTIONAL MATCH (p)-[r]-()
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
        // Checks if any disallowed relationships exist for the post
        WITH p, NOT (COUNT(r) = 0) AS flag
        RETURN flag
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
