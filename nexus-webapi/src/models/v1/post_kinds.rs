//! The `exclude_kinds=` list on `/v1`: the `/v0` [`PostKinds`](crate::models::PostKinds) in
//! the `social/v1` names, `unknown` included.

use serde::Deserialize;
use utoipa::ToSchema;

use super::post::PostKind;
use crate::models::bounded_vec;
use nexus_common::models::post;

/// Comma-separated list of `/v1` post kinds (min=1, max=8 tokens; duplicates are dropped,
/// order preserved). Parsing is strict: a token that is not one of `note`, `article`, `image`,
/// `video`, `link`, `file`, `collection` or `unknown` is rejected, so a `/v0` name is a 400
/// rather than a filter that excludes nothing. `unknown` is a name here, unlike on `/v0`: a
/// client excludes the posts it cannot render with it.
#[derive(Clone, Debug, PartialEq, Eq, ToSchema)]
#[schema(value_type = String, example = "collection,link")]
pub struct PostKinds(pub Vec<PostKind>);

/// `deserialize_csv` requires `TryFrom<String>`; one token, with the parameter in the error.
struct KindToken(PostKind);

impl TryFrom<String> for KindToken {
    type Error = String;
    fn try_from(name: String) -> Result<Self, Self::Error> {
        PostKind::parse(&name)
            .map(KindToken)
            .map_err(|error| format!("`exclude_kinds`: {error}"))
    }
}

impl<'de> Deserialize<'de> for PostKinds {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let tokens = bounded_vec::deserialize_csv::<KindToken, D, 1, 8>(d)?;
        let mut kinds: Vec<PostKind> = Vec::with_capacity(tokens.len());
        for KindToken(kind) in tokens {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        Ok(Self(kinds))
    }
}

/// The stored kinds the list names, for the graph query.
impl From<PostKinds> for Vec<post::PostKind> {
    fn from(kinds: PostKinds) -> Self {
        kinds.0.into_iter().map(|kind| kind.0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::post::KIND_NAMES;
    use super::*;

    fn parse(input: &str) -> Result<PostKinds, serde_json::Error> {
        serde_json::from_str(&format!("\"{input}\""))
    }

    fn stored(kinds: PostKinds) -> Vec<post::PostKind> {
        kinds.into()
    }

    #[test]
    fn single_kind_under_its_v1_name() {
        assert_eq!(stored(parse("note").unwrap()), vec![post::PostKind::Short]);
        assert_eq!(
            stored(parse("article").unwrap()),
            vec![post::PostKind::Long]
        );
    }

    #[test]
    fn multiple_kinds_with_whitespace() {
        assert_eq!(
            stored(parse("collection, link").unwrap()),
            vec![post::PostKind::Collection, post::PostKind::Link]
        );
    }

    /// `unknown` is a kind a client may exclude on `/v1`.
    #[test]
    fn accepts_unknown() {
        assert_eq!(
            stored(parse("unknown").unwrap()),
            vec![post::PostKind::Unknown]
        );
    }

    /// Strict against the `/v1` names: a stored name, an unknown token and a differently cased
    /// name are rejected, naming the parameter and the names a client may use.
    #[test]
    fn rejects_v0_unknown_and_cased_names() {
        for input in ["short", "long", "bogus", "Collection", "note,short"] {
            let error = parse(input).expect_err(input).to_string();
            assert!(error.starts_with("`exclude_kinds`: "), "{input}: {error}");
            assert!(error.contains(KIND_NAMES), "{input}: {error}");
        }
    }

    #[test]
    fn deduplicates_preserving_order() {
        assert_eq!(
            stored(parse("collection,link,collection").unwrap()),
            vec![post::PostKind::Collection, post::PostKind::Link]
        );
    }

    #[test]
    fn rejects_empty() {
        assert!(parse("").is_err());
    }

    #[test]
    fn accepts_all_eight_names_and_rejects_a_ninth() {
        let all = KIND_NAMES.replace(' ', "");
        assert_eq!(parse(&all).unwrap().0.len(), 8);
        assert!(parse(&format!("{all},note")).is_err());
    }
}
