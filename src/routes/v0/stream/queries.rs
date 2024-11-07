use crate::types::Pagination;
use crate::types::StreamSorting;
use serde::de::Deserializer;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema)]
pub struct PostStreamQuery {
    #[serde(flatten, default)]
    pub source: Option<StreamSource>,
    #[serde(flatten)]
    pub pagination: Pagination,
    pub sorting: Option<StreamSorting>,
    pub viewer_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(StreamSorting::Timeline);
    }
}

// Custom deserializer for comma-separated tags
fn deserialize_comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    if let Some(s) = s {
        // Split by comma and trim any excess whitespace
        let tags: Vec<String> = s.split(',').map(|tag| tag.trim().to_string()).collect();
        return Ok(Some(tags));
    }
    Ok(None)
}

#[derive(ToSchema, Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum StreamSource {
    // Author replies or post replies related streams
    Replies {
        post_id: Option<String>,
        author_id: String,
    },
    Following {
        observer_id: String,
    },
    Followers {
        observer_id: String,
    },
    Friends {
        observer_id: String,
    },
    Bookmarks {
        observer_id: String,
    },
    Author {
        author_id: Option<String>,
    },
    #[default]
    All,
}

impl StreamSource {
    pub fn has_observer(&self) -> Option<&String> {
        match self {
            StreamSource::Followers { observer_id }
            | StreamSource::Following { observer_id }
            | StreamSource::Friends { observer_id }
            | StreamSource::Bookmarks { observer_id } => Some(observer_id),
            _ => None,
        }
    }

    pub fn has_author(&self) -> Option<&String> {
        match self {
            StreamSource::Replies {
                author_id,
                post_id: _,
            } => Some(author_id),
            StreamSource::Author {
                author_id: Some(author_id),
            } => Some(author_id),
            _ => None,
        }
    }
}
