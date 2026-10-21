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
use pubky_social_specs::{
    epoch_segment, ParsedUri, PubkyId, Resource, Visibility, SOCIAL_NAMESPACE,
};

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
/// reads from it; a `social/v1` address the key the v1 parser reads from it, so a post has one
/// key whichever epoch's address names it. A bare `pubky://<pk>` names the user, as the v1
/// parser reads it. Every other address is [`ReferenceTarget::Other`]: a URL, a path no parser
/// accepts, a `social/vN` epoch the crate does not speak.
pub fn resolve_reference(address: &str) -> Reference {
    let target = match route(address) {
        Routed::Legacy(parsed) => legacy_target(parsed),
        Routed::Social(parsed) => social_target(parsed),
        Routed::Malformed { .. } => {
            bare_user(address).map_or(ReferenceTarget::Other, ReferenceTarget::User)
        }
        Routed::UnsupportedEpoch { .. } => ReferenceTarget::Other,
    };
    Reference {
        uri: address.to_string(),
        target,
    }
}

/// The address a takedown of `target` is recorded and looked up under: the `pubky.app`
/// rendering of the resolved key, as the frozen v0 reader renders it, whichever epoch's
/// address or app's folder named it (a tag has one id whatever folder holds it). An event's
/// address resolves the same way (see [`super::remove_if_taken_down`]), so every address that
/// names one record finds its takedown. A `social/v1` file keeps its raw `{hash}.{ext}` name.
///
/// `Err` for a user or an unresolved address: neither is taken down.
pub fn takedown_address(target: &ReferenceTarget) -> Result<String, String> {
    let pubky_app = |key: &ResourceKey, resource: fn(String) -> legacy_v0::Resource| {
        ExtendedParsedUri::PubkyApp {
            user_id: key.owner_id.clone(),
            resource: resource(key.resource_id.clone()),
        }
    };
    let parsed = match target {
        ReferenceTarget::Post(key) => pubky_app(key, legacy_v0::Resource::Post),
        ReferenceTarget::File(key) => pubky_app(key, legacy_v0::Resource::File),
        ReferenceTarget::Tag { key, .. } => pubky_app(key, legacy_v0::Resource::Tag),
        ReferenceTarget::User(_) => return Err("a user is never taken down".to_string()),
        ReferenceTarget::Other => return Err("names nothing Nexus takes down".to_string()),
    };
    parsed.try_to_uri_str()
}

/// The user a bare `pubky://<pk>` names. It has no namespace segment to route by, so the
/// router calls it malformed; the v1 parser reads it as a user reference. The only other
/// routed-malformed addresses it reads as one are the SDK short forms the v0 splitter does not
/// read, `pubky<pk>` and `pubky<pk>/pub/social/v1/profile.json`; a canonical `profile.json`
/// path routes to its epoch first.
fn bare_user(address: &str) -> Option<PubkyId> {
    match ParsedUri::try_from(address) {
        Ok(ParsedUri {
            user_id,
            visibility: Visibility::Public,
            resource: Resource::User,
        }) => Some(user_id),
        _ => None,
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

/// The target a `social/v1` address names; the resources Nexus never references are `Other`.
///
/// A post is keyed by its id whether the address carries a version or not: every version of a
/// post is one node. A file keeps its raw `{hash}.{ext}` name as its id. A private path names
/// nothing: a draft or a bookmark is never linked.
fn social_target(parsed: ParsedUri) -> ReferenceTarget {
    let ParsedUri {
        user_id,
        visibility,
        resource,
    } = parsed;
    if visibility == Visibility::Private {
        return ReferenceTarget::Other;
    }
    let key = |resource_id: String| ResourceKey {
        owner_id: user_id.clone(),
        resource_id,
    };
    match resource {
        Resource::Post { id, .. } => ReferenceTarget::Post(key(id)),
        Resource::User => ReferenceTarget::User(user_id),
        Resource::File(filename) => ReferenceTarget::File(key(filename)),
        Resource::Tag(tag_id) => ReferenceTarget::Tag {
            key: key(tag_id),
            app: None,
        },
        // A follow, mute, bookmark, feed, foreign, unknown or unsupported resource, and one the
        // non-exhaustive `Resource` adds later: not referenced until the resolver says so.
        _ => ReferenceTarget::Other,
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
    use pubky_social_specs::{
        bookmark_uri_builder as v1_bookmark_uri_builder, feed_uri_builder as v1_feed_uri_builder,
        file_uri_builder as v1_file_uri_builder, follow_uri_builder as v1_follow_uri_builder,
        mute_uri_builder as v1_mute_uri_builder, post_uri_builder as v1_post_uri_builder,
        tag_uri_builder as v1_tag_uri_builder, user_uri_builder as v1_user_uri_builder,
        PubkySocialPost, Root,
    };

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    /// A timestamp id later than [`POST_ID`], as a post version's edit id must be.
    const EDIT_ID: &str = "0032SSN7Q4EVJ";
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

    /// The v1 address of a stored post version, with or without a label.
    fn v1_version_uri(label: Option<&str>) -> String {
        let path = PubkySocialPost::create_path_in(Root::Pub, POST_ID, EDIT_ID, label);
        format!("pubky://{USER}{path}")
    }

    #[test]
    fn social_v1_post_addresses_resolve_to_the_post_whatever_their_form() {
        let expected = ReferenceTarget::Post(key(POST_ID));
        for address in [
            v1_post_uri_builder(USER.into(), POST_ID.into()),
            v1_version_uri(None),
            v1_version_uri(Some("hello-world")),
        ] {
            assert_eq!(target(&address), expected, "{address}");
        }
        // The same node the `pubky.app` address of that post names.
        assert_eq!(
            target(&post_uri_builder(USER.into(), POST_ID.into())),
            expected
        );
    }

    /// The v1 profile path, the bare key, and the SDK short forms of both.
    #[test]
    fn social_v1_user_addresses_resolve_to_the_user_in_both_forms() {
        for address in [
            v1_user_uri_builder(USER.into()),
            format!("pubky://{USER}"),
            format!("pubky{USER}"),
            format!("pubky{USER}/pub/social/v1/profile.json"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::User(user()), "{address}");
        }
    }

    #[test]
    fn social_v1_file_resolves_to_the_file_with_its_raw_name() {
        let filename = format!("{HASH_ID}.jpg");
        assert_eq!(
            target(&v1_file_uri_builder(USER.into(), filename.clone())),
            ReferenceTarget::File(key(&filename))
        );
    }

    #[test]
    fn social_v1_tag_resolves_to_a_tag_without_an_app() {
        assert_eq!(
            target(&v1_tag_uri_builder(USER.into(), HASH_ID.into())),
            ReferenceTarget::Tag {
                key: key(HASH_ID),
                app: None,
            }
        );
    }

    #[test]
    fn unreferenced_social_v1_resources_resolve_to_other() {
        for address in [
            v1_follow_uri_builder(USER.into(), OTHER_USER.into()),
            v1_feed_uri_builder(USER.into(), HASH_ID.into()),
            format!("pubky://{USER}/pub/social/v1/feeds/{HASH_ID}.json"),
            // A file name the parser cannot classify, and an unknown path.
            format!("pubky://{USER}/pub/social/v1/files/{HASH_ID}.exe"),
            format!("pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}"),
            // A post version path with a malformed leaf.
            format!("pubky://{USER}/pub/social/v1/posts/{POST_ID}/not-an-id.json"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Other, "{address}");
        }
    }

    /// A private path names nothing, whatever resource it spells: a draft is never linked.
    #[test]
    fn private_social_v1_paths_resolve_to_other() {
        for address in [
            v1_mute_uri_builder(USER.into(), OTHER_USER.into()),
            v1_bookmark_uri_builder(USER.into(), "~".to_string() + HASH_ID),
            format!("pubky://{USER}/priv/social/v1/posts/{POST_ID}"),
            format!("pubky://{USER}/priv/social/v1/profile.json"),
            format!("pubky://{USER}/priv/social/v1/files/{HASH_ID}.jpg"),
            format!("pubky://{USER}/priv/social/v1/tags/{HASH_ID}.json"),
            format!("pubky://{USER}/priv"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Other, "{address}");
        }
    }

    /// A takedown is recorded under the `pubky.app` address of the resolved key, whatever
    /// address or app's folder named it.
    #[test]
    fn takedown_addresses_are_the_pubky_app_ones_whatever_address_resolved_the_target() {
        let post = post_uri_builder(USER.into(), POST_ID.into());
        for address in [
            post.clone(),
            format!("pubky://{USER}/pub/pubky.app/posts/{POST_ID}/extra"),
            v1_post_uri_builder(USER.into(), POST_ID.into()),
            v1_version_uri(None),
            v1_version_uri(Some("hello-world")),
        ] {
            assert_eq!(
                takedown_address(&target(&address)).as_deref(),
                Ok(post.as_str()),
                "{address}"
            );
        }
        // A tag has one id whatever folder holds it, another app's included.
        let tag = tag_uri_builder(USER.into(), HASH_ID.into());
        for address in [
            tag.clone(),
            v1_tag_uri_builder(USER.into(), HASH_ID.into()),
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}"),
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}?x=1"),
        ] {
            assert_eq!(
                takedown_address(&target(&address)).as_deref(),
                Ok(tag.as_str()),
                "{address}"
            );
        }
        let file = file_uri_builder(USER.into(), POST_ID.into());
        assert_eq!(
            takedown_address(&target(&file)).as_deref(),
            Ok(file.as_str())
        );
        // A `social/v1` file keeps its raw `{hash}.{ext}` name: no event carries this address.
        let filename = format!("{HASH_ID}.jpg");
        assert_eq!(
            takedown_address(&target(&v1_file_uri_builder(USER.into(), filename.clone())))
                .as_deref(),
            Ok(format!("pubky://{USER}/pub/pubky.app/files/{filename}").as_str())
        );
        for address in [
            format!("pubky://{USER}"),
            user_uri_builder(USER.into()),
            format!("pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}"),
            format!("pubky://{USER}/pub/social/v2/posts/{POST_ID}"),
            "https://example.com/article".to_string(),
        ] {
            assert!(takedown_address(&target(&address)).is_err(), "{address}");
        }
    }

    #[test]
    fn addresses_outside_pubky_app_resolve_to_other() {
        for address in [
            // A `social/vN` epoch the crate does not speak.
            format!("pubky://{USER}/pub/social/v2/posts/{POST_ID}"),
            // A bare key with a trailing slash, or a short-form address no parser reads.
            format!("pubky://{USER}/"),
            format!("pubky{USER}/pub/social/v1/posts/{POST_ID}"),
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
            v1_version_uri(Some("hello-world")),
        ] {
            assert_eq!(resolve_reference(&address).uri, address);
        }
    }
}
