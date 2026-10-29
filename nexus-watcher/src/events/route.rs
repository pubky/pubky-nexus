//! Routing of a pubky address to the spec epoch whose parser judges it.
//!
//! The namespace segment (`pubky.app`, `social`, another app) picks the parser, and a path is
//! never handed to another epoch's parser as a fallback. That keeps every sloppy shape the
//! frozen v0 parser accepts away from the v1 canonicaliser (see the parse tests in `event.rs`).
//! Routing takes no skip decision: [`super::classify`] decides what the watcher does with an
//! event's route, and [`resolve_reference`] applies the same routing to the addresses found
//! inside objects.

use super::inputs::{Reference, ReferenceTarget, ResourceKey};
use pubky_social_specs::legacy_v0::traits::{HasIdPath, HasPath};
use pubky_social_specs::legacy_v0::{
    self, try_parse_pubky_path, ExtendedParsedUri, PubkyAppBlob, PubkyAppBookmark, PubkyAppFeed,
    PubkyAppFile, PubkyAppFollow, PubkyAppLastRead, PubkyAppMute, PubkyAppPost, PubkyAppTag,
    PubkyAppUser, APP_PATH,
};
use pubky_social_specs::traits::{HasIdPath as _, HasPath as _};
use pubky_social_specs::{
    epoch_segment, is_pubky_scheme, ParsedUri, PubkyId, PubkySocialBookmark, PubkySocialFeed,
    PubkySocialFile, PubkySocialFollow, PubkySocialMute, PubkySocialPost, PubkySocialTag,
    PubkySocialUser, Resource, Visibility, SOCIAL_NAMESPACE,
};
use url::Url;

/// The folder segment tag files sit in, in every namespace.
const TAGS_SEGMENT: &str = "tags";

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
/// key whichever epoch's address names it, and a tag one key whatever folder holds it. A bare
/// `pubky://<pk>` names the user, as the v1 parser reads it. Every other address is
/// [`ReferenceTarget::Foreign`] or [`ReferenceTarget::Other`], as those variants describe.
pub fn resolve_reference(address: &str) -> Reference {
    let target = match route(address) {
        Routed::Legacy(parsed) => legacy_target(parsed, address),
        Routed::Social(parsed) => social_target(parsed, address),
        Routed::Malformed { .. } => unparsed_target(address),
        Routed::UnsupportedEpoch { .. } => ReferenceTarget::Other,
    };
    Reference {
        uri: address.to_string(),
        target,
    }
}

/// The address a takedown of the moderated `address` is recorded under, which a PUT event's
/// own address is looked up by as written (see [`super::remove_if_taken_down`]). A `social/v1`
/// address is kept as written: the spec only accepts it in canonical form. A `pubky.app` or
/// another app's address is rewritten by the frozen v0 reader, so a trailing slash or a query
/// still finds the record's events.
///
/// `Err` for an address neither epoch's parser reads.
pub fn takedown_key(address: &str) -> Result<String, String> {
    match route(address) {
        Routed::Social(_) => Ok(address.to_string()),
        Routed::Legacy(parsed) => parsed.try_to_uri_str(),
        Routed::UnsupportedEpoch { version } => Err(format!("unsupported epoch {version}")),
        Routed::Malformed { reason } => Err(reason),
    }
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

/// The target of an address no epoch's parser reads (see [`Routed::Malformed`]).
///
/// The user a bare `pubky://<pk>` names; [`ReferenceTarget::Foreign`] for an address of another
/// scheme or a public path under another app's namespace; `Other` for the rest. A path under
/// another app's tag folder the universal tag parser rejects is `Other`: it is a malformed tag
/// file, not another app's object.
fn unparsed_target(address: &str) -> ReferenceTarget {
    if let Some(user_id) = bare_user(address) {
        return ReferenceTarget::User(user_id);
    }
    let foreign = match try_parse_pubky_path(address) {
        Ok(path) => {
            !path.app.is_empty()
                && !is_nexus_namespace(&path.app)
                && path.segments.first().map(String::as_str) != Some(TAGS_SEGMENT)
        }
        // Not a public `pubky://<pk>/pub/<ns>` path: foreign only when another scheme's.
        Err(_) => Url::parse(address).is_ok_and(|url| !is_pubky_scheme(url.scheme())),
    };
    if foreign {
        ReferenceTarget::Foreign
    } else {
        ReferenceTarget::Other
    }
}

/// Whether `namespace` is one of Nexus's own: `pubky.app` or `social`.
fn is_nexus_namespace(namespace: &str) -> bool {
    namespace == APP_PATH.trim_end_matches('/') || namespace == SOCIAL_NAMESPACE
}

/// The first path segment of each v0 kind.
const V0_KIND_SEGMENTS: [&str; 10] = [
    PubkyAppUser::PATH_SEGMENT,
    PubkyAppLastRead::PATH_SEGMENT,
    PubkyAppPost::PATH_SEGMENT,
    PubkyAppFollow::PATH_SEGMENT,
    PubkyAppMute::PATH_SEGMENT,
    PubkyAppBookmark::PATH_SEGMENT,
    PubkyAppTag::PATH_SEGMENT,
    PubkyAppFile::PATH_SEGMENT,
    PubkyAppBlob::PATH_SEGMENT,
    PubkyAppFeed::PATH_SEGMENT,
];

/// The first path segment after the epoch of each v1 kind, private ones included.
const V1_KIND_SEGMENTS: [&str; 8] = [
    PubkySocialUser::PATH_SEGMENT,
    PubkySocialPost::PATH_SEGMENT,
    PubkySocialFollow::PATH_SEGMENT,
    PubkySocialMute::PATH_SEGMENT,
    PubkySocialBookmark::PATH_SEGMENT,
    PubkySocialTag::PATH_SEGMENT,
    PubkySocialFile::PATH_SEGMENT,
    PubkySocialFeed::PATH_SEGMENT,
];

/// The target of a public path its parser reads as `Unknown`: a kind the parser doesn't know, or
/// a malformed path of one it does. The segment after the namespace and `epoch_segments` tells
/// them apart: one that is none of `kinds` names another app's object (the Shop's
/// `pubky.app/marketplace/v1/listings/{id}`), `Foreign`; a known kind, or none at all, `Other`.
fn unknown_kind_target(address: &str, epoch_segments: usize, kinds: &[&str]) -> ReferenceTarget {
    let foreign = try_parse_pubky_path(address)
        .ok()
        .and_then(|path| path.segments.into_iter().nth(epoch_segments))
        .is_some_and(|first| !first.is_empty() && !spells_a_kind(&first, kinds));
    if foreign {
        ReferenceTarget::Foreign
    } else {
        ReferenceTarget::Other
    }
}

/// Whether `segment` is one of `kinds` spelled another way: in another ASCII case, with or
/// without `.json` (`Posts`, `profile`), or percent-encoded (`%74ags`), which the v0 splitter
/// keeps as written. A misspelt kind is a malformed path of it, never another app's object.
fn spells_a_kind(segment: &str, kinds: &[&str]) -> bool {
    let stem = |name: &str| {
        let name = name.trim_end_matches('/').to_ascii_lowercase();
        name.strip_suffix(".json")
            .map(str::to_owned)
            .unwrap_or(name)
    };
    segment.contains('%') || kinds.iter().any(|kind| stem(kind) == stem(segment))
}

/// The target a v0 address names; the resources Nexus never references are `Other`, a path
/// that names no v0 kind is `Foreign` (see [`unknown_kind_target`]).
fn legacy_target(parsed: ExtendedParsedUri, address: &str) -> ReferenceTarget {
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
            },
            legacy_v0::Resource::Follow(_)
            | legacy_v0::Resource::Bookmark(_)
            | legacy_v0::Resource::Blob(_)
            | legacy_v0::Resource::Mute(_)
            | legacy_v0::Resource::Feed(_)
            | legacy_v0::Resource::LastRead => ReferenceTarget::Other,
            legacy_v0::Resource::Unknown => unknown_kind_target(address, 0, &V0_KIND_SEGMENTS),
        },
        ExtendedParsedUri::UniversalTag {
            user_id,
            resource: legacy_v0::Resource::Tag(tag_id),
            ..
        } => ReferenceTarget::Tag {
            key: key(user_id, tag_id),
        },
        // Another app's path that is not a tag file: its own object.
        ExtendedParsedUri::UniversalTag { .. } => ReferenceTarget::Foreign,
    }
}

/// The target a `social/v1` address names; the resources Nexus never references are `Other`,
/// another namespace's path and a path that names no v1 kind are `Foreign` (see
/// [`unknown_kind_target`]).
///
/// A post is keyed by its id whether the address carries a version or not: every version of a
/// post is one node. A file keeps its raw `{hash}.{ext}` name as its id. A private path names
/// nothing: a draft or a bookmark is never linked.
fn social_target(parsed: ParsedUri, address: &str) -> ReferenceTarget {
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
        Resource::Tag(tag_id) => ReferenceTarget::Tag { key: key(tag_id) },
        // Unreachable while [`route`] hands this only `social` paths.
        Resource::Foreign { namespace, .. } if !is_nexus_namespace(&namespace) => {
            ReferenceTarget::Foreign
        }
        Resource::Unknown => unknown_kind_target(address, 1, &V1_KIND_SEGMENTS),
        // A follow, mute, bookmark, feed or unsupported resource, and one the non-exhaustive
        // `Resource` adds later: not referenced until the resolver says so.
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
    /// A Shop listing id, as the Shop writes it.
    const LISTING_ID: &str = "5d277f7f913f48659cbc6822362c8deb";

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
                ReferenceTarget::Tag { key: key(HASH_ID) },
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

    /// A tag has one id whatever folder holds it, so every folder's tag file resolves to the
    /// same target: `pubky.app`, `social/v1` and another app's.
    #[test]
    fn a_tag_file_resolves_to_its_key_in_every_folder() {
        let expected = ReferenceTarget::Tag { key: key(HASH_ID) };
        for address in [
            tag_uri_builder(USER.into(), HASH_ID.into()),
            v1_tag_uri_builder(USER.into(), HASH_ID.into()),
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}"),
            format!("pubky://{USER}/pub/eventky.app/tags/{HASH_ID}"),
        ] {
            assert_eq!(target(&address), expected, "{address}");
        }
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
            // A v0 kind with no id, and the namespace root.
            format!("pubky://{USER}/pub/pubky.app/posts/"),
            format!("pubky://{USER}/pub/pubky.app/bookmarks"),
            format!("pubky://{USER}/pub/pubky.app/"),
            // A v0 kind spelt another way: another case, no `.json`, percent-encoded.
            format!("pubky://{USER}/pub/pubky.app/Posts/{POST_ID}"),
            format!("pubky://{USER}/pub/pubky.app/profile"),
            format!("pubky://{USER}/pub/pubky.app/%74ags/{HASH_ID}"),
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
    fn unreferenced_social_v1_resources_resolve_to_other() {
        for address in [
            v1_follow_uri_builder(USER.into(), OTHER_USER.into()),
            v1_feed_uri_builder(USER.into(), HASH_ID.into()),
            format!("pubky://{USER}/pub/social/v1/feeds/{HASH_ID}.json"),
            // A file name the parser cannot classify.
            format!("pubky://{USER}/pub/social/v1/files/{HASH_ID}.exe"),
            // A post version path with a malformed leaf, and an extension on the post itself.
            format!("pubky://{USER}/pub/social/v1/posts/{POST_ID}/not-an-id.json"),
            format!("pubky://{USER}/pub/social/v1/posts/{POST_ID}.json"),
            // A follow with no key, a private kind at the public root, and the epoch root.
            format!("pubky://{USER}/pub/social/v1/follows/not-a-key.json"),
            format!("pubky://{USER}/pub/social/v1/mutes/{OTHER_USER}.json"),
            format!("pubky://{USER}/pub/social/v1/"),
            // A v1 kind spelt another way: another case, no `.json`.
            format!("pubky://{USER}/pub/social/v1/Posts/{POST_ID}"),
            format!("pubky://{USER}/pub/social/v1/profile"),
            // A path the v1 canonicaliser rejects (a trailing slash), even one naming no kind.
            format!("pubky://{USER}/pub/social/v1/marketplace/"),
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
            format!("pubky://{USER}/priv/social/v1/marketplace/v1/listings/{LISTING_ID}"),
            format!("pubky://{USER}/priv"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Other, "{address}");
        }
    }

    /// A takedown is recorded under the moderated address: a `social/v1` one as written, a
    /// `pubky.app` or another app's one as the v0 reader rewrites it, in its own folder.
    #[test]
    fn takedown_keys_keep_social_v1_addresses_and_rewrite_v0_ones() {
        let post = post_uri_builder(USER.into(), POST_ID.into());
        let tag = format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}");
        let v1_post = v1_post_uri_builder(USER.into(), POST_ID.into());
        let v1_tag = v1_tag_uri_builder(USER.into(), HASH_ID.into());
        for (address, expected) in [
            (post.clone(), &post),
            (format!("{post}/"), &post),
            (format!("{tag}?x=1"), &tag),
            (v1_post.clone(), &v1_post),
            (v1_tag.clone(), &v1_tag),
        ] {
            assert_eq!(takedown_key(&address).as_ref(), Ok(expected), "{address}");
        }
        for address in [
            format!("pubky://{USER}/pub/social/v2/posts/{POST_ID}"),
            "https://example.com/article".to_string(),
        ] {
            assert!(takedown_key(&address).is_err(), "{address}");
        }
    }

    /// Another scheme's address, and a public path under another app's namespace that is not a
    /// tag file, name a resource outside Nexus's namespaces.
    #[test]
    fn foreign_resources_resolve_to_foreign() {
        for address in [
            // External URLs, whatever the scheme's case.
            "https://example.com/article".to_string(),
            "HTTPS://Example.COM:443/path#frag".to_string(),
            "nostr:note1abc123".to_string(),
            // Another app's objects: a marketplace listing, a mapky review, an event.
            format!("pubky://{OTHER_USER}/pub/marketplace/listings/{POST_ID}"),
            format!("pubky://{OTHER_USER}/pub/mapky/reviews/{POST_ID}"),
            format!("pubky://{OTHER_USER}/pub/eventky.app/events/E001"),
            format!("PUBKY://{OTHER_USER}/pub/eventky.app/events/E001"),
            // A namespace that only shares a prefix with `social` or `pubky.app`.
            format!("pubky://{OTHER_USER}/pub/social.app/posts/{POST_ID}"),
            // Another app's objects under `pubky.app`, which name no v0 kind: the Shop's
            // listing and shop, and an unknown kind.
            format!("pubky://{OTHER_USER}/pub/pubky.app/marketplace/v1/listings/{LISTING_ID}"),
            format!("pubky://{OTHER_USER}/pub/pubky.app/marketplace/v1/shop.json"),
            format!("pubky://{OTHER_USER}/pub/pubky.app/unknown_kind/{POST_ID}"),
            // The same under `social/v1`, which names no v1 kind.
            format!("pubky://{OTHER_USER}/pub/social/v1/marketplace"),
            format!("pubky://{OTHER_USER}/pub/social/v1/marketplace/v1/listings/{LISTING_ID}"),
            format!("pubky://{OTHER_USER}/pub/social/v1/unknown_kind/{POST_ID}"),
        ] {
            assert_eq!(target(&address), ReferenceTarget::Foreign, "{address}");
        }
    }

    /// An address inside Nexus's namespaces that names nothing Nexus references, a private
    /// path, a `pubky://` address with no namespace, and a string that isn't a URL.
    #[test]
    fn addresses_naming_nothing_resolve_to_other() {
        for address in [
            // A `social/vN` epoch the crate does not speak, and a `social` path without one.
            format!("pubky://{USER}/pub/social/v2/posts/{POST_ID}"),
            format!("pubky://{USER}/pub/social/posts/{POST_ID}"),
            // A bare key with a trailing slash, an empty namespace, or a short-form address no
            // parser reads.
            format!("pubky://{USER}/"),
            format!("pubky://{USER}/pub/"),
            format!("pubky{USER}/pub/social/v1/posts/{POST_ID}"),
            // Malformed `pubky.app` addresses.
            format!("pubky://{USER}/pub/pubky.app/follows/not-a-key"),
            "pubky://not-a-key/pub/pubky.app/profile.json".to_string(),
            // A key that doesn't parse under another app's namespace.
            "pubky://not-a-key/pub/mapky/reviews/1".to_string(),
            // Private paths, another app's included.
            format!("pubky://{USER}/priv/pubky.app/profile.json"),
            format!("pubky://{USER}/priv/mapky/reviews/{POST_ID}"),
            // Malformed tag files in another app's folder.
            format!("pubky://{USER}/pub/mapky/tags/"),
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}/extra"),
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
