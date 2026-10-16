use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

type TagFieldsTuple<'a> = (Vec<(f64, &'a str)>, (Vec<&'a str>, Vec<Vec<&'a str>>));

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Default)]
pub struct TagDetails {
    pub label: String,
    pub taggers: Vec<String>,
    pub taggers_count: usize,
    /// Whether the viewer tagged the label: exactly when `tag_uri` is set.
    #[serde(default)]
    pub relationship: bool,
    /// The address of the viewer's tag file for this label: the stored `uri`
    /// of their `TAGGED` edge. `None` if the viewer hasn't tagged the label,
    /// or there is no viewer.
    #[serde(default)]
    #[schema(
        example = "pubky://db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy/pub/pubky.app/tags/7T0R9X6A2E1L"
    )]
    pub tag_uri: Option<String>,
}

impl TagDetails {
    /// Creates a list of `TagDetails` from cached tag scores and taggers.
    /// # Arguments
    /// * `tag_scores` - A `Vec` of tuples where each tuple contains a tag label as a `String` and a score as an `f64`.
    /// * `taggers_list` - A `Vec` of `Option` tuples where each tuple contains a `Vec` of tagger identifiers as `String`, a `usize`
    ///   and whether the viewer is one of the taggers.
    ///
    /// # Returns
    ///
    /// Each `TagDetails` with no viewer's tag yet, paired with whether the viewer is one of its taggers;
    /// filtered to include only those where `taggers_list` contains `Some` data.
    pub fn from_index(
        tag_scores: Vec<(String, f64)>,
        taggers_list: Vec<Option<(Vec<String>, usize, bool)>>,
    ) -> Vec<(TagDetails, bool)> {
        tag_scores
            .into_iter()
            .zip(taggers_list)
            .filter_map(|((label, _), taggers)| {
                // TIP: MAP will not process None types and it will be automatically passed through unchanged
                taggers.map(|(taggers, taggers_count, is_viewer_tagger)| {
                    let tag = TagDetails {
                        label,
                        taggers,
                        taggers_count,
                        ..Default::default()
                    };
                    (tag, is_viewer_tagger)
                })
            })
            .collect()
    }

    /// Splits fields of `TagDetails` and calculates scores based on the number of taggers.
    /// # Arguments
    /// * `tag_details` - A reference to a slice of `TagDetails` instances.
    /// # Returns
    /// - A list of tag scores paired with their corresponding labels.
    /// - A list of labels and a corresponding list of lists containing tagger identifiers.
    ///
    pub fn process_tag_details<'a>(tag_details: &'a [TagDetails]) -> TagFieldsTuple<'a> {
        let mut tag_scores: Vec<(f64, &str)> = Vec::with_capacity(tag_details.len());
        let mut labels = Vec::with_capacity(tag_details.len());
        let mut taggers_id = Vec::with_capacity(tag_details.len());
        for tag in tag_details {
            let label: &str = &tag.label;
            let taggers = tag
                .taggers
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&'a str>>();
            tag_scores.push((tag.taggers.len() as f64, label));
            labels.push(tag.label.as_str());
            taggers_id.push(taggers);
        }
        (tag_scores, (labels, taggers_id))
    }
}
