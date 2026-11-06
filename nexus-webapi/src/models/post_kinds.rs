//! The `exclude_kinds=` list of the post streams, one type for both versions.
//!
//! `K` is the version's token, which parses one name by the version's rule, `/v0`'s
//! [`StoredKindToken`] or `/v1`'s, and `MAX` the version's bound. The list itself holds the
//! stored kinds the graph query takes, deduplicated in the order written, whichever names the
//! client wrote them under.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde_json::Value;
use utoipa::openapi::schema::{ObjectBuilder, Type};
use utoipa::openapi::{RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};

use crate::models::bounded_vec;
use nexus_common::models::post::PostKind;

/// One token of an `exclude_kinds=` list, as a version reads it. `deserialize_csv` requires
/// `TryFrom<String>`; the token names the stored kind it read, with the parameter in its error
/// where the version says it.
pub trait KindToken: TryFrom<String, Error = String> + Into<PostKind> {
    /// The list's schema description: the version's bound and names, spelled out.
    const DESCRIPTION: &'static str;
}

/// Comma-separated list of post kinds (min=1, max=`MAX` tokens; duplicates are dropped, order
/// preserved), each token parsed by `K`'s rule. Documented as one string schema named
/// `PostKinds` on both versions, by hand, with `K`'s description: the derive would name a
/// reference written with its arguments.
pub struct PostKinds<K, const MAX: usize>(pub Vec<PostKind>, PhantomData<K>);

impl<K, const MAX: usize> Clone for PostKinds<K, MAX> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<K, const MAX: usize> fmt::Debug for PostKinds<K, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PostKinds").field(&self.0).finish()
    }
}

/// The stored kinds the list names, for the graph query.
impl<K, const MAX: usize> From<PostKinds<K, MAX>> for Vec<PostKind> {
    fn from(kinds: PostKinds<K, MAX>) -> Self {
        kinds.0
    }
}

impl<'de, K: KindToken, const MAX: usize> Deserialize<'de> for PostKinds<K, MAX> {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let tokens = bounded_vec::deserialize_csv::<K, D, 1, MAX>(d)?;
        let mut kinds: Vec<PostKind> = Vec::with_capacity(tokens.len());
        for token in tokens {
            let kind = token.into();
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        Ok(Self(kinds, PhantomData))
    }
}

impl<K: KindToken, const MAX: usize> PartialSchema for PostKinds<K, MAX> {
    fn schema() -> RefOr<Schema> {
        // `example`, not `examples`: the key the derive wrote, and the frozen `/v0` document
        // carries.
        #[allow(deprecated)]
        ObjectBuilder::new()
            .schema_type(Type::String)
            .description(Some(K::DESCRIPTION))
            .example(Some(Value::String("collection,link".to_string())))
            .into()
    }
}

impl<K: KindToken, const MAX: usize> ToSchema for PostKinds<K, MAX> {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("PostKinds")
    }
}

/// `/v0`'s token: a stored name, case-insensitively on our side, through the strict
/// [`FromStr`](std::str::FromStr) of [`PostKind`], so `unknown` is rejected and exclusion lists
/// cannot target it.
pub struct StoredKindToken(PostKind);

impl TryFrom<String> for StoredKindToken {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.to_lowercase().parse::<PostKind>().map(Self)
    }
}

impl From<StoredKindToken> for PostKind {
    fn from(token: StoredKindToken) -> Self {
        token.0
    }
}

impl KindToken for StoredKindToken {
    const DESCRIPTION: &'static str = "Comma-separated list of post kinds (min=1, max=7 tokens; duplicates are\ndropped, order preserved). Parsing is strict: values outside the known\nkinds (short, long, image, video, link, file, collection) are rejected,\nunlike the lenient single `kind` param which falls back to `Unknown`.";
}

/// The `/v0` list: one to seven stored names.
pub type StoredPostKinds = PostKinds<StoredKindToken, 7>;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<StoredPostKinds, serde_json::Error> {
        serde_json::from_str(&format!("\"{input}\""))
    }

    #[test]
    fn single_kind() {
        let kinds = parse("collection").unwrap();
        assert_eq!(kinds.0, vec![PostKind::Collection]);
    }

    #[test]
    fn multiple_kinds_with_whitespace() {
        let kinds = parse("collection, link").unwrap();
        assert_eq!(kinds.0, vec![PostKind::Collection, PostKind::Link]);
    }

    #[test]
    fn case_insensitive() {
        let kinds = parse("Collection").unwrap();
        assert_eq!(kinds.0, vec![PostKind::Collection]);
    }

    #[test]
    fn rejects_unrecognized_value() {
        assert!(parse("bogus").is_err());
    }

    #[test]
    fn rejects_unknown_variant() {
        // `unknown` is the serde catch-all, not a real kind; the strict
        // parser must reject it so exclusion lists can't target it.
        assert!(parse("unknown").is_err());
    }

    #[test]
    fn deduplicates_preserving_order() {
        let kinds = parse("collection,link,collection").unwrap();
        assert_eq!(kinds.0, vec![PostKind::Collection, PostKind::Link]);
    }

    #[test]
    fn rejects_empty() {
        assert!(parse("").is_err());
    }

    #[test]
    fn rejects_over_max() {
        assert!(parse("short,long,image,video,link,file,collection,short").is_err());
    }

    #[test]
    fn accepts_all_seven_kinds() {
        let kinds = parse("short,long,image,video,link,file,collection").unwrap();
        assert_eq!(kinds.0.len(), 7);
    }

    /// The schema is the one the derive wrote for the `/v0` list: a string, the description,
    /// the `example` key.
    #[test]
    fn schema_is_the_derived_one() {
        let schema = serde_json::to_value(StoredPostKinds::schema()).unwrap();
        assert_eq!(
            schema,
            serde_json::json!({
                "type": "string",
                "description": StoredKindToken::DESCRIPTION,
                "example": "collection,link"
            })
        );
        assert_eq!(StoredPostKinds::name(), "PostKinds");
    }
}
