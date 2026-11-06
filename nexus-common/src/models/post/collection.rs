use super::{PostCounts, PostDetails};
use crate::db::{fetch_key_from_graph, queries};
use crate::models::error::ModelResult;
use pubky_social_specs::legacy_v0::{self, PubkyAppCollectionContent};
use pubky_social_specs::{ParsedUri, PubkyId, PubkySocialCollectionContent, Resource};

/// Post keys `(author_id, post_id)` referenced by a Collection envelope, in
/// curator order, duplicates kept. The `social/v1` envelope (object items
/// `{uri, note}`) is read first, then the legacy one (string items); an item
/// that names a post in either epoch's address gives a key, anything else (a
/// profile, a file, a URL, another app's object, an unsupported epoch, a
/// malformed address) is dropped. Errors when `content` is neither envelope,
/// with the `social/v1` attempt's error.
pub fn collection_item_keys(content: &str) -> Result<Vec<(PubkyId, String)>, serde_json::Error> {
    let items: Vec<String> = match serde_json::from_str::<PubkySocialCollectionContent>(content) {
        Ok(envelope) => envelope.items.into_iter().map(|item| item.uri).collect(),
        Err(v1_error) => {
            serde_json::from_str::<PubkyAppCollectionContent>(content)
                .map_err(|_| v1_error)?
                .items
        }
    };
    Ok(items.iter().filter_map(|uri| post_key(uri)).collect())
}

/// The post `uri` names, read with the `social/v1` parser first, then the
/// legacy one; `None` for anything that is not a post of either epoch.
///
/// This classifies, it does not validate. The spec gate at ingest admits only
/// canonical public references into a stored collection (a private reference
/// inside a public post and a non-canonical spelling are refused there), so
/// the fallback never sees an address the PUT path would drop.
fn post_key(uri: &str) -> Option<(PubkyId, String)> {
    if let Ok(ParsedUri {
        user_id,
        resource: Resource::Post { id, .. },
        ..
    }) = ParsedUri::try_from(uri)
    {
        return Some((user_id, id));
    }
    match legacy_v0::ParsedUri::try_from(uri) {
        Ok(legacy_v0::ParsedUri {
            user_id,
            resource: legacy_v0::Resource::Post(id),
        }) => Some((user_id, id)),
        _ => None,
    }
}

/// Reconciles the COLLECTED edges of `author_id:post_id` with `items` and
/// invalidates the counts of every item the graph reports as touched.
/// Idempotent, so any retry path may call it freely. `derived_from` is the post
/// state `items` came from; see [`queries::put::sync_collection_items`].
pub async fn sync_collected_edges(
    author_id: &str,
    post_id: &str,
    items: &[(PubkyId, String)],
    derived_from: Option<&PostDetails>,
) -> ModelResult<()> {
    let query = queries::put::sync_collection_items(author_id, post_id, items, derived_from);
    // No row: the post is not in the graph, or it moved on since `items` were
    // parsed, so there is nothing to reconcile.
    let Some(touched) = fetch_key_from_graph::<Vec<Vec<String>>>(query, "touched").await? else {
        return Ok(());
    };
    let keys: Vec<Vec<&str>> = touched
        .iter()
        .map(|key| key.iter().map(String::as_str).collect())
        .collect();
    let keys: Vec<&[&str]> = keys.iter().map(Vec::as_slice).collect();
    PostCounts::invalidate_many(&keys).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::legacy_v0::post_uri_builder as legacy_post_uri_builder;
    use pubky_social_specs::{file_uri_builder, post_uri_builder, user_uri_builder};

    const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    const CAIRO: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";
    const POST_A: &str = "00000039YD9BM";
    const POST_C: &str = "00000039YD9C0";
    const HASH_ID: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";

    /// The legacy envelope.
    fn legacy_envelope(items: &[&str]) -> String {
        serde_json::json!({ "name": "n", "items": items }).to_string()
    }

    /// The `social/v1` envelope.
    fn v1_envelope(items: &[&str]) -> String {
        let items: Vec<_> = items
            .iter()
            .map(|uri| serde_json::json!({ "uri": uri, "note": "worth a read" }))
            .collect();
        serde_json::json!({ "name": "n", "items": items }).to_string()
    }

    fn key(user: &str, post_id: &str) -> (PubkyId, String) {
        (PubkyId::try_from(user).unwrap(), post_id.to_string())
    }

    fn v1_post(user: &str, post_id: &str) -> String {
        post_uri_builder(user.to_string(), post_id.to_string())
    }

    fn legacy_post(user: &str, post_id: &str) -> String {
        legacy_post_uri_builder(user.to_string(), post_id.to_string())
    }

    #[test]
    fn keeps_post_items_in_curator_order() {
        let content = legacy_envelope(&[&legacy_post(CAIRO, POST_C), &legacy_post(BOGOTA, POST_A)]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(CAIRO, POST_C), key(BOGOTA, POST_A)]);
    }

    #[test]
    fn drops_non_post_and_malformed_uris() {
        let content = legacy_envelope(&[
            &format!("pubky://{BOGOTA}/pub/pubky.app/profile.json"),
            "pubky://not-a-pubky-id/pub/pubky.app/posts/00000039YD9BM",
            &legacy_post(BOGOTA, POST_A),
        ]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(BOGOTA, POST_A)]);
    }

    #[test]
    fn errors_on_non_envelope_content() {
        assert!(collection_item_keys("just a short post").is_err());
    }

    /// An envelope neither shape reads: the error names the `social/v1` item the first
    /// attempt expected, not the legacy string.
    #[test]
    fn errors_with_the_v1_attempt_when_neither_envelope_parses() {
        let content = serde_json::json!({ "name": "n", "items": [1] }).to_string();
        let error = collection_item_keys(&content).unwrap_err().to_string();
        assert!(error.contains("PubkySocialCollectionItem"), "{error}");
    }

    #[test]
    fn v1_envelope_keeps_v1_post_items_in_curator_order() {
        let content = v1_envelope(&[&v1_post(CAIRO, POST_C), &v1_post(BOGOTA, POST_A)]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(CAIRO, POST_C), key(BOGOTA, POST_A)]);
    }

    #[test]
    fn v1_envelope_keeps_posts_of_either_epoch_in_curator_order() {
        let content = v1_envelope(&[&legacy_post(BOGOTA, POST_A), &v1_post(CAIRO, POST_C)]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(BOGOTA, POST_A), key(CAIRO, POST_C)]);
    }

    #[test]
    fn v1_envelope_keeps_only_the_posts() {
        let content = v1_envelope(&[
            &user_uri_builder(BOGOTA.to_string()),
            &v1_post(BOGOTA, POST_A),
            &file_uri_builder(BOGOTA.to_string(), format!("{HASH_ID}.png")),
            "https://example.com/article",
            &legacy_post(CAIRO, POST_C),
            &format!("pubky://{BOGOTA}/pub/other.app/v1/things/{POST_A}"),
            &format!("pubky://{BOGOTA}/pub/social/v2/posts/{POST_A}"),
        ]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(BOGOTA, POST_A), key(CAIRO, POST_C)]);
    }

    /// A `social/v1` post version address, plain or labelled, is keyed by its post, as the
    /// router keys it; `POST_C` stands in for the edit id, a later timestamp id.
    #[test]
    fn v1_envelope_keys_a_post_version_by_its_post() {
        let content = v1_envelope(&[
            &format!("pubky://{BOGOTA}/pub/social/v1/posts/{POST_A}/{POST_C}.json"),
            &format!("pubky://{BOGOTA}/pub/social/v1/posts/{POST_A}/{POST_C}-hello.json"),
        ]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(BOGOTA, POST_A); 2]);
    }

    #[test]
    fn v1_envelope_keeps_a_post_listed_twice_twice() {
        let content = v1_envelope(&[&v1_post(BOGOTA, POST_A), &v1_post(BOGOTA, POST_A)]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys, vec![key(BOGOTA, POST_A); 2]);
    }

    #[test]
    fn v1_envelope_without_items_gives_no_keys() {
        let keys = collection_item_keys(&v1_envelope(&[])).unwrap();
        assert!(keys.is_empty());
    }
}
