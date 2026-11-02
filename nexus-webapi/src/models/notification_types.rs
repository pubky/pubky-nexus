use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::bounded_vec;
use nexus_common::models::notification::NotificationType;

/// Comma-separated list of notification types (min=1, max=12 tokens; duplicates
/// are dropped, order preserved). Unknown values are rejected.
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "reply,mention")]
pub struct NotificationTypes(pub Vec<NotificationType>);

/// `deserialize_csv` requires `TryFrom<String>`; delegate to `FromStr`.
struct TypeToken(NotificationType);

impl TryFrom<String> for TypeToken {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.to_lowercase().parse::<NotificationType>().map(TypeToken)
    }
}

impl<'de> Deserialize<'de> for NotificationTypes {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let tokens = bounded_vec::deserialize_csv::<TypeToken, D, 1, 12>(d)?;
        let mut types: Vec<NotificationType> = Vec::with_capacity(tokens.len());
        for TypeToken(t) in tokens {
            if !types.contains(&t) {
                types.push(t);
            }
        }
        Ok(Self(types))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<NotificationTypes, serde_json::Error> {
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

    #[test]
    fn every_body_type_tag_parses() {
        for tag in [
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
        ] {
            assert!(parse(tag).is_ok(), "{tag} should parse");
        }
    }

    #[test]
    fn rejects_unknown_and_empty() {
        assert!(parse("bogus").is_err());
        assert!(parse("reply,bogus").is_err());
        assert!(parse(",").is_err());
    }
}
