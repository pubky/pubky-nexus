use crate::db::{kv::RedisResult, RedisOps};
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::error;

/// An event line as Nexus stores it in the `Event:Events` list: the line the homeserver sent,
/// then the public key of the homeserver the event was read from, after a space.
///
/// ```text
/// PUT pubky://<pk>/<path> <hs_pk>
/// ```
///
/// # Reading a line
/// [`EventLine::split`] is the one reader. A line is keyed only when the text after its last
/// space is a valid [`PubkyId`] and what precedes it still contains a space: the event type and
/// the address. Homeservers write file names unencoded, so the address may contain spaces. The
/// key is after the last space, not the second.
///
/// # Invariant
/// The format is unambiguous only while **a line without a key never has a space in its
/// address**. Lines stored before Nexus kept the key have none, and some keep none for good:
/// those whose user had no `HOSTED_BY` edge when `EventLineHomeserverBackfill1794005092` ran.
/// Every one of them has exactly one space, because the watcher of its day skipped an address
/// with spaces as unrecognized. Every line stored since carries a key: [`EventLine::new`] takes
/// a [`PubkyId`], so no line can be built with an empty or malformed one.
///
/// Were a keyless line with a space in its address ever stored, a user could name a file
/// `x <some_hs_pk>` and readers would attribute the event to a homeserver of the user's
/// choosing. **Any build that stores addresses containing spaces must include this type**, so
/// those lines are keyed from the first.
///
/// # What the key means
/// The homeserver Nexus read the event from. It is not proof of where the user is hosted: a
/// user with no `HOSTED_BY` edge is read from the primary homeserver whenever its feed lists
/// them and their published record names no other homeserver, a failed lookup included. On a
/// line stored before Nexus kept the key it is inferred, not observed: the backfill gave the
/// line the homeserver of its user's `HOSTED_BY` edge at the time it ran. Consumers treat it
/// as a hint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLine(String);

impl EventLine {
    /// The line stored for `line`, as the homeserver sent it, read from `homeserver_id`.
    pub fn new(line: &str, homeserver_id: &PubkyId) -> Self {
        Self(format!("{line} {homeserver_id}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Splits a stored `line` into the line as the homeserver sent it and the key of the
    /// homeserver it was read from, if it has one: the reader rule of [`EventLine`].
    pub fn split(line: &str) -> (&str, Option<&str>) {
        match line.rsplit_once(' ') {
            Some((received, key)) if received.contains(' ') && PubkyId::try_from(key).is_ok() => {
                (received, Some(key))
            }
            _ => (line, None),
        }
    }
}

#[async_trait::async_trait]
impl RedisOps for EventLine {
    async fn prefix() -> String {
        "Event".to_string()
    }
}

impl AsRef<[String]> for EventLine {
    fn as_ref(&self) -> &[String] {
        std::slice::from_ref(&self.0)
    }
}

impl EventLine {
    #[tracing::instrument(name = "event.index.write", skip_all)]
    pub async fn store(&self) -> RedisResult<()> {
        self.put_index_list(&["Events"]).await
    }

    pub async fn get_from_index(
        cursor: Option<u64>,
        limit: usize,
    ) -> RedisResult<(Vec<String>, u64)> {
        let start = cursor.unwrap_or(0);
        let start_u = usize::try_from(start).unwrap_or(usize::MAX);
        let result = EventLine::try_from_index_list(&["Events"], Some(start_u), Some(limit)).await;

        let events = match result {
            Ok(r) => r.unwrap_or_default(),
            Err(error) => {
                error!("IndexReadFailed: Failed to read from list due to Redis error: {error}");
                return Err(error);
            }
        };

        let next_cursor = start + events.len() as u64;
        Ok((events, next_cursor))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER: &str = "984orjzbusofbqhsqz9axpez3uuwd3hbpqztd6rtx3pr78y9s1my";
    const HS: &str = "8rsrmfrn1anbrzuxiffwy1174o58emf4qgbfk5h7s8a33r3bd8dy";

    fn homeserver() -> PubkyId {
        PubkyId::try_from(HS).expect("a valid key")
    }

    fn line(path: &str) -> String {
        format!("PUT pubky://{USER}/pub/{path}")
    }

    #[test]
    fn a_keyless_line_is_returned_whole_without_a_key() {
        let line = line("pubky.app/posts/0032SSN7Q4EVG");
        assert_eq!(EventLine::split(&line), (line.as_str(), None));
    }

    #[test]
    fn a_keyed_line_splits_into_the_received_line_and_its_key() {
        let received = line("pubky.app/posts/0032SSN7Q4EVG");
        let stored = format!("{received} {HS}");
        assert_eq!(EventLine::split(&stored), (received.as_str(), Some(HS)));
    }

    /// A follow's address ends in a key, and a keyless line's last field is its whole address:
    /// neither is a homeserver key.
    #[test]
    fn a_key_shaped_path_tail_on_a_keyless_line_is_no_key() {
        let line = format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}");
        assert_eq!(EventLine::split(&line), (line.as_str(), None));
    }

    #[test]
    fn a_path_with_spaces_keeps_them_and_gives_up_only_the_key() {
        let received = line("pubky.app/files/my holiday photo.jpg");
        let stored = format!("{received} {HS}");
        assert_eq!(EventLine::split(&stored), (received.as_str(), Some(HS)));
    }

    /// Only the last field is the key: a file named like one stays in the address.
    #[test]
    fn a_path_ending_in_a_key_shaped_token_gives_the_real_key() {
        let received = line(&format!("pubky.app/files/photo {OTHER}"));
        let stored = format!("{received} {HS}");
        assert_eq!(EventLine::split(&stored), (received.as_str(), Some(HS)));
    }

    #[test]
    fn a_trailing_space_is_no_key() {
        let line = format!("{} ", line("pubky.app/posts/0032SSN7Q4EVG"));
        assert_eq!(EventLine::split(&line), (line.as_str(), None));
    }

    /// A key needs an event type and an address before it: a line of two fields is not keyed,
    /// and a line that doesn't split at all is returned as it is.
    #[test]
    fn a_line_without_an_address_before_the_key_is_not_keyed() {
        for line in [format!("PUT {HS}"), "garbage".to_string(), String::new()] {
            assert_eq!(EventLine::split(&line), (line.as_str(), None), "{line:?}");
        }
    }

    #[test]
    fn a_line_round_trips_through_new() {
        for received in [
            line("pubky.app/posts/0032SSN7Q4EVG"),
            line("pubky.app/files/my holiday photo.jpg"),
            line(&format!("pubky.app/files/photo {OTHER}")),
            format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}"),
        ] {
            let stored = EventLine::new(&received, &homeserver());
            assert_eq!(stored.as_str(), format!("{received} {HS}"));
            assert_eq!(
                EventLine::split(stored.as_str()),
                (received.as_str(), Some(HS)),
                "{received}"
            );
        }
    }
}
