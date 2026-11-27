use crate::{
    post::{CAIRO_USER, ENCRYPTION_TAG, ROOT_PATH},
    stream::post::{
        kind::{COL_BOGOTA_1, DETROIT, SHORT_BOGOTA},
        BOGOTA, POST_H, TAG_LABEL_2,
    },
    utils::{
        get_request, invalid_get_request,
        recommended::{DELETED, TOMBSTONE_POST},
        server::TestServiceServer,
    },
};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::db::{exec_single_row, get_neo4j_graph, graph::Query, queries};
use nexus_common::models::post::{PostCounts, PostDetails, PostRelationships};
use nexus_common::models::tag::TagDetails;
use nexus_common::models::ResourceKey;
use nexus_webapi::models::PostKind;

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view() -> Result<()> {
    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let body = get_request(&format!(
        "/v0/post/{author_id}/{post_id}?viewer_id={author_id}"
    ))
    .await?;

    assert_eq!(body["details"]["content"], "I am told we can reply now!");
    assert_eq!(body["details"]["indexed_at"].as_u64(), Some(1718616844478));
    assert_eq!(body["details"]["id"], post_id);
    assert_eq!(body["details"]["author"], author_id);
    assert_eq!(body["details"]["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(
        (body["details"]["attachments"].as_array().unwrap())[0],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/files/2ZKH7K7M9G3G0".to_string()
    );
    assert_eq!(
        body["details"]["uri"],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/posts/2ZCW1TGR5BKG0"
    );
    assert_eq!(body["counts"]["tags"].as_u64(), Some(5));
    assert_eq!(body["counts"]["unique_tags"].as_u64(), Some(1));
    assert_eq!(body["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(body["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(body["counts"]["collections"].as_u64(), Some(0));
    assert_eq!(body["bookmark"]["indexed_at"].as_u64(), Some(1721764200000));
    assert_eq!(body["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = body["tags"][0].clone();
    let post_tag: TagDetails = serde_json::from_value(post_tag_object.clone())?;
    assert_eq!(post_tag.label, "pubky");

    // Test non-existing post
    invalid_get_request(
        &format!("/v0/post/{}/{}", author_id, "2ZCA1TGR5BKG0"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

/// Assert that the `deleted` field appears on both the `/details` and the post
/// view responses, serialized as a plain bool: false for a live post and true
/// for a tombstone.
#[tokio_shared_rt::test(shared)]
async fn test_deleted_flag_in_details_and_view() -> Result<()> {
    let live = (
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "2ZCW1TGR5BKG0",
        false,
    );
    let tombstone = (DELETED, TOMBSTONE_POST, true);

    for (author_id, post_id, deleted) in [live, tombstone] {
        // /v0/post/{author_id}/{post_id}/details
        let details = get_request(&format!("{ROOT_PATH}/{author_id}/{post_id}/details")).await?;
        assert!(
            details["deleted"].is_boolean(),
            "details response must serialize deleted as a boolean"
        );
        assert_eq!(
            details["deleted"], deleted,
            "{post_id} must report deleted: {deleted}"
        );

        // /v0/post/{author_id}/{post_id} (PostView embeds PostDetails)
        let view = get_request(&format!("{ROOT_PATH}/{author_id}/{post_id}")).await?;
        assert!(
            view["details"]["deleted"].is_boolean(),
            "view response must serialize deleted as a boolean"
        );
        assert_eq!(
            view["details"]["deleted"], deleted,
            "{post_id} must report deleted: {deleted} in view response"
        );
    }

    Ok(())
}

/// A tombstoned reply and a tombstoned repost keep their parent edges and their cached
/// relationships, which name the parent and the reposted post, yet their views name neither, on
/// `/v0` as on `/v1`, as for any deleted post. Live, the same posts name both. The tombstones are
/// written through the models, as `del` leaves them (deleted details, kept edges and cached
/// relationships); the watcher's `a_deleted_posts_view_names_no_relationships` deletes for real.
#[tokio_shared_rt::test(shared)]
async fn v0_names_no_parent_or_repost_for_a_tombstoned_reply_or_repost() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let author = pubky::Keypair::random().public_key().to_z32();
    let owner =
        pubky_social_specs::PubkyId::try_from(author.as_str()).map_err(anyhow::Error::msg)?;
    let graph = get_neo4j_graph()?;
    graph
        .run(
            Query::new("v0_tombstone_user", "CREATE (:User {id: $author})")
                .param("author", author.clone()),
        )
        .await?;
    let at = chrono::Utc::now().timestamp_millis();
    let stamp = chrono::Utc::now().timestamp_micros() & !1;
    let id = |n: i64| format!("{:013X}", stamp + 2 * n);
    let details = |n: i64, deleted: bool| PostDetails {
        author: author.clone(),
        uri: format!("pubky://{author}/pub/pubky.app/posts/{}", id(n)),
        id: id(n),
        content: if deleted { "" } else { "v0 tombstone" }.into(),
        indexed_at: at + n,
        kind: PostKind::Short,
        attachments: vec![],
        lock: None,
        deleted,
    };
    let parent = details(0, false);
    parent.put_to_graph(None, None).await?;
    parent.put_to_index(&author, None, true, None).await?;
    let parent_key = ResourceKey {
        owner_id: owner,
        resource_id: parent.id.clone(),
    };
    // The details only, as an edit writes them: no stream of the shared index changes
    let write = |post: PostDetails, field: &'static str| {
        let parent_key = &parent_key;
        async move {
            let (replied, reposted) = match field {
                "replied" => (Some(parent_key), None),
                _ => (None, Some(parent_key)),
            };
            post.put_to_graph(replied, reposted).await?;
            post.put_to_index(&post.author, None, true, None).await?;
            anyhow::Ok(())
        }
    };
    let cases = [("reply", 1, "replied"), ("repost", 2, "reposted")];
    for (case, n, field) in cases {
        write(details(n, false), field).await?;
        PostRelationships::reindex(&author, &id(n)).await?;
        for version in ["v0", "v1"] {
            let view = get_request(&format!("/{version}/post/{author}/{}", id(n))).await?;
            assert_eq!(
                view["details"]["deleted"], false,
                "live {case} on {version}"
            );
            assert_eq!(
                view["relationships"][field],
                parent.uri.as_str(),
                "live {case} on {version}: {view}"
            );
        }
    }

    // Deleted, each is a tombstone: its edge and its cached relationships stay
    for (case, n, field) in cases {
        write(details(n, true), field).await?;
        let cached = PostRelationships::get_from_index(&author, &id(n))
            .await?
            .unwrap_or_else(|| panic!("{case}: cached relationships"));
        let kept = match field {
            "replied" => cached.replied,
            _ => cached.reposted,
        };
        assert_eq!(kept.as_deref(), Some(parent.uri.as_str()), "{case}");
        for version in ["v0", "v1"] {
            let view = get_request(&format!("/{version}/post/{author}/{}", id(n))).await?;
            assert_eq!(
                view["details"]["deleted"], true,
                "tombstoned {case} on {version}"
            );
            assert_eq!(
                view["relationships"][field],
                serde_json::Value::Null,
                "tombstoned {case} on {version}: {view}"
            );
        }
    }

    for n in 0..3 {
        PostRelationships::delete(&author, &id(n)).await?;
        PostCounts::invalidate(&[&author, &id(n)]).await?;
        PostDetails::delete_from_index(&author, &id(n), None).await?;
    }
    graph
        .run(
            Query::new(
                "v0_tombstone_cleanup",
                "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u",
            )
            .param("author", author),
        )
        .await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_counts() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}/counts");

    let body = get_request(&path).await?;
    //let post_tag: PostView = serde_json::from_value(body.clone())?;
    assert!(body.is_object());

    // Check the post counts
    assert_eq!(body["tags"], 6);
    assert_eq!(body["unique_tags"], 2);
    assert_eq!(body["replies"], 0);
    assert_eq!(body["reposts"], 0);
    assert_eq!(body["collections"], 0);

    Ok(())
}

/// Seed: COLW1TGL5BKG1 and COLW1TGL5BKG3 curate SHORT_BOGOTA, NEST1TGL5BKG8 curates COLW1TGL5BKG1.
#[tokio_shared_rt::test(shared)]
async fn test_get_post_counts_collections() -> Result<()> {
    let body = get_request(&format!("{ROOT_PATH}/{BOGOTA}/{SHORT_BOGOTA}/counts")).await?;
    assert_eq!(body["collections"], 2, "two collections curate the post");

    let body = get_request(&format!("{ROOT_PATH}/{BOGOTA}/{COL_BOGOTA_1}/counts")).await?;
    assert_eq!(body["collections"], 1, "a nested collection counts too");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1");

    let body = get_request(&path).await?;
    //let post_tag: PostView = serde_json::from_value(body.clone())?;
    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 4);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_taggers() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_taggers=2");

    let body = get_request(&path).await?;

    assert!(body.is_object());
    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let opensource_taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(opensource_taggers.len(), 2);

    assert_eq!(tags[1]["label"], ENCRYPTION_TAG);
    assert_eq!(tags[1]["taggers_count"], 2);
    let encryption_taggers = tags[1]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(encryption_taggers.len(), 2);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags_and_taggers() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1&limit_taggers=2");

    let body = get_request(&path).await?;

    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 2);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_attachment_metadata() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?include_attachment_metadata=true");

    let body = get_request(&path).await?;
    assert!(body.is_object());

    let attachments_metadata = body["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);

    // First attachment
    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[0]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[0]["name"], "cairo_file1");
    assert_eq!(attachments_metadata[0]["content_type"], "image/png");

    // Second attachment
    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");
    assert_eq!(attachments_metadata[1]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[1]["name"], "cairo_file2");
    assert_eq!(attachments_metadata[1]["content_type"], "image/jpeg");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags_and_taggers_and_attachment_metadata() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1&limit_taggers=2&include_attachment_metadata=true"
    );

    let body = get_request(&path).await?;
    assert!(body.is_object());

    // Check tags with limits
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 2);

    // Check attachments metadata
    let attachments_metadata = body["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);
    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_viewer() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?viewer_id={DETROIT}");

    let body = get_request(&path).await?;
    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    assert_eq!(
        tags[0]["relationship"], true,
        "Expected to be part of the taggers"
    );
    assert_eq!(
        tags[0]["tag_uri"], "pubky://7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o/pub/pubky.app/tags/V8N1P3L9J45R",
        "Expected the viewer's tag address"
    );
    assert_eq!(
        tags[1]["relationship"], false,
        "Expected not to be part of the taggers"
    );
    assert!(tags[1]["tag_uri"].is_null(), "Expected no tag address");

    Ok(())
}

/// A post of a migrated user names a `social/v1` file address; a v1 file has no `File` node, so
/// the frozen v0 view carries no metadata for it and the read is an ordinary 200. The post is
/// this test's own: written to the graph, read once, then taken out of the graph and the index,
/// the keys the read wrote on its miss included.
#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_v1_attachment_has_empty_metadata() -> Result<()> {
    // The shared server initialises the DB connectors the seed below writes through.
    let _ = TestServiceServer::get_test_server().await;

    // A user of the mock graph ("JBro" in `docker/test-graph/skunk.cypher`) with no followers,
    // so an extra post leaves every ranking as it is: the influencer score is
    // `(tags + posts) * sqrt(followers)`, zero for them whatever they post, and no test names
    // them. The post id is this test's own.
    let author = "dzz8cshisfst7dthpy7eio9a3byecmrym1ymn75hwqt67a9fs7zo";
    let post_id = "2ZV1ATTCHMT00";
    let attachment = format!("pubky://{author}/pub/social/v1/files/PZBQ010FF079VVZPQG1RNFN6DR.png");
    let post = PostDetails {
        content: "a post of a migrated user".to_string(),
        id: post_id.to_string(),
        // The oldest post there is, so it never reaches the first page of any stream.
        indexed_at: 0,
        author: author.to_string(),
        kind: PostKind::Short,
        uri: format!("pubky://{author}/pub/social/v1/posts/{post_id}"),
        attachments: vec![attachment.clone()],
        lock: None,
        deleted: false,
    };
    post.put_to_graph(None, None).await?;

    let outcome = async {
        let body = get_request(&format!(
            "{ROOT_PATH}/{author}/{post_id}?include_attachment_metadata=true"
        ))
        .await?;
        anyhow::ensure!(
            body["details"]["attachments"] == serde_json::json!([attachment]),
            "the address is kept as written: {}",
            body["details"]["attachments"]
        );
        let metadata = body.get("attachments_metadata");
        anyhow::ensure!(
            metadata.is_none_or(|m| m.as_array().is_some_and(Vec::is_empty)),
            "no metadata for a v1 attachment, got {metadata:?}"
        );
        Ok::<(), anyhow::Error>(())
    }
    .await;

    // The read indexed the post on its miss: its details, its counts and its relationships.
    // Take them out of the index, then the post out of the graph.
    PostDetails::delete_from_index(author, post_id, None).await?;
    PostCounts::delete(author, post_id, false).await?;
    PostRelationships::delete(author, post_id).await?;
    exec_single_row(queries::del::delete_post(author, post_id)).await?;
    outcome
}
