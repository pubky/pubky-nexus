use crate::routes::v0::queries::PaginationQuery;
use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PostStreamSorting {
    Timeline,
    TotalEngagement,
}

#[derive(Deserialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ViewerStreamSource {
    All,
    Following,
    Followers,
    Friends,
    Bookmarks,
    Replies, // 4U,
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct PostStreamQuery {
    #[serde(flatten)]
    pub source: ViewerStreamSourceQuery,
    #[serde(flatten)]
    pub pagination: PaginationQuery,
    pub sorting: Option<PostStreamSorting>,
    pub viewer_id: Option<String>,
    #[serde(flatten)]
    // Not sure if filters needed
    pub filters: Filters,
}

#[derive(Deserialize, Debug, ToSchema)]
pub struct Filters {
    #[serde(default, deserialize_with = "deserialize_comma_separated")]
    pub tags: Option<Vec<String>>,
}

impl PostStreamQuery {
    pub fn initialize_defaults(&mut self) {
        self.pagination.skip.get_or_insert(0);
        self.pagination.limit = Some(self.pagination.limit.unwrap_or(10).min(30));
        self.sorting.get_or_insert(PostStreamSorting::Timeline);
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

#[derive(ToSchema, Debug, Clone, PartialEq)]
pub enum ViewerStreamSourceQuery {
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

impl ViewerStreamSourceQuery {
    pub fn has_observer(&self) -> Option<&String> {
        match self {
            ViewerStreamSourceQuery::Followers { observer_id }
            | ViewerStreamSourceQuery::Following { observer_id }
            | ViewerStreamSourceQuery::Friends { observer_id }
            | ViewerStreamSourceQuery::Bookmarks { observer_id } => Some(observer_id),
            _ => None,
        }
    }

    pub fn has_author(&self) -> Option<&String> {
        match self {
            ViewerStreamSourceQuery::Replies {
                author_id,
                post_id: _,
            } => Some(author_id),
            ViewerStreamSourceQuery::All {
                author_id: Some(author_id),
            } => Some(author_id),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for ViewerStreamSourceQuery {
    fn deserialize<D>(deserializer: D) -> Result<ViewerStreamSourceQuery, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(ViewerStreamSourceVisitor)
    }
}

struct ViewerStreamSourceVisitor;

impl<'de> Visitor<'de> for ViewerStreamSourceVisitor {
    type Value = ViewerStreamSourceQuery;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid ViewerStreamSource")
    }

    fn visit_map<M>(self, mut map: M) -> Result<ViewerStreamSourceQuery, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut source: Option<String> = None;
        let mut post_id: Option<String> = None;
        let mut author_id: Option<String> = None;
        let mut observer_id: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "source" => {
                    if source.is_some() {
                        return Err(de::Error::duplicate_field("source"));
                    }
                    source = Some(map.next_value()?);
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

        let source = source.ok_or_else(|| de::Error::missing_field("source"))?;
        match source.as_str() {
            "replies" => {
                let author_id = author_id.ok_or_else(|| de::Error::missing_field("author_id"))?;
                Ok(ViewerStreamSourceQuery::Replies { post_id, author_id })
            }
            "following" => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(ViewerStreamSourceQuery::Following { observer_id })
            }
            "followers" => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(ViewerStreamSourceQuery::Followers { observer_id })
            }
            "friends" => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(ViewerStreamSourceQuery::Friends { observer_id })
            }
            "bookmarks" => {
                let observer_id =
                    observer_id.ok_or_else(|| de::Error::missing_field("observer_id"))?;
                Ok(ViewerStreamSourceQuery::Bookmarks { observer_id })
            }
            "all" => Ok(ViewerStreamSourceQuery::All { author_id }),
            // Not sure if we want to throw an error or set the default source `All`
            other => Err(de::Error::unknown_variant(
                other,
                &[
                    "replies",
                    "following",
                    "followers",
                    "friends",
                    "bookmarks",
                    "all",
                ],
            )),
        }
    }
}
