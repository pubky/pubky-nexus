//! A resource on `/v1`: its tags without `relationship`, in the view the resource stream
//! lists and in the envelope the resource tag routes serve, each with the counts of the live
//! posts replying to, embedding and collecting it, which `/v0`, frozen, does not serve.

use nexus_common::models::resource::view::{self, ResourceView as StoredResourceView};
use nexus_common::models::resource::ResourceDetails;
use serde::Serialize;
use utoipa::ToSchema;

use super::TagDetails;

/// A resource with a page of its tags, as `/v0`'s [`view::ResourceView`] with the tags
/// without `relationship`, and the counts of the live posts that reference it.
#[derive(Serialize, ToSchema, Debug)]
pub struct ResourceView {
    /// The average of the ratings the resource received, 1–10; 0 with none. `/v0`, frozen, does
    /// not serve it. Plain fields, not the flattened summary: the view's schema stays an object
    /// with its own properties.
    pub rating_avg: f64,
    /// How many ratings the resource received.
    pub rating_count: i64,
    pub details: ResourceDetails,
    pub tags: Vec<TagDetails>,
    pub taggers_count: usize,
    /// Live posts replying to the resource.
    pub replies_count: usize,
    /// Live posts embedding the resource.
    pub embeds_count: usize,
    /// Live collections listing the resource.
    pub collections_count: usize,
}

impl From<StoredResourceView> for ResourceView {
    fn from(view: StoredResourceView) -> Self {
        let view::ResourceView {
            rating,
            details,
            tags,
            taggers_count,
            replies_count,
            embeds_count,
            collections_count,
        } = view;
        Self {
            rating_avg: rating.rating_avg,
            rating_count: rating.rating_count,
            details,
            tags: tags.into_iter().map(TagDetails::from).collect(),
            taggers_count,
            replies_count,
            embeds_count,
            collections_count,
        }
    }
}

/// The envelope of `GET /resource/{resource_id}/tags` and `GET /resource/by-uri`, as `/v0`'s
/// [`ResourceTagsResponse`] with the tags without `relationship`, and the counts of the live
/// posts that reference the resource.
///
/// [`ResourceTagsResponse`]: crate::routes::v0::resource::ResourceTagsResponse
#[derive(Serialize, ToSchema, Debug)]
pub struct ResourceTagsResponse {
    /// The average of the ratings the resource received, 1–10; 0 with none. `/v0`, frozen, does
    /// not serve it. Plain fields, not the flattened summary: the view's schema stays an object
    /// with its own properties.
    pub rating_avg: f64,
    /// How many ratings the resource received.
    pub rating_count: i64,
    pub resource: ResourceDetails,
    pub tags: Vec<TagDetails>,
    /// Live posts replying to the resource.
    pub replies_count: usize,
    /// Live posts embedding the resource.
    pub embeds_count: usize,
    /// Live collections listing the resource.
    pub collections_count: usize,
}

impl From<StoredResourceView> for ResourceTagsResponse {
    fn from(view: StoredResourceView) -> Self {
        Self {
            rating_avg: view.rating.rating_avg,
            rating_count: view.rating.rating_count,
            resource: view.details,
            tags: view.tags.into_iter().map(TagDetails::from).collect(),
            replies_count: view.replies_count,
            embeds_count: view.embeds_count,
            collections_count: view.collections_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    /// Both shapes are object schemas whose own properties hold the rating summary and the
    /// counts: no `allOf`, so a client reads one object.
    #[test]
    fn schemas_hold_the_rating_as_their_own_properties() {
        use utoipa::PartialSchema;
        for schema in [ResourceView::schema(), ResourceTagsResponse::schema()] {
            let schema = serde_json::to_value(schema).unwrap();
            assert!(schema.get("allOf").is_none(), "{schema}");
            let properties = schema["properties"].as_object().expect("properties");
            for field in [
                "rating_avg",
                "rating_count",
                "replies_count",
                "embeds_count",
                "collections_count",
            ] {
                assert!(properties.contains_key(field), "{field}: {schema}");
            }
        }
    }

    fn stored() -> StoredResourceView {
        let mut view: StoredResourceView = serde_json::from_value(json!({
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
        .unwrap();
        // The counts are read from the graph, never from JSON: `/v0` does not serialize them.
        view.replies_count = 4;
        view.embeds_count = 5;
        view.collections_count = 6;
        view
    }

    /// The counts `/v1` adds and `/v0`, frozen, leaves out.
    const V1_COUNTS: [&str; 3] = ["replies_count", "embeds_count", "collections_count"];

    /// The rating summary `/v1` adds and `/v0`, frozen, leaves out.
    const V1_RATING: [&str; 2] = ["rating_avg", "rating_count"];

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

    /// The `/v1` view is the stored view with each tag's `relationship` taken out and the
    /// reference counts added, nothing else; `/v0` serializes no count.
    #[test]
    fn view_differs_from_v0_by_the_tags_relationship_and_the_counts() {
        let v0 = serde_json::to_value(stored()).unwrap();
        let mut v1 = serde_json::to_value(ResourceView::from(stored())).unwrap();
        assert_eq!(v0["tags"][0]["relationship"], true, "{v0}");
        for tag in v1["tags"].as_array().unwrap() {
            assert!(tag.get("relationship").is_none(), "{tag}");
            assert!(tag.get("tag_uri").is_some(), "{tag}");
        }
        assert_eq!(v1["taggers_count"], 3);
        for (count, expected) in V1_COUNTS.into_iter().zip([4, 5, 6]) {
            assert!(v0.get(count).is_none(), "{v0}");
            assert_eq!(v1[count], expected, "{v1}");
            v1.as_object_mut().unwrap().remove(count);
        }
        for field in V1_RATING {
            assert!(v0.get(field).is_none(), "{v0}");
            assert!(
                v1.as_object_mut().unwrap().remove(field).is_some(),
                "{field}"
            );
        }
        assert_eq!(v1, without_relationship(v0));
    }

    /// The `/v1` envelope is the stored view's details, its tags without `relationship` and
    /// the reference counts.
    #[test]
    fn envelope_carries_the_details_and_the_tags() {
        let v1 = serde_json::to_value(ResourceTagsResponse::from(stored())).unwrap();
        assert_eq!(v1["resource"]["id"], "450a72e3da164bfc3ac5f4056f9e5c7c");
        assert!(v1.get("taggers_count").is_none(), "{v1}");
        for (count, expected) in V1_COUNTS.into_iter().zip([4, 5, 6]) {
            assert_eq!(v1[count], expected, "{v1}");
        }
        let tags = v1["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0]["tag_uri"], "pubky://alice/pub/mapky/tags/T1");
        assert!(tags[0].get("relationship").is_none(), "{v1}");
        assert_eq!(tags[1]["tag_uri"], Value::Null);
    }
}
