use nexus_common::models::app_post::AppPostUri;
use pubky_social_specs::legacy_v0::ExtendedParsedUri;
use pubky_social_specs::{ParsedUri, PubkyId};

/// The user and resource an event addresses, parsed by the reader of the event's epoch.
///
/// Every variant names a user and a resource, so the accessors are total, and the orchestrator
/// and the metrics read them without an epoch branch. Code that needs one variant's own parsed
/// resource matches on it, so a third epoch fails to compile there and forces a decision.
#[derive(Debug, Clone)]
pub enum EventTarget {
    /// A stored version of an app-owned v1 post.
    AppPost(AppPostUri),
    /// A `pubky.app` path or another app's universal tag (`/pub/mapky/tags/…`), parsed by the
    /// frozen v0 reader.
    Legacy(ExtendedParsedUri),
    /// A `/pub/social/v1/…` path parsed by the v1 parser. The classifier enables public profiles, stored post versions, follows and tags.
    Social(ParsedUri),
}

impl EventTarget {
    /// The user whose storage the event addresses.
    pub fn user_id(&self) -> &PubkyId {
        match self {
            Self::Legacy(parsed) => parsed.user_id(),
            Self::Social(parsed) => &parsed.user_id,
            Self::AppPost(parsed) => &parsed.author,
        }
    }

    /// The resource kind the event addresses, without its id (`posts`, `profile.json`, …).
    pub fn resource_name(&self) -> String {
        match self {
            Self::Legacy(parsed) => parsed.resource().to_string(),
            Self::Social(parsed) => parsed.resource.to_string(),
            Self::AppPost(_) => "posts".into(),
        }
    }

    /// The id of the addressed resource; empty for a resource without one (the profile).
    ///
    /// A `social/v1` post version is identified by its post id, never its edit id: every
    /// version of a post is one resource.
    pub fn resource_id(&self) -> String {
        match self {
            Self::Legacy(parsed) => parsed.resource().id().unwrap_or_default(),
            Self::Social(parsed) => parsed.resource.id().unwrap_or_default(),
            Self::AppPost(parsed) => parsed.id.clone(),
        }
    }

    /// Whether `uri` is the address this target's epoch renders for it.
    ///
    /// The v0 parsers ignore segments past the resource id, so `posts/ID/extra` reads as
    /// `posts/ID`; the v1 parser folds the SDK short form `pubky<pk>/…` into `pubky://<pk>/…`.
    /// Only the canonical address may act on a resource, so an alias path can neither
    /// overwrite nor delete the resource it parses to.
    pub fn is_canonical(&self, uri: &str) -> bool {
        let canonical = match self {
            Self::Legacy(parsed) => parsed.try_to_uri_str(),
            Self::Social(parsed) => parsed.try_to_uri_str(),
            Self::AppPost(parsed) => Ok(parsed.version_uri()),
        };
        canonical.is_ok_and(|canonical| canonical == uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::route::{route, Routed};
    use pubky_social_specs::{
        file_uri_builder, follow_uri_builder, post_uri_builder, tag_uri_builder, user_uri_builder,
        PubkySocialPost, Root,
    };

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    /// A timestamp id later than [`POST_ID`], as a post version's edit id must be.
    const EDIT_ID: &str = "0032SSN7Q4EVJ";
    const HASH_ID: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";

    /// The target the router builds for a `social/v1` address.
    fn social(address: &str) -> EventTarget {
        match route(address) {
            Routed::Social(parsed) => EventTarget::Social(parsed),
            other => panic!("{address}: expected a social/v1 route, got {other:?}"),
        }
    }

    /// The v1 address of a stored post version, with or without a label.
    fn version_uri(label: Option<&str>) -> String {
        let path = PubkySocialPost::create_path_in(Root::Pub, POST_ID, EDIT_ID, label);
        format!("pubky://{USER}{path}")
    }

    /// The accessors are total: the profile reads as an empty id, a post version as its post
    /// id whatever its leaf, a file as the id the v1 resource reads from its name.
    #[test]
    fn social_v1_accessors_name_the_user_and_resource() {
        let cases = [
            (user_uri_builder(USER.into()), "profile.json", ""),
            (
                post_uri_builder(USER.into(), POST_ID.into()),
                "posts",
                POST_ID,
            ),
            (version_uri(None), "posts", POST_ID),
            (version_uri(Some("hello-world")), "posts", POST_ID),
            (
                follow_uri_builder(USER.into(), OTHER_USER.into()),
                "follows",
                OTHER_USER,
            ),
            (
                tag_uri_builder(USER.into(), HASH_ID.into()),
                "tags",
                HASH_ID,
            ),
            (
                file_uri_builder(USER.into(), format!("{HASH_ID}.jpg")),
                "files",
                HASH_ID,
            ),
        ];
        for (address, name, id) in cases {
            let target = social(&address);
            assert_eq!(target.user_id().to_string(), USER, "{address}");
            assert_eq!(target.resource_name(), name, "{address}");
            assert_eq!(target.resource_id(), id, "{address}");
        }
    }

    #[test]
    fn social_v1_version_paths_are_canonical_plain_and_labelled() {
        for address in [
            version_uri(None),
            version_uri(Some("hello-world")),
            post_uri_builder(USER.into(), POST_ID.into()),
            user_uri_builder(USER.into()),
        ] {
            assert!(social(&address).is_canonical(&address), "{address}");
        }
    }

    /// The SDK short form is the one alias the v1 parser accepts; it is not the address the
    /// parser renders back, so it can neither overwrite nor delete.
    #[test]
    fn social_v1_alias_path_is_not_canonical() {
        let canonical = version_uri(Some("hello-world"));
        let alias = canonical.replacen("pubky://", "pubky", 1);
        let parsed = ParsedUri::try_from(alias.as_str()).unwrap();
        let target = EventTarget::Social(parsed);
        assert!(!target.is_canonical(&alias));
        assert!(target.is_canonical(&canonical));
        // Nor is another spelling of the same resource.
        assert!(!social(&canonical).is_canonical(&version_uri(None)));
    }

    /// A target whose resource the v1 parser cannot spell is never canonical.
    #[test]
    fn social_v1_unknown_resource_is_not_canonical() {
        let address = format!("pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}");
        let target = social(&address);
        assert_eq!(target.resource_name(), "unknown");
        assert_eq!(target.resource_id(), "");
        assert!(!target.is_canonical(&address));
    }
}
