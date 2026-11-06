//! One attachment's address after the flip: the pure rewrite the migrator applies to every
//! reference, mirrored so each address Nexus stores equals the one the migrator wrote.
//!
//! The owner's `files/<tsid>` in the file map and the owner's `blobs/{hash}` become
//! `social/v1/files/{hash}.{ext}`, an own blob no file names under `bin`. A post, profile or
//! follow becomes its v1 URI, host unchanged, whether or not that user has migrated. Anything
//! else keeps its canonical form: another user's file or blob, another app's path, a tag, a
//! feed, a web or external URL. A spelling nothing can canonicalize stays as written. The input
//! is trimmed first, as the migrator trims it.
//!
//! The crate's public helpers do the work. The migrator's `fold_scheme` is private, so this
//! module carries its own copy.

use super::files::FileMap;
use pubky_social_specs::legacy_v0::{ParsedUri, Resource, APP_PATH, PUBLIC_PATH};
use pubky_social_specs::{
    canonicalize_external_uri, canonicalize_pubky_uri, canonicalize_web_uri, file_uri_builder,
    follow_uri_builder, frozen_trim, post_uri_builder, user_uri_builder, PubkyId, PROTOCOL,
};

/// The address `uri` takes in `owner`'s migrated post, given `owner`'s file map.
pub fn rewrite(owner: &PubkyId, files: &FileMap, uri: &str) -> String {
    let folded = fold_scheme(frozen_trim(uri));
    let uri = folded.as_str();
    if !uri.starts_with("pubky") {
        let canonical = if uri.starts_with("http://") || uri.starts_with("https://") {
            canonicalize_web_uri(uri)
        } else {
            canonicalize_external_uri(uri)
        };
        return canonical.unwrap_or_else(|_| uri.to_string());
    }
    // A spelling nothing can canonicalize stays as written
    let Ok(canonical) = canonicalize_pubky_uri(uri) else {
        return uri.to_string();
    };
    let rest = &canonical[PROTOCOL.len()..];
    let Some((host, path)) = rest.split_once('/') else {
        return canonical;
    };
    // The legacy app folder under the public root, where every v0 object lived
    let legacy = format!("{}{APP_PATH}", PUBLIC_PATH.trim_start_matches('/'));
    if !path.starts_with(&legacy) {
        return canonical;
    }
    let Ok(parsed) = ParsedUri::try_from(canonical.as_str()) else {
        return canonical;
    };
    let own = host == owner.as_ref();
    match parsed.resource {
        // Another user's media migrates under their file map, which this run cannot see
        Resource::File(tsid) => match own.then(|| files.hash_of(&tsid)).flatten() {
            Some(hash) => migrated_file_uri(owner, files, hash),
            None => canonical,
        },
        Resource::Blob(hash) if own => migrated_file_uri(owner, files, &hash),
        Resource::User => user_uri_builder(host.to_string()),
        Resource::Post(id) => post_uri_builder(host.to_string(), id),
        Resource::Follow(pk) => follow_uri_builder(host.to_string(), pk.to_string()),
        // No v1 spelling of these resolves; the legacy one does
        Resource::Blob(_)
        | Resource::Tag(_)
        | Resource::Feed(_)
        | Resource::Mute(_)
        | Resource::Bookmark(_)
        | Resource::LastRead
        | Resource::Unknown => canonical,
    }
}

/// The owner's blob `hash` at its migrated address, `social/v1/files/{hash}.{ext}`: the
/// address a tag on that file hashes too.
pub(super) fn migrated_file_uri(owner: &PubkyId, files: &FileMap, hash: &str) -> String {
    let filename = format!("{hash}.{}", files.ext_of(hash));
    file_uri_builder(owner.to_string(), filename)
}

/// A scheme is case-insensitive, and the 0.x reader kept a reference as written, so the fold
/// the external arm applies to its scheme happens before dispatch for every arm. Anything that
/// is not scheme-shaped before the first colon is left alone. A copy of the migrator's private
/// `fold_scheme`.
fn fold_scheme(uri: &str) -> String {
    let Some(colon) = uri.find(':') else {
        return uri.to_string();
    };
    let (scheme, rest) = uri.split_at(colon);
    let mut chars = scheme.chars();
    let shaped = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'));
    if !shaped {
        return uri.to_string();
    }
    [&scheme.to_ascii_lowercase(), rest].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    const TSID: &str = "0032SSN7Q4EVG";
    const UNKNOWN_TSID: &str = "0032SSN7Q4EVH";
    const HASH: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";
    const OTHER_HASH: &str = "2ZK3A1B2C3D4E5F6G7H8J9K0MN";

    fn owner() -> PubkyId {
        PubkyId::try_from(OWNER).unwrap()
    }

    fn files() -> FileMap {
        FileMap::from_files(
            &owner(),
            [(
                TSID.to_string(),
                format!("pubky://{OWNER}/pub/pubky.app/blobs/{HASH}"),
                "image/png".to_string(),
            )],
        )
    }

    fn v0(user: &str, path: &str) -> String {
        format!("pubky://{user}/pub/pubky.app/{path}")
    }

    fn v1(user: &str, path: &str) -> String {
        format!("pubky://{user}/pub/social/v1/{path}")
    }

    #[test]
    fn an_own_file_in_the_map_and_an_own_blob_become_the_v1_file() {
        let expected = v1(OWNER, &format!("files/{HASH}.png"));
        assert_eq!(
            rewrite(&owner(), &files(), &v0(OWNER, &format!("files/{TSID}"))),
            expected
        );
        assert_eq!(
            rewrite(&owner(), &files(), &v0(OWNER, &format!("blobs/{HASH}"))),
            expected
        );
        // The migrator trims first
        assert_eq!(
            rewrite(
                &owner(),
                &files(),
                &format!("  {} \n", v0(OWNER, &format!("files/{TSID}")))
            ),
            expected
        );
    }

    #[test]
    fn an_own_blob_no_file_names_gets_bin() {
        assert_eq!(
            rewrite(
                &owner(),
                &files(),
                &v0(OWNER, &format!("blobs/{OTHER_HASH}"))
            ),
            v1(OWNER, &format!("files/{OTHER_HASH}.bin"))
        );
    }

    #[test]
    fn an_own_file_out_of_the_map_keeps_its_address() {
        let uri = v0(OWNER, &format!("files/{UNKNOWN_TSID}"));
        assert_eq!(rewrite(&owner(), &files(), &uri), uri);
    }

    #[test]
    fn another_users_file_and_blob_keep_their_address() {
        for uri in [
            v0(OTHER, &format!("files/{TSID}")),
            v0(OTHER, &format!("blobs/{HASH}")),
        ] {
            assert_eq!(rewrite(&owner(), &files(), &uri), uri);
        }
    }

    #[test]
    fn a_post_profile_and_follow_take_their_v1_uri_host_unchanged() {
        for user in [OWNER, OTHER] {
            assert_eq!(
                rewrite(&owner(), &files(), &v0(user, "posts/0032SSN7Q4EVG")),
                v1(user, "posts/0032SSN7Q4EVG")
            );
            assert_eq!(
                rewrite(&owner(), &files(), &v0(user, "profile.json")),
                v1(user, "profile.json")
            );
            assert_eq!(
                rewrite(&owner(), &files(), &v0(user, &format!("follows/{OTHER}"))),
                v1(user, &format!("follows/{OTHER}.json"))
            );
        }
    }

    #[test]
    fn everything_else_keeps_its_canonical_form() {
        let kept = [
            v0(OWNER, "tags/0032SSN7Q4EVG"),
            v0(OWNER, "feeds/0032SSN7Q4EVG"),
            v0(OWNER, "bookmarks/0032SSN7Q4EVG"),
            v0(OWNER, "last_read"),
            format!("pubky://{OWNER}/pub/other.app/items/1"),
            format!("pubky://{OWNER}"),
            v1(OWNER, &format!("files/{HASH}.png")),
            "https://example.com/a.png?x=1#top".to_string(),
            "nostr:npub1abc".to_string(),
        ];
        for uri in kept {
            assert_eq!(rewrite(&owner(), &files(), &uri), uri);
        }
        // The SDK short form folds to the full form
        assert_eq!(
            rewrite(&owner(), &files(), &format!("pubky{OWNER}/pub/other.app/x")),
            format!("pubky://{OWNER}/pub/other.app/x")
        );
        // A scheme folds to lowercase
        assert_eq!(
            rewrite(&owner(), &files(), "NOSTR:npub1abc"),
            "nostr:npub1abc"
        );
        assert_eq!(
            rewrite(&owner(), &files(), "HTTPS://example.com/a.png"),
            "https://example.com/a.png"
        );
    }

    #[test]
    fn a_spelling_nothing_can_canonicalize_stays_as_written() {
        for uri in [
            "pubky://not-a-key/pub/pubky.app/posts/0032SSN7Q4EVG",
            "pubky://operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo//pub/pubky.app/posts/x",
            "https://",
            "no scheme here",
            "",
        ] {
            assert_eq!(rewrite(&owner(), &files(), uri), uri);
        }
    }

    #[test]
    fn a_rewrite_is_idempotent() {
        let inputs = [
            v0(OWNER, &format!("files/{TSID}")),
            v0(OWNER, &format!("blobs/{OTHER_HASH}")),
            v0(OTHER, "posts/0032SSN7Q4EVG"),
            v0(OTHER, &format!("files/{TSID}")),
            "https://example.com/a.png".to_string(),
        ];
        for uri in inputs {
            let once = rewrite(&owner(), &files(), &uri);
            assert_eq!(rewrite(&owner(), &files(), &once), once);
        }
    }

    #[test]
    fn fold_scheme_folds_only_a_scheme_shaped_prefix() {
        assert_eq!(fold_scheme("PUBKY://x"), "pubky://x");
        assert_eq!(fold_scheme("Web+App.1-x:rest"), "web+app.1-x:rest");
        // An unshaped prefix is left alone, its case included, as the migrator leaves it
        assert_eq!(fold_scheme("1ABC:rest"), "1ABC:rest");
        assert_eq!(fold_scheme("A B:rest"), "A B:rest");
        assert_eq!(fold_scheme(":Rest"), ":Rest");
        assert_eq!(fold_scheme("nocolon"), "nocolon");
    }
}
