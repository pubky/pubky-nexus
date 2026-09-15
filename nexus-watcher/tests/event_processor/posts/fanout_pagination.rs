use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::db::graph::keyset_scan_composite;
use nexus_common::db::{fetch_all_rows_from_graph, queries::get};
use nexus_common::models::error::ModelError;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppBookmark, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
    PubkyAppUser,
};
use std::cell::RefCell;

/// Fan-out pages at 1000 rows, so the notification tests never cross a page boundary and
/// never read a cursor. This drives the real scan loop at batch_size 1 over every fan-out
/// query, which is the only thing that proves their `[..] AS cursor` row value is readable
/// and strictly increasing.
#[tokio_shared_rt::test(shared)]
async fn test_fanout_queries_paginate_across_pages() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let new_user = |name: &str| {
        let kp = Keypair::random();
        let user = PubkyAppUser {
            bio: None,
            image: None,
            links: None,
            name: format!("FanoutPag:{name}"),
            status: None,
        };
        (kp, user)
    };

    let (author_kp, author) = new_user("Author");
    let author_id = test.create_user(&author_kp, &author).await?;

    let (post_id, _) = test
        .create_post(
            &author_kp,
            &PubkyAppPost {
                content: "FanoutPag parent post".to_string(),
                kind: PubkyAppPostKind::Short,
                parent: None,
                embed: None,
                attachments: None,
                lock: None,
            },
        )
        .await?;
    let post_uri = post_uri_builder(author_id.clone(), post_id.clone());

    // Two of each interaction, so every scan needs at least one cursor hop at batch_size 1.
    for name in ["BookmarkerA", "BookmarkerB"] {
        let (kp, user) = new_user(name);
        test.create_user(&kp, &user).await?;
        let bookmark = PubkyAppBookmark {
            uri: post_uri.clone(),
            created_at: 0,
        };
        let path = bookmark.hs_path();
        test.put(&kp, &path, bookmark).await?;
    }

    for name in ["ReplierA", "ReplierB"] {
        let (kp, user) = new_user(name);
        test.create_user(&kp, &user).await?;
        test.create_post(
            &kp,
            &PubkyAppPost {
                content: format!("FanoutPag reply from {name}"),
                kind: PubkyAppPostKind::Short,
                parent: Some(post_uri.clone()),
                embed: None,
                attachments: None,
                lock: None,
            },
        )
        .await?;
    }

    for name in ["ReposterA", "ReposterB"] {
        let (kp, user) = new_user(name);
        test.create_user(&kp, &user).await?;
        test.create_post(
            &kp,
            &PubkyAppPost {
                content: format!("FanoutPag repost from {name}"),
                kind: PubkyAppPostKind::Short,
                parent: None,
                embed: Some(PubkyAppPostEmbed {
                    kind: PubkyAppPostKind::Short,
                    uri: post_uri.clone(),
                }),
                attachments: None,
                lock: None,
            },
        )
        .await?;
    }

    // Same shape as the notification fan-out, but batch_size 1 forces a cursor hop per row.
    for (label, query_fn) in [
        (
            "get_post_bookmarks",
            get::get_post_bookmarks as fn(&str, &str, &[String], i64) -> _,
        ),
        ("get_post_replies", get::get_post_replies),
        ("get_post_reposts", get::get_post_reposts),
    ] {
        let seen: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());

        keyset_scan_composite(1, label, |cursor| {
            let seen = &seen;
            let author_id = author_id.clone();
            let post_id = post_id.clone();
            async move {
                let rows =
                    fetch_all_rows_from_graph(query_fn(&author_id, &post_id, &cursor, 1)).await?;
                let count = rows.len();
                let mut last_cursor = None;
                for row in &rows {
                    // A row value that will not deserialize here would strand the scan.
                    let row_cursor = row.get::<Vec<String>>("cursor")?;
                    assert!(
                        cursor.is_empty() || row_cursor > cursor,
                        "{label}: cursor went backwards: {cursor:?} -> {row_cursor:?}"
                    );
                    seen.borrow_mut().push(row_cursor.clone());
                    last_cursor = Some(row_cursor);
                }
                Ok::<(usize, Option<Vec<String>>), ModelError>((count, last_cursor))
            }
        })
        .await?;

        let seen = seen.into_inner();
        assert_eq!(seen.len(), 2, "{label}: expected 2 rows, got {seen:?}");
        assert!(seen[0] < seen[1], "{label}: rows not ordered: {seen:?}");
    }

    Ok(())
}
