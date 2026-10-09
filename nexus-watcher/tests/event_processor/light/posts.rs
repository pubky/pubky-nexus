use crate::event_processor::mentions::utils::find_post_mentions;
use crate::event_processor::posts::utils::{
    assert_notification_count, collection_post_with_items, find_collections_of, pubky_id,
    short_post, test_user,
};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::db::{exec_single_row, fetch_key_from_graph, graph::Query, RedisOps};
use nexus_common::models::post::PostDetails;
use nexus_common::utils::test_utils::default_ingestor_tests;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppBookmark, PubkyAppPostKind};

async fn graph_post(author_id: &str, post_id: &str) -> Result<(String, Option<String>)> {
    let query = Query::new(
        "test_light_graph_post",
        "MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         RETURN [p.content, p.content_hash] AS post",
    )
    .param("author_id", author_id)
    .param("post_id", post_id);
    let post: Vec<Option<String>> = fetch_key_from_graph(query, "post")
        .await?
        .expect("the post is in the graph");
    Ok((post[0].clone().unwrap_or_default(), post[1].clone()))
}

async fn delete_mention_edge(author_id: &str, post_id: &str, mentioned_id: &str) -> Result<()> {
    exec_single_row(
        Query::new(
            "test_light_delete_mention",
            "MATCH (:User {id: $author_id})-[:AUTHORED]->(:Post {id: $post_id})-[m:MENTIONED]->(:User {id: $mentioned_id})
             DELETE m",
        )
        .param("author_id", author_id)
        .param("post_id", post_id)
        .param("mentioned_id", mentioned_id),
    )
    .await?;
    Ok(())
}

/// A light Nexus stores a post's links and hash but not its content. Mentions are still
/// worked out from the content while the event is processed.
#[tokio_shared_rt::test(shared)]
async fn test_light_post_keeps_links_and_hash_but_no_content() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(&alice_kp, &test_user("Watcher:Light:Posts:Alice", "bio"))
        .await?;
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(&bob_kp, &test_user("Watcher:Light:Posts:Bob", "bio"))
        .await?;

    let post = short_post(format!("Hello pubky{bob_id}, light mode here"));
    let (post_id, post_path) = test.create_post(&alice_kp, &post).await?;

    let (content, hash) = graph_post(&alice_id, &post_id).await?;
    assert_eq!(content, "", "the graph holds no content");
    assert_eq!(hash, Some(PostDetails::hash_content(&post.content)));

    let cached = PostDetails::get_from_index(&alice_id, &post_id)
        .await?
        .expect("the post is cached");
    let json = serde_json::to_value(&cached)?;
    assert!(json.get("content").is_none(), "no content: {json}");
    assert_eq!(
        json["content_hash"],
        PostDetails::hash_content(&post.content)
    );

    assert!(
        find_post_mentions(&alice_id, &post_id)
            .await?
            .contains(&bob_id),
        "the mention is indexed from the content"
    );
    assert_notification_count(&bob_id, 1, "the mention notifies").await;
    let (mentioned, _) = PostDetails::get_link_lists_from_graph(&alice_id, &post_id)
        .await?
        .expect("the post is in the graph");
    assert_eq!(
        mentioned,
        Some(vec![bob_id.clone()]),
        "the mentions are kept on the node, though the content is not"
    );

    test.cleanup_post(&alice_kp, &post_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// With no stored content to compare, an edit is detected by its hash: interactors are
/// notified, and the hash moves.
#[tokio_shared_rt::test(shared)]
async fn test_light_post_edit_is_detected_by_hash() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(&alice_kp, &test_user("Watcher:Light:Edit:Alice", "bio"))
        .await?;
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(&bob_kp, &test_user("Watcher:Light:Edit:Bob", "bio"))
        .await?;

    let mut post = short_post("Watcher:Light:Edit:Original");
    let (post_id, post_path) = test.create_post(&alice_kp, &post).await?;

    // Bob bookmarks the post, so an edit notifies him.
    let bookmark = PubkyAppBookmark {
        uri: post_uri_builder(alice_id.clone(), post_id.clone()),
        created_at: 0,
    };
    test.put(&bob_kp, &bookmark.hs_path(), bookmark).await?;
    assert_notification_count(&bob_id, 0, "a bookmark does not notify its maker").await;

    post.content = "Watcher:Light:Edit:Edited".to_string();
    test.put(&alice_kp, &post_path, &post).await?;

    assert_notification_count(&bob_id, 1, "the edit notifies the bookmarker").await;
    let (_, hash) = graph_post(&alice_id, &post_id).await?;
    assert_eq!(hash, Some(PostDetails::hash_content(&post.content)));

    // Re-putting the same content is not an edit.
    test.put(&alice_kp, &post_path, &post).await?;
    assert_notification_count(&bob_id, 1, "an unchanged re-put does not notify").await;

    test.cleanup_post(&alice_kp, &post_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// Recovery after a partial write re-merges MENTIONED edges from the incoming post,
/// since a light graph holds no content to read them back from.
#[tokio_shared_rt::test(shared)]
async fn test_light_post_recovery_restores_mentions() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(&alice_kp, &test_user("Watcher:Light:Recover:Alice", "bio"))
        .await?;
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(&bob_kp, &test_user("Watcher:Light:Recover:Bob", "bio"))
        .await?;

    let post = short_post(format!("Hi pubky{bob_id}"));
    let (post_id, post_path) = test.create_post(&alice_kp, &post).await?;

    // A crash after the graph write: the mention edge and the cache are gone.
    delete_mention_edge(&alice_id, &post_id, &bob_id).await?;
    PostDetails::remove_from_index_multiple_json(&[&[alice_id.as_str(), post_id.as_str()]]).await?;

    handlers::post::sync_put(
        post.clone(),
        post_uri_builder(alice_id.clone(), post_id.clone()),
        pubky_id(&alice_id)?,
        post_id.clone(),
        &default_ingestor_tests(),
    )
    .await?;

    assert!(
        find_post_mentions(&alice_id, &post_id)
            .await?
            .contains(&bob_id),
        "recovery restores the mention"
    );
    assert_notification_count(&bob_id, 1, "recovery does not notify again").await;

    test.cleanup_post(&alice_kp, &post_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}

/// Collections are out of scope in light mode: a collection post is indexed with its
/// kind, but no COLLECTED edges are written. Its items are kept on the node.
#[tokio_shared_rt::test(shared)]
async fn test_light_collection_writes_no_edges() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user("Watcher:Light:Collection:Alice", "bio"),
        )
        .await?;
    let item = short_post("Watcher:Light:Collection:Item");
    let (item_id, item_path) = test.create_post(&alice_kp, &item).await?;

    let collection = collection_post_with_items(
        "Watcher:Light:Collection",
        &[post_uri_builder(alice_id.clone(), item_id.clone())],
    );
    let (collection_id, collection_path) = test.create_post(&alice_kp, &collection).await?;

    let cached = PostDetails::get_from_index(&alice_id, &collection_id)
        .await?
        .expect("the collection is indexed");
    assert_eq!(cached.kind, PubkyAppPostKind::Collection);
    assert!(
        find_collections_of(&alice_id, &item_id).await.is_empty(),
        "no COLLECTED edge in light mode"
    );
    let (_, items) = PostDetails::get_link_lists_from_graph(&alice_id, &collection_id)
        .await?
        .expect("the collection is in the graph");
    assert_eq!(
        items,
        Some(vec![post_uri_builder(alice_id.clone(), item_id.clone())]),
        "the items are kept on the node, so light collections need no re-index"
    );

    test.cleanup_post(&alice_kp, &collection_path).await?;
    test.cleanup_post(&alice_kp, &item_path).await?;
    test.cleanup_user(&alice_kp).await?;
    Ok(())
}
