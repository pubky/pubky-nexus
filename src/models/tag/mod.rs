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
    pub fn split_fields<'a>(
        details: &'a [TagDetails]
    ) -> (Vec<&'a str>, Vec<Vec<&'a str>>) {
        let labels: Vec<&'a str> = details
            .iter()
            .map(|detail| detail.label.as_str()).collect();
        let taggers: Vec<Vec<&'a str>> = details
            .iter()
            .map(|detail| detail.taggers.iter().map(|s| s.as_str()).collect::<Vec<&'a str>>())
            .collect();
        (labels, taggers)
    }

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