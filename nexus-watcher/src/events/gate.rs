//! The epoch gate: every user is read through one path, the one their epoch names.

use super::epoch_metrics::{record_deferred, record_gated};
use super::event::{Event, EventType};
use super::retry::{IndexKey, RetryEvent, RetryStore};
use super::target::EventTarget;
use crate::errors::EventProcessorError;
use nexus_common::models::user::Epoch;
use pubky_social_specs::traits::HasPath as _;
use pubky_social_specs::{PubkyId, PubkySocialUser, Resource};

/// What the gate does with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gate {
    /// The event goes on to the handlers.
    Handle,
    /// The flip: its user moves to `v1` first (the internal migration, then the epoch write),
    /// then the event goes on to the handlers as a `v1` user's profile PUT.
    Flip,
    /// The event is skipped: no fetch, no stored event line, no error, no retry.
    Ignore,
    /// The event waits for its user's flip, queued for retry: an event of a user at `v0` whose
    /// `social/v1` profile PUT sits in the retry queue, of a kind [`waits_for_flip`] names.
    /// A `pubky.app` DEL handled now would delete content the flip's internal migration is
    /// about to move; retried after the flip, the user is at `v1` and the gate ignores it. A
    /// `social/v1` write ignored now would be lost, the user writing from their `v1` app once
    /// the flip is written; retried after the flip, the gate handles it.
    Defer,
}

/// The gate's decision for `event` of a user at `user_epoch`. Pure: the caller reads the
/// user's epoch.
fn decide(event: &Event, user_epoch: Epoch) -> Gate {
    match (event.target.epoch(), user_epoch) {
        (Epoch::V0, Epoch::V0) | (Epoch::V1, Epoch::V1) => Gate::Handle,
        // A user at `v1` is no longer read through `pubky.app`, PUT or DEL.
        (Epoch::V0, Epoch::V1) => Gate::Ignore,
        // The migrator writes the `social/v1` profile last, so its PUT ends the user's migration.
        (Epoch::V1, Epoch::V0) if is_profile_put(event) => Gate::Flip,
        // A user at `v0` is not read through `social/v1`, PUT or DEL: these are the migrator's
        // copies, which must not go through the edit path.
        (Epoch::V1, Epoch::V0) => Gate::Ignore,
    }
}

/// Whether `event` is a PUT of a `social/v1` profile: the flip, for a user at `v0`.
pub(super) fn is_profile_put(event: &Event) -> bool {
    event.event_type == EventType::Put && is_profile(event)
}

/// Whether `event` is a flip: a `social/v1` profile PUT of a user still at `v0` in the graph.
/// Losing one strands that user, so a flip dead-lettered at its limit is logged at `error`; a
/// `v1` user's own edit keeps the level other events have. No flip is dropped as not worth
/// retrying: its step runs before the fetch and every failure of it is queued. A failed graph
/// read counts as a flip: on a failing graph, over-reporting a lost flip is the safe side.
pub(crate) async fn is_flip(event: &Event) -> bool {
    !matches!(confirm_flip(event).await, Ok(false))
}

/// Whether `event` is a flip, by its user's epoch in the graph; a failed graph read is the
/// caller's to classify.
pub(crate) async fn confirm_flip(event: &Event) -> Result<bool, EventProcessorError> {
    Ok(is_profile_put(event) && decide_by_graph(event).await?.0 == Gate::Flip)
}

/// Whether `event` addresses a `social/v1` profile, PUT or DEL.
fn is_profile(event: &Event) -> bool {
    matches!(&event.target, EventTarget::Social(parsed) if parsed.resource == Resource::User)
}

/// Whether `event`, decided `gate` for a user at `user_epoch`, waits for that user's flip
/// while it is queued. Pure: the caller reads the epoch and the queue.
///
/// - A `pubky.app` DEL of a `v0` user (`Handle`): handled now, it would delete content the
///   flip's internal migration is about to move.
/// - A `social/v1` event of a `v0` user other than the profile (`Ignore`): ignored now, it
///   would be lost, since the user writes from their `v1` app once the flip is written.
///
/// The profile's own events never wait: they share the flip's URI, so a held one queued under
/// it would overwrite the flip's entry. A second profile PUT is the flip itself; a profile DEL
/// stays ignored, and the flip's later fetch answers 404 and keeps the previous profile.
fn waits_for_flip(event: &Event, gate: Gate, user_epoch: Epoch) -> bool {
    match gate {
        Gate::Handle => event.event_type == EventType::Del && event.target.epoch() == Epoch::V0,
        Gate::Ignore => user_epoch == Epoch::V0 && !is_profile(event),
        Gate::Flip | Gate::Defer => false,
    }
}

/// Whether `entry`, the retry queue's entry under a user's `social/v1` profile URI, is their
/// queued flip: a PUT. A DEL entry under that URI is no flip. Pure: the caller reads the queue.
fn is_queued_flip(entry: Option<&RetryEvent>) -> bool {
    entry.is_some_and(|entry| entry.event_type == EventType::Put)
}

/// The URI of `user_id`'s `social/v1` profile: the flip's event URI, which keys its retry entry.
pub(super) fn flip_profile_uri(user_id: &PubkyId) -> String {
    format!("pubky://{user_id}{}", PubkySocialUser::create_path())
}

/// The gate's decision for `event` by its user's epoch in the graph, with that epoch: never
/// the cached `User:Details` copy, which a concurrent profile write can leave stale.
async fn decide_by_graph(event: &Event) -> Result<(Gate, Epoch), EventProcessorError> {
    let user_epoch = Epoch::get_from_graph(event.target.user_id()).await?;
    Ok((decide(event, user_epoch), user_epoch))
}

/// The gate's decision for `event`: by its user's epoch in the graph ([`decide_by_graph`]),
/// then, for an event that [`waits_for_flip`] and only then, by `retry_store`: with the
/// user's flip queued there, a PUT entry under the profile URI, the event is deferred. A DEL
/// entry under that URI is no flip. An ignored or deferred event is counted on
/// `watcher.epoch.skipped`.
pub(super) async fn check(
    event: &Event,
    retry_store: &dyn RetryStore,
) -> Result<Gate, EventProcessorError> {
    let (gate, user_epoch) = decide_by_graph(event).await?;
    if waits_for_flip(event, gate, user_epoch) {
        let flip_uri = flip_profile_uri(event.target.user_id());
        let entry = retry_store.get(&IndexKey::for_uri(&flip_uri)).await?;
        if is_queued_flip(entry.as_ref()) {
            record_deferred(&event.target, &event.uri, &flip_uri);
            return Ok(Gate::Defer);
        }
    }
    if gate == Gate::Ignore {
        record_gated(&event.target, &event.uri, user_epoch);
    }
    Ok(gate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ParseResult;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    fn event(line: &str) -> Event {
        match Event::parse_event(line) {
            Ok(ParseResult::Parsed(event)) => event,
            other => panic!("{line}: expected Parsed, got {other:?}"),
        }
    }

    /// Lines of PUT and DEL events of each of `paths`, under `/pub/`.
    fn lines(paths: &[&str]) -> Vec<String> {
        ["PUT", "DEL"]
            .iter()
            .flat_map(|kind| {
                paths
                    .iter()
                    .map(move |path| format!("{kind} pubky://{USER}/pub/{path}"))
            })
            .collect()
    }

    /// Lines of `pubky.app` events, PUT and DEL, of a profile and a post.
    fn pubky_app_lines() -> Vec<String> {
        lines(&["pubky.app/profile.json", "pubky.app/posts/0032SSN7Q4EVG"])
    }

    /// Lines of `social/v1` events, PUT and DEL, of every resource the parse step indexes.
    fn social_v1_lines() -> Vec<String> {
        lines(&[
            "social/v1/profile.json",
            "social/v1/posts/0032SSN7Q4EVG/0032SSN7Q4EVJ.json",
            &format!("social/v1/follows/{USER}.json"),
            "social/v1/tags/8Z8CWH8NVYQY39ZEBFGKQWWEKG.json",
        ])
    }

    #[test]
    fn a_v0_user_has_pubky_app_events_handled() {
        for line in pubky_app_lines() {
            assert_eq!(decide(&event(&line), Epoch::V0), Gate::Handle, "{line}");
        }
    }

    #[test]
    fn a_v1_user_has_pubky_app_events_ignored() {
        for line in pubky_app_lines() {
            assert_eq!(decide(&event(&line), Epoch::V1), Gate::Ignore, "{line}");
        }
    }

    /// All but the v1 profile PUT, which flips the user.
    #[test]
    fn a_v0_user_has_social_v1_events_ignored() {
        let profile_put = format!("PUT pubky://{USER}/pub/social/v1/profile.json");
        for line in social_v1_lines() {
            let expected = if line == profile_put {
                Gate::Flip
            } else {
                Gate::Ignore
            };
            assert_eq!(decide(&event(&line), Epoch::V0), expected, "{line}");
        }
    }

    /// A `pubky.app` profile PUT is no flip: only the `social/v1` one ends a migration.
    #[test]
    fn only_the_social_v1_profile_put_flips() {
        let line = format!("PUT pubky://{USER}/pub/pubky.app/profile.json");
        assert_eq!(decide(&event(&line), Epoch::V0), Gate::Handle);
        let line = format!("PUT pubky://{USER}/pub/social/v1/profile.json");
        assert_eq!(
            decide(&event(&line), Epoch::V1),
            Gate::Handle,
            "a profile edit"
        );
    }

    #[test]
    fn a_v1_user_has_social_v1_events_handled() {
        for line in social_v1_lines() {
            assert_eq!(decide(&event(&line), Epoch::V1), Gate::Handle, "{line}");
        }
    }

    /// Which events wait for a queued flip, for every line at both epochs: a `v0` user's
    /// `pubky.app` DELs and `social/v1` events other than the profile's, nothing of a `v1`
    /// user's.
    #[test]
    fn only_a_v0_users_pubky_app_dels_and_social_v1_writes_wait_for_the_flip() {
        let profile = format!("pubky://{USER}/pub/social/v1/profile.json");
        for line in pubky_app_lines().into_iter().chain(social_v1_lines()) {
            let event = event(&line);
            let is_social_v1 = event.target.epoch() == Epoch::V1;
            let expected_at_v0 = if is_social_v1 {
                !line.ends_with(&profile)
            } else {
                event.event_type == EventType::Del
            };
            for (user_epoch, expected) in [(Epoch::V0, expected_at_v0), (Epoch::V1, false)] {
                let gate = decide(&event, user_epoch);
                assert_eq!(
                    waits_for_flip(&event, gate, user_epoch),
                    expected,
                    "{line} of a {} user",
                    user_epoch.as_str()
                );
            }
        }
    }

    /// Only a PUT entry under the profile URI is a queued flip.
    #[test]
    fn only_a_put_entry_is_a_queued_flip() {
        let entry = |kind: &str| {
            let line = format!("{kind} pubky://{USER}/pub/social/v1/profile.json");
            RetryEvent::new(&event(&line), 0, "hs")
        };
        assert!(is_queued_flip(Some(&entry("PUT"))));
        assert!(
            !is_queued_flip(Some(&entry("DEL"))),
            "a profile DEL is no flip"
        );
        assert!(!is_queued_flip(None), "nothing queued");
    }

    /// The deferral: `check` against the graph and an in-memory retry queue.
    mod check {
        use super::*;
        use crate::events::retry::InMemoryRetryStore;
        use async_trait::async_trait;
        use nexus_common::db::{exec_single_row, queries};
        use nexus_common::models::traits::Collection as _;
        use nexus_common::models::user::UserDetails;
        use nexus_common::types::DynError;
        use nexus_common::utils::test_utils::random_pubky_id;
        use nexus_common::{StackConfig, StackManager};
        use std::sync::atomic::{AtomicUsize, Ordering};

        /// An in-memory retry queue that counts its reads, so a test can tell which events
        /// made the gate look at it.
        struct CountingStore {
            inner: InMemoryRetryStore,
            reads: AtomicUsize,
        }

        impl CountingStore {
            fn new() -> Self {
                Self {
                    inner: InMemoryRetryStore::new(),
                    reads: AtomicUsize::new(0),
                }
            }

            fn reads(&self) -> usize {
                self.reads.load(Ordering::SeqCst)
            }

            /// Queues `user_id`'s flip, as a failed flip step on its first attempt does.
            async fn queue_flip(&self, user_id: &PubkyId) -> Result<(), EventProcessorError> {
                self.queue_profile_event(user_id, "PUT").await
            }

            /// Queues a `kind` event of `user_id`'s `social/v1` profile, under the flip's key.
            async fn queue_profile_event(
                &self,
                user_id: &PubkyId,
                kind: &str,
            ) -> Result<(), EventProcessorError> {
                let line = format!("{kind} {}", flip_profile_uri(user_id));
                self.put(&RetryEvent::new(&event(&line), i64::MAX, "hs"))
                    .await
            }
        }

        #[async_trait]
        impl RetryStore for CountingStore {
            async fn put(&self, event: &RetryEvent) -> Result<(), EventProcessorError> {
                self.inner.put(event).await
            }

            async fn get(
                &self,
                index_key: &IndexKey,
            ) -> Result<Option<RetryEvent>, EventProcessorError> {
                self.reads.fetch_add(1, Ordering::SeqCst);
                self.inner.get(index_key).await
            }

            async fn remove(&self, index_key: &IndexKey) -> Result<(), EventProcessorError> {
                self.inner.remove(index_key).await
            }

            async fn remove_if(
                &self,
                index_key: &IndexKey,
                expected_nonce: i64,
            ) -> Result<bool, EventProcessorError> {
                self.inner.remove_if(index_key, expected_nonce).await
            }

            async fn put_if(
                &self,
                event: &RetryEvent,
                expected_nonce: i64,
            ) -> Result<bool, EventProcessorError> {
                self.inner.put_if(event, expected_nonce).await
            }

            async fn fetch_ready(
                &self,
                now: i64,
                limit: Option<usize>,
            ) -> Result<Vec<(IndexKey, RetryEvent)>, EventProcessorError> {
                self.inner.fetch_ready(now, limit).await
            }
        }

        /// A `User` node without an epoch, which reads as `v0`.
        async fn v0_user() -> Result<PubkyId, DynError> {
            StackManager::setup(&StackConfig::default()).await?;
            let user = random_pubky_id();
            UserDetails::from_pubky(user.clone()).put_to_graph().await?;
            Ok(user)
        }

        async fn delete_user(user: &PubkyId) -> Result<(), DynError> {
            exec_single_row(queries::del::delete_user(user)).await?;
            Ok(())
        }

        /// `pubky.app` DEL lines of `user`'s post and profile.
        fn pubky_app_dels(user: &PubkyId) -> [String; 2] {
            [
                format!("DEL pubky://{user}/pub/pubky.app/posts/0032SSN7Q4EVG"),
                format!("DEL pubky://{user}/pub/pubky.app/profile.json"),
            ]
        }

        /// A `v0` user's `pubky.app` DELs are handled while no flip of theirs is queued, and
        /// deferred once one is: the post's and the profile's alike.
        #[tokio_shared_rt::test(shared)]
        async fn a_v0_users_pubky_app_del_is_deferred_while_their_flip_is_queued(
        ) -> Result<(), DynError> {
            let user = v0_user().await?;
            let store = CountingStore::new();

            for line in pubky_app_dels(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Handle, "{line}");
            }
            assert_eq!(store.reads(), 2, "each DEL asks the queue once");

            store.queue_flip(&user).await?;
            for line in pubky_app_dels(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Defer, "{line}");
            }

            delete_user(&user).await
        }

        /// A DEL entry under the profile URI is no flip: a `v0` user's `pubky.app` DELs are
        /// handled while one is queued.
        #[tokio_shared_rt::test(shared)]
        async fn a_queued_profile_del_defers_nothing() -> Result<(), DynError> {
            let user = v0_user().await?;
            let store = CountingStore::new();
            store.queue_profile_event(&user, "DEL").await?;

            for line in pubky_app_dels(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Handle, "{line}");
            }
            assert_eq!(store.reads(), 2, "each DEL asks the queue once");

            delete_user(&user).await
        }

        /// Only a `v0` user's `social/v1` profile PUT is a flip: a `v1` user's is an edit, and
        /// a `pubky.app` profile PUT of either is no flip.
        #[tokio_shared_rt::test(shared)]
        async fn is_flip_names_the_v0_users_social_v1_profile_put_only() -> Result<(), DynError> {
            let v0 = v0_user().await?;
            let v1 = v0_user().await?;
            Epoch::set_v1_in_graph(&v1).await?;

            let flip = |user: &PubkyId| event(&format!("PUT {}", flip_profile_uri(user)));
            assert!(
                is_flip(&flip(&v0)).await,
                "a v0 user's social/v1 profile PUT"
            );
            assert!(
                !is_flip(&flip(&v1)).await,
                "a v1 user's social/v1 profile PUT"
            );
            for user in [&v0, &v1] {
                let line = format!("PUT pubky://{user}/pub/pubky.app/profile.json");
                assert!(!is_flip(&event(&line)).await, "{line}");
            }

            delete_user(&v0).await?;
            delete_user(&v1).await
        }

        /// `social/v1` lines of `user`'s post, PUT and DEL.
        fn social_v1_post_lines(user: &PubkyId) -> Vec<String> {
            ["PUT", "DEL"]
                .map(|kind| {
                    format!(
                        "{kind} pubky://{user}/pub/social/v1/posts/0032SSN7Q4EVG/0032SSN7Q4EVJ.json"
                    )
                })
                .to_vec()
        }

        /// A `v0` user's `social/v1` writes are ignored while no flip of theirs is queued, as
        /// the migrator's copies are, and deferred once one is.
        #[tokio_shared_rt::test(shared)]
        async fn a_v0_users_social_v1_write_is_deferred_while_their_flip_is_queued(
        ) -> Result<(), DynError> {
            let user = v0_user().await?;
            let store = CountingStore::new();

            for line in social_v1_post_lines(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Ignore, "{line}");
            }
            assert_eq!(store.reads(), 2, "each write asks the queue once");

            store.queue_flip(&user).await?;
            for line in social_v1_post_lines(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Defer, "{line}");
            }

            delete_user(&user).await
        }

        /// With the flip queued, the same user's `pubky.app` PUT and `social/v1` profile events
        /// decide as before, without a look at the queue: a profile DEL shares the flip's URI.
        #[tokio_shared_rt::test(shared)]
        async fn a_v0_users_other_events_are_unchanged_and_never_read_the_queue(
        ) -> Result<(), DynError> {
            let user = v0_user().await?;
            let store = CountingStore::new();
            store.queue_flip(&user).await?;

            let post_put = format!("PUT pubky://{user}/pub/pubky.app/posts/0032SSN7Q4EVG");
            assert_eq!(check(&event(&post_put), &store).await?, Gate::Handle);
            let profile_put = format!("PUT {}", flip_profile_uri(&user));
            assert_eq!(check(&event(&profile_put), &store).await?, Gate::Flip);
            let profile_del = format!("DEL {}", flip_profile_uri(&user));
            assert_eq!(check(&event(&profile_del), &store).await?, Gate::Ignore);
            assert_eq!(
                store.reads(),
                0,
                "only a pubky.app DEL or a social/v1 write asks the queue"
            );

            delete_user(&user).await
        }

        /// A `v1` user's events never read the queue: their `pubky.app` DEL is ignored and
        /// their `social/v1` DEL handled, flip entry or not.
        #[tokio_shared_rt::test(shared)]
        async fn a_v1_users_events_never_read_the_queue() -> Result<(), DynError> {
            let user = v0_user().await?;
            Epoch::set_v1_in_graph(&user).await?;
            let store = CountingStore::new();
            store.queue_flip(&user).await?;

            for line in pubky_app_dels(&user) {
                assert_eq!(check(&event(&line), &store).await?, Gate::Ignore, "{line}");
            }
            let v1_del =
                format!("DEL pubky://{user}/pub/social/v1/posts/0032SSN7Q4EVG/0032SSN7Q4EVJ.json");
            assert_eq!(check(&event(&v1_del), &store).await?, Gate::Handle);
            assert_eq!(store.reads(), 0);

            delete_user(&user).await
        }
    }
}
