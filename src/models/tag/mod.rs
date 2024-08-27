use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod global;
pub mod post;
pub mod stream;
pub mod user;

// Atomic struct to save in the cache
#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
pub struct Tag {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: i64,
    tagger_id: String,
}

impl Default for Tag {
    fn default() -> Self {
        Self {
            tag_id: String::new(),
            indexed_at: Utc::now().timestamp(),
            tagger_id: String::new(),
        }
    }
}


/// Represents a tag that refers to the current user
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Default)]
pub struct TagDetails {
    pub label: String,
    taggers: Vec<String>,
}

impl TagDetails {

    /// Creates a list of `TagDetails` from cached tag scores and taggers.
    /// # Arguments
    /// * `tag_scores` - A `Vec` of tuples where each tuple contains a tag label as a `String` and a score as an `f64`.
    /// * `taggers_list` - A `Vec` of `Option` tuples where each tuple contains a `Vec` of tagger identifiers as `String` and a `usize`.
    ///
    /// # Returns
    ///
    /// A `Vec` of `TagDetails` instances, filtered to include only those where `taggers_list` contains `Some` data.
    pub fn from_cache(tag_scores: Vec<(String, f64)>, taggers_list: Vec<Option<(Vec<String>, usize)>>) -> Vec<TagDetails> {
        let tag_details_list = tag_scores
            .into_iter()
            .zip(taggers_list)
            .filter_map(|((label, _), taggers)| match taggers {
                Some((taggers, _)) => {
                    Some(TagDetails { label, taggers })
                }
                None => None,
            })
            .collect();
        tag_details_list
    }

    /// Splits fields of `TagDetails` and calculates scores based on the number of taggers.
    /// # Arguments
    /// * `tag_details` - A reference to a slice of `TagDetails` instances.
    /// # Returns
    /// - A list of tag scores paired with their corresponding labels.
    /// - A list of labels and a corresponding list of lists containing tagger identifiers.
    ///
    pub fn split_fields_and_calculate_scores<'a>(
        tag_details: &'a [TagDetails]
    ) -> (Vec<(f64, &'a str)>, (Vec<&'a str>, Vec<Vec<&'a str>>)) {
        let mut tag_scores: Vec<(f64, &str)> = Vec::with_capacity(tag_details.len());
        let mut labels = Vec::with_capacity(tag_details.len());
        let mut taggers_id = Vec::with_capacity(tag_details.len());
        for tag in tag_details {
            let label: &str = &tag.label;
            let taggers = tag.taggers.iter().map(|s| s.as_str()).collect::<Vec<&'a str>>();
            tag_scores.push((tag.taggers.len() as f64, label));
            labels.push(tag.label.as_str());
            taggers_id.push(taggers);
        }
        (tag_scores, (labels, taggers_id))  
    }
}