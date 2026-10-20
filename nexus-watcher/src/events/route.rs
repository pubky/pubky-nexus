//! Routing of a pubky address to the spec epoch whose parser judges it.
//!
//! The namespace segment (`pubky.app`, `social`, another app) picks the parser, and a path is
//! never handed to another epoch's parser as a fallback. That keeps every sloppy shape the
//! frozen v0 parser accepts away from the v1 canonicaliser (see the parse tests in `event.rs`).
//! Routing takes no skip decision: [`super::classify`] decides what the watcher does with an
//! event's route, and [`resolve_reference`] applies the same routing to the addresses found
//! inside objects.

use super::inputs::{Reference, ReferenceTarget, ResourceKey};
use pubky_social_specs::legacy_v0::{self, try_parse_pubky_path, ExtendedParsedUri};
use pubky_social_specs::{epoch_segment, ParsedUri, PubkyId, Resource, SOCIAL_NAMESPACE};

/// A pubky address classified by its namespace segment and parsed by that epoch's parser.
#[derive(Debug)]
pub enum Routed {
    /// A `pubky.app` path or another app's universal tag, parsed by the frozen v0 reader.
    Legacy(ExtendedParsedUri),
    /// A `/pub/social/v1/…` path parsed by the v1 parser, an unknown v1 resource included.
    Social(ParsedUri),
    /// A `/pub/social/vN/…` path of an epoch the pinned crate does not speak.
    UnsupportedEpoch { version: String },
    /// No namespace, a `social` path without an epoch segment, or a path its own namespace's
    /// parser rejects.
    Malformed { reason: String },
}

/// Routes `uri` by its namespace segment, taken from the frozen reader's lenient splitter.
pub fn route(uri: &str) -> Routed {
    let path = match try_parse_pubky_path(uri) {
        Ok(path) => path,
        Err(reason) => return Routed::Malformed { reason },
    };
    if path.app == SOCIAL_NAMESPACE {
        return route_social(uri, path.segments.first().map(String::as_str));
    }
    match ExtendedParsedUri::try_from(uri) {
        Ok(parsed) => Routed::Legacy(parsed),
        Err(reason) => Routed::Malformed { reason },
    }
}

/// Routes a `social` path; `epoch` is the segment after the namespace as the splitter read it.
///
/// The v1 parser judges the shape and names an epoch it does not speak, but it reads an
/// epoch-less path as an unknown v1 resource, so the epoch segment is checked here: the
/// `social` namespace is reserved for the spec, never a universal tag of an app named `social`.
fn route_social(uri: &str, epoch: Option<&str>) -> Routed {
    let parsed = match ParsedUri::try_from(uri) {
        Ok(parsed) => parsed,
        Err(reason) => return Routed::Malformed { reason },
    };
    if let Resource::UnsupportedVersion { version } = parsed.resource {
        return Routed::UnsupportedEpoch { version };
    }
    match epoch {
        Some(epoch) if epoch == epoch_segment() => Routed::Social(parsed),
        _ => Routed::Malformed {
            reason: "social path without an epoch segment".to_string(),
        },
    }
}

/// Resolves an address an object holds into what it names, keeping the address as written.
///
/// A `pubky.app` address, or another app's universal tag, gets the key the frozen v0 parser
/// reads from it. Every other address is [`ReferenceTarget::Other`]: a URL, a bare
/// `pubky://<pk>`, a path no parser accepts, and for now any `social/*` address.
pub fn resolve_reference(address: &str) -> Reference {
    let target = match route(address) {
        Routed::Legacy(parsed) => legacy_target(parsed),
        // TODO(specs-migration): resolve `social/v1` addresses with the v1 translator.
        Routed::Social(_) | Routed::UnsupportedEpoch { .. } | Routed::Malformed { .. } => {
            ReferenceTarget::Other
        }
    };
    Reference {
        uri: address.to_string(),
        target,
    }
}

/// The target a v0 address names; the resources Nexus never references are `Other`.
fn legacy_target(parsed: ExtendedParsedUri) -> ReferenceTarget {
    let key = |owner_id: PubkyId, resource_id: String| ResourceKey {
        owner_id,
        resource_id,
    };
    match parsed {
        ExtendedParsedUri::PubkyApp { user_id, resource } => match resource {
            legacy_v0::Resource::Post(post_id) => ReferenceTarget::Post(key(user_id, post_id)),
            legacy_v0::Resource::User => ReferenceTarget::User(user_id),
            legacy_v0::Resource::File(file_id) => ReferenceTarget::File(key(user_id, file_id)),
            legacy_v0::Resource::Tag(tag_id) => ReferenceTarget::Tag {
                key: key(user_id, tag_id),
                app: None,
            },
            legacy_v0::Resource::Follow(_)
            | legacy_v0::Resource::Bookmark(_)
            | legacy_v0::Resource::Blob(_)
            | legacy_v0::Resource::Mute(_)
            | legacy_v0::Resource::Feed(_)
            | legacy_v0::Resource::LastRead
            | legacy_v0::Resource::Unknown => ReferenceTarget::Other,
        },
        ExtendedParsedUri::UniversalTag {
            user_id,
            app,
            resource: legacy_v0::Resource::Tag(tag_id),
        } => ReferenceTarget::Tag {
            key: key(user_id, tag_id),
            app: Some(app),
        },
        ExtendedParsedUri::UniversalTag { .. } => ReferenceTarget::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::legacy_v0::{
        blob_uri_builder, bookmark_uri_builder, feed_uri_builder, file_uri_builder,
        follow_uri_builder, last_read_uri_builder, mute_uri_builder, post_uri_builder,
        tag_uri_builder, user_uri_builder, ParsedUri as V0ParsedUri,
    };

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    const HASH_ID: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";

    fn user() -> PubkyId {
        PubkyId::try_from(USER).unwrap()
    }

    fn key(resource_id: &str) -> ResourceKey {
        ResourceKey {
            owner_id: user(),
            resource_id: resource_id.to_string(),
        }
    }

    fn target(address: &str) -> ReferenceTarget {
        resolve_reference(address).target
    }

    /// The target the v0 parser's reading of a `pubky.app` address names, built independently.
    fn v0_target(address: &str) -> ReferenceTarget {
        let parsed = V0ParsedUri::try_from(address).unwrap();
        let owner_id = parsed.user_id;
        match parsed.resource {
            legacy_v0::Resource::Post(id) => ReferenceTarget::Post(ResourceKey {
                owner_id,
                resource_id: id,
            }),
            legacy_v0::Resource::User => ReferenceTarget::User(owner_id),
            legacy_v0::Resource::File(id) => ReferenceTarget::File(ResourceKey {
                owner_id,
                resource_id: id,
            }),
            legacy_v0::Resource::Tag(id) => ReferenceTarget::Tag {
                key: ResourceKey {
                    owner_id,
                    resource_id: id,
                },
                app: None,
            },
            other => panic!("not a referenced resource: {other:?}"),
        }
    }

    #[test]
    fn pubky_app_addresses_resolve_to_the_v0_parsers_key() {
        let cases = [
            (
                post_uri_builder(USER.into(), POST_ID.into()),
                ReferenceTarget::Post(key(POST_ID)),
            ),
            (user_uri_builder(USER.into()), ReferenceTarget::User(user())),
            (
                file_uri_builder(USER.into(), POST_ID.into()),
                ReferenceTarget::File(key(POST_ID)),
            ),
            (
                tag_uri_builder(USER.into(), HASH_ID.into()),
                ReferenceTarget::Tag {
                    key: key(HASH_ID),
                    app: None,
                },
            ),
            // The parser ignores segments past the id and reads the scheme case-insensitively.
            (
                format!("pubky://{USER}/pub/pubky.app/posts/{POST_ID}/extra"),
                ReferenceTarget::Post(key(POST_ID)),
            ),
            (
                format!("PUBKY://{USER}/pub/pubky.app/posts/{POST_ID}"),
                ReferenceTarget::Post(key(POST_ID)),
            ),
        ];
        for (address, expected) in cases {
            assert_eq!(target(&address), v0_target(&address), "{address}");
            assert_eq!(target(&address), expected, "{address}");
        }
    }

    #[test]
    fn another_apps_tag_resolves_to_a_tag_with_its_app() {
        let address = format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}");
        assert_eq!(
            target(&address),
            ReferenceTarget::Tag {
                key: key(HASH_ID),
                app: Some("mapky".to_string()),
            }
        );
    }

    #[test]
    fn unreferenced_pubky_app_resources_resolve_to_other() {
        for address in [
            follow_uri_builder(USER.into(), OTHER_USER.into()),
            bookmark_uri_builder(USER.into(), HASH_ID.into()),
            blob_uri_builder(USER.into(), HASH_ID.into()),
            mute_uri_builder(USER.into(), OTHER_USER.into()),
            feed_uri_builder(USER.into(), HASH_ID.into()),
            last_read_uri_builder(USER.into()),
            format!("pubky://{USER}/pub/pubky.app/unknown_kind/{POST_ID}"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Other, "{address}");
        }
    }

    #[test]
    fn addresses_outside_pubky_app_resolve_to_other() {
        for address in [
            // A `social/v1` address: other until the v1 arm exists.
            format!("pubky://{USER}/pub/social/v1/posts/{POST_ID}"),
            // A bare public key.
            format!("pubky://{USER}"),
            // Malformed `pubky.app` addresses.
            format!("pubky://{USER}/pub/pubky.app/follows/not-a-key"),
            "pubky://not-a-key/pub/pubky.app/profile.json".to_string(),
            format!("pubky://{USER}/priv/pubky.app/profile.json"),
            // External URLs and another app's objects, whatever the scheme's case.
            "https://example.com/article".to_string(),
            "nostr:note1abc123".to_string(),
            format!("pubky://{OTHER_USER}/pub/eventky.app/events/E001"),
            format!("PUBKY://{OTHER_USER}/pub/eventky.app/events/E001"),
            "not a url".to_string(),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Other, "{address}");
        }
    }

    #[test]
    fn the_address_is_kept_as_written() {
        for address in [
            format!("PUBKY://{USER}/pub/pubky.app/posts/{POST_ID}/extra"),
            "HTTPS://Example.COM:443/path#frag".to_string(),
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}"),
        ] {
            assert_eq!(resolve_reference(&address).uri, address);
        }
    }
}
