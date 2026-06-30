use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::{fetch_all_rows_from_graph, get_neo4j_graph, queries::get::get_post_tags};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};

/// Two taggers, same label → identical t.id, distinct elementId(t).
/// A regression to t.id keyset drops the second edge; this test catches it.
#[tokio_shared_rt::test(shared)]
async fn test_tagged_pagination_no_dup_or_drop() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &PubkyAppUser {
                bio: None,
                image: None,
                links: None,
                name: "TagPag:Author".to_string(),
                status: None,
            },
        )
        .await?;

    let (post_id, _) = test
        .create_post(
            &author_kp,
            &PubkyAppPost {
                content: "TagPag test post".to_string(),
                kind: PubkyAppPost::default().kind,
                parent: None,
                embed: None,
                attachments: None,
            },
        )
        .await?;

    let tagger_b_kp = Keypair::random();
    let tagger_b_id = test
        .create_user(
            &tagger_b_kp,
            &PubkyAppUser {
                bio: None,
                image: None,
                links: None,
                name: "TagPag:TaggerB".to_string(),
                status: None,
            },
        )
        .await?;

    let tagger_c_kp = Keypair::random();
    let tagger_c_id = test
        .create_user(
            &tagger_c_kp,
            &PubkyAppUser {
                bio: None,
                image: None,
                links: None,
                name: "TagPag:TaggerC".to_string(),
                status: None,
            },
        )
        .await?;

    let post_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}");

    let tag_b_shared_id = PubkyAppTag {
        uri: post_uri.clone(),
        label: "TagPag:shared".to_string(),
        created_at: 1_700_000_000_001,
    }
    .create_id();
    let tag_c_shared_id = PubkyAppTag {
        uri: post_uri.clone(),
        label: "TagPag:shared".to_string(),
        created_at: 1_700_000_000_002,
    }
    .create_id();
    let tag_b_unique_id = PubkyAppTag {
        uri: post_uri.clone(),
        label: "TagPag:unique_to_b".to_string(),
        created_at: 1_700_000_000_003,
    }
    .create_id();

    // Confirm the t.id collision: same label on same post → same hash regardless of tagger.
    assert_eq!(
        tag_b_shared_id, tag_c_shared_id,
        "same-label tags from different taggers must share t.id (hash of uri+label)"
    );

    let graph = get_neo4j_graph()?;
    let cypher = "MATCH (p:Post {id: $post_id})<-[:AUTHORED]-(a:User {id: $author_id}),
               (tb:User {id: $tagger_b_id}), (tc:User {id: $tagger_c_id})
         CREATE (tb)-[:TAGGED {id: $tag_b_shared_id, label: 'TagPag:shared'}]->(p),
                (tc)-[:TAGGED {id: $tag_c_shared_id, label: 'TagPag:shared'}]->(p),
                (tb)-[:TAGGED {id: $tag_b_unique_id, label: 'TagPag:unique_to_b'}]->(p)";
    let query = nexus_common::db::graph::Query::new("test_insert_tags", cypher)
        .param("author_id", author_id.as_str())
        .param("post_id", post_id.as_str())
        .param("tagger_b_id", tagger_b_id.as_str())
        .param("tagger_c_id", tagger_c_id.as_str())
        .param("tag_b_shared_id", tag_b_shared_id.as_str())
        .param("tag_c_shared_id", tag_c_shared_id.as_str())
        .param("tag_b_unique_id", tag_b_unique_id.as_str());
    graph.run(query).await?;

    // Page with limit=1 so the cursor must advance between every edge.
    // Track by elementId (cursor), not tag_id — two edges legitimately share a tag_id.
    let mut cursor = String::new();
    let mut seen_cursors: std::collections::HashSet<String> = std::collections::HashSet::new();

    loop {
        let query = get_post_tags(&author_id, &post_id, &cursor, 1);
        let rows = fetch_all_rows_from_graph(query).await?;
        if rows.is_empty() {
            break;
        }
        for row in &rows {
            let edge_cursor = row.get::<String>("cursor")?;
            assert!(
                seen_cursors.insert(edge_cursor.clone()),
                "duplicate elementId(t) cursor: {}",
                edge_cursor
            );
            cursor = edge_cursor;
        }
    }

    // t.id keyset regression → second shared-label edge is dropped → count == 2.
    assert_eq!(
        seen_cursors.len(),
        3,
        "expected 3 edges, got {}",
        seen_cursors.len()
    );

    Ok(())
}
