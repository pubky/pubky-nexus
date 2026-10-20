//! Per-epoch translators between homeserver resources and handler calls, one module each.
//!
//! [`TranslatedPut`] and [`TranslatedDel`] are the operations every epoch maps into.
//! [`translate_put`] and [`translate_del`] dispatch an event to the translator of its target's
//! epoch, which gets its own parsed target and never sees the [`EventTarget`]. Skips are not
//! translated: the classifier takes them before the fetch.

pub(super) mod legacy_v0;

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
    /// `app` is set only for another app's universal tag (`/pub/mapky/tags/…`).
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
    /// `app` is set only for another app's universal tag (`/pub/mapky/tags/…`).
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
    }
}

/// Translates a DEL of `target` with the translator of its epoch.
///
/// A resource with no handler is [`EventProcessorError::InvalidEventLine`], as in
/// [`translate_put`].
pub(super) fn translate_del(target: &EventTarget) -> Result<TranslatedDel, EventProcessorError> {
    match target {
        EventTarget::Legacy(parsed) => legacy_v0::translate_del(parsed),
    }
}
