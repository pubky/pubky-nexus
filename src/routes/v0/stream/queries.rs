use crate::types::Pagination;
use crate::types::StreamSorting;
use crate::Error;
use serde::de::Deserializer;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema)]
pub struct PostStreamQuery {
    #[serde(flatten)]
    pub source: StreamSource,
    #[serde(flatten)]
    pub pagination: Pagination,
    pub sorting: Option<StreamSorting>,
    pub viewer_id: Option<String>,
    #[serde(flatten)]
    pub filters: Filters,
}

const MAX_TAGS: usize = 5;

#[derive(Deserialize, Debug, ToSchema)]
pub struct Filters {
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
}

impl Filters {
    /// Validates the number of tags to ensure it does not exceed the maximum allowed limit.
    ///
    /// # Errors
    /// Returns an error if the number of tags exceeds `MAX_TAGS`.
    pub fn validate_tags(&self) -> Result<(), Error> {
        if let Some(ref tags) = self.tags {
            if tags.len() > MAX_TAGS {
                return Err(Error::InvalidInput {
                    message: format!("Too many tags provided; maximum allowed is {}", MAX_TAGS),
                });
            }
        }
        Ok(())
    }
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(StreamSorting::Timeline);
    }

    /// Validates all constraints and requirements of `PostStreamQuery`.
    ///
    /// # Errors
    /// Returns an error if any constraints in `PostStreamQuery` are violated.
    pub fn validate(&mut self) -> Result<(), Error> {
        self.filters.validate_tags()?;
        Ok(())
    }
}

/// A custom deserializer for parsing an optional comma-separated string into a vector of strings.
///
/// This function is designed for use with Serde's `#[serde(deserialize_with)]` attribute.
/// It deserializes an optional string containing comma-separated values into an `Option<Vec<String>>`.
/// If the input is `None`, it returns `None`. If a string is provided, it calls `parse_comma_separated`
/// to perform the parsing.
///
/// # Type Parameters
/// - `D`: A deserializer that implements the `Deserializer` trait.
///
/// # Parameters
/// - `deserializer`: The deserializer used to retrieve and process the input string
fn deserialize_comma_separated<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    Ok(s.map(|s| parse_comma_separated(&s)))
}

/// Parses a comma-separated string into a vector of trimmed, non-empty strings.
///
/// This function splits the input string by commas, trims whitespace around each item,
/// and filters out any empty strings. It returns a vector containing the parsed items.
///
/// # Parameters
/// - `input`: A comma-separated string containing items to parse.
///
/// # Returns
/// - A `Vec<String>` containing the parsed and trimmed items.
fn parse_comma_separated(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[derive(ToSchema, Deserialize, Debug, Clone, PartialEq)]
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
    // Global stream or author related stream
    All {
        author_id: Option<String>,
    },
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
            StreamSource::All {
                author_id: Some(author_id),
            } => Some(author_id),
            _ => None,
        }
    }
}
