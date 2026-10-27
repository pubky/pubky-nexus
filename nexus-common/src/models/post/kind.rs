use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use utoipa::ToSchema;

/// What kind of post a post is, in Nexus vocabulary: every epoch's translator maps its spec's
/// kinds into it.
///
/// Its names are the ones storage and `/v0` have always held (`short`, `long`, …). Serde reads
/// an unknown name as [`PostKind::Unknown`]; [`FromStr`] rejects it, for strict filters.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, Default, PartialEq, Eq)]
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
    /// A `social/v1` calendar event, its occurrences indexed for the event routes.
    Event,
    /// An attendance answer to a `social/v1` event.
    Rsvp,
    /// A rating of a post or an external URI.
    Rating,
    /// An author's availability blocks.
    Availability,
    /// A kind the spec reader didn't recognise.
    #[serde(other)]
    Unknown,
}

impl PostKind {
    /// The stored and `/v0` name, as serde writes it.
    pub fn as_str(self) -> &'static str {
        match self {
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
        }
    }

    /// Whether the kind is in the frozen `/v0` vocabulary: the seven `pubky.app` kinds and
    /// `unknown`. The `social/v1` event kinds are stored under their own names but are no
    /// kind `/v0` knows: it serves and filters them as `unknown`, as it did before Nexus read
    /// them.
    pub fn is_v0(self) -> bool {
        match self {
            Self::Short
            | Self::Long
            | Self::Image
            | Self::Video
            | Self::Link
            | Self::File
            | Self::Collection
            | Self::Unknown => true,
            Self::Event | Self::Rsvp | Self::Rating | Self::Availability => false,
        }
    }

    /// The `social/v1` name, for the `/v1` API: `Short` is `note`, `Long` is `article`, the
    /// rest keep their name. Every variant is spelled out, so a new one needs its v1 name here.
    pub fn v1_name(self) -> &'static str {
        match self {
            Self::Short => "note",
            Self::Long => "article",
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
        }
    }
}

impl fmt::Display for PostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Strict: only a known kind's name parses, so `unknown` is an error too. It reads the names the
/// serde derive generates, so a new variant parses with no second list to keep in step.
impl FromStr for PostKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match serde_json::from_value(serde_json::Value::from(s)) {
            Ok(kind) if kind != Self::Unknown => Ok(kind),
            _ => Err(format!("Invalid content kind: {s}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::PubkySocialPostKind;

    /// The names in storage and on both versions, so none may change: the seven `pubky.app`
    /// kinds and the four `social/v1` event kinds.
    #[test]
    fn names_are_the_stored_ones() {
        let names = [
            (PostKind::Short, "short"),
            (PostKind::Long, "long"),
            (PostKind::Image, "image"),
            (PostKind::Video, "video"),
            (PostKind::Link, "link"),
            (PostKind::File, "file"),
            (PostKind::Collection, "collection"),
            (PostKind::Event, "event"),
            (PostKind::Rsvp, "rsvp"),
            (PostKind::Rating, "rating"),
            (PostKind::Availability, "availability"),
            (PostKind::Unknown, "unknown"),
        ];
        for (kind, name) in names {
            assert_eq!(serde_json::to_value(kind).unwrap(), name);
            assert_eq!(
                serde_json::from_value::<PostKind>(name.into()).unwrap(),
                kind
            );
            assert_eq!(kind.to_string(), name);
            let parsed = name.parse::<PostKind>().ok();
            assert_eq!(
                parsed,
                (kind != PostKind::Unknown).then_some(kind),
                "{name}"
            );
        }
    }

    /// `/v0` knows the `pubky.app` kinds and `unknown`, none of the event kinds.
    #[test]
    fn v0_vocabulary_is_the_pubky_app_kinds() {
        let v0: Vec<&str> = [
            "short",
            "long",
            "image",
            "video",
            "link",
            "file",
            "collection",
        ]
        .into_iter()
        .chain(["unknown"])
        .collect();
        for name in [
            "short",
            "long",
            "image",
            "video",
            "link",
            "file",
            "collection",
            "event",
            "rsvp",
            "rating",
            "availability",
            "unknown",
        ] {
            let kind: PostKind = serde_json::from_value(name.into()).unwrap();
            assert_eq!(kind.is_v0(), v0.contains(&name), "{name}");
        }
    }

    #[test]
    fn serde_is_lenient_and_from_str_strict() {
        let read: PostKind = serde_json::from_str(r#""marketplace""#).unwrap();
        assert_eq!(read, PostKind::Unknown);
        assert!("marketplace".parse::<PostKind>().is_err());
        assert!("Short".parse::<PostKind>().is_err());
    }

    /// Pinned to the spec crate's v1 wire names, so a renamed v1 kind fails here.
    #[test]
    fn v1_names_match_the_v1_spec() {
        let pairs = [
            (PostKind::Short, PubkySocialPostKind::Note),
            (PostKind::Long, PubkySocialPostKind::Article),
            (PostKind::Image, PubkySocialPostKind::Image),
            (PostKind::Video, PubkySocialPostKind::Video),
            (PostKind::Link, PubkySocialPostKind::Link),
            (PostKind::File, PubkySocialPostKind::File),
            (PostKind::Collection, PubkySocialPostKind::Collection),
            (PostKind::Event, PubkySocialPostKind::Event),
            (PostKind::Rsvp, PubkySocialPostKind::Rsvp),
            (PostKind::Rating, PubkySocialPostKind::Rating),
            (PostKind::Availability, PubkySocialPostKind::Availability),
        ];
        for (kind, v1) in pairs {
            assert_eq!(kind.v1_name(), v1.wire_name());
        }
        let unknown = PubkySocialPostKind::from(PostKind::Unknown.v1_name().to_string());
        assert!(!unknown.is_known());
    }
}
