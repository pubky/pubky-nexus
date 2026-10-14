//! The inbound bridge brings a seed scope what users outside it did to the seed: each
//! notification's record is fetched from its author's homeserver, never from the big
//! Nexus, and removed when its homeserver no longer has it.

use super::utils::{create_external_test_homeserver, short_post, user_on};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use crate::utils::default_moderation_tests;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::fetch_row_from_graph;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::{PostCounts, PostDetails, POST_TIMELINE_KEY_PARTS};
use nexus_common::models::scope::{PinnedScope, Scope, SeedHopsScope};
use nexus_common::models::user::{is_stub, UserIngestor};
use nexus_common::types::DynError;
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::service::inbound_bridge::{InboundBridge, NotificationSource, Outcome};
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, PubkyAppPostKind, PubkyAppTag, PubkyAppUser, PubkyId};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// A big Nexus serving notifications from memory, newest first; `None` is down.
struct FakeBigNexus {
    name: String,
    notifications: Mutex<Option<Vec<Notification>>>,
}

impl FakeBigNexus {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            notifications: Mutex::new(Some(Vec::new())),
        }
    }

    fn notify(&self, body: NotificationBody) {
        let timestamp = Utc::now().timestamp_millis();
        if let Some(notifications) = self.notifications.lock().unwrap().as_mut() {
            notifications.push(Notification { timestamp, body });
        }
    }
}

/// The bridge's handle on a fake, which the test keeps to add notifications.
struct Source(Arc<FakeBigNexus>);

#[async_trait::async_trait]
impl NotificationSource for Source {
    fn name(&self) -> &str {
        &self.0.name
    }

    async fn page(
        &self,
        _user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<Vec<Notification>, DynError> {
        let guard = self.0.notifications.lock().unwrap();
        let notifications = guard.as_ref().ok_or("the big Nexus is down")?;
        let mut matching: Vec<&Notification> = notifications
            .iter()
            .filter(|n| start.is_none_or(|start| n.timestamp <= start))
            .filter(|n| end.is_none_or(|end| n.timestamp >= end))
            .collect();
        matching.sort_by_key(|n| std::cmp::Reverse(n.timestamp));
        Ok(matching
            .into_iter()
            .skip(skip)
            .take(100)
            .map(|n| Notification {
                timestamp: n.timestamp,
                body: n.body.clone(),
            })
            .collect())
    }
}

/// The seed (with one post), an outsider on another homeserver, and a seed-scoped
/// bridge reading `sources`.
struct Setup {
    test: WatcherTest,
    seed: PubkyId,
    seed_post_id: String,
    outsider_kp: Keypair,
    outsider: PubkyId,
}

async fn setup() -> Result<Setup> {
    let mut test = WatcherTest::setup(None).await?;
    let seed_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:Bridge:Seed".to_string(),
        status: None,
    };
    test.create_user(&seed_kp, &user).await?;
    let seed = PubkyId::from(seed_kp.public_key());
    let (seed_post_id, _) = test
        .create_post(&seed_kp, &short_post("Watcher:Bridge:SeedPost", None))
        .await?;

    let hs_pk = create_external_test_homeserver(&mut test).await?;
    let (outsider_kp, outsider) = user_on(&test, &hs_pk).await?;

    Ok(Setup {
        test,
        seed,
        seed_post_id,
        outsider_kp,
        outsider,
    })
}

fn bridge(seed: &PubkyId, sources: Vec<Arc<FakeBigNexus>>) -> InboundBridge {
    let scope: Arc<dyn Scope> = Arc::new(PinnedScope::new(
        Arc::new(SeedHopsScope::new(seed.clone(), 2)),
        [default_moderation_tests().id.clone()],
    ));
    InboundBridge::new(
        seed.clone(),
        sources
            .into_iter()
            .map(|source| Box::new(Source(source)) as Box<dyn NotificationSource>)
            .collect(),
        Arc::new(UserIngestor::with_scope(Vec::<PubkyId>::new(), scope)),
        default_moderation_tests(),
        PathBuf::from("/tmp/nexus-watcher-test"),
        DEFAULT_MAX_FILE_SIZE,
        0,
    )
}

/// An outsider's reply to the seed reaches the seed's thread, fetched from the
/// outsider's homeserver; the outsider stays a stub and its reply stays out of the
/// global timeline. Two sources reporting it index it once.
#[tokio_shared_rt::test(shared)]
async fn test_an_outsiders_reply_to_the_seed_is_bridged_once() -> Result<()> {
    let Setup {
        mut test,
        seed,
        seed_post_id,
        outsider_kp,
        outsider,
    } = setup().await?;
    let seed_post_uri = post_uri_builder(seed.to_string(), seed_post_id.clone());
    // Written to the outsider's homeserver, which nothing polls.
    let (reply_id, _) = test
        .create_post(
            &outsider_kp,
            &short_post("Watcher:Bridge:Reply", Some(seed_post_uri.clone())),
        )
        .await?;
    let reply_uri = post_uri_builder(outsider.to_string(), reply_id.clone());

    let (first, second) = (
        Arc::new(FakeBigNexus::new("first")),
        Arc::new(FakeBigNexus::new("second")),
    );
    for source in [&first, &second] {
        source.notify(NotificationBody::Reply {
            replied_by: outsider.to_string(),
            parent_post_uri: seed_post_uri.clone(),
            reply_uri: reply_uri.clone(),
            post_kind: PubkyAppPostKind::Short,
        });
    }

    bridge(&seed, vec![first, second])
        .poll()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    assert!(
        PostDetails::get_by_id(&outsider, &reply_id)
            .await?
            .is_some(),
        "the reply landed"
    );
    let counts = PostCounts::get_by_id(&seed, &seed_post_id)
        .await?
        .expect("counts");
    assert_eq!(counts.replies, 1, "counted once");
    assert!(
        is_stub(&outsider).await?,
        "its author stays outside the scope"
    );
    let in_timeline = PostDetails::check_sorted_set_member(
        None,
        &POST_TIMELINE_KEY_PARTS,
        &[&outsider, &reply_id],
    )
    .await?;
    assert!(in_timeline.is_none());
    Ok(())
}

/// A tag the outsider puts on the seed's post is bridged; when it is removed from the
/// outsider's homeserver, the untag notification removes it here too.
#[tokio_shared_rt::test(shared)]
async fn test_an_outsiders_tag_is_bridged_and_its_removal_mirrored() -> Result<()> {
    let Setup {
        mut test,
        seed,
        seed_post_id,
        outsider_kp,
        outsider,
    } = setup().await?;
    let seed_post_uri = post_uri_builder(seed.to_string(), seed_post_id.clone());
    let tag = PubkyAppTag {
        uri: seed_post_uri.clone(),
        label: "bridged".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&outsider_kp, &tag.hs_path(), tag.clone()).await?;

    let source = Arc::new(FakeBigNexus::new("big"));
    let bridge = bridge(&seed, vec![source.clone()]);
    source.notify(NotificationBody::TagPost {
        tagged_by: outsider.to_string(),
        tag_label: tag.label.clone(),
        post_uri: seed_post_uri.clone(),
        post_kind: PubkyAppPostKind::Short,
    });
    bridge.poll().await.map_err(|e| anyhow::anyhow!(e))?;
    assert!(
        is_tagged(&outsider, &seed_post_id, &tag.label).await?,
        "tag bridged"
    );

    test.del(&outsider_kp, &tag.hs_path()).await?;
    // A later timestamp than the tag's.
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    source.notify(NotificationBody::UntagPost {
        untagged_by: outsider.to_string(),
        tag_label: tag.label.clone(),
        post_uri: seed_post_uri,
        post_kind: PubkyAppPostKind::Short,
    });
    bridge.poll().await.map_err(|e| anyhow::anyhow!(e))?;
    assert!(
        !is_tagged(&outsider, &seed_post_id, &tag.label).await?,
        "removal mirrored"
    );
    Ok(())
}

/// A member's record isn't bridged (its own stream covers it), and a source that is
/// down changes nothing and keeps its position, so the record comes on the next poll.
#[tokio_shared_rt::test(shared)]
async fn test_members_are_left_to_their_streams_and_a_down_source_waits() -> Result<()> {
    let Setup {
        mut test,
        seed,
        seed_post_id,
        outsider_kp,
        outsider,
    } = setup().await?;
    let seed_post_uri = post_uri_builder(seed.to_string(), seed_post_id.clone());

    let bridge_for_seed = bridge(&seed, Vec::new());
    assert_eq!(
        bridge_for_seed.bridge_record(&seed_post_uri).await?,
        Outcome::Skipped,
        "the seed is in scope"
    );

    let (reply_id, _) = test
        .create_post(
            &outsider_kp,
            &short_post("Watcher:Bridge:LateReply", Some(seed_post_uri.clone())),
        )
        .await?;
    let source = Arc::new(FakeBigNexus::new("flaky"));
    source.notify(NotificationBody::Reply {
        replied_by: outsider.to_string(),
        parent_post_uri: seed_post_uri,
        reply_uri: post_uri_builder(outsider.to_string(), reply_id.clone()),
        post_kind: PubkyAppPostKind::Short,
    });
    let stored = source.notifications.lock().unwrap().take();
    let bridge = bridge(&seed, vec![source.clone()]);

    assert!(bridge.poll().await.is_err(), "the only source is down");
    assert!(PostDetails::get_by_id(&outsider, &reply_id)
        .await?
        .is_none());

    *source.notifications.lock().unwrap() = stored;
    bridge
        .poll_source(&Source(source))
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    assert!(
        PostDetails::get_by_id(&outsider, &reply_id)
            .await?
            .is_some(),
        "comes later"
    );
    Ok(())
}

async fn is_tagged(tagger: &str, post_id: &str, label: &str) -> Result<bool> {
    let query = Query::new(
        "test_is_tagged",
        "MATCH (:User {id: $tagger})-[t:TAGGED {label: $label}]->(:Post {id: $post_id})
         RETURN t.label AS label",
    )
    .param("tagger", tagger.to_string())
    .param("label", label.to_string())
    .param("post_id", post_id.to_string());
    Ok(fetch_row_from_graph(query).await?.is_some())
}
