use super::{PostCounts, PostDetails};
use crate::db::{fetch_key_from_graph, queries};
use crate::models::error::ModelResult;
use crate::universal_tag::normalize::{normalize_uri, resource_id};
use pubky_social_specs::legacy_v0::{ParsedUri, PubkyAppCollectionContent, Resource};
use pubky_social_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Most items one page of [`collection_items_page`] returns.
pub const COLLECTION_ITEMS_MAX_PAGE: usize = 100;

/// The `(uri, note)` items of a v0 or social v1 Collection envelope, in curator order. Errors
/// when `content` is not an envelope. A v0 envelope has no notes.
fn envelope_items(content: &str) -> Result<Vec<(String, Option<String>)>, serde_json::Error> {
    match serde_json::from_str::<pubky_social_specs::PubkySocialCollectionContent>(content) {
        Ok(envelope) => Ok(envelope
            .items
            .into_iter()
            .map(|item| (item.uri, item.note))
            .collect()),
        Err(_) => Ok(serde_json::from_str::<PubkyAppCollectionContent>(content)?
            .items
            .into_iter()
            .map(|uri| (uri, None))
            .collect()),
    }
}

/// The post a collection item names, from a social v1 or a v0 post address.
fn post_key(uri: &str) -> Option<(PubkyId, String)> {
    if let Ok(parsed) = pubky_social_specs::ParsedUri::try_from(uri) {
        if let pubky_social_specs::Resource::Post { id, .. } = parsed.resource {
            return Some((parsed.user_id, id));
        }
    }
    match ParsedUri::try_from(uri) {
        Ok(parsed) => match parsed.resource {
            Resource::Post(post_id) => Some((parsed.user_id, post_id)),
            _ => None,
        },
        Err(_) => None,
    }
}

/// Post keys `(author_id, post_id)` referenced by a v0 or social v1 Collection envelope, in
/// curator order. Malformed and non-post URIs are dropped. Errors when
/// `content` is not an envelope.
pub fn collection_item_keys(content: &str) -> Result<Vec<(PubkyId, String)>, serde_json::Error> {
    Ok(envelope_items(content)?
        .iter()
        .filter_map(|(uri, _)| post_key(uri))
        .collect())
}

/// One item of a Collection, as the curator listed it.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, PartialEq)]
pub struct CollectionItem {
    /// 0-based place in the curator's order.
    pub position: usize,
    /// The address exactly as the curator wrote it.
    pub uri: String,
    /// The curator's note on the item (social v1 only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// `author_id:post_id` when the item is a Pubky post.
    pub post_key: Option<String>,
    /// The Resource id when the item is any other address (an external resource), the id
    /// `/v0/resource/{resource_id}` and the `COLLECTED` edge use. Null for a post or an
    /// address that does not parse.
    pub resource_id: Option<String>,
}

/// A page of a Collection's items with the cursor of the next page.
#[derive(Serialize, Deserialize, ToSchema, Debug, Default, PartialEq)]
pub struct CollectionItemsPage {
    pub items: Vec<CollectionItem>,
    /// Pass back as `cursor` for the next page; null on the last page.
    pub next_cursor: Option<String>,
}

/// The items of a Collection envelope from position `cursor`, at most `limit` (capped at
/// [`COLLECTION_ITEMS_MAX_PAGE`]) of them, posts and external resources alike. The page size is
/// bounded and the cursor has no ceiling, so a Collection of any length can be read to its end.
/// Errors when `content` is not an envelope.
pub fn collection_items_page(
    content: &str,
    cursor: usize,
    limit: usize,
) -> Result<CollectionItemsPage, serde_json::Error> {
    let items = envelope_items(content)?;
    let limit = limit.clamp(1, COLLECTION_ITEMS_MAX_PAGE);
    let end = cursor.saturating_add(limit);
    let next_cursor = (end < items.len()).then(|| end.to_string());
    let page = items
        .into_iter()
        .enumerate()
        .skip(cursor)
        .take(limit)
        .map(|(position, (uri, note))| {
            let post = post_key(&uri).map(|(author, id)| format!("{author}:{id}"));
            let resource = match post {
                Some(_) => None,
                None => normalize_uri(&uri)
                    .ok()
                    .map(|(normalized, _)| resource_id(&normalized)),
            };
            CollectionItem {
                position,
                uri,
                note,
                post_key: post,
                resource_id: resource,
            }
        })
        .collect();
    Ok(CollectionItemsPage {
        items: page,
        next_cursor,
    })
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

    /// A Collection over the spec's current 100-item limit pages to its end: a bounded page,
    /// a cursor with no ceiling, posts and external resources told apart.
    #[test]
    fn pages_a_long_mixed_collection_to_its_end() {
        let uris: Vec<String> = (0..250)
            .map(|n| match n % 2 {
                0 => format!("pubky://{BOGOTA}/pub/pubky.app/posts/{:013}", n),
                _ => format!("https://example.com/books/{n}"),
            })
            .collect();
        let content = envelope(&uris.iter().map(String::as_str).collect::<Vec<_>>());
        let mut cursor = 0;
        let mut seen = vec![];
        loop {
            let page = collection_items_page(&content, cursor, 100).unwrap();
            assert!(page.items.len() <= COLLECTION_ITEMS_MAX_PAGE);
            seen.extend(page.items);
            match page.next_cursor {
                Some(next) => cursor = next.parse().unwrap(),
                None => break,
            }
        }
        assert_eq!(seen.len(), 250);
        assert!(seen
            .iter()
            .enumerate()
            .all(|(n, item)| item.position == n && item.uri == uris[n]));
        assert_eq!(
            seen[0].post_key.as_deref(),
            Some(format!("{BOGOTA}:0000000000000").as_str())
        );
        assert_eq!(seen[0].resource_id, None);
        assert_eq!(seen[1].post_key, None);
        assert_eq!(
            seen[1].resource_id.as_deref(),
            Some(resource_id(&normalize_uri("https://example.com/books/1").unwrap().0).as_str())
        );
    }

    #[test]
    fn a_page_is_bounded_and_ends_without_a_cursor() {
        let uris: Vec<String> = (0..5).map(|n| format!("https://example.com/{n}")).collect();
        let content = envelope(&uris.iter().map(String::as_str).collect::<Vec<_>>());
        let page = collection_items_page(&content, 0, 2).unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.next_cursor.as_deref(), Some("2"));
        let page = collection_items_page(&content, 4, 2).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.next_cursor, None);
        let page = collection_items_page(&content, 9, 2).unwrap();
        assert!(page.items.is_empty() && page.next_cursor.is_none());
        // A limit over the page cap is clamped, not refused.
        let content = envelope(&vec!["https://example.com/x"; 150]);
        let page = collection_items_page(&content, 0, 1000).unwrap();
        assert_eq!(page.items.len(), COLLECTION_ITEMS_MAX_PAGE);
        assert_eq!(page.next_cursor.as_deref(), Some("100"));
    }

    #[test]
    fn notes_come_from_a_social_v1_envelope() {
        let content = serde_json::json!({
            "name": "n",
            "items": [{"uri": "https://example.com/a", "note": "read first"}, {"uri": "https://example.com/b"}]
        })
        .to_string();
        let page = collection_items_page(&content, 0, 10).unwrap();
        assert_eq!(page.items[0].note.as_deref(), Some("read first"));
        assert_eq!(page.items[1].note, None);
    }

    #[test]
    fn errors_on_non_envelope_content() {
        assert!(collection_item_keys("just a short post").is_err());
    }
}
