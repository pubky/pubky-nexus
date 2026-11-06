//! The `exclude_kinds=` list on `/v1`: the shared [`PostKinds`](crate::models::PostKinds) with
//! a `/v1` token, so it reads the `social/v1` names, `unknown` included, one to eight of them.

use super::post::PostKind;
use crate::models::post_kinds::KindToken;
use const_format::concatcp;
use nexus_common::models::post;

/// `/v1`'s token: a `/v1` name, `unknown` included, through [`PostKind::parse`], with the
/// parameter in the error.
pub struct V1KindToken(PostKind);

impl TryFrom<String> for V1KindToken {
    type Error = String;
    fn try_from(name: String) -> Result<Self, Self::Error> {
        PostKind::parse(&name)
            .map(V1KindToken)
            .map_err(|error| format!("`exclude_kinds`: {error}"))
    }
}

impl From<V1KindToken> for post::PostKind {
    fn from(token: V1KindToken) -> Self {
        token.0.into()
    }
}

impl KindToken for V1KindToken {
    const MAX: usize = 8;
    const DESCRIPTION: &'static str = concatcp!("Comma-separated list of `/v1` post kinds (min=1, max=", V1KindToken::MAX, " tokens; duplicates are dropped,\norder preserved). Parsing is strict: a token that is not one of `note`, `article`, `image`,\n`video`, `link`, `file`, `collection` or `unknown` is rejected, so a `/v0` name is a 400\nrather than a filter that excludes nothing. `unknown` is a name here, unlike on `/v0`: a\nclient excludes the posts it cannot render with it.");
}

/// The `/v1` list: one to eight `/v1` names, `unknown` among them.
pub type PostKinds = crate::models::PostKinds<V1KindToken, { V1KindToken::MAX }>;

#[cfg(test)]
mod tests {
    use super::super::post::KIND_NAMES;
    use super::*;
    use utoipa::{PartialSchema, ToSchema};

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

    /// Named as the `/v0` component is, with the `/v1` description.
    #[test]
    fn schema_carries_the_v1_description_under_the_v0_name() {
        let schema = serde_json::to_value(PostKinds::schema()).unwrap();
        assert_eq!(schema["description"], V1KindToken::DESCRIPTION);
        assert_eq!(schema["example"], "collection,link");
        assert_eq!(PostKinds::name(), "PostKinds");
    }
}
