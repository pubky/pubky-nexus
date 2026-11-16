use neo4rs::Row;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::fetch_row_from_graph;
use crate::db::queries;
use crate::models::error::ModelError;
use crate::models::error::ModelResult;

/// Represents a Pubky tag with uri, label, indexed at timestamp.
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct TagView {
    pub uri: String,
    pub label: String,
    pub indexed_at: i64,
}

impl TagView {
    /// Retrieves a user by ID, checking the cache first and then the graph database.
    pub async fn get_by_tagger_and_id(tagger_id: &str, tag_id: &str) -> ModelResult<Option<Self>> {
        let query = queries::get::get_tag_by_tagger_and_id(tagger_id, tag_id);
        let result = fetch_row_from_graph(query).await?;

        result.as_ref().map(Self::from_row).transpose()
    }

    /// Builds the view from a row of `get_tag_by_tagger_and_id`. The `uri` is the tagged
    /// object's: a post's or a resource's is required, a user's may be missing.
    fn from_row(row: &Row) -> ModelResult<Self> {
        let tagged_labels: Vec<String> = row.get("tagged_labels")?;
        let tagged_uri: Option<String> = row.get("tagged_uri")?;
        let uri = if tagged_labels.iter().any(|label| label == "Post") {
            tagged_uri.ok_or_else(|| ModelError::from_generic("Tagged post has no uri"))?
        } else if tagged_labels.iter().any(|label| label == "User") {
            tagged_uri.unwrap_or_default()
        } else if tagged_labels.iter().any(|label| label == "Resource") {
            tagged_uri.ok_or_else(|| ModelError::from_generic("Tagged resource has no uri"))?
        } else {
            return Err(ModelError::from_generic(format!(
                "Tagged resource has unsupported labels: {:?}",
                tagged_labels
            )));
        };

        Ok(Self {
            uri,
            label: row.get("label")?,
            indexed_at: row.get("indexed_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neo4rs::{BoltList, BoltType};

    /// A row of `get_tag_by_tagger_and_id` for a tag on a target with these labels and `uri`.
    fn tag_row(tagged_labels: &[&str], tagged_uri: Option<&str>) -> Row {
        let fields = ["tagged_labels", "tagged_uri", "id", "indexed_at", "label"];
        let data = vec![
            BoltType::from(tagged_labels.to_vec()),
            BoltType::from(tagged_uri),
            BoltType::from("RES_TAG_001"),
            BoltType::from(1_724_544_095_000_i64),
            BoltType::from("bitcoin"),
        ];
        Row::new(
            BoltList::from(fields.map(BoltType::from).to_vec()),
            BoltList::from(data),
        )
    }

    fn error_of(row: &Row) -> String {
        TagView::from_row(row)
            .expect_err("the row has no view")
            .to_string()
    }

    #[test]
    fn a_tag_on_a_resource_returns_the_resource_uri() {
        let row = tag_row(&["Resource"], Some("https://example.com/article"));
        let view = TagView::from_row(&row).unwrap();

        assert_eq!(view.uri, "https://example.com/article");
        assert_eq!(view.label, "bitcoin");
        assert_eq!(view.indexed_at, 1_724_544_095_000);
    }

    #[test]
    fn a_tag_on_a_resource_without_uri_is_an_error() {
        let row = tag_row(&["Resource"], None);
        assert_eq!(error_of(&row), "Generic: Tagged resource has no uri");
    }

    #[test]
    fn a_tag_on_a_post_without_uri_is_an_error() {
        let row = tag_row(&["Post"], None);
        assert_eq!(error_of(&row), "Generic: Tagged post has no uri");
    }

    #[test]
    fn a_tag_on_a_user_without_uri_returns_an_empty_uri() {
        let row = tag_row(&["User"], None);
        assert_eq!(TagView::from_row(&row).unwrap().uri, "");
    }

    #[test]
    fn a_tag_on_another_target_is_an_error() {
        let row = tag_row(&["File"], Some("pubky://somepk/pub/pubky.app/files/F001"));
        assert_eq!(
            error_of(&row),
            "Generic: Tagged resource has unsupported labels: [\"File\"]"
        );
    }
}
