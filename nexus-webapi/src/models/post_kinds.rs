use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::bounded_vec;
use pubky_app_specs::PubkyAppPostKind;

/// Comma-separated list of post kinds (min=1, max=7 tokens; duplicates are
/// dropped, order preserved). Parsing is strict: values outside the known
/// kinds (short, long, image, video, link, file, collection) are rejected,
/// unlike the lenient single `kind` param which falls back to `Unknown`.
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "collection,link")]
pub struct PostKinds(pub Vec<PubkyAppPostKind>);

/// `deserialize_csv` requires `TryFrom<String>`; delegate to the strict
/// `FromStr` of the specs enum (case-insensitive on our side).
struct KindToken(PubkyAppPostKind);

impl TryFrom<String> for KindToken {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.to_lowercase().parse::<PubkyAppPostKind>().map(KindToken)
    }
}

impl<'de> Deserialize<'de> for PostKinds {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let tokens = bounded_vec::deserialize_csv::<KindToken, D, 1, 7>(d)?;
        let mut kinds: Vec<PubkyAppPostKind> = Vec::with_capacity(tokens.len());
        for KindToken(kind) in tokens {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
        }
        Ok(Self(kinds))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<PostKinds, serde_json::Error> {
        serde_json::from_str(&format!("\"{input}\""))
    }

    #[test]
    fn single_kind() {
        let kinds = parse("collection").unwrap();
        assert_eq!(kinds.0, vec![PubkyAppPostKind::Collection]);
    }

    #[test]
    fn multiple_kinds_with_whitespace() {
        let kinds = parse("collection, link").unwrap();
        assert_eq!(
            kinds.0,
            vec![PubkyAppPostKind::Collection, PubkyAppPostKind::Link]
        );
    }

    #[test]
    fn case_insensitive() {
        let kinds = parse("Collection").unwrap();
        assert_eq!(kinds.0, vec![PubkyAppPostKind::Collection]);
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
        assert_eq!(
            kinds.0,
            vec![PubkyAppPostKind::Collection, PubkyAppPostKind::Link]
        );
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
}
