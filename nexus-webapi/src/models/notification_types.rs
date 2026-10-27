//! The `types=` list of the notification route, one type for both versions.
//!
//! `K` is the version's token, which parses one name by the version's rule, `/v0`'s
//! [`StoredTypeToken`] or `/v1`'s [`V1TypeToken`], and `MAX` the version's bound. `/v0` is
//! frozen: it takes the twelve types it had, and `event_changed` is an unknown name to it,
//! refused with the parser's own error; `/v1` takes all thirteen.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use const_format::concatcp;
use serde::Deserialize;
use serde_json::Value;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use crate::models::bounded_vec;
use nexus_common::models::notification::NotificationType;

/// One token of a `types=` list, as a version reads it. `deserialize_csv` requires
/// `TryFrom<String>`; the token names the notification type it read.
pub trait TypeToken: TryFrom<String, Error = String> + Into<NotificationType> {
    /// The version's bound on the list: the one place it is written.
    const MAX: usize;
    /// The list's schema description, with the version's bound, as the derive wrote it from
    /// the doc comment.
    const DESCRIPTION: &'static str;
}

/// Comma-separated list of notification types (min=1, max=`MAX` tokens; duplicates are dropped,
/// order preserved), each token parsed by `K`'s rule. Unknown values are rejected. Documented
/// as one string schema named `NotificationTypes` on both versions, by hand, with `K`'s
/// description: the derive would name a reference written with its arguments.
pub struct NotificationTypes<K, const MAX: usize>(pub Vec<NotificationType>, PhantomData<K>);

impl<K, const MAX: usize> fmt::Debug for NotificationTypes<K, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NotificationTypes").field(&self.0).finish()
    }
}

/// The types the list names, for the filtered read.
impl<K, const MAX: usize> From<NotificationTypes<K, MAX>> for Vec<NotificationType> {
    fn from(types: NotificationTypes<K, MAX>) -> Self {
        types.0
    }
}

impl<K: TypeToken, const MAX: usize> NotificationTypes<K, MAX> {
    /// Compile-time check that the list's bound is the token's.
    const BOUND_IS_THE_TOKENS: () = assert!(MAX == K::MAX, "`MAX` must equal `K::MAX`");
}

impl<'de, K: TypeToken, const MAX: usize> Deserialize<'de> for NotificationTypes<K, MAX> {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let () = Self::BOUND_IS_THE_TOKENS;
        let tokens = bounded_vec::deserialize_csv::<K, D, 1, MAX>(d)?;
        let mut types: Vec<NotificationType> = Vec::with_capacity(tokens.len());
        for token in tokens {
            let t = token.into();
            if !types.contains(&t) {
                types.push(t);
            }
        }
        Ok(Self(types, PhantomData))
    }
}

impl<K: TypeToken, const MAX: usize> PartialSchema for NotificationTypes<K, MAX> {
    fn schema() -> RefOr<Schema> {
        // `example`, not `examples`: the key the derive wrote, and the frozen `/v0` document
        // carries.
        #[allow(deprecated)]
        ObjectBuilder::new()
            .schema_type(Type::String)
            .description(Some(K::DESCRIPTION))
            .example(Some(Value::String("reply,mention".to_string())))
            .into()
    }
}

impl<K: TypeToken, const MAX: usize> ToSchema for NotificationTypes<K, MAX> {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("NotificationTypes")
    }
}

/// `/v0`'s token: a type `/v0` has, through `FromStr`, case-insensitively on our side.
/// `event_changed` is no type `/v0` has, so it is refused with the error any unknown name gets.
pub struct StoredTypeToken(NotificationType);

impl TryFrom<String> for StoredTypeToken {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        let name = s.to_lowercase();
        match name.parse::<NotificationType>()? {
            NotificationType::EventChanged => Err(format!("unknown notification type: {name}")),
            t => Ok(Self(t)),
        }
    }
}

impl From<StoredTypeToken> for NotificationType {
    fn from(token: StoredTypeToken) -> Self {
        token.0
    }
}

impl TypeToken for StoredTypeToken {
    const MAX: usize = 12;
    const DESCRIPTION: &'static str = concatcp!(
        "Comma-separated list of notification types (min=1, max=",
        StoredTypeToken::MAX,
        " tokens; duplicates\nare dropped, order preserved). Unknown values are rejected."
    );
}

/// The `/v0` list: one to twelve of the types `/v0` has.
pub type StoredNotificationTypes = NotificationTypes<StoredTypeToken, { StoredTypeToken::MAX }>;

/// `/v1`'s token: every notification type, `event_changed` included.
pub struct V1TypeToken(NotificationType);

impl TryFrom<String> for V1TypeToken {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.to_lowercase().parse::<NotificationType>().map(Self)
    }
}

impl From<V1TypeToken> for NotificationType {
    fn from(token: V1TypeToken) -> Self {
        token.0
    }
}

impl TypeToken for V1TypeToken {
    const MAX: usize = 13;
    const DESCRIPTION: &'static str = concatcp!(
        "Comma-separated list of notification types (min=1, max=",
        V1TypeToken::MAX,
        " tokens; duplicates\nare dropped, order preserved). Unknown values are rejected."
    );
}

/// The `/v1` list: one to thirteen types.
pub type V1NotificationTypes = NotificationTypes<V1TypeToken, { V1TypeToken::MAX }>;

#[cfg(test)]
mod tests {
    use super::*;

    /// The twelve types `/v0` has, in the order of the stored enum.
    const V0_TAGS: [&str; 12] = [
        "follow",
        "new_friend",
        "lost_friend",
        "tag_post",
        "tag_profile",
        "untag_post",
        "untag_profile",
        "reply",
        "repost",
        "mention",
        "post_deleted",
        "post_edited",
    ];

    fn parse(input: &str) -> Result<StoredNotificationTypes, serde_json::Error> {
        serde_json::from_str(&format!("\"{input}\""))
    }

    fn parse_v1(input: &str) -> Result<V1NotificationTypes, serde_json::Error> {
        serde_json::from_str(&format!("\"{input}\""))
    }

    #[test]
    fn parses_and_dedupes() {
        let types = parse("reply, Mention,reply").unwrap();
        assert_eq!(
            types.0,
            vec![NotificationType::Reply, NotificationType::Mention]
        );
    }

    /// The schema is the one the derive wrote: a string, the description, the `example` key;
    /// `/v0`'s says twelve, as it always has, `/v1`'s thirteen.
    #[test]
    fn schema_is_the_derived_one() {
        for (schema, description) in [
            (
                StoredNotificationTypes::schema(),
                StoredTypeToken::DESCRIPTION,
            ),
            (V1NotificationTypes::schema(), V1TypeToken::DESCRIPTION),
        ] {
            assert_eq!(
                serde_json::to_value(schema).unwrap(),
                serde_json::json!({
                    "type": "string",
                    "description": description,
                    "example": "reply,mention"
                })
            );
        }
        assert!(StoredTypeToken::DESCRIPTION.contains("max=12 tokens"));
        assert!(V1TypeToken::DESCRIPTION.contains("max=13 tokens"));
        assert_eq!(StoredNotificationTypes::name(), "NotificationTypes");
        assert_eq!(V1NotificationTypes::name(), "NotificationTypes");
    }

    /// Every body type tag parses on `/v1`; on `/v0` every one but `event_changed`, which is
    /// refused as any unknown name is.
    #[test]
    fn every_body_type_tag_parses() {
        for tag in V0_TAGS {
            assert!(parse(tag).is_ok(), "{tag} should parse");
            assert!(parse_v1(tag).is_ok(), "{tag} should parse on v1");
        }
        assert!(parse_v1("event_changed").is_ok());
        let error = parse("event_changed").unwrap_err().to_string();
        assert!(
            error.contains("unknown notification type: event_changed"),
            "{error}"
        );
        assert!(parse("follow,event_changed").is_err());
    }

    /// `/v0` keeps its bound of twelve tokens, duplicates included; `/v1` takes thirteen.
    #[test]
    fn each_version_keeps_its_bound() {
        let twelve = V0_TAGS.join(",");
        assert_eq!(parse(&twelve).unwrap().0.len(), 12);
        assert!(parse(&format!("{twelve},follow")).is_err());
        let thirteen = format!("{twelve},event_changed");
        assert_eq!(parse_v1(&thirteen).unwrap().0.len(), 13);
        assert!(parse_v1(&format!("{thirteen},follow")).is_err());
    }

    #[test]
    fn rejects_unknown_and_empty() {
        for ok in [parse_v0_ok as fn(&str) -> bool, parse_v1_ok] {
            assert!(!ok("bogus"));
            assert!(!ok("reply,bogus"));
            assert!(!ok(","));
        }
    }

    fn parse_v0_ok(input: &str) -> bool {
        parse(input).is_ok()
    }

    fn parse_v1_ok(input: &str) -> bool {
        parse_v1(input).is_ok()
    }
}
