use pubky_social_specs::legacy_v0::ExtendedParsedUri;
use pubky_social_specs::PubkyId;

/// The user and resource an event addresses, parsed by the reader of the event's epoch.
///
/// Every variant names a user and a resource, so the accessors are total. Code that needs a
/// variant's own parsed resource destructures it irrefutably: a second variant then fails to
/// compile there and forces a decision.
#[derive(Debug, Clone)]
pub enum EventTarget {
    /// A `pubky.app` path or another app's universal tag (`/pub/mapky/tags/…`), parsed by the
    /// frozen v0 reader.
    Legacy(ExtendedParsedUri),
}

impl EventTarget {
    /// The user whose storage the event addresses.
    pub fn user_id(&self) -> &PubkyId {
        match self {
            Self::Legacy(parsed) => parsed.user_id(),
        }
    }

    /// The resource kind the event addresses, without its id (`posts`, `profile.json`, …).
    pub fn resource_name(&self) -> String {
        match self {
            Self::Legacy(parsed) => parsed.resource().to_string(),
        }
    }

    /// The id of the addressed resource; empty for a resource without one (the profile).
    pub fn resource_id(&self) -> String {
        match self {
            Self::Legacy(parsed) => parsed.resource().id().unwrap_or_default(),
        }
    }

    /// Whether `uri` is the address this target's epoch renders for it.
    ///
    /// The parsers ignore segments past the resource id, so `posts/ID/extra` reads as
    /// `posts/ID`. Only the canonical address may act on a resource, so an alias path can
    /// neither overwrite nor delete the resource it parses to.
    pub fn is_canonical(&self, uri: &str) -> bool {
        match self {
            Self::Legacy(parsed) => parsed
                .try_to_uri_str()
                .is_ok_and(|canonical| canonical == uri),
        }
    }
}
