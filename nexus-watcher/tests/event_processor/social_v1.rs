use super::posts::utils::test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use nexus_common::db::{fetch_row_from_graph, graph::Query, RedisOps};
use nexus_common::models::post::PostDetails;
use nexus_common::models::user::UserDetails;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::{
    post_uri_builder,
    traits::{HasIdPath, HashId},
    PubkySocialFollow, PubkySocialTag,
};
use serde_json::json;

#[tokio_shared_rt::test(shared)]
async fn moderated_v1_resource_tag_stays_deleted_on_replay() -> Result<()> {
    use nexus_common::models::takedown::record_takedown;
    use nexus_common::universal_tag::normalize::resource_id;
    use nexus_common::utils::test_utils::default_ingestor_tests;
    use nexus_watcher::events::remove_if_taken_down;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test
        .create_user(&key, &test_user("Moderated v1 tag", ""))
        .await?;
    let target = format!("https://example.com/moderated-v1/{author}");
    let resource = resource_id(&target);
    let tag = PubkySocialTag::new(target, "v1-moderated".into());
    let path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
    let uri = format!("pubky://{author}{path}");
    test.put(&key, &path, &tag).await?;
    async fn counts(id: &str) -> Result<(i64, i64)> {
        let row = fetch_row_from_graph(
            Query::new(
                "moderated_v1_resource",
                r"
            OPTIONAL MATCH (r:Resource {id:$id})
            OPTIONAL MATCH ()-[t:TAGGED]->(r)
            RETURN count(DISTINCT r) AS resources, count(t) AS tags
        ",
            )
            .param("id", id),
        )
        .await?
        .unwrap();
        Ok((row.get("resources")?, row.get("tags")?))
    }
    assert_eq!(counts(&resource).await?, (1, 1));
    record_takedown(&uri, "pubky://uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko/pub/pubky.app/tags/MODERATORTAG").await?;
    remove_if_taken_down(
        &uri,
        std::path::Path::new("/tmp/nexus-watcher-test"),
        &default_ingestor_tests(),
    )
    .await?;
    assert_eq!(counts(&resource).await?, (0, 0));
    retrieve_and_handle_event_line(
        &format!("PUT {uri}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert_eq!(counts(&resource).await?, (0, 0));
    test.cleanup_user(&key).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn indexes_v1_versions_follows_collections_and_resource_tags() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    let author = test.create_user(&key, &test_user("Social v1", "")).await?;
    let other_key = Keypair::random();
    let other = test
        .create_user(&other_key, &test_user("Social v1 followee", ""))
        .await?;
    let profile: ResourcePath = "/pub/social/v1/profile.json".parse()?;
    test.put(&key, &profile, &json!({"name":"Social v1 profile"}))
        .await?;
    assert_eq!(
        UserDetails::get_by_id(&author).await?.unwrap().name,
        "Social v1 profile"
    );
    let id = generate_post_id();
    let edit = generate_post_id();
    let first: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let second: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    let uri = post_uri_builder(author.clone(), id.clone());
    test.put(
        &key,
        &first,
        &json!({"kind":"note", "content":"first", "attachments":[]}),
    )
    .await?;
    assert_eq!(
        PostDetails::get_by_id(&author, &id).await?.unwrap().uri,
        uri
    );
    test.put(
        &key,
        &second,
        &json!({"kind":"note", "content":"edited", "attachments":[]}),
    )
    .await?;
    assert_eq!(
        PostDetails::get_by_id(&author, &id).await?.unwrap().content,
        "edited"
    );

    let follow: ResourcePath = PubkySocialFollow::create_path(&other).parse()?;
    test.put(&key, &follow, &PubkySocialFollow::new()).await?;
    let row = fetch_row_from_graph(
        Query::new(
            "test_v1_follow",
            "MATCH (:User {id: $author})-[f:FOLLOWS]->(:User {id: $other}) RETURN count(f) AS n",
        )
        .param("author", author.clone())
        .param("other", other.clone()),
    )
    .await?
    .unwrap();
    assert_eq!(row.get::<i64>("n")?, 1);

    let collection_id = generate_post_id();
    let collection: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{collection_id}.json").parse()?;
    let content =
        json!({"name":"v1 collection", "items":[{"uri":uri}], "layout":"grid"}).to_string();
    test.put(
        &key,
        &collection,
        &json!({"kind":"collection", "content":content, "attachments":[]}),
    )
    .await?;
    let row = fetch_row_from_graph(Query::new("test_v1_collection", "MATCH (c:Post {id: $collection})-[:COLLECTED]->(p:Post {id: $post}) RETURN count(p) AS n").param("collection", collection_id.clone()).param("post", id.clone())).await?.unwrap();
    assert_eq!(row.get::<i64>("n")?, 1);

    // Replay after the graph commit but before the Redis details write must keep
    // v1 collection items, whose envelopes differ from legacy collections.
    PostDetails::remove_from_index_multiple_json(&[&[author.as_str(), collection_id.as_str()]])
        .await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{collection}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    let row = fetch_row_from_graph(Query::new("test_v1_collection_recovery", "MATCH (c:Post {id: $collection})-[:COLLECTED]->(p:Post {id: $post}) RETURN count(p) AS n").param("collection", collection_id.clone()).param("post", id.clone())).await?.unwrap();
    assert_eq!(row.get::<i64>("n")?, 1);

    let mut tag_paths = vec![];
    for target in [
        format!("https://example.com/v1/{author}"),
        format!("pubky://{author}/pub/music/v1/posts/{id}"),
    ] {
        let tag = PubkySocialTag::new(target, "v1-test".into());
        let path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
        test.put(&key, &path, &tag).await?;
        tag_paths.push(path);
    }
    let row = fetch_row_from_graph(Query::new("test_v1_resource_tags", "MATCH (:User {id: $author})-[:TAGGED {app: 'social'}]->(r:Resource) RETURN count(r) AS n").param("author", author.clone())).await?;
    // Resource-target tags are routed through the existing resource writer.
    assert_eq!(row.unwrap().get::<i64>("n")?, 2);
    for path in tag_paths {
        test.del(&key, &path).await?;
    }
    let row = fetch_row_from_graph(Query::new("test_v1_resource_tags_deleted", "MATCH (:User {id: $author})-[t:TAGGED {app: 'social'}]->(:Resource) RETURN count(t) AS n").param("author", author.clone())).await?.unwrap();
    assert_eq!(row.get::<i64>("n")?, 0);
    test.del(&key, &collection).await?;
    test.del(&key, &follow).await?;
    // N2 contract: deleting any version deletes the logical post; later DELs no-op.
    test.del(&key, &first).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.is_none());
    test.del(&key, &second).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{second}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    test.cleanup_user(&other_key).await?;
    test.cleanup_user(&key).await?;
    Ok(())
}
