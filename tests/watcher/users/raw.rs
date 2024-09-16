use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use pubky_common::crypto::Keypair;
use pubky_nexus::{
    get_neo4j_graph,
    models::{
        pubky_app::{PubkyAppUser, UserLink},
        user::{UserCounts, UserDetails, UserSearch, UserView, USER_NAME_KEY_PARTS},
    },
    queries::read::get_users_details_by_id,
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_user_event() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_user_event".to_string()),
        image: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAAXNSR0IArs4c6QAAAA1JREFUGFdjiO4O+w8ABL0CPPcYQa4AAAAASUVORK5CYII=".to_string()),
        links: Some(vec![UserLink {
            title: "User Event".to_string(),
            url: "pubky://watcher.nexus".to_string(),
        }]),
        name: "Watcher:UserEvent:User".to_string(),
        status: Some("Running Nexus Watcher".to_string()),
    };

    let user_id = test.create_user(&keypair, &user).await?;

    // GRAPH_OP: Assert if the event writes the graph
    // Cannot use UserDetails::from_graph because it indexes also, Sorted:Users:Name and that
    // operation has to be executed in the ingest_user
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = get_users_details_by_id(&user_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await?;
    }

    let result = row_stream.next().await.unwrap();

    // Assert the user details
    assert_eq!(result.is_some(), true);

    let user_details: UserDetails = result.unwrap().get("details").unwrap();

    assert_eq!(user_details.name, user.name);
    assert_eq!(user_details.bio, user.bio);
    assert_eq!(user_details.status, user.status);

    let result_links = user_details.links.unwrap_or_default();
    let expected_links = user.links.unwrap_or_default();
    for (result_link, expected_link) in result_links.iter().zip(expected_links.iter()) {
        assert_eq!(
            result_link.title, expected_link.title,
            "Link titles do not match."
        );
        assert_eq!(
            result_link.url, expected_link.url,
            "Link URLs do not match."
        );
    }

    // CACHE_OP: Check if the event write in the graph
    // User:Counts:user_id
    let user_counts = UserCounts::try_from_index_json(&[&user_id])
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(user_counts.followers, 0);
    assert_eq!(user_counts.tags, 0);
    assert_eq!(user_counts.posts, 0);

    // Sorted:Users:Name
    let is_member = UserSearch::check_sorted_set_member(
        &USER_NAME_KEY_PARTS,
        &[&user.name.to_lowercase(), &user_id],
    )
    .await
    .unwrap();

    assert_eq!(is_member.is_some(), true);
    assert_eq!(is_member.unwrap(), 0);

    // Cleanup
    test.cleanup_user(&user_id).await?;

    // Assert the new user does not exist in Nexus
    let result = UserView::get_by_id(&user_id, None).await.unwrap();
    assert!(result.is_none(), "The user should have been deleted");

    Ok(())
}
