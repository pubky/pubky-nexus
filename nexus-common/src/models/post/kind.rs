//! Nexus storage vocabulary, independent of either wire epoch.
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use utoipa::ToSchema;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PostKind {
    #[default]
    Short,
    Long,
    Image,
    Video,
    Link,
    File,
    Collection,
    Event,
    Rsvp,
    Rating,
    Availability,
    #[serde(other)]
    Unknown,
}
impl fmt::Display for PostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Short => "short",
            Self::Long => "long",
            Self::Image => "image",
            Self::Video => "video",
            Self::Link => "link",
            Self::File => "file",
            Self::Collection => "collection",
            Self::Event => "event",
            Self::Rsvp => "rsvp",
            Self::Rating => "rating",
            Self::Availability => "availability",
            Self::Unknown => "unknown",
        })
    }
}
impl FromStr for PostKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind: Self = serde_json::from_value(s.into()).map_err(|e| e.to_string())?;
        if kind == Self::Unknown {
            Err(format!("Unknown post kind: {s}"))
        } else {
            Ok(kind)
        }
    }
}
impl From<pubky_social_specs::legacy_v0::PubkyAppPostKind> for PostKind {
    fn from(kind: pubky_social_specs::legacy_v0::PubkyAppPostKind) -> Self {
        use pubky_social_specs::legacy_v0::PubkyAppPostKind as Old;
        match kind {
            Old::Short => Self::Short,
            Old::Long => Self::Long,
            Old::Image => Self::Image,
            Old::Video => Self::Video,
            Old::Link => Self::Link,
            Old::File => Self::File,
            Old::Collection => Self::Collection,
            Old::Unknown => Self::Unknown,
        }
    }
}
impl From<pubky_social_specs::PubkySocialPostKind> for PostKind {
    fn from(kind: pubky_social_specs::PubkySocialPostKind) -> Self {
        use pubky_social_specs::PubkySocialPostKind as Social;
        match kind {
            Social::Note => Self::Short,
            Social::Article => Self::Long,
            Social::Image => Self::Image,
            Social::Video => Self::Video,
            Social::Link => Self::Link,
            Social::File => Self::File,
            Social::Collection => Self::Collection,
            Social::Event => Self::Event,
            Social::Rsvp => Self::Rsvp,
            Social::Rating => Self::Rating,
            Social::Availability => Self::Availability,
            _ => Self::Unknown,
        }
    }
}
impl PartialEq<pubky_social_specs::legacy_v0::PubkyAppPostKind> for PostKind {
    fn eq(&self, other: &pubky_social_specs::legacy_v0::PubkyAppPostKind) -> bool {
        use pubky_social_specs::legacy_v0::PubkyAppPostKind as Old;
        matches!(
            (self, other),
            (Self::Short, Old::Short)
                | (Self::Long, Old::Long)
                | (Self::Image, Old::Image)
                | (Self::Video, Old::Video)
                | (Self::Link, Old::Link)
                | (Self::File, Old::File)
                | (Self::Collection, Old::Collection)
                | (Self::Unknown, Old::Unknown)
        )
    }
}
