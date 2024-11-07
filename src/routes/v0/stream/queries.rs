use crate::types::Pagination;
use crate::types::StreamSorting;
use crate::Error;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::Deserialize;
use std::fmt;
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

#[derive(Deserialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StreamSourceQuery {
    All,
    Following,
    Followers,
    Friends,
    Bookmarks,
    Replies, // 4U,
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

#[derive(ToSchema, Debug, Clone, PartialEq)]
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

/// Custom deserialization implementation for the `StreamSource` enum.
///
/// This implementation allows `StreamSource` to be deserialized from a map,
/// with specific handling based on the type of `StreamSource` specified in the input.
/// The `StreamSourceVisitor` is used to parse the map and extract relevant fields,
/// constructing a `StreamSource` variant based on the provided values.
///
/// This custom deserialization ensures that required fields are present and valid for
/// each `StreamSource` variant, and handles fields such as `author_id`, `observer_id`,
/// and optional `post_id` as appropriate. It also supports custom error messages for
/// missing or invalid fields.
///
/// # Type Parameters
/// - `'de`: The lifetime for deserialization, supporting deserialization from borrowed data.
/// - `D`: The deserializer type implementing the `Deserializer` trait.
///
/// # Parameters
/// - `deserializer`: The deserializer used to parse the map and construct the `StreamSource` instance.
///
/// # Returns
/// - `Result<StreamSource, D::Error>`: Returns an instance of `StreamSource` if deserialization succeeds,
///   or an error if the input data is missing required fields or contains invalid values.
///
/// # Errors
/// - Returns a deserialization error if the map does not contain necessary fields for the chosen
///   `StreamSource` variant or if field types are invalid.
/// ```
impl<'de> Deserialize<'de> for StreamSource {
    fn deserialize<D>(deserializer: D) -> Result<StreamSource, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(StreamSourceVisitor)
    }
}

struct StreamSourceVisitor;

impl<'de> Visitor<'de> for StreamSourceVisitor {
    type Value = StreamSource;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid StreamSource")
    }

    fn visit_map<M>(self, mut map: M) -> Result<StreamSource, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut source: Option<StreamSourceQuery> = None;
        let mut post_id: Option<String> = None;
        let mut author_id: Option<String> = None;
        let mut observer_id: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "source" => {
                    if source.is_some() {
                        return Err(de::Error::duplicate_field("source"));
                    }
                    source = Some(map.next_value::<StreamSourceQuery>()?);
                }
                "post_id" => {
                    post_id = Some(map.next_value()?);
                }
                "author_id" => {
                    author_id = Some(map.next_value()?);
                }
                "observer_id" => {
                    observer_id = Some(map.next_value()?);
                }
                _ => {
                    // Skip unknown fields
                    let _: de::IgnoredAny = map.next_value()?;
                }
            }
        }

        match source {
            Some(StreamSourceQuery::Replies) => {
                let author_id = author_id.ok_or_else(|| de::Error::missing_field("author_id"))?;
                Ok(StreamSource::Replies { post_id, author_id })
            }
            Some(StreamSourceQuery::Following) => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(StreamSource::Following { observer_id })
            }
            Some(StreamSourceQuery::Followers) => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(StreamSource::Followers { observer_id })
            }
            Some(StreamSourceQuery::Friends) => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(StreamSource::Friends { observer_id })
            }
            Some(StreamSourceQuery::Bookmarks) => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(StreamSource::Bookmarks { observer_id })
            }
            _ => Ok(StreamSource::All { author_id }),
        }
    }
}
