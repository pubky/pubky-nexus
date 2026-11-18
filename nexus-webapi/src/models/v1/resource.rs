//! A resource on `/v1`: its tags without `relationship`, in the view the resource stream
//! lists and in the envelope the resource tag routes serve.

use nexus_common::models::resource::view::{self, ResourceView as StoredResourceView};
use nexus_common::models::resource::ResourceDetails;
use serde::Serialize;
use utoipa::ToSchema;

use super::TagDetails;

/// A resource with a page of its tags, as `/v0`'s [`view::ResourceView`] with the tags
/// without `relationship`.
#[derive(Serialize, ToSchema, Debug)]
pub struct ResourceView {
    pub details: ResourceDetails,
    pub tags: Vec<TagDetails>,
    pub taggers_count: usize,
}

impl From<StoredResourceView> for ResourceView {
    fn from(view: StoredResourceView) -> Self {
        let view::ResourceView {
            details,
            tags,
            taggers_count,
        } = view;
        Self {
            details,
            tags: tags.into_iter().map(TagDetails::from).collect(),
            taggers_count,
        }
    }
}

/// The envelope of `GET /resource/{resource_id}/tags` and `GET /resource/by-uri`, as `/v0`'s
/// [`ResourceTagsResponse`] with the tags without `relationship`.
///
/// [`ResourceTagsResponse`]: crate::routes::v0::resource::ResourceTagsResponse
#[derive(Serialize, ToSchema, Debug)]
pub struct ResourceTagsResponse {
    pub resource: ResourceDetails,
    pub tags: Vec<TagDetails>,
}

impl From<StoredResourceView> for ResourceTagsResponse {
    fn from(view: StoredResourceView) -> Self {
        Self {
            resource: view.details,
            tags: view.tags.into_iter().map(TagDetails::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn stored() -> StoredResourceView {
        serde_json::from_value(json!({
            "details": {
                "id": "450a72e3da164bfc3ac5f4056f9e5c7c",
                "uri": "https://example.com/article",
                "scheme": "https",
                "indexed_at": 1_724_134_095_000_i64
            },
            "tags": [
                { "label": "bitcoin", "taggers": ["alice"], "taggers_count": 2, "relationship": true,
                  "tag_uri": "pubky://alice/pub/mapky/tags/T1" },
                { "label": "interesting", "taggers": ["alice"], "taggers_count": 1, "relationship": false,
                  "tag_uri": null }
            ],
            "taggers_count": 3
        }))
        .unwrap()
    }

    /// `body` without every boolean `relationship`, the flag the `/v1` tags leave out.
    fn without_relationship(body: Value) -> Value {
        match body {
            Value::Object(fields) => Value::Object(
                fields
                    .into_iter()
                    .filter(|(key, value)| !(key == "relationship" && value.is_boolean()))
                    .map(|(key, value)| (key, without_relationship(value)))
                    .collect(),
            ),
            Value::Array(items) => {
                Value::Array(items.into_iter().map(without_relationship).collect())
            }
            other => other,
        }
    }

    /// The `/v1` view is the stored view with each tag's `relationship` taken out, nothing else.
    #[test]
    fn view_differs_from_v0_only_by_the_tags_relationship() {
        let v0 = serde_json::to_value(stored()).unwrap();
        let v1 = serde_json::to_value(ResourceView::from(stored())).unwrap();
        assert_eq!(v0["tags"][0]["relationship"], true, "{v0}");
        for tag in v1["tags"].as_array().unwrap() {
            assert!(tag.get("relationship").is_none(), "{tag}");
            assert!(tag.get("tag_uri").is_some(), "{tag}");
        }
        assert_eq!(v1["taggers_count"], 3);
        assert_eq!(v1, without_relationship(v0));
    }

    /// The `/v1` envelope is the stored view's details and its tags without `relationship`.
    #[test]
    fn envelope_carries_the_details_and_the_tags() {
        let v1 = serde_json::to_value(ResourceTagsResponse::from(stored())).unwrap();
        assert_eq!(v1["resource"]["id"], "450a72e3da164bfc3ac5f4056f9e5c7c");
        assert!(v1.get("taggers_count").is_none(), "{v1}");
        let tags = v1["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0]["tag_uri"], "pubky://alice/pub/mapky/tags/T1");
        assert!(tags[0].get("relationship").is_none(), "{v1}");
        assert_eq!(tags[1]["tag_uri"], Value::Null);
    }
}
