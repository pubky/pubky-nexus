//! Tags on `/v1`: the details and the taggers without `relationship`, and the label parameter
//! and the label list folded by the `social/v1` rule.
//!
//! On `/v1`, `tag_uri` is the one signal that the viewer tagged something: it is the address
//! of their tag file when they did and `null` when they did not, so a flag that says the same
//! is left out. A label a `/v1` client queries is folded as the `social/v1` builder folds the
//! label it stores, with the spec crate's own functions: the frozen trim and the ASCII-only
//! lowercase, then its validator. `/v0` keeps the frozen `pubky.app` rule, Unicode lowercase
//! and all; the two only disagree on a non-ASCII uppercase letter. A `social/v1` label can
//! hold one: the spec crate's sanitizer lowercases ASCII only and keeps a non-Latin label's
//! case, so a client that publishes `Ärger` stores `Ärger`. Such a label is reachable on `/v1`
//! only, which folds a query as the store folded the label, the prefix search included: it
//! matches a `/v1` prefix as the fold left it, where `/v0` lowercases the whole prefix again
//! and so folds the same spelling to `ärger` and misses it, its frozen rule kept on purpose.
//! That is why neither version falls back to the other's fold on a miss.

use std::fmt;
use std::ops::Deref;

use nexus_common::models::tag::traits::taggers::TaggersTuple;
use nexus_common::models::tag::{self, Taggers};
use pubky_social_specs::{sanitize_tag_label, validate_tag_label};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Error;

/// A label with its taggers, as `/v0`'s [`tag::TagDetails`] without `relationship`.
#[derive(Serialize, ToSchema, Debug)]
pub struct TagDetails {
    pub label: String,
    pub taggers: Vec<String>,
    pub taggers_count: usize,
    /// The address of the viewer's tag file for this label: the stored `uri` of their
    /// `TAGGED` edge. `None` if the viewer hasn't tagged the label, or there is no viewer.
    #[schema(
        example = "pubky://db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy/pub/social/v1/tags/7T0R9X6A2E1L"
    )]
    pub tag_uri: Option<String>,
}

impl From<tag::TagDetails> for TagDetails {
    fn from(details: tag::TagDetails) -> Self {
        let tag::TagDetails {
            label,
            taggers,
            taggers_count,
            relationship: _,
            tag_uri,
        } = details;
        Self {
            label,
            taggers,
            taggers_count,
            tag_uri,
        }
    }
}

/// The taggers of one label, as `/v0`'s [`TaggersInfoResponse`] without `relationship`.
///
/// [`TaggersInfoResponse`]: crate::routes::v0::TaggersInfoResponse
#[derive(Serialize, ToSchema, Debug)]
pub struct TaggersInfoResponse {
    pub users: Taggers,
    /// The address of the viewer's tag file for this label: the stored `uri` of their
    /// `TAGGED` edge. `None` if the viewer hasn't tagged the label, or there is no viewer.
    #[schema(
        example = "pubky://db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy/pub/social/v1/tags/7T0R9X6A2E1L"
    )]
    pub tag_uri: Option<String>,
}

impl From<TaggersTuple> for TaggersInfoResponse {
    fn from((users, tag_uri): TaggersTuple) -> Self {
        Self { users, tag_uri }
    }
}

/// Tag label as `social/v1` stores it (1-20 code points, trimmed of the frozen whitespace set
/// and ASCII-lowercased; no commas, colons, or whitespace).
#[derive(Debug, Deserialize, ToSchema)]
#[schema(value_type = String, example = "rust")]
#[serde(try_from = "String")]
pub struct TagLabel(pub String);

/// Comma-separated list of `/v1` labels (min=1, max=5), as `tags=` and `domain_tags=` take
/// it: each label folded and validated by the `social/v1` rule.
pub type Tags = crate::models::Tags<TagLabel>;

impl fmt::Display for TagLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for TagLabel {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The label folded as the `social/v1` builder folds it, if the fold is a valid label; what a
/// request's label deserializes through.
impl TryFrom<String> for TagLabel {
    type Error = Error;

    fn try_from(label: String) -> Result<Self, Self::Error> {
        let folded = sanitize_tag_label(&label);
        validate_tag_label(&folded).map_err(Error::invalid_input)?;
        Ok(Self(folded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TagLabels;
    use serde_json::{json, Value};

    fn label(written: &str) -> Result<String, Error> {
        TagLabel::try_from(written.to_string()).map(|label| label.0)
    }

    /// The list as a query string carries it.
    fn list(csv: &str) -> Result<Tags, serde_json::Error> {
        serde_json::from_value(Value::String(csv.to_string()))
    }

    /// The `/v1` details are the stored details without `relationship` and nothing else.
    #[test]
    fn details_differ_from_v0_only_by_relationship() {
        let stored = tag::TagDetails {
            label: "rust".to_string(),
            taggers: vec!["alice".to_string()],
            taggers_count: 3,
            relationship: true,
            tag_uri: Some("pubky://alice/pub/social/v1/tags/T1".to_string()),
        };
        let mut v0 = serde_json::to_value(&stored).unwrap();
        let v1 = serde_json::to_value(TagDetails::from(stored)).unwrap();
        assert_eq!(v0["relationship"], true);
        assert!(v1.get("relationship").is_none(), "{v1}");
        assert_eq!(v1["tag_uri"], "pubky://alice/pub/social/v1/tags/T1");
        v0.as_object_mut().unwrap().remove("relationship");
        assert_eq!(v1, v0);

        // Untagged: `tag_uri` is `null`, present.
        let untagged = TagDetails::from(tag::TagDetails {
            label: "rust".to_string(),
            ..Default::default()
        });
        let untagged = serde_json::to_value(untagged).unwrap();
        assert_eq!(untagged["tag_uri"], Value::Null, "{untagged}");
    }

    /// The `/v1` taggers are the users and the viewer's tag address, nothing else.
    #[test]
    fn taggers_carry_users_and_tag_uri_only() {
        let tagged: TaggersTuple = (
            vec!["alice".to_string(), "bob".to_string()],
            Some("pubky://alice/pub/social/v1/tags/T1".to_string()),
        );
        assert_eq!(
            serde_json::to_value(TaggersInfoResponse::from(tagged)).unwrap(),
            json!({ "users": ["alice", "bob"], "tag_uri": "pubky://alice/pub/social/v1/tags/T1" })
        );
        let untagged: TaggersTuple = (vec!["bob".to_string()], None);
        assert_eq!(
            serde_json::to_value(TaggersInfoResponse::from(untagged)).unwrap(),
            json!({ "users": ["bob"], "tag_uri": null })
        );
    }

    /// Neither `/v1` schema has `relationship`; both have `tag_uri`, nullable.
    #[test]
    fn schemas_have_no_relationship() {
        for schema in [
            serde_json::to_value(<TagDetails as utoipa::PartialSchema>::schema()).unwrap(),
            serde_json::to_value(<TaggersInfoResponse as utoipa::PartialSchema>::schema()).unwrap(),
        ] {
            let properties = schema["properties"].as_object().unwrap();
            assert!(!properties.contains_key("relationship"), "{schema}");
            assert_eq!(
                properties["tag_uri"]["type"],
                json!(["string", "null"]),
                "{schema}"
            );
        }
    }

    /// The fold: the frozen whitespace set is trimmed, ASCII letters are lowercased.
    #[test]
    fn folds_frozen_whitespace_and_ascii_case() {
        assert_eq!(label("  Rust  ").unwrap(), "rust");
        assert_eq!(label("\u{00A0}RUST\t").unwrap(), "rust");
        assert_eq!(label("rust").unwrap(), "rust");
    }

    /// A non-ASCII uppercase letter is kept, where `/v0` lowercases it: the one place the
    /// rules disagree.
    #[test]
    fn keeps_non_ascii_case() {
        assert_eq!(label("Ärger").unwrap(), "Ärger");
        assert_eq!(
            crate::models::TagLabel::try_from("Ärger".to_string())
                .unwrap()
                .0,
            "ärger"
        );
    }

    #[test]
    fn rejects_empty_and_whitespace_only() {
        assert!(label("").is_err());
        assert!(label("   ").is_err());
    }

    /// The length is in code points, 20 at most.
    #[test]
    fn rejects_too_long_and_accepts_the_maximum() {
        assert!(label(&"a".repeat(21)).is_err());
        assert!(label(&"a".repeat(20)).is_ok());
        assert_eq!(label(&"é".repeat(20)).unwrap(), "é".repeat(20));
        assert!(label(&"é".repeat(21)).is_err());
    }

    /// Inner whitespace from the frozen set is rejected, an ASCII space and an em space alike.
    #[test]
    fn rejects_frozen_whitespace_inside() {
        assert!(label("co ol").is_err());
        assert!(label("co\tol").is_err());
        assert!(label("co\u{2003}ol").is_err());
        assert!(label("co\u{00A0}ol").is_err());
    }

    #[test]
    fn rejects_comma_and_colon() {
        assert!(label("tag,other").is_err());
        assert!(label("tag:other").is_err());
    }

    /// Reading a label from a request folds it the same way and rejects the same labels.
    #[test]
    fn deserializes_folded_and_rejects_invalid() {
        let folded: TagLabel = serde_json::from_str("\"  Rust  \"").unwrap();
        assert_eq!(folded.0, "rust");
        assert!(serde_json::from_str::<TagLabel>("\"a,b\"").is_err());
        assert!(serde_json::from_str::<TagLabel>("\"co ol\"").is_err());
    }

    #[test]
    fn displays_and_derefs_as_the_label() {
        let label = TagLabel("rust".to_string());
        assert_eq!(label.to_string(), "rust");
        assert_eq!(&*label, "rust");
    }

    /// The list folds each label by the `/v1` rule: the frozen whitespace trimmed, ASCII
    /// letters lowercased, a non-ASCII uppercase letter kept, where `/v0`'s list lowercases it.
    #[test]
    fn list_folds_each_label_by_the_v1_rule() {
        let labels = list(" Dev ,\u{00A0}FREE\t,Ärger").unwrap();
        assert_eq!(labels.to_string_vec(), ["dev", "free", "Ärger"]);
        let v0: crate::models::Tags<crate::models::TagLabel> =
            serde_json::from_value(Value::String(" Dev , FREE,Ärger".to_string())).unwrap();
        assert_eq!(v0.to_string_vec(), ["dev", "free", "ärger"]);
    }

    /// One invalid label rejects the whole list: too long once folded, a frozen whitespace
    /// character inside, a colon.
    #[test]
    fn list_rejects_an_invalid_label() {
        assert!(list("dev,co ol").is_err());
        assert!(list("dev,co\u{00A0}ol").is_err());
        assert!(list(&format!("dev,{}", "a".repeat(21))).is_err());
        assert!(list("dev,a:b").is_err());
        assert!(list("dev,,free").is_ok(), "an empty token is skipped");
    }
}
