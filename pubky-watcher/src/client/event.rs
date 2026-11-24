use std::fmt;

/// What a homeserver event did to its resource: written (`PUT`) or deleted (`DEL`).
///
/// Neither carries a body: for a `PUT` the resource is fetched separately.
/// Carried by [`HomeserverEvent`](crate::HomeserverEvent) and
/// [`HsStreamEvent`](crate::HsStreamEvent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventMethod {
    /// The resource was written; read it with
    /// [`ResourceReader::get_resource`](crate::ResourceReader::get_resource).
    Put,
    /// The resource was deleted.
    /// Its resource can no longer be read with [`ResourceReader`](crate::ResourceReader).
    Del,
}

impl From<pubky::EventType> for EventMethod {
    fn from(value: pubky::EventType) -> Self {
        match value {
            pubky::EventType::Put { .. } => Self::Put,
            pubky::EventType::Delete => Self::Del,
        }
    }
}

/// `PUT` or `DEL`, as on an `/events/` line.
impl fmt::Display for EventMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Put => "PUT",
            Self::Del => "DEL",
        })
    }
}
