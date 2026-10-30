use neo4rs::Row;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ResourceDetails;
use crate::db::{fetch_row_from_graph, queries};
use crate::models::error::ModelResult;
use crate::models::post::Bookmark;
use crate::models::tag::traits::collection::tag_page;
use crate::models::tag::traits::tag_details_from_row;
use crate::models::tag::TagDetails;

#[derive(Debug, Serialize, Deserialize, Default, ToSchema)]
pub struct ResourceView {
    #[serde(flatten, default)]
    pub rating: crate::models::social_event::RatingSummary,
    pub details: ResourceDetails,
    pub tags: Vec<TagDetails>,
    pub taggers_count: usize,
    pub replies_count: usize,
    pub embeds_count: usize,
    pub collections_count: usize,
    /// The viewer's bookmark of this resource; `None` without `viewer_id` or a bookmark.
    #[serde(default)]
    pub bookmark: Option<Bookmark>,
}

impl ResourceView {
    /// A resource's details and a page of its tags, in one graph read. The tags
    /// are paged by `tag_page`, as for user and post tags. Returns `None` only
    /// when the resource does not exist.
    pub async fn get_by_id(
        resource_id: &str,
        skip_tags: Option<usize>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        viewer_id: Option<&str>,
    ) -> ModelResult<Option<Self>> {
        let (skip_tags, limit_tags, limit_taggers) = tag_page(skip_tags, limit_tags, limit_taggers);
        let query = queries::get::resource_view(
            resource_id,
            viewer_id,
            skip_tags,
            limit_tags,
            limit_taggers,
        );
        match fetch_row_from_graph(query).await? {
            Some(row) => Self::from_row(&row),
            None => Ok(None),
        }
    }

    /// A `resource_view` row as a view: `None` when `details` is null, the
    /// resource being missing. `taggers_count` sums the listed labels' tagger
    /// counts.
    fn from_row(row: &Row) -> ModelResult<Option<Self>> {
        let Some(details) = row.get::<Option<ResourceDetails>>("details")? else {
            return Ok(None);
        };
        let tags = tag_details_from_row(row)?;
        let taggers_count = tags.iter().map(|tag| tag.taggers_count).sum();
        Ok(Some(Self {
            rating: crate::models::social_event::RatingSummary {
                rating_avg: row.get("rating_avg").unwrap_or(0.0),
                rating_count: row.get("rating_count").unwrap_or(0),
            },
            details,
            tags,
            taggers_count,
            replies_count: row.get::<i64>("replies_count").unwrap_or(0) as usize,
            embeds_count: row.get::<i64>("embeds_count").unwrap_or(0) as usize,
            collections_count: row.get::<i64>("collections_count").unwrap_or(0) as usize,
            bookmark: row.get::<Option<Bookmark>>("bookmark").unwrap_or(None),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neo4rs::{BoltList, BoltNull, BoltType};
    use std::collections::HashMap;

    fn view_row(details: BoltType, tags: Vec<BoltType>) -> Row {
        let fields = BoltList::from(vec![BoltType::from("details"), BoltType::from("tags")]);
        let data = BoltList::from(vec![details, BoltType::from(tags)]);
        Row::new(fields, data)
    }

    /// A tag entry: `taggers` is the capped preview, `taggers_count` the full count.
    fn tag(label: &str, taggers: &[&str], taggers_count: i64, tag_uri: Option<&str>) -> BoltType {
        BoltType::from(HashMap::from([
            ("label", BoltType::from(label)),
            ("taggers", BoltType::from(taggers.to_vec())),
            ("taggers_count", BoltType::from(taggers_count)),
            ("tag_uri", BoltType::from(tag_uri)),
        ]))
    }

    #[test]
    fn a_missing_resource_has_no_view() {
        let row = view_row(BoltType::Null(BoltNull), Vec::new());
        assert!(ResourceView::from_row(&row).unwrap().is_none());
    }

    #[test]
    fn a_view_sums_the_listed_tagger_counts_and_flags_the_viewer() {
        let details = BoltType::from(HashMap::from([
            ("id", BoltType::from("450a72e3da164bfc3ac5f4056f9e5c7c")),
            ("uri", BoltType::from("https://example.com/article")),
            ("scheme", BoltType::from("https")),
            ("indexed_at", BoltType::from(1_724_134_095_000_i64)),
        ]));
        let row = view_row(
            details,
            vec![
                // The preview lists one of bitcoin's two taggers
                tag(
                    "bitcoin",
                    &["alice"],
                    2,
                    Some("pubky://alice/pub/mapky/tags/T1"),
                ),
                tag("interesting", &["alice"], 1, None),
            ],
        );

        let view = ResourceView::from_row(&row)
            .unwrap()
            .expect("the resource exists");

        assert_eq!(view.details.uri, "https://example.com/article");
        assert_eq!(view.taggers_count, 3);
        let flags: Vec<bool> = view.tags.iter().map(|tag| tag.relationship).collect();
        assert_eq!(flags, [true, false]);
    }
}
