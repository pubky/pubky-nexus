use crate::db::{kv::RedisResult, RedisOps};
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Serialize};
use tracing::error;

/// An event line as Nexus stores it in the `Event:Events` list: the public key of the
/// homeserver the event was read from, then the line the homeserver sent, after a space.
///
/// ```text
/// <hs_pk> PUT pubky://<pk>/<path>
/// ```
///
/// # Reading a line
/// [`EventLine::split`] is the one reader. A line is keyed when its first field is a valid
/// [`PubkyId`]; the rest, after one space, is the line as the homeserver sent it.
///
/// The key comes first so that nothing a user controls can pass for it. A line without a key
/// starts with its event type, `PUT` or `DEL`, which is no public key, and everything a user
/// chooses comes after: homeservers write file names unencoded, so an address may hold spaces
/// and key-shaped words, and none of them is ever the first field.
///
/// # Lines without a key
/// Lines stored before Nexus kept the key have none, and some keep none for good: those whose
/// user had no `HOSTED_BY` edge when `EventLineHomeserverBackfill1794005092` ran. Every line
/// stored since carries one: [`EventLine::new`] takes a [`PubkyId`], so no line can be built
/// with an empty or malformed key.
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
    /// The list event lines are stored in, under this type's prefix: `Event:Events`.
    pub const LIST: &'static str = "Events";

    /// The line stored for `line`, as the homeserver sent it, read from `homeserver_id`.
    pub fn new(line: &str, homeserver_id: &PubkyId) -> Self {
        Self(format!("{homeserver_id} {line}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Splits a stored `line` into the line as the homeserver sent it and the key of the
    /// homeserver it was read from, if it has one: the reader rule of [`EventLine`].
    pub fn split(line: &str) -> (&str, Option<&str>) {
        match line.split_once(' ') {
            Some((key, received)) if PubkyId::try_from(key).is_ok() => (received, Some(key)),
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
        self.put_index_list(&[Self::LIST]).await
    }

    pub async fn get_from_index(
        cursor: Option<u64>,
        limit: usize,
    ) -> RedisResult<(Vec<String>, u64)> {
        let start = cursor.unwrap_or(0);
        let start_u = usize::try_from(start).unwrap_or(usize::MAX);
        let result =
            EventLine::try_from_index_list(&[Self::LIST], Some(start_u), Some(limit)).await;

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
    fn a_stored_line_starts_with_the_key() {
        let received = line("pubky.app/posts/0032SSN7Q4EVG");
        assert_eq!(
            EventLine::new(&received, &homeserver()).as_str(),
            format!("{HS} {received}")
        );
    }

    #[test]
    fn a_keyless_line_is_returned_whole_without_a_key() {
        let line = line("pubky.app/posts/0032SSN7Q4EVG");
        assert_eq!(EventLine::split(&line), (line.as_str(), None));
    }

    #[test]
    fn a_keyed_line_splits_into_the_received_line_and_its_key() {
        let received = line("pubky.app/posts/0032SSN7Q4EVG");
        let stored = format!("{HS} {received}");
        assert_eq!(EventLine::split(&stored), (received.as_str(), Some(HS)));
    }

    /// A keyless line starts with its event type, so nothing in its address can pass for a
    /// key: not a follow's key-shaped tail, and not a file named like a key after a space,
    /// with or without a trailing space.
    #[test]
    fn nothing_in_the_address_of_a_keyless_line_is_a_key() {
        for line in [
            format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}"),
            line(&format!("pubky.app/files/photo {HS}")),
            line(&format!("pubky.app/files/{HS} photo.jpg")),
            format!("{} ", line("pubky.app/posts/0032SSN7Q4EVG")),
        ] {
            assert_eq!(EventLine::split(&line), (line.as_str(), None), "{line:?}");
        }
    }

    /// Only the first field is the key: spaces and key-shaped tokens after it stay in the
    /// received line.
    #[test]
    fn a_keyed_line_keeps_its_received_line_whole() {
        for received in [
            line("pubky.app/files/my holiday photo.jpg"),
            line(&format!("pubky.app/files/photo {OTHER}")),
            format!("DEL pubky://{USER}/pub/pubky.app/follows/{OTHER}"),
            format!("{} ", line("pubky.app/posts/0032SSN7Q4EVG")),
        ] {
            let stored = format!("{HS} {received}");
            assert_eq!(
                EventLine::split(&stored),
                (received.as_str(), Some(HS)),
                "{received:?}"
            );
        }
    }

    /// A key is a first field with a line after it: a bare key, a line that doesn't split and
    /// an empty line are returned as they are.
    #[test]
    fn a_line_without_a_first_field_that_is_a_key_is_not_keyed() {
        for line in [
            HS.to_string(),
            format!(" {HS}"),
            "garbage".to_string(),
            "not-a-key PUT pubky://x/pub/a".to_string(),
            String::new(),
        ] {
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
            assert_eq!(
                EventLine::split(stored.as_str()),
                (received.as_str(), Some(HS)),
                "{received}"
            );
        }
    }
}
