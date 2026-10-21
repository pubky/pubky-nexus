//! The `social/v1` translator, over the v1 reader.
//!
//! [`translate_del`] turns a v1 address into one variant per DEL handler call. A DEL carries
//! no body, so the keys come from the path alone.
//!
//! A v1 post is one node whatever version its address carries: every version path maps to the
//! post id, and the edit id is never carried.

// TODO(specs-migration): the `social/v1` PUT translator joins this module.

use crate::errors::EventProcessorError;
use crate::events::inputs::ResourceKey;
use crate::events::translation::TranslatedDel;
use pubky_social_specs::{epoch_segment, ParsedUri, PubkyId, Resource};

/// Maps the ids `parsed` names to the matching delete variant.
///
/// Any post version path, plain or labelled, deletes the post by its id: a client deleting a
/// post sends one DEL per version, the first deletes the post and the rest are no-ops in the
/// handler. A resource with no handler is [`EventProcessorError::InvalidEventLine`],
/// deterministic and never retried.
pub fn translate_del(parsed: &ParsedUri) -> Result<TranslatedDel, EventProcessorError> {
    let user_id = parsed.user_id.clone();
    Ok(match &parsed.resource {
        Resource::User => TranslatedDel::User { user_id },
        Resource::Post {
            id,
            version: Some(_),
            ..
        } => TranslatedDel::Post(owned_key(user_id, id)),
        Resource::Follow(followee_id) => TranslatedDel::Follow {
            user_id,
            followee_id: followee_id.clone(),
        },
        // TODO(specs-migration): the tag flow PR decides which `app` a v1 tag carries.
        Resource::Tag(tag_id) => TranslatedDel::Tag {
            key: owned_key(user_id, tag_id),
            app: None,
        },
        // TODO(specs-migration): N5 translates v1 files. A versionless post address names no
        // stored file; a mute, bookmark, feed, foreign, unknown or unsupported resource, and one
        // the non-exhaustive `Resource` adds later, has no v1 handler.
        resource => return Err(no_handler(resource)),
    })
}

/// A resource of the path user, keyed by its id.
fn owned_key(owner_id: PubkyId, resource_id: &str) -> ResourceKey {
    ResourceKey {
        owner_id,
        resource_id: resource_id.to_string(),
    }
}

/// A `social/v1` resource no handler takes; the classifier skips or rejects it before
/// translation.
fn no_handler(resource: &Resource) -> EventProcessorError {
    let epoch = epoch_segment();
    EventProcessorError::InvalidEventLine(format!(
        "Resource `{resource}` of epoch `{epoch}` has no handler and must not reach translation"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::translation::legacy_v0;
    use pubky_social_specs::legacy_v0::{
        follow_uri_builder as v0_follow_uri_builder, post_uri_builder as v0_post_uri_builder,
        tag_uri_builder as v0_tag_uri_builder, user_uri_builder as v0_user_uri_builder,
        ExtendedParsedUri,
    };
    use pubky_social_specs::{
        bookmark_uri_builder, feed_uri_builder, file_uri_builder, follow_uri_builder,
        mute_uri_builder, post_uri_builder, tag_uri_builder, user_uri_builder, PubkySocialPost,
        Root,
    };

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    /// A timestamp id later than [`POST_ID`], as a post version's edit id must be.
    const EDIT_ID: &str = "0032SSN7Q4EVJ";
    const HASH_ID: &str = "8Z8CWH8NVYQY39ZEBFGKQWWEKG";

    fn pubky_id(id: &str) -> PubkyId {
        PubkyId::try_from(id).unwrap()
    }

    /// Translates a DEL at the v1 `address`, parsed as the router does for a `social/v1` event.
    fn del_at(address: &str) -> Result<TranslatedDel, EventProcessorError> {
        translate_del(&ParsedUri::try_from(address).unwrap())
    }

    /// The v1 address of a stored post version, with or without a label.
    fn version_uri(label: Option<&str>) -> String {
        let path = PubkySocialPost::create_path_in(Root::Pub, POST_ID, EDIT_ID, label);
        format!("pubky://{USER}{path}")
    }

    /// A resource of the path user, keyed by its id.
    fn key(resource_id: &str) -> ResourceKey {
        owned_key(pubky_id(USER), resource_id)
    }

    #[test]
    fn del_paths_translate_to_their_variants() {
        let cases = [
            (
                user_uri_builder(USER.into()),
                TranslatedDel::User {
                    user_id: pubky_id(USER),
                },
            ),
            (version_uri(None), TranslatedDel::Post(key(POST_ID))),
            (
                follow_uri_builder(USER.into(), OTHER_USER.into()),
                TranslatedDel::Follow {
                    user_id: pubky_id(USER),
                    followee_id: pubky_id(OTHER_USER),
                },
            ),
            (
                tag_uri_builder(USER.into(), HASH_ID.into()),
                TranslatedDel::Tag {
                    key: key(HASH_ID),
                    app: None,
                },
            ),
        ];
        for (address, expected) in cases {
            assert_eq!(del_at(&address).unwrap(), expected, "{address}");
        }
    }

    /// A plain and a labelled version path delete the same post, keyed by the post id; the
    /// edit id and the label are not carried.
    #[test]
    fn del_of_any_version_path_deletes_the_post_by_its_id() {
        let expected = TranslatedDel::Post(key(POST_ID));
        for address in [version_uri(None), version_uri(Some("hello-world"))] {
            assert_eq!(del_at(&address).unwrap(), expected, "{address}");
        }
    }

    /// A v1 DEL of a profile, a post, a follow or a tag is the operation the `pubky.app` DEL
    /// of that resource translates to, so both epochs reach one handler with one key.
    #[test]
    fn del_translates_to_the_same_operation_as_the_pubky_app_del() {
        let cases = [
            (
                user_uri_builder(USER.into()),
                v0_user_uri_builder(USER.into()),
            ),
            (
                version_uri(Some("hello-world")),
                v0_post_uri_builder(USER.into(), POST_ID.into()),
            ),
            (
                follow_uri_builder(USER.into(), OTHER_USER.into()),
                v0_follow_uri_builder(USER.into(), OTHER_USER.into()),
            ),
            (
                tag_uri_builder(USER.into(), HASH_ID.into()),
                v0_tag_uri_builder(USER.into(), HASH_ID.into()),
            ),
        ];
        for (v1, v0) in cases {
            let legacy = ExtendedParsedUri::try_from(v0.as_str()).unwrap();
            assert_eq!(
                del_at(&v1).unwrap(),
                legacy_v0::translate_del(&legacy).unwrap(),
                "{v1} vs {v0}"
            );
        }
    }

    /// A versionless post address names no stored file, and the resources with no v1 handler
    /// yet fail the same way: deterministic, never retried, nothing guessed.
    #[test]
    fn unhandled_resources_are_invalid_event_line() {
        for address in [
            post_uri_builder(USER.into(), POST_ID.into()),
            file_uri_builder(USER.into(), format!("{HASH_ID}.jpg")),
            mute_uri_builder(USER.into(), OTHER_USER.into()),
            bookmark_uri_builder(USER.into(), "~".to_string() + HASH_ID),
            feed_uri_builder(USER.into(), HASH_ID.into()),
            format!("pubky://{USER}/pub/social/v1/feeds/{HASH_ID}.json"),
            // A foreign namespace, as the v1 parser alone reads it.
            format!("pubky://{USER}/pub/mapky/tags/{HASH_ID}"),
            format!("pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}"),
            // A post version path with a malformed leaf.
            format!("pubky://{USER}/pub/social/v1/posts/{POST_ID}/not-an-id.json"),
            format!("pubky://{USER}/pub/social/v2/posts/{POST_ID}"),
        ] {
            let del = del_at(&address);
            assert!(
                matches!(del, Err(EventProcessorError::InvalidEventLine(_))),
                "{address}: {del:?}"
            );
        }
    }
}
