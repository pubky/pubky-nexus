use crate::event_processor::posts::utils::{collection_post_with_items, short_post, test_user};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::fetch_key_from_graph;
use nexus_common::db::graph::Query;
use nexus_common::models::post::PostDetails;
use pubky::Keypair;
use pubky_app_specs::post_uri_builder;

async fn graph_content_hash(author_id: &str, post_id: &str) -> Result<Option<String>> {
    let query = Query::new(
        "test_post_content_hash",
        "MATCH (:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
         RETURN p.content_hash AS content_hash",
    )
    .param("author_id", author_id)
    .param("post_id", post_id);
    Ok(
        fetch_key_from_graph::<Option<String>>(query, "content_hash")
            .await?
            .flatten(),
    )
}

/// A post is stored with the hash of its content in the graph and the index,
/// and an edit replaces the hash.
#[tokio_shared_rt::test(shared)]
async fn test_post_stores_content_hash() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user(
                "Watcher:Post:ContentHash:Author",
                "test_post_stores_content_hash",
            ),
        )
        .await?;

    let post = short_post("Watcher:Post:ContentHash:Original");
    let (post_id, post_path) = test.create_post(&author_kp, &post).await?;

    let expected = PostDetails::hash_content(&post.content);
    assert_eq!(
        graph_content_hash(&author_id, &post_id).await?.as_deref(),
        Some(expected.as_str()),
        "the graph node holds the content hash"
    );
    let cached = PostDetails::get_from_index(&author_id, &post_id)
        .await?
        .expect("the post is indexed");
    assert_eq!(cached.content_hash.as_deref(), Some(expected.as_str()));

    // Edit the content: the hash follows it.
    let edited = short_post("Watcher:Post:ContentHash:Edited");
    test.put(&author_kp, &post_path, &edited).await?;

    let expected_edited = PostDetails::hash_content(&edited.content);
    assert_ne!(expected, expected_edited);
    assert_eq!(
        graph_content_hash(&author_id, &post_id).await?.as_deref(),
        Some(expected_edited.as_str()),
        "an edit replaces the graph hash"
    );
    let cached = PostDetails::get_from_index(&author_id, &post_id)
        .await?
        .expect("the post is still indexed");
    assert_eq!(
        cached.content_hash.as_deref(),
        Some(expected_edited.as_str())
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// The links a post's content carries are stored on its node with the post, so they can
/// be rebuilt from the node alone: the mentioned users, and a collection's items in
/// curator order. Any other post stores empty lists.
#[tokio_shared_rt::test(shared)]
async fn test_post_stores_link_lists() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let alice_kp = Keypair::random();
    let alice_id = test
        .create_user(
            &alice_kp,
            &test_user(
                "Watcher:Post:LinkLists:Alice",
                "test_post_stores_link_lists",
            ),
        )
        .await?;
    let bob_kp = Keypair::random();
    let bob_id = test
        .create_user(
            &bob_kp,
            &test_user("Watcher:Post:LinkLists:Bob", "test_post_stores_link_lists"),
        )
        .await?;

    let mention = short_post(format!("Hi pubky{bob_id} and pk:{bob_id}"));
    let (mention_id, mention_path) = test.create_post(&alice_kp, &mention).await?;
    let items = [
        post_uri_builder(bob_id.clone(), "0034ZZZZZZZZ0".into()),
        post_uri_builder(alice_id.clone(), mention_id.clone()),
    ];
    let collection = collection_post_with_items("Watcher:Post:LinkLists", &items);
    let (collection_id, collection_path) = test.create_post(&alice_kp, &collection).await?;

    let lists = |post_id: String| {
        let alice_id = alice_id.clone();
        async move {
            PostDetails::get_link_lists_from_graph(&alice_id, &post_id)
                .await
                .map(|lists| lists.expect("the post is in the graph"))
        }
    };
    assert_eq!(
        lists(mention_id.clone()).await?,
        (Some(vec![bob_id.clone()]), Some(vec![])),
        "a short post stores its mentions, once each"
    );
    assert_eq!(
        lists(collection_id.clone()).await?,
        (Some(vec![]), Some(items.to_vec())),
        "a collection stores its items in curator order, indexed or not"
    );

    test.cleanup_post(&alice_kp, &collection_path).await?;
    test.cleanup_post(&alice_kp, &mention_path).await?;
    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}
