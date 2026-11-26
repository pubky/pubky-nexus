//! A collection PUT looks up the unknown authors of its items only when it wrote a new post or
//! changed one: an unchanged replay, a refused edit and a PUT that waits for a missing
//! dependency look up nobody and create no stub user.

use super::utils::{collection_post_with_items, resource_key, short_post, test_user};
use crate::event_processor::utils::watcher::{generate_post_id, WatcherTest};
use anyhow::Result;
use nexus_common::db::{exec_single_row, queries, PubkyClientResult};
use nexus_common::models::user::{users_exist, PkdnsHomeserverResolver, PkdnsRecord, UserIngestor};
use nexus_common::utils::test_utils::{random_pubky_id, signed_packet_naming};
use nexus_watcher::events::handlers;
use nexus_watcher::events::inputs::PostInput;
use nexus_watcher::EventProcessorError;
use pubky::{Keypair, PublicKey};
use pubky_social_specs::legacy_v0::{post_uri_builder, PubkyAppPost};
use pubky_social_specs::PubkyId;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Counts the lookups per user. A user it holds a key for publishes a record naming a
/// homeserver, so a lookup of them creates a stub user; anyone else publishes none, as an
/// author with no homeserver record, whom every lookup resolves again.
struct CountingResolver {
    published: HashMap<String, Keypair>,
    homeserver: PubkyId,
    calls: Mutex<HashMap<String, usize>>,
}

impl CountingResolver {
    fn calls(&self, users: &[PubkyId]) -> Vec<usize> {
        let calls = self.calls.lock().unwrap();
        users
            .iter()
            .map(|user| *calls.get(user.as_ref()).unwrap_or(&0))
            .collect()
    }
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for CountingResolver {
    async fn resolve_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        let user_id = user_pk.z32();
        *self
            .calls
            .lock()
            .unwrap()
            .entry(user_id.clone())
            .or_default() += 1;
        Ok(match self.published.get(&user_id) {
            Some(keypair) => {
                PkdnsRecord::Published(signed_packet_naming(keypair, Some(&self.homeserver), 1))
            }
            None => PkdnsRecord::Missing,
        })
    }
}

/// `count` users nobody indexed, with their keys.
fn unknown_users(count: usize) -> Vec<(PubkyId, Keypair)> {
    (0..count)
        .map(|_| {
            let keypair = Keypair::random();
            (PubkyId::from(keypair.public_key()), keypair)
        })
        .collect()
}

/// A post address of each user: items whose authors Nexus has never seen.
fn items_of(users: &[PubkyId]) -> Vec<String> {
    users
        .iter()
        .map(|user| post_uri_builder(user.to_string(), generate_post_id()))
        .collect()
}

/// The spec gives a collection no parent and no embed, so the refused edit is a reply edited
/// into a collection (it drops the reply parent), and the PUT that waits for a missing
/// dependency is a collection whose author is not indexed yet.
#[tokio_shared_rt::test(shared)]
async fn a_collection_put_looks_up_item_authors_only_when_it_writes_or_changes_the_post(
) -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let author_kp = Keypair::random();
    let author = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Post:ItemLookups", "collection item lookups"),
        )
        .await?;
    let (parent_id, parent_path) = test
        .create_post(&author_kp, &short_post("a parent"))
        .await?;

    // Authors with no homeserver record: only a lookup's count tells it ran
    let unrecorded: Vec<PubkyId> = unknown_users(3).into_iter().map(|(id, _)| id).collect();
    // Authors with a record: a lookup of them creates a stub user
    let recorded = unknown_users(2);
    let recorded_ids: Vec<PubkyId> = recorded.iter().map(|(id, _)| id.clone()).collect();
    let recorded_refs: Vec<&str> = recorded_ids.iter().map(|id| id.as_ref()).collect();
    let resolver = Arc::new(CountingResolver {
        published: recorded
            .into_iter()
            .map(|(id, keypair)| (id.to_string(), keypair))
            .collect(),
        homeserver: random_pubky_id(),
        calls: Mutex::new(HashMap::new()),
    });
    let ingestor = UserIngestor::new([], resolver.clone());
    let put = |author: &str, id: &str, post: &PubkyAppPost| {
        let ingestor = ingestor.clone();
        let input = PostInput::from_v0(post.clone(), post_uri_builder(author.into(), id.into()));
        let key = resource_key(author, id).expect("a post key");
        async move { handlers::post::sync_put(input, key, &ingestor).await }
    };

    // A new collection looks each unknown author up once
    let id = generate_post_id();
    let items = items_of(&unrecorded);
    let collection = collection_post_with_items("Curated", &items);
    put(&author, &id, &collection).await?;
    assert_eq!(resolver.calls(&unrecorded), [1, 1, 1], "a new collection");

    // Its unchanged replay looks nobody up
    put(&author, &id, &collection).await?;
    assert_eq!(
        resolver.calls(&unrecorded),
        [1, 1, 1],
        "an unchanged replay"
    );

    // A reply edited into a collection drops its reply parent and is refused: no lookup and no
    // stub user, also of the authors it names for the first time
    let reply_id = generate_post_id();
    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(author.clone(), parent_id.clone())),
        ..short_post("a reply")
    };
    put(&author, &reply_id, &reply).await?;
    let new_items: Vec<String> = items
        .iter()
        .cloned()
        .chain(items_of(&recorded_ids))
        .collect();
    let refused = collection_post_with_items("Refused", &new_items);
    let err = put(&author, &reply_id, &refused)
        .await
        .expect_err("a parent change");
    assert!(
        matches!(err, EventProcessorError::PostParentChanged(_)),
        "{err}"
    );
    assert_eq!(resolver.calls(&unrecorded), [1, 1, 1], "a refused edit");
    assert_eq!(resolver.calls(&recorded_ids), [0, 0], "a refused edit");
    for user in &recorded_refs {
        assert!(
            !users_exist(&[user]).await?,
            "a refused edit creates no stub user"
        );
    }

    // An edit that changes the items looks each unknown author up once: the ones still
    // unrecorded again, the new ones for the first time, who get their stub user
    let changed = collection_post_with_items("Changed", &new_items);
    put(&author, &id, &changed).await?;
    assert_eq!(
        resolver.calls(&unrecorded),
        [2, 2, 2],
        "a changed collection"
    );
    assert_eq!(
        resolver.calls(&recorded_ids),
        [1, 1],
        "a changed collection"
    );
    let stubs = users_exist(&recorded_refs).await;
    for user in &recorded_ids {
        exec_single_row(queries::del::delete_user(user.as_ref())).await?;
    }
    assert!(
        stubs?,
        "the lookups of a changed collection create the stub users"
    );

    // A collection whose author is not indexed yet fails before its write, as each of its
    // retries does: none of them looks an item author up
    let unindexed = random_pubky_id().to_string();
    let waiting_id = generate_post_id();
    for _ in 0..2 {
        let err = put(&unindexed, &waiting_id, &collection)
            .await
            .expect_err("a missing dependency");
        assert!(
            matches!(err, EventProcessorError::MissingDependency { .. }),
            "{err}"
        );
    }
    assert_eq!(
        resolver.calls(&unrecorded),
        [2, 2, 2],
        "a PUT that waits for a missing dependency"
    );

    for post_id in [&id, &reply_id] {
        handlers::post::del(resource_key(&author, post_id)?, &ingestor).await?;
    }
    test.cleanup_post(&author_kp, &parent_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
