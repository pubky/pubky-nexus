use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::models::post::{PostStream, PostView, POST_TOTAL_ENGAGEMENT_KEY_PARTS};
use pubky_nexus::models::pubky_app::{PubkyAppPost, PubkyAppTag, PubkyAppUser};
use pubky_nexus::models::tag::search::{TagSearch, TAG_GLOBAL_POST_ENGAGEMENT};
use pubky_nexus::models::tag::stream::{Taggers, TAG_GLOBAL_HOT};
use pubky_nexus::RedisOps;

#[tokio::test]
async fn test_homeserver_tag_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("This is a test user for tagging posts".to_string()),
        image: None,
        links: None,
        name: "Test User: PostTags".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "This is a tag test post!".to_string(),
        kind: PubkyAppPost::default().kind,
        embed: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Step 3: Add a tag to the post
    let label = "cool";
    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.client.put(tag_url.as_str(), &tag_blob).await?;
    test.ensure_event_processing_complete().await?;

    // Step 4: Verify the tag exists in Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The tag should have been created");


    println!("User_id: {:?}, Post_id: {:?}, label {:?}", user_id, post_id, label);

    // Count post tag taggers: Sorted:Post:Tag:user_id:post_id:{label}
    assert_eq!(result_post.tags[0].label, label);
    assert_eq!(result_post.tags[0].taggers_count, 1);
    // Find user as the post tagger id: Tag:Taggers:tag_name
    assert_eq!(result_post.tags[0].taggers[0], user_id);
    // Check if post counts updated: Post:Counts:user_id:post_id
    assert_eq!(result_post.counts.tags, 1);
    
    // Check the redis indexes if it is consistent
    let author_post_slice: Vec<&str> = vec![&user_id, &post_id];
    let tag_label_slice = [label];
    // Check global post engagement: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &author_post_slice).await.unwrap().unwrap();
    assert_eq!(total_engagement, 1);
    // Missing: Sorted:Tags:Global:Post:Timeline
    // Tag global engagement
    let total_engagement = TagSearch::check_sorted_set_member(&[&TAG_GLOBAL_POST_ENGAGEMENT[..], &tag_label_slice].concat(), &author_post_slice).await.unwrap().unwrap();
    assert_eq!(total_engagement, 1);
    // TODO: Hot tag. Uncomment when DEL is impl
    // let total_engagement = Taggers::check_sorted_set_member(&TAG_GLOBAL_HOT, &tag_label_slice).await.unwrap().unwrap();
    // assert_eq!(total_engagement, 1);
    // Check if the user is related with tag
    let (_exist, member) = Taggers::check_set_member(&[label], &user_id).await.unwrap();
    assert_eq!(member, true);

    // Step 5: Delete the tag
    test.client.delete(tag_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Step 6: Verify the tag has been deleted
    let _result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .unwrap();

    // TODO: uncomment tests when fixed redis indexing
    // assert_eq!(
    //     result_post.tags[0].taggers_count, 0,
    //     "The tag should have been deleted"
    // );

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;

    Ok(())

}
