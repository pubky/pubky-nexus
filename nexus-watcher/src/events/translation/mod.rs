//! Per-epoch translators between homeserver resources and handler calls, one module each.
//!
//! [`TranslatedPut`] and [`TranslatedDel`] are the operations every epoch maps into.
//! [`translate_put`] and [`translate_del`] dispatch an event to the translator of its target's
//! epoch, which gets its own parsed target and never sees the [`EventTarget`]. Skips are not
//! translated: the classifier takes them before the fetch. Public v1 profiles, stored post versions,
//! follows and tags pass through to their handlers.

pub(super) mod legacy_v0;
pub(super) mod social_v1;

use crate::errors::EventProcessorError;
use crate::events::inputs::{
    BookmarkInput, FileInput, PostInput, ResourceKey, TagInput, UserInput,
};
use crate::events::target::EventTarget;
use pubky_social_specs::PubkyId;

/// A fetched object with the ids its path carries, one variant per PUT handler.
///
/// Every variant but `Follow`, which reads nothing from the bytes, carries its handler's input,
/// and every input but the bookmark's carries the address its object is stored under.
#[derive(Debug)]
pub enum TranslatedPut {
    User {
        user_id: PubkyId,
        user: UserInput,
    },
    Post {
        key: ResourceKey,
        post: PostInput,
    },
    Follow {
        user_id: PubkyId,
        followee_id: PubkyId,
    },
    /// Resource-tag scope: social for v1; the legacy app name for v0 universal tags.
    Tag {
        key: ResourceKey,
        tag: TagInput,
        app: Option<String>,
    },
    Bookmark {
        key: ResourceKey,
        bookmark: BookmarkInput,
    },
    File {
        key: ResourceKey,
        file: FileInput,
    },
}

/// A deletion with the ids its path carries, one variant per DEL handler.
#[derive(Debug, PartialEq, Eq)]
pub enum TranslatedDel {
    User {
        user_id: PubkyId,
    },
    Post(ResourceKey),
    Follow {
        user_id: PubkyId,
        followee_id: PubkyId,
    },
    /// Resource-tag scope: social for v1; the legacy app name for v0 universal tags.
    Tag {
        key: ResourceKey,
        app: Option<String>,
    },
    Bookmark(ResourceKey),
    File(ResourceKey),
}

/// Translates a PUT of `bytes` at `event_uri` with the translator of `target`'s epoch.
///
/// The errors are the epoch translator's, deterministic and never retried: a resource with no
/// handler is [`EventProcessorError::InvalidEventLine`], bytes the epoch's reader rejects are
/// [`EventProcessorError::SpecValidation`].
pub(super) fn translate_put(
    event_uri: &str,
    target: &EventTarget,
    bytes: &[u8],
) -> Result<TranslatedPut, EventProcessorError> {
    match target {
        EventTarget::Legacy(parsed) => legacy_v0::translate_put(event_uri, parsed, bytes),
        EventTarget::Social(parsed) => social_v1::translate_put(event_uri, parsed, bytes),
    }
}

/// Translates a DEL of `target` with the translator of its epoch.
///
/// A resource with no handler is [`EventProcessorError::InvalidEventLine`], as in
/// [`translate_put`].
pub(super) fn translate_del(target: &EventTarget) -> Result<TranslatedDel, EventProcessorError> {
    match target {
        EventTarget::Legacy(parsed) => legacy_v0::translate_del(parsed),
        EventTarget::Social(parsed) => social_v1::translate_del(parsed),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::route::{route, Routed};
    use pubky_social_specs::{user_uri_builder, PubkySocialPost, PubkySocialUser, Root};

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const POST_ID: &str = "0032SSN7Q4EVG";
    /// A timestamp id later than [`POST_ID`], as a post version's edit id must be.
    const EDIT_ID: &str = "0032SSN7Q4EVJ";

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

    /// A PUT dispatches to its epoch's translator: a `social/v1` profile reaches the v1
    /// translator and becomes the user input stored under the event's address, where the
    /// `pubky.app` reader would read nothing.
    #[test]
    fn social_v1_put_dispatches_to_the_v1_translator() {
        let address = user_uri_builder(USER.into());
        let profile = PubkySocialUser::new("Alice".to_string(), None, None, None, None);
        let bytes = serde_json::to_vec(&profile).unwrap();
        let translated = translate_put(&address, &social(&address), &bytes).unwrap();
        let TranslatedPut::User { user_id, user } = translated else {
            panic!("expected User, got {translated:?}");
        };
        assert_eq!(user_id, PubkyId::try_from(USER).unwrap());
        assert_eq!(user.uri, address);
        assert_eq!(user.name, "Alice");
        // A v1 resource with no handler fails through the same arm, whatever the bytes.
        let address = format!("pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}");
        let put = translate_put(&address, &social(&address), b"{}");
        assert!(
            matches!(put, Err(EventProcessorError::InvalidEventLine(_))),
            "{put:?}"
        );
    }

    /// A DEL dispatches to its epoch's translator: a `social/v1` version path reaches the v1
    /// translator and deletes the post by its id, where the `pubky.app` parser would read
    /// nothing.
    #[test]
    fn social_v1_del_dispatches_to_the_v1_translator() {
        let address = version_uri(Some("hello-world"));
        let expected = TranslatedDel::Post(ResourceKey {
            owner_id: PubkyId::try_from(USER).unwrap(),
            resource_id: POST_ID.to_string(),
        });
        assert_eq!(translate_del(&social(&address)).unwrap(), expected);
        // A v1 resource with no handler fails through the same arm.
        let del = translate_del(&social(&format!(
            "pubky://{USER}/pub/social/v1/unknown_kind/{POST_ID}"
        )));
        assert!(
            matches!(del, Err(EventProcessorError::InvalidEventLine(_))),
            "{del:?}"
        );
    }
}
