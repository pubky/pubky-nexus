use super::PostCounts;
use crate::db::{fetch_key_from_graph, queries};
use crate::models::error::ModelResult;
use pubky_app_specs::{ParsedUri, PubkyAppCollectionContent, PubkyId, Resource};

/// Post keys `(author_id, post_id)` referenced by a Collection envelope, in
/// curator order. Malformed and non-post URIs are dropped. Errors when
/// `content` is not an envelope.
pub fn collection_item_keys(content: &str) -> Result<Vec<(PubkyId, String)>, serde_json::Error> {
    let envelope: PubkyAppCollectionContent = serde_json::from_str(content)?;
    Ok(envelope
        .items
        .iter()
        .filter_map(|uri| match ParsedUri::try_from(uri.as_str()) {
            Ok(parsed) => match parsed.resource {
                Resource::Post(post_id) => Some((parsed.user_id, post_id)),
                _ => None,
            },
            Err(_) => None,
        })
        .collect())
}

/// Reconciles the COLLECTED edges of `author_id:post_id` with `items` and
/// invalidates the counts of every item the graph reports as touched.
/// Idempotent, so any retry path may call it freely. `envelope` is the content
/// `items` came from; see [`queries::put::sync_collection_items`].
pub async fn sync_collected_edges(
    author_id: &str,
    post_id: &str,
    items: &[(PubkyId, String)],
    envelope: Option<&str>,
) -> ModelResult<()> {
    let query = queries::put::sync_collection_items(author_id, post_id, items, envelope);
    // No row: the post is not in the graph, or its content moved on since
    // `items` were parsed, so there is nothing to reconcile.
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

    const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    const CAIRO: &str = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";

    fn envelope(items: &[&str]) -> String {
        serde_json::json!({ "name": "n", "items": items }).to_string()
    }

    #[test]
    fn keeps_post_items_in_curator_order() {
        let content = envelope(&[
            &format!("pubky://{CAIRO}/pub/pubky.app/posts/00000039YD9C0"),
            &format!("pubky://{BOGOTA}/pub/pubky.app/posts/00000039YD9BM"),
        ]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys.len(), 2);
        assert_eq!(
            (keys[0].0.to_string(), keys[0].1.as_str()),
            (CAIRO.to_string(), "00000039YD9C0")
        );
        assert_eq!(
            (keys[1].0.to_string(), keys[1].1.as_str()),
            (BOGOTA.to_string(), "00000039YD9BM")
        );
    }

    #[test]
    fn drops_non_post_and_malformed_uris() {
        let content = envelope(&[
            &format!("pubky://{BOGOTA}/pub/pubky.app/profile.json"),
            "pubky://not-a-pubky-id/pub/pubky.app/posts/00000039YD9BM",
            &format!("pubky://{BOGOTA}/pub/pubky.app/posts/00000039YD9BM"),
        ]);
        let keys = collection_item_keys(&content).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].1, "00000039YD9BM");
    }

    #[test]
    fn errors_on_non_envelope_content() {
        assert!(collection_item_keys("just a short post").is_err());
    }
}
