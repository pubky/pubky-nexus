//! The epoch gate: every user is read through one path, the one their epoch names.

use super::epoch_metrics::record_gated;
use super::event::{Event, EventType};
use super::target::EventTarget;
use crate::errors::EventProcessorError;
use nexus_common::models::user::Epoch;
use pubky_social_specs::Resource;

/// What the gate does with an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Gate {
    /// The event goes on to the handlers.
    Handle,
    /// The event goes on to the handlers, then its user moves to `v1`: the flip.
    Flip,
    /// The event is skipped: no fetch, no stored event line, no error, no retry.
    Ignore,
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

/// Whether `event` is a PUT of a `social/v1` profile.
fn is_profile_put(event: &Event) -> bool {
    event.event_type == EventType::Put
        && matches!(&event.target, EventTarget::Social(parsed) if parsed.resource == Resource::User)
}

/// The gate's decision for `event`, by its user's epoch in the graph: never the cached
/// `User:Details` copy, which a concurrent profile write can leave stale. An ignored event is
/// counted on `watcher.epoch.skipped`.
pub(super) async fn check(event: &Event) -> Result<Gate, EventProcessorError> {
    let user_epoch = Epoch::get_from_graph(event.target.user_id()).await?;
    let gate = decide(event, user_epoch);
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
}
