//! The `pubky.app` (epoch v0) translator, over the frozen v0 reader.
//!
//! Two pure steps turn a fetched resource into a handler call: [`classify`] decides at parse
//! time whether an address is indexed at all, and [`translate_put`] / [`translate_del`] turn an
//! indexed address (and, for a PUT, its bytes) into one variant per handler call. Both read the
//! same table of indexed resources, [`Indexed`], so a resource skipped at parse time can never
//! translate.

use crate::errors::EventProcessorError;
use crate::events::classify::{Classified, SkipReason};
use crate::events::target::EventTarget;
use pubky_social_specs::legacy_v0::traits::Validatable;
use pubky_social_specs::legacy_v0::{
    ExtendedParsedUri, PubkyAppBookmark, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppTag,
    PubkyAppUser, Resource,
};
use pubky_social_specs::PubkyId;
use std::fmt;

/// A fetched `pubky.app` object with the ids its path carries, one variant per PUT handler.
///
/// The variants carry the frozen v0 spec types, so the handlers keep their signatures. None
/// carries the event URI: the dispatcher has it.
#[derive(Debug)]
pub enum TranslatedPut {
    User {
        user_id: PubkyId,
        user: PubkyAppUser,
    },
    Post {
        author_id: PubkyId,
        post_id: String,
        post: PubkyAppPost,
    },
    Follow {
        user_id: PubkyId,
        followee_id: PubkyId,
    },
    /// `app` is set only for another app's universal tag (`/pub/mapky/tags/…`).
    Tag {
        tagger_id: PubkyId,
        tag_id: String,
        tag: PubkyAppTag,
        app: Option<String>,
    },
    Bookmark {
        user_id: PubkyId,
        bookmark_id: String,
        bookmark: PubkyAppBookmark,
    },
    File {
        user_id: PubkyId,
        file_id: String,
        file: PubkyAppFile,
    },
}

/// A `pubky.app` deletion with the ids its path carries, one variant per DEL handler.
#[derive(Debug, PartialEq, Eq)]
pub enum TranslatedDel {
    User {
        user_id: PubkyId,
    },
    Post {
        author_id: PubkyId,
        post_id: String,
    },
    Follow {
        user_id: PubkyId,
        followee_id: PubkyId,
    },
    /// The tag handler keys its delete by the event URI, which the dispatcher has.
    Tag,
    Bookmark {
        user_id: PubkyId,
        bookmark_id: String,
    },
    File {
        user_id: PubkyId,
        file_id: String,
    },
}

/// Index, skip or reject a v0 address; the resources Nexus never indexed are skipped.
pub fn classify(parsed: ExtendedParsedUri) -> Classified {
    match Indexed::try_from(parsed.resource()) {
        Ok(_) => Classified::Index(EventTarget::Legacy(parsed)),
        Err(NotIndexed::Skipped(resource)) => {
            Classified::Skip(SkipReason::LegacyNotIndexed { resource })
        }
        Err(unknown @ NotIndexed::Unknown) => Classified::Invalid(unknown.to_string()),
    }
}

/// Reads `bytes` with the frozen v0 reader for the resource `target` names and pairs the
/// object with the ids from the path.
///
/// Every error is deterministic and never retried: a resource that is skipped at parse time
/// or unknown is [`EventProcessorError::InvalidEventLine`], a pipeline bug surfacing as a
/// dropped, logged event; bytes the reader rejects are
/// [`EventProcessorError::SpecValidation`].
pub fn translate_put(
    target: &EventTarget,
    bytes: &[u8],
) -> Result<TranslatedPut, EventProcessorError> {
    let EventTarget::Legacy(parsed) = target;
    let app = match parsed {
        ExtendedParsedUri::PubkyApp { .. } => None,
        ExtendedParsedUri::UniversalTag { app, .. } => Some(app.as_str()),
    };
    Indexed::try_from(parsed.resource())?.put(parsed.user_id(), app, bytes)
}

/// Maps the ids `target` names to the matching delete variant.
///
/// A resource that is skipped at parse time or unknown is
/// [`EventProcessorError::InvalidEventLine`], as in [`translate_put`].
pub fn translate_del(target: &EventTarget) -> Result<TranslatedDel, EventProcessorError> {
    let EventTarget::Legacy(parsed) = target;
    Ok(Indexed::try_from(parsed.resource())?.del(parsed.user_id()))
}

/// The v0 resources Nexus indexes, with the ids their paths carry.
enum Indexed<'a> {
    User,
    Post(&'a str),
    Follow(&'a PubkyId),
    Bookmark(&'a str),
    Tag(&'a str),
    File(&'a str),
}

/// Why a v0 resource is not indexed.
enum NotIndexed {
    /// Skipped at parse time: last-read, feed, blob or mute. Carries the resource name.
    Skipped(String),
    /// A `pubky.app` path naming no resource the frozen reader knows.
    Unknown,
}

impl<'a> TryFrom<&'a Resource> for Indexed<'a> {
    type Error = NotIndexed;

    fn try_from(resource: &'a Resource) -> Result<Self, NotIndexed> {
        match resource {
            Resource::User => Ok(Self::User),
            Resource::Post(post_id) => Ok(Self::Post(post_id)),
            Resource::Follow(followee_id) => Ok(Self::Follow(followee_id)),
            Resource::Bookmark(bookmark_id) => Ok(Self::Bookmark(bookmark_id)),
            Resource::Tag(tag_id) => Ok(Self::Tag(tag_id)),
            Resource::File(file_id) => Ok(Self::File(file_id)),
            Resource::LastRead | Resource::Feed(_) | Resource::Blob(_) | Resource::Mute(_) => {
                Err(NotIndexed::Skipped(resource.to_string()))
            }
            Resource::Unknown => Err(NotIndexed::Unknown),
        }
    }
}

impl fmt::Display for NotIndexed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skipped(resource) => write!(
                f,
                "Resource `{resource}` is skipped at parse time and must not reach translation"
            ),
            Self::Unknown => f.write_str("Unknown resource in URI"),
        }
    }
}

impl From<NotIndexed> for EventProcessorError {
    fn from(not_indexed: NotIndexed) -> Self {
        Self::InvalidEventLine(not_indexed.to_string())
    }
}

impl Indexed<'_> {
    /// Reads `bytes` as this resource's object and pairs it with the ids from the path.
    fn put(
        self,
        user_id: &PubkyId,
        app: Option<&str>,
        bytes: &[u8],
    ) -> Result<TranslatedPut, EventProcessorError> {
        let user_id = user_id.clone();
        Ok(match self {
            Self::User => TranslatedPut::User {
                user_id,
                user: read(bytes, "")?,
            },
            Self::Post(post_id) => TranslatedPut::Post {
                author_id: user_id,
                post_id: post_id.to_string(),
                post: read(bytes, post_id)?,
            },
            Self::Follow(followee_id) => {
                read::<PubkyAppFollow>(bytes, followee_id)?;
                TranslatedPut::Follow {
                    user_id,
                    followee_id: followee_id.clone(),
                }
            }
            Self::Bookmark(bookmark_id) => TranslatedPut::Bookmark {
                user_id,
                bookmark_id: bookmark_id.to_string(),
                bookmark: read(bytes, bookmark_id)?,
            },
            Self::Tag(tag_id) => TranslatedPut::Tag {
                tagger_id: user_id,
                tag_id: tag_id.to_string(),
                tag: read(bytes, tag_id)?,
                app: app.map(str::to_owned),
            },
            Self::File(file_id) => TranslatedPut::File {
                user_id,
                file_id: file_id.to_string(),
                file: read(bytes, file_id)?,
            },
        })
    }

    /// The delete variant for this resource.
    fn del(self, user_id: &PubkyId) -> TranslatedDel {
        let user_id = user_id.clone();
        match self {
            Self::User => TranslatedDel::User { user_id },
            Self::Post(post_id) => TranslatedDel::Post {
                author_id: user_id,
                post_id: post_id.to_string(),
            },
            Self::Follow(followee_id) => TranslatedDel::Follow {
                user_id,
                followee_id: followee_id.clone(),
            },
            Self::Bookmark(bookmark_id) => TranslatedDel::Bookmark {
                user_id,
                bookmark_id: bookmark_id.to_string(),
            },
            Self::Tag(_) => TranslatedDel::Tag,
            Self::File(file_id) => TranslatedDel::File {
                user_id,
                file_id: file_id.to_string(),
            },
        }
    }
}

/// Runs the frozen v0 reader for one object type; a rejection is spec validation.
fn read<T: Validatable>(bytes: &[u8], id: &str) -> Result<T, EventProcessorError> {
    T::try_from(bytes, id).map_err(EventProcessorError::SpecValidation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_social_specs::legacy_v0::traits::{HashId, TimestampId};
    use pubky_social_specs::legacy_v0::PubkyAppPostKind;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";
    const POST_ID: &str = "0032SSN7Q4EVG";

    fn pubky_id(id: &str) -> PubkyId {
        PubkyId::try_from(id).unwrap()
    }

    fn target(path: &str) -> EventTarget {
        let uri = format!("pubky://{USER}/pub/{path}");
        EventTarget::Legacy(ExtendedParsedUri::try_from(uri.as_str()).unwrap())
    }

    fn bytes<T: serde::Serialize>(object: &T) -> Vec<u8> {
        serde_json::to_vec(object).unwrap()
    }

    fn post_uri() -> String {
        format!("pubky://{OTHER_USER}/pub/pubky.app/posts/{POST_ID}")
    }

    fn user_fixture() -> PubkyAppUser {
        PubkyAppUser::new("Alice".to_string(), None, None, None, None)
    }

    fn post_fixture() -> PubkyAppPost {
        PubkyAppPost::new(
            "hello".to_string(),
            PubkyAppPostKind::Short,
            None,
            None,
            None,
        )
    }

    fn tag_fixture() -> PubkyAppTag {
        PubkyAppTag::new(post_uri(), "nexus".to_string())
    }

    fn bookmark_fixture() -> PubkyAppBookmark {
        PubkyAppBookmark::new(post_uri())
    }

    fn file_fixture() -> PubkyAppFile {
        PubkyAppFile::new(
            "photo.png".to_string(),
            format!("pubky://{USER}/pub/pubky.app/blobs/{POST_ID}"),
            "image/png".to_string(),
            1024,
        )
    }

    // ---- one fixture per v0 object kind ---------------------------------------------------

    #[test]
    fn put_user_translates_with_the_path_user() {
        let user = user_fixture();
        let translated = translate_put(&target("pubky.app/profile.json"), &bytes(&user)).unwrap();
        let TranslatedPut::User {
            user_id,
            user: read,
        } = translated
        else {
            panic!("expected User, got {translated:?}");
        };
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(read.name, user.name);
    }

    #[test]
    fn put_post_translates_with_the_path_ids() {
        let post = post_fixture();
        let target = target(&format!("pubky.app/posts/{POST_ID}"));
        let translated = translate_put(&target, &bytes(&post)).unwrap();
        let TranslatedPut::Post {
            author_id,
            post_id,
            post: read,
        } = translated
        else {
            panic!("expected Post, got {translated:?}");
        };
        assert_eq!(author_id, pubky_id(USER));
        assert_eq!(post_id, POST_ID);
        assert_eq!(read.content, post.content);
    }

    #[test]
    fn put_follow_translates_with_the_followee_from_the_path() {
        let target = target(&format!("pubky.app/follows/{OTHER_USER}"));
        let translated = translate_put(&target, &bytes(&PubkyAppFollow::new())).unwrap();
        let TranslatedPut::Follow {
            user_id,
            followee_id,
        } = translated
        else {
            panic!("expected Follow, got {translated:?}");
        };
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(followee_id, pubky_id(OTHER_USER));
    }

    #[test]
    fn put_pubky_app_tag_translates_without_an_app() {
        let tag = tag_fixture();
        let id = tag.create_id();
        let target = target(&format!("pubky.app/tags/{id}"));
        let translated = translate_put(&target, &bytes(&tag)).unwrap();
        let TranslatedPut::Tag {
            tagger_id,
            tag_id,
            tag: read,
            app,
        } = translated
        else {
            panic!("expected Tag, got {translated:?}");
        };
        assert_eq!(tagger_id, pubky_id(USER));
        assert_eq!(tag_id, id);
        assert_eq!(read.label, tag.label);
        assert_eq!(app, None);
    }

    #[test]
    fn put_universal_tag_translates_with_its_app() {
        let tag = tag_fixture();
        let id = tag.create_id();
        let target = target(&format!("mapky/tags/{id}"));
        let translated = translate_put(&target, &bytes(&tag)).unwrap();
        let TranslatedPut::Tag {
            tagger_id,
            tag_id,
            app,
            ..
        } = translated
        else {
            panic!("expected Tag, got {translated:?}");
        };
        assert_eq!(tagger_id, pubky_id(USER));
        assert_eq!(tag_id, id);
        assert_eq!(app.as_deref(), Some("mapky"));
    }

    #[test]
    fn put_bookmark_translates_with_the_path_ids() {
        let bookmark = bookmark_fixture();
        let id = bookmark.create_id();
        let target = target(&format!("pubky.app/bookmarks/{id}"));
        let translated = translate_put(&target, &bytes(&bookmark)).unwrap();
        let TranslatedPut::Bookmark {
            user_id,
            bookmark_id,
            bookmark: read,
        } = translated
        else {
            panic!("expected Bookmark, got {translated:?}");
        };
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(bookmark_id, id);
        assert_eq!(read.uri, bookmark.uri);
    }

    #[test]
    fn put_file_translates_with_the_path_ids() {
        let file = file_fixture();
        let id = file.create_id();
        let target = target(&format!("pubky.app/files/{id}"));
        let translated = translate_put(&target, &bytes(&file)).unwrap();
        let TranslatedPut::File {
            user_id,
            file_id,
            file: read,
        } = translated
        else {
            panic!("expected File, got {translated:?}");
        };
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(file_id, id);
        assert_eq!(read.name, file.name);
    }

    // ---- error classes ------------------------------------------------------------------

    #[test]
    fn put_invalid_bytes_are_spec_validation() {
        let target = target(&format!("pubky.app/posts/{POST_ID}"));
        let result = translate_put(&target, b"not json");
        assert!(
            matches!(result, Err(EventProcessorError::SpecValidation(_))),
            "{result:?}"
        );
    }

    #[test]
    fn put_spec_invalid_object_is_spec_validation() {
        // An empty post has neither content, embed nor attachments.
        let post = PubkyAppPost::new(String::new(), PubkyAppPostKind::Short, None, None, None);
        let target = target(&format!("pubky.app/posts/{POST_ID}"));
        let result = translate_put(&target, &bytes(&post));
        assert!(
            matches!(result, Err(EventProcessorError::SpecValidation(_))),
            "{result:?}"
        );
    }

    #[test]
    fn skipped_resource_is_invalid_event_line_for_put_and_del() {
        for path in [
            "pubky.app/last_read",
            "pubky.app/feeds/8Z8CWH8NVYQY39ZEBFGKQWWEKG",
            "pubky.app/blobs/0032SSN7Q4EVG",
            &format!("pubky.app/mutes/{OTHER_USER}"),
        ] {
            let put = translate_put(&target(path), &bytes(&user_fixture()));
            let del = translate_del(&target(path));
            assert!(
                matches!(put, Err(EventProcessorError::InvalidEventLine(_))),
                "{path}: {put:?}"
            );
            assert!(
                matches!(del, Err(EventProcessorError::InvalidEventLine(_))),
                "{path}: {del:?}"
            );
        }
    }

    #[test]
    fn unknown_resource_is_invalid_event_line_for_put_and_del() {
        let target = target("pubky.app/unknown/thing");
        let put = translate_put(&target, &bytes(&user_fixture()));
        let del = translate_del(&target);
        assert!(
            matches!(put, Err(EventProcessorError::InvalidEventLine(_))),
            "{put:?}"
        );
        assert!(
            matches!(del, Err(EventProcessorError::InvalidEventLine(_))),
            "{del:?}"
        );
    }

    // ---- DEL paths ----------------------------------------------------------------------

    #[test]
    fn del_paths_translate_to_their_variants() {
        let user = pubky_id(USER);
        let other = pubky_id(OTHER_USER);
        let tag_id = tag_fixture().create_id();
        let bookmark_id = bookmark_fixture().create_id();
        let file_id = file_fixture().create_id();
        let cases = [
            (
                "pubky.app/profile.json".to_string(),
                TranslatedDel::User {
                    user_id: user.clone(),
                },
            ),
            (
                format!("pubky.app/posts/{POST_ID}"),
                TranslatedDel::Post {
                    author_id: user.clone(),
                    post_id: POST_ID.to_string(),
                },
            ),
            (
                format!("pubky.app/follows/{OTHER_USER}"),
                TranslatedDel::Follow {
                    user_id: user.clone(),
                    followee_id: other,
                },
            ),
            (format!("pubky.app/tags/{tag_id}"), TranslatedDel::Tag),
            (format!("mapky/tags/{tag_id}"), TranslatedDel::Tag),
            (
                format!("pubky.app/bookmarks/{bookmark_id}"),
                TranslatedDel::Bookmark {
                    user_id: user.clone(),
                    bookmark_id,
                },
            ),
            (
                format!("pubky.app/files/{file_id}"),
                TranslatedDel::File {
                    user_id: user,
                    file_id,
                },
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(translate_del(&target(&path)).unwrap(), expected, "{path}");
        }
    }
}
