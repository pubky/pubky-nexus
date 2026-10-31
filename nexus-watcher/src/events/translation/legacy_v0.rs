//! The `pubky.app` (epoch v0) translator, over the frozen v0 reader.
//!
//! [`translate_put`] turns a v0 address and its fetched bytes into one variant per PUT handler
//! call; [`translate_del`] turns a v0 address into one variant per DEL handler call.
//!
//! A v0 object is stored under the address of its event.

use crate::errors::EventProcessorError;
use crate::events::inputs::{
    BlobSource, BookmarkInput, FileInput, PostInput, PostKind, Reference, ResourceKey, TagInput,
    UserInput, UserLink,
};
use crate::events::route::resolve_reference;
use crate::events::translation::{TranslatedDel, TranslatedPut};
use pubky_social_specs::legacy_v0::traits::Validatable;
use pubky_social_specs::legacy_v0::{
    ExtendedParsedUri, ParsedUri, PubkyAppBlob, PubkyAppBookmark, PubkyAppCollectionContent,
    PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
    PubkyAppUserLink, Resource,
};

/// Reads `bytes` with the frozen v0 reader for the resource `parsed` names and pairs the
/// object with the ids from the path. `event_uri` is the event's address, which `parsed` was
/// parsed from; the object is stored under it.
///
/// Every error is deterministic and never retried: a resource with no handler is
/// [`EventProcessorError::InvalidEventLine`], a pipeline bug surfacing as a dropped, logged
/// event, since parsing skips or rejects those first; bytes the reader rejects, and a file
/// whose `src` is no v0 blob address, are [`EventProcessorError::SpecValidation`].
pub fn translate_put(
    event_uri: &str,
    parsed: &ExtendedParsedUri,
    bytes: &[u8],
) -> Result<TranslatedPut, EventProcessorError> {
    let user_id = parsed.user_id().clone();
    let uri = event_uri.to_string();
    Ok(match parsed.resource() {
        Resource::User => TranslatedPut::User {
            user_id,
            user: UserInput::from_v0(read::<PubkyAppUser>(bytes, "")?, uri),
        },
        Resource::Post(post_id) => TranslatedPut::Post {
            post: PostInput::from_v0(read::<PubkyAppPost>(bytes, post_id)?, uri),
            key: ResourceKey {
                owner_id: user_id,
                resource_id: post_id.clone(),
            },
        },
        Resource::Follow(followee_id) => {
            read::<PubkyAppFollow>(bytes, followee_id)?;
            TranslatedPut::Follow {
                user_id,
                followee_id: followee_id.clone(),
            }
        }
        Resource::Bookmark(bookmark_id) => TranslatedPut::Bookmark {
            bookmark: read::<PubkyAppBookmark>(bytes, bookmark_id)?.into(),
            key: ResourceKey {
                owner_id: user_id,
                resource_id: bookmark_id.clone(),
            },
        },
        Resource::Tag(tag_id) => TranslatedPut::Tag {
            tag: TagInput::from_v0(read::<PubkyAppTag>(bytes, tag_id)?, uri),
            key: ResourceKey {
                owner_id: user_id,
                resource_id: tag_id.clone(),
            },
        },
        Resource::File(file_id) => TranslatedPut::File {
            file: FileInput::from_v0(read::<PubkyAppFile>(bytes, file_id)?, uri)?,
            key: ResourceKey {
                owner_id: user_id,
                resource_id: file_id.clone(),
            },
        },
        resource @ (Resource::LastRead
        | Resource::Feed(_)
        | Resource::Blob(_)
        | Resource::Mute(_)
        | Resource::Unknown) => return Err(no_handler(resource)),
    })
}

/// Maps the ids `parsed` names to the matching delete variant.
///
/// A resource with no handler is [`EventProcessorError::InvalidEventLine`], as in
/// [`translate_put`].
pub fn translate_del(parsed: &ExtendedParsedUri) -> Result<TranslatedDel, EventProcessorError> {
    let user_id = parsed.user_id().clone();
    Ok(match parsed.resource() {
        Resource::User => TranslatedDel::User { user_id },
        Resource::Post(post_id) => TranslatedDel::Post(ResourceKey {
            owner_id: user_id,
            resource_id: post_id.clone(),
        }),
        Resource::Follow(followee_id) => TranslatedDel::Follow {
            user_id,
            followee_id: followee_id.clone(),
        },
        Resource::Bookmark(bookmark_id) => TranslatedDel::Bookmark(ResourceKey {
            owner_id: user_id,
            resource_id: bookmark_id.clone(),
        }),
        Resource::Tag(tag_id) => TranslatedDel::Tag(ResourceKey {
            owner_id: user_id,
            resource_id: tag_id.clone(),
        }),
        Resource::File(file_id) => TranslatedDel::File(ResourceKey {
            owner_id: user_id,
            resource_id: file_id.clone(),
        }),
        resource @ (Resource::LastRead
        | Resource::Feed(_)
        | Resource::Blob(_)
        | Resource::Mute(_)
        | Resource::Unknown) => return Err(no_handler(resource)),
    })
}

/// A resource no handler takes; parsing skips or rejects it before translation.
fn no_handler(resource: &Resource) -> EventProcessorError {
    EventProcessorError::InvalidEventLine(format!(
        "Resource `{resource}` has no handler and must not reach translation"
    ))
}

impl UserInput {
    /// The input for a v0 profile stored under `uri`; a missing or `null` link list is empty.
    pub fn from_v0(user: PubkyAppUser, uri: String) -> Self {
        Self {
            uri,
            name: user.name,
            bio: user.bio,
            image: user.image,
            links: user
                .links
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            status: user.status,
        }
    }
}

impl From<PubkyAppUserLink> for UserLink {
    fn from(PubkyAppUserLink { title, url }: PubkyAppUserLink) -> Self {
        Self { title, url }
    }
}

impl PostInput {
    /// The input for a v0 post stored under `uri`, with its parent, embed and collection items
    /// resolved; a missing or `null` attachment list is empty.
    pub fn from_v0(post: PubkyAppPost, uri: String) -> Self {
        Self {
            uri,
            collection_items: collection_items(&post),
            parent: post.parent.as_deref().map(resolve_reference),
            embed: post.embed.map(|embed| resolve_reference(&embed.uri)),
            content: post.content,
            kind: post.kind.into(),
            attachments: post.attachments.unwrap_or_default(),
            lock: post.lock,
        }
    }
}

impl From<PubkyAppPostKind> for PostKind {
    fn from(kind: PubkyAppPostKind) -> Self {
        match kind {
            PubkyAppPostKind::Short => Self::Short,
            PubkyAppPostKind::Long => Self::Long,
            PubkyAppPostKind::Image => Self::Image,
            PubkyAppPostKind::Video => Self::Video,
            PubkyAppPostKind::Link => Self::Link,
            PubkyAppPostKind::File => Self::File,
            PubkyAppPostKind::Collection => Self::Collection,
            PubkyAppPostKind::Unknown => Self::Unknown,
        }
    }
}

/// The items a collection post's content envelope lists, in order; none for any other kind.
///
/// The reader has already validated a collection's envelope, so content that doesn't parse
/// only reaches here from a direct conversion, and gives no items.
fn collection_items(post: &PubkyAppPost) -> Vec<Reference> {
    if post.kind != PubkyAppPostKind::Collection {
        return Vec::new();
    }
    serde_json::from_str::<PubkyAppCollectionContent>(&post.content)
        .map(|envelope| {
            envelope
                .items
                .iter()
                .map(|item| resolve_reference(item))
                .collect()
        })
        .unwrap_or_default()
}

impl From<PubkyAppBookmark> for BookmarkInput {
    fn from(bookmark: PubkyAppBookmark) -> Self {
        Self {
            bookmarked: resolve_reference(&bookmark.uri),
        }
    }
}

impl FileInput {
    /// The input for a v0 file's metadata stored under `uri`, with its `src` parsed into the
    /// blob it names.
    ///
    /// A `src` that is no v0 blob address is [`EventProcessorError::SpecValidation`]: the
    /// reader admits any URL there, but only a blob can back a file, and that can't change on
    /// a retry.
    pub fn from_v0(file: PubkyAppFile, uri: String) -> Result<Self, EventProcessorError> {
        Ok(Self {
            uri,
            name: file.name,
            blob: blob_source(&file.src)?,
            src: file.src,
            content_type: file.content_type,
            size: file.size,
            created_at: file.created_at,
        })
    }
}

/// The blob a v0 file's `src` names: a `pubky.app/blobs/{blob_id}` address in some user's
/// storage. A `src` that is no `pubky.app` URI (another scheme, another app, a bad user id) is
/// [`EventProcessorError::UnsupportedTarget`], as for any target Nexus can never fetch; a
/// `pubky.app` address of another resource is [`EventProcessorError::SpecValidation`]. Neither
/// is retried.
fn blob_source(src: &str) -> Result<BlobSource, EventProcessorError> {
    let parsed = ParsedUri::try_from(src).map_err(|e| {
        EventProcessorError::UnsupportedTarget(format!("Invalid file source URI {src}: {e}"))
    })?;
    match parsed.resource {
        Resource::Blob(blob_id) => Ok(BlobSource {
            owner_id: parsed.user_id,
            blob_id,
        }),
        resource => Err(EventProcessorError::SpecValidation(format!(
            "File source {src} is a `{resource}` address, not a blob"
        ))),
    }
}

/// Runs the frozen v0 reader over a downloaded blob: `bytes` must hash to `blob_id`, the id
/// its address carries, and be neither empty nor over the spec's cap. A rejection is spec
/// validation, never retried, since the same bytes fail the same way.
///
/// The bytes come back for writing, so a blob is never copied to check it.
pub fn read_blob(bytes: Vec<u8>, blob_id: &str) -> Result<Vec<u8>, EventProcessorError> {
    let blob = PubkyAppBlob::new(bytes);
    blob.validate(Some(blob_id))
        .map_err(EventProcessorError::SpecValidation)?;
    Ok(blob.0)
}

impl TagInput {
    /// The input for a v0 tag stored under `uri`, with its target resolved.
    pub fn from_v0(tag: PubkyAppTag, uri: String) -> Self {
        Self {
            uri,
            tagged: resolve_reference(&tag.uri),
            label: tag.label,
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
    use crate::events::inputs::{Reference, ReferenceTarget};
    use crate::events::retry::RetryScheduler;
    use pubky_social_specs::legacy_v0::traits::{HashId, TimestampId};
    use pubky_social_specs::legacy_v0::{PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppUserLink};
    use pubky_social_specs::PubkyId;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";
    const OTHER_USER: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";
    const POST_ID: &str = "0032SSN7Q4EVG";

    fn pubky_id(id: &str) -> PubkyId {
        PubkyId::try_from(id).unwrap()
    }

    /// The address of the event for `path` in the path user's storage.
    fn event_uri(path: &str) -> String {
        format!("pubky://{USER}/pub/{path}")
    }

    fn target(path: &str) -> ExtendedParsedUri {
        ExtendedParsedUri::try_from(event_uri(path).as_str()).unwrap()
    }

    /// Translates a PUT of `bytes` at `path`, as the dispatcher does for its event.
    fn put_at(path: &str, bytes: &[u8]) -> Result<TranslatedPut, EventProcessorError> {
        translate_put(&event_uri(path), &target(path), bytes)
    }

    fn bytes<T: serde::Serialize>(object: &T) -> Vec<u8> {
        serde_json::to_vec(object).unwrap()
    }

    fn post_uri() -> String {
        format!("pubky://{OTHER_USER}/pub/pubky.app/posts/{POST_ID}")
    }

    fn user_fixture() -> PubkyAppUser {
        PubkyAppUser::new(
            "Alice".to_string(),
            Some("Indexing the social graph".to_string()),
            Some(format!("pubky://{USER}/pub/pubky.app/files/{POST_ID}")),
            Some(vec![PubkyAppUserLink::new(
                "site".to_string(),
                "https://example.com/alice".to_string(),
            )]),
            Some("online".to_string()),
        )
    }

    /// Translates a PUT of the v0 profile `bytes` to its user id and input.
    fn translate_user(bytes: &[u8]) -> (PubkyId, UserInput) {
        let translated = put_at("pubky.app/profile.json", bytes).unwrap();
        let TranslatedPut::User { user_id, user } = translated else {
            panic!("expected User, got {translated:?}");
        };
        (user_id, user)
    }

    fn link(title: &str, url: &str) -> UserLink {
        UserLink {
            title: title.to_string(),
            url: url.to_string(),
        }
    }

    /// The links the user details built from `input` store, as JSON.
    fn stored_links(input: UserInput) -> serde_json::Value {
        serde_json::to_value(input.into_details(&pubky_id(USER)).links).unwrap()
    }

    fn post_fixture() -> PubkyAppPost {
        PubkyAppPost::new_with_lock(
            "hello".to_string(),
            PubkyAppPostKind::Image,
            Some(post_uri()),
            None,
            Some(vec![format!(
                "pubky://{USER}/pub/pubky.app/files/{POST_ID}"
            )]),
            Some(format!("pubky://{OTHER_USER}/pub/lock")),
        )
    }

    fn repost_fixture() -> PubkyAppPost {
        PubkyAppPost::new(
            String::new(),
            PubkyAppPostKind::Short,
            None,
            Some(PubkyAppPostEmbed {
                kind: PubkyAppPostKind::Long,
                uri: post_uri(),
            }),
            None,
        )
    }

    fn post_key() -> ResourceKey {
        owned_key(POST_ID.to_string())
    }

    /// The path of the post the fixtures are put at.
    fn post_path() -> String {
        format!("pubky.app/posts/{POST_ID}")
    }

    fn translate_post(bytes: &[u8]) -> (ResourceKey, PostInput) {
        let translated = put_at(&post_path(), bytes).unwrap();
        let TranslatedPut::Post { key, post } = translated else {
            panic!("expected Post, got {translated:?}");
        };
        (key, post)
    }

    /// The attachments the post details built from `input` store, as JSON.
    fn stored_attachments(input: PostInput) -> serde_json::Value {
        serde_json::to_value(input.into_details(&post_key()).attachments).unwrap()
    }

    fn tag_fixture() -> PubkyAppTag {
        PubkyAppTag::new(post_uri(), "nexus".to_string())
    }

    /// The reference to the post at [`post_uri`].
    fn post_reference() -> Reference {
        Reference {
            uri: post_uri(),
            target: ReferenceTarget::Post(ResourceKey {
                owner_id: pubky_id(OTHER_USER),
                resource_id: POST_ID.to_string(),
            }),
        }
    }

    fn translate_tag(tag: &PubkyAppTag) -> TagInput {
        let path = format!("pubky.app/tags/{}", tag.create_id());
        let translated = put_at(&path, &bytes(tag)).unwrap();
        let TranslatedPut::Tag { tag, .. } = translated else {
            panic!("expected Tag, got {translated:?}");
        };
        tag
    }

    fn translate_bookmark(bookmark: &PubkyAppBookmark) -> BookmarkInput {
        let path = format!("pubky.app/bookmarks/{}", bookmark.create_id());
        let translated = put_at(&path, &bytes(bookmark)).unwrap();
        let TranslatedPut::Bookmark { bookmark, .. } = translated else {
            panic!("expected Bookmark, got {translated:?}");
        };
        bookmark
    }

    /// A resource of the path user, keyed by its id.
    fn owned_key(resource_id: String) -> ResourceKey {
        ResourceKey {
            owner_id: pubky_id(USER),
            resource_id,
        }
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
    fn put_user_translates_to_its_input_with_the_path_user() {
        let user = user_fixture();
        let (user_id, input) = translate_user(&bytes(&user));
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(
            input,
            UserInput {
                uri: event_uri("pubky.app/profile.json"),
                name: user.name,
                bio: user.bio,
                image: user.image,
                links: vec![link("site", "https://example.com/alice")],
                status: user.status,
            }
        );
    }

    #[test]
    fn put_user_with_no_links_carries_an_empty_list_and_stores_one() {
        for written in [
            r#"{"name": "Alice"}"#,
            r#"{"name": "Alice", "links": null}"#,
            r#"{"name": "Alice", "links": []}"#,
        ] {
            let (_, input) = translate_user(written.as_bytes());
            assert_eq!(input.links, Vec::new(), "{written}");
            assert_eq!(stored_links(input), serde_json::json!([]), "{written}");
        }
    }

    #[test]
    fn put_user_links_keep_their_order_and_values_on_the_input_and_when_stored() {
        let bytes = br#"{"name": "Alice", "links": [
            {"title": "site", "url": "https://example.com/alice"},
            {"title": "blog", "url": "https://blog.example.com/posts"}
        ]}"#;
        let (_, input) = translate_user(bytes);
        assert_eq!(
            input.links,
            vec![
                link("site", "https://example.com/alice"),
                link("blog", "https://blog.example.com/posts"),
            ]
        );
        assert_eq!(
            stored_links(input),
            serde_json::json!([
                {"title": "site", "url": "https://example.com/alice"},
                {"title": "blog", "url": "https://blog.example.com/posts"},
            ])
        );
    }

    #[test]
    fn put_post_translates_to_its_input_with_the_path_ids() {
        let post = post_fixture();
        let (key, input) = translate_post(&bytes(&post));
        assert_eq!(key, post_key());
        assert_eq!(input.uri, event_uri(&post_path()));
        assert_eq!(input.content, post.content);
        assert_eq!(input.kind, PostKind::Image);
        assert_eq!(input.parent, Some(post_reference()));
        assert_eq!(input.embed, None);
        assert_eq!(input.collection_items, Vec::new());
        assert_eq!(Some(input.attachments), post.attachments);
        assert_eq!(input.lock, post.lock);
    }

    #[test]
    fn put_post_with_an_embed_carries_only_the_embed_reference() {
        let post = repost_fixture();
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input,
            PostInput {
                uri: event_uri(&post_path()),
                content: String::new(),
                kind: PostKind::Short,
                parent: None,
                embed: Some(post_reference()),
                collection_items: Vec::new(),
                attachments: Vec::new(),
                lock: None,
            }
        );
    }

    /// A reply to, or a repost of, a post written as its `social/v1` address links to the node
    /// the `pubky.app` address of that post names.
    #[test]
    fn put_post_parent_and_embed_at_a_social_v1_address_resolve_to_the_same_post() {
        let address = format!("pubky://{OTHER_USER}/pub/social/v1/posts/{POST_ID}");
        let post = PubkyAppPost::new(
            "hello".to_string(),
            PubkyAppPostKind::Short,
            Some(address.clone()),
            Some(PubkyAppPostEmbed {
                kind: PubkyAppPostKind::Short,
                uri: address.clone(),
            }),
            None,
        );
        let (_, input) = translate_post(&bytes(&post));
        let expected = Reference {
            uri: address,
            ..post_reference()
        };
        assert_eq!(input.parent, Some(expected.clone()));
        assert_eq!(input.embed, Some(expected));
        let v0 = translate_post(&bytes(&post_fixture())).1.parents();
        let v1 = input.parents();
        assert_eq!(v1.replied.map(|p| p.key), v0.replied.map(|p| p.key));
    }

    #[test]
    fn put_post_parent_and_embed_outside_posts_resolve_to_their_targets() {
        let file = format!("pubky://{OTHER_USER}/pub/pubky.app/files/{POST_ID}");
        let url = "https://example.com/article".to_string();
        let post = PubkyAppPost::new(
            "hello".to_string(),
            PubkyAppPostKind::Short,
            Some(url.clone()),
            Some(PubkyAppPostEmbed {
                kind: PubkyAppPostKind::Image,
                uri: file.clone(),
            }),
            None,
        );
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input.parent,
            Some(Reference {
                uri: url,
                target: ReferenceTarget::Foreign,
            })
        );
        assert_eq!(
            input.embed,
            Some(Reference {
                uri: file,
                target: ReferenceTarget::File(ResourceKey {
                    owner_id: pubky_id(OTHER_USER),
                    resource_id: POST_ID.to_string(),
                }),
            })
        );
    }

    fn collection_post(items: &[String]) -> PubkyAppPost {
        let envelope = PubkyAppCollectionContent {
            name: "Reading list".to_string(),
            items: items.to_vec(),
            ..Default::default()
        };
        PubkyAppPost::new(
            serde_json::to_string(&envelope).unwrap(),
            PubkyAppPostKind::Collection,
            None,
            None,
            None,
        )
    }

    #[test]
    fn put_collection_post_resolves_its_items_in_order() {
        let first = format!("pubky://{USER}/pub/pubky.app/posts/{POST_ID}");
        let second = post_uri();
        let (_, input) = translate_post(&bytes(&collection_post(&[first.clone(), second])));
        assert_eq!(
            input.collection_items,
            vec![
                Reference {
                    uri: first,
                    target: ReferenceTarget::Post(post_key()),
                },
                post_reference(),
            ]
        );
    }

    #[test]
    fn collection_item_outside_posts_resolves_to_other() {
        // The reader admits only canonical post items, so convert the object directly.
        let follow = format!("pubky://{USER}/pub/pubky.app/follows/{OTHER_USER}");
        let input = PostInput::from_v0(
            collection_post(&[follow.clone(), post_uri()]),
            event_uri(&post_path()),
        );
        assert_eq!(
            input.collection_items,
            vec![
                Reference {
                    uri: follow,
                    target: ReferenceTarget::Other,
                },
                post_reference(),
            ]
        );
    }

    #[test]
    fn non_collection_post_has_no_collection_items() {
        // The content is an envelope, but only a collection's items are read.
        let post = PubkyAppPost {
            kind: PubkyAppPostKind::Short,
            ..collection_post(&[post_uri()])
        };
        let input = PostInput::from_v0(post, event_uri(&post_path()));
        assert_eq!(input.collection_items, Vec::new());
    }

    #[test]
    fn put_post_kind_translates_to_its_nexus_kind_and_is_stored_as_before() {
        let cases = [
            (PubkyAppPostKind::Short, PostKind::Short),
            (PubkyAppPostKind::Long, PostKind::Long),
            (PubkyAppPostKind::Image, PostKind::Image),
            (PubkyAppPostKind::Video, PostKind::Video),
            (PubkyAppPostKind::Link, PostKind::Link),
            (PubkyAppPostKind::File, PostKind::File),
            (PubkyAppPostKind::Collection, PostKind::Collection),
        ];
        for (v0_kind, nexus_kind) in cases {
            let post = match v0_kind {
                PubkyAppPostKind::Collection => collection_post(&[post_uri()]),
                _ => PubkyAppPost::new("hello".to_string(), v0_kind.clone(), None, None, None),
            };
            let (key, input) = translate_post(&bytes(&post));
            assert_eq!(input.kind, nexus_kind, "{v0_kind:?}");
            assert_eq!(input.into_details(&key).kind, v0_kind);
        }
    }

    #[test]
    fn post_with_an_unrecognised_kind_carries_the_unknown_kind() {
        // The reader reads the kind as unknown and then rejects the post, so read it directly.
        let bytes = br#"{"content": "hello", "kind": "hyperverse_post"}"#;
        assert!(matches!(
            put_at(&post_path(), bytes),
            Err(EventProcessorError::SpecValidation(_))
        ));
        let post: PubkyAppPost = serde_json::from_slice(bytes).unwrap();
        let input = PostInput::from_v0(post, event_uri(&post_path()));
        assert_eq!(input.kind, PostKind::Unknown);
        assert_eq!(
            input.into_details(&post_key()).kind,
            PubkyAppPostKind::Unknown
        );
    }

    #[test]
    fn put_post_with_no_attachments_carries_an_empty_list_and_stores_one() {
        for written in [
            r#"{"content": "hello", "kind": "short"}"#,
            r#"{"content": "hello", "kind": "short", "attachments": null}"#,
            r#"{"content": "hello", "kind": "short", "attachments": []}"#,
        ] {
            let (_, input) = translate_post(written.as_bytes());
            assert_eq!(input.attachments, Vec::<String>::new(), "{written}");
            assert_eq!(
                stored_attachments(input),
                serde_json::json!([]),
                "{written}"
            );
        }
    }

    #[test]
    fn put_post_attachments_keep_their_order_and_values_on_the_input_and_when_stored() {
        let attachments = vec![
            format!("pubky://{USER}/pub/pubky.app/files/{POST_ID}"),
            format!("pubky://{OTHER_USER}/pub/pubky.app/files/{POST_ID}"),
        ];
        let post = PubkyAppPost::new(
            "hello".to_string(),
            PubkyAppPostKind::Image,
            None,
            None,
            Some(attachments.clone()),
        );
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(input.attachments, attachments);
        assert_eq!(stored_attachments(input), serde_json::json!(attachments));
    }

    #[test]
    fn put_follow_translates_with_the_followee_from_the_path() {
        let path = format!("pubky.app/follows/{OTHER_USER}");
        let translated = put_at(&path, &bytes(&PubkyAppFollow::new())).unwrap();
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

    /// A tag translates to its key and an input that names no app, only the address the tag is
    /// stored under.
    #[test]
    fn put_tag_translates_to_its_key_and_input() {
        let tag = tag_fixture();
        let id = tag.create_id();
        let path = format!("pubky.app/tags/{id}");
        let translated = put_at(&path, &bytes(&tag)).unwrap();
        let TranslatedPut::Tag { key, tag: input } = translated else {
            panic!("expected Tag, got {translated:?}");
        };
        assert_eq!(key, owned_key(id));
        assert_eq!(
            input,
            TagInput {
                uri: event_uri(&path),
                tagged: post_reference(),
                label: tag.label,
            }
        );
    }

    #[test]
    fn put_tag_on_a_user_resolves_its_target_to_the_user() {
        let address = format!("pubky://{OTHER_USER}/pub/pubky.app/profile.json");
        let tag = PubkyAppTag::new(address.clone(), "nexus".to_string());
        assert_eq!(
            translate_tag(&tag).tagged,
            Reference {
                uri: address,
                target: ReferenceTarget::User(pubky_id(OTHER_USER)),
            }
        );
    }

    /// A bare `pubky://<pk>` is the v1 user reference: a tag on it tags that user, as a tag on
    /// the user's `profile.json` address does.
    #[test]
    fn put_tag_on_a_bare_key_resolves_its_target_to_the_user() {
        let address = format!("pubky://{OTHER_USER}");
        let tag = PubkyAppTag::new(address.clone(), "nexus".to_string());
        assert_eq!(
            translate_tag(&tag).tagged,
            Reference {
                uri: address,
                target: ReferenceTarget::User(pubky_id(OTHER_USER)),
            }
        );
    }

    /// A reply whose parent is a bare `pubky://<pk>` names the user, which the post handler
    /// rejects as an unsupported target: no reply edge points at a user.
    #[test]
    fn put_post_with_a_bare_key_parent_resolves_it_to_the_user() {
        let address = format!("pubky://{OTHER_USER}");
        let post = PubkyAppPost::new(
            "hello".to_string(),
            PubkyAppPostKind::Short,
            Some(address.clone()),
            None,
            None,
        );
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input.parent,
            Some(Reference {
                uri: address,
                target: ReferenceTarget::User(pubky_id(OTHER_USER)),
            })
        );
    }

    #[test]
    fn put_tag_on_a_url_resolves_its_target_to_foreign() {
        let address = "https://example.com/article".to_string();
        let tag = PubkyAppTag::new(address.clone(), "nexus".to_string());
        assert_eq!(
            translate_tag(&tag).tagged,
            Reference {
                uri: address,
                target: ReferenceTarget::Foreign,
            }
        );
    }

    #[test]
    fn put_bookmark_translates_to_its_input_with_the_path_ids() {
        let bookmark = bookmark_fixture();
        let id = bookmark.create_id();
        let path = format!("pubky.app/bookmarks/{id}");
        let translated = put_at(&path, &bytes(&bookmark)).unwrap();
        let TranslatedPut::Bookmark {
            key,
            bookmark: input,
        } = translated
        else {
            panic!("expected Bookmark, got {translated:?}");
        };
        assert_eq!(key, owned_key(id));
        assert_eq!(
            input,
            BookmarkInput {
                bookmarked: post_reference(),
            }
        );
    }

    #[test]
    fn put_bookmark_of_a_file_resolves_its_target_to_the_file() {
        let address = format!("pubky://{OTHER_USER}/pub/pubky.app/files/{POST_ID}");
        let bookmark = PubkyAppBookmark::new(address.clone());
        assert_eq!(
            translate_bookmark(&bookmark).bookmarked,
            Reference {
                uri: address,
                target: ReferenceTarget::File(ResourceKey {
                    owner_id: pubky_id(OTHER_USER),
                    resource_id: POST_ID.to_string(),
                }),
            }
        );
    }

    /// A `social/v1` address of a post names the same node its `pubky.app` address does.
    #[test]
    fn put_bookmark_of_a_social_v1_post_resolves_its_target_to_the_post() {
        let address = format!("pubky://{OTHER_USER}/pub/social/v1/posts/{POST_ID}");
        let bookmark = PubkyAppBookmark::new(address.clone());
        assert_eq!(
            translate_bookmark(&bookmark).bookmarked,
            Reference {
                uri: address,
                ..post_reference()
            }
        );
    }

    #[test]
    fn put_file_translates_to_its_input_with_the_path_ids() {
        let file = file_fixture();
        let id = file.create_id();
        let path = format!("pubky.app/files/{id}");
        let translated = put_at(&path, &bytes(&file)).unwrap();
        let TranslatedPut::File { key, file: input } = translated else {
            panic!("expected File, got {translated:?}");
        };
        assert_eq!(key, owned_key(id));
        assert_eq!(
            input,
            FileInput {
                uri: event_uri(&path),
                name: file.name,
                src: file.src,
                blob: BlobSource {
                    owner_id: pubky_id(USER),
                    blob_id: POST_ID.to_string(),
                },
                content_type: file.content_type,
                size: file.size,
                created_at: file.created_at,
            }
        );
    }

    /// Translates the PUT of a v0 file whose metadata points at `src`.
    fn put_file_with_src(src: &str) -> Result<TranslatedPut, EventProcessorError> {
        let file = PubkyAppFile {
            src: src.to_string(),
            ..file_fixture()
        };
        let path = format!("pubky.app/files/{}", file.create_id());
        put_at(&path, &bytes(&file))
    }

    /// A file's `src` is parsed once, in translation: the blob keeps its owner, who need not be
    /// the file's owner, and its id, the hash the handler checks the download against.
    #[test]
    fn put_file_source_in_another_users_storage_keeps_that_owner() {
        let src = format!("pubky://{OTHER_USER}/pub/pubky.app/blobs/{POST_ID}");
        let translated = put_file_with_src(&src).unwrap();
        let TranslatedPut::File { file: input, .. } = translated else {
            panic!("expected File, got {translated:?}");
        };
        assert_eq!(input.src, src);
        assert_eq!(
            input.blob,
            BlobSource {
                owner_id: pubky_id(OTHER_USER),
                blob_id: POST_ID.to_string(),
            }
        );
    }

    /// A `src` that is no `pubky.app` URI: a URL of another scheme, an address with no path, a
    /// bad user id, another app's storage, or a v1 file, which the frozen v0 reader does not
    /// speak. The reader admits it, so translation rejects it as an unsupported target, before
    /// any handler could download it, and it is never retried.
    #[test]
    fn put_file_with_a_source_that_is_not_a_pubky_app_uri_is_unsupported_target() {
        for src in [
            "https://example.com/photo.png".to_string(),
            "nostr:npub1example".to_string(),
            format!("pubky://{USER}"),
            "pubky://notakey/pub/pubky.app/blobs/0033SSE3B1FQ0".to_string(),
            format!("pubky://{USER}/pub/mapky/blobs/0033SSE3B1FQ0"),
            format!("pubky://{USER}/pub/other.app/blobs/{POST_ID}"),
            format!("pubky://{USER}/pub/social/v1/files/{POST_ID}.png"),
        ] {
            let result = put_file_with_src(&src);
            assert!(
                matches!(result, Err(EventProcessorError::UnsupportedTarget(_))),
                "{src}: {result:?}"
            );
            let err = result.unwrap_err();
            assert!(
                !RetryScheduler::should_enqueue_related_event(&err),
                "{src}: a deterministic rejection must not be retried"
            );
        }
    }

    /// A `src` that is a `pubky.app` address of anything but a blob: spec validation, before
    /// any download.
    #[test]
    fn put_file_with_a_source_that_is_not_a_blob_address_is_spec_validation() {
        for src in [
            format!("pubky://{USER}/pub/pubky.app/posts/{POST_ID}"),
            format!("pubky://{USER}/pub/pubky.app/files/{POST_ID}"),
            format!("pubky://{USER}/pub/pubky.app/profile.json"),
            format!("pubky://{USER}/pub/pubky.app/blobs/"),
        ] {
            let result = put_file_with_src(&src);
            assert!(
                matches!(result, Err(EventProcessorError::SpecValidation(_))),
                "{src}: {result:?}"
            );
        }
    }

    /// The downloaded bytes must hash to the id in the blob's address; anything else is spec
    /// validation, since the same bytes would fail again.
    #[test]
    fn read_blob_keeps_bytes_that_hash_to_the_id_and_rejects_the_rest() {
        let bytes = b"Hello World!".to_vec();
        let blob_id = PubkyAppBlob::new(bytes.clone()).create_id();
        assert_eq!(read_blob(bytes.clone(), &blob_id).unwrap(), bytes);

        let other_id = PubkyAppBlob::new(b"other bytes".to_vec()).create_id();
        for (bytes, id) in [
            (bytes.clone(), other_id.as_str()),
            (bytes, "INVALIDID"),
            (Vec::new(), blob_id.as_str()),
        ] {
            let result = read_blob(bytes, id);
            assert!(
                matches!(result, Err(EventProcessorError::SpecValidation(_))),
                "{id}: {result:?}"
            );
        }
    }

    // ---- error classes ------------------------------------------------------------------

    #[test]
    fn put_invalid_bytes_are_spec_validation() {
        let path = format!("pubky.app/posts/{POST_ID}");
        let result = put_at(&path, b"not json");
        assert!(
            matches!(result, Err(EventProcessorError::SpecValidation(_))),
            "{result:?}"
        );
    }

    #[test]
    fn put_spec_invalid_object_is_spec_validation() {
        // An empty post has neither content, embed nor attachments.
        let post = PubkyAppPost::new(String::new(), PubkyAppPostKind::Short, None, None, None);
        let path = format!("pubky.app/posts/{POST_ID}");
        let result = put_at(&path, &bytes(&post));
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
            let put = put_at(path, &bytes(&user_fixture()));
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
        let path = "pubky.app/unknown/thing";
        let put = put_at(path, &bytes(&user_fixture()));
        let del = translate_del(&target(path));
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
                TranslatedDel::Post(post_key()),
            ),
            (
                format!("pubky.app/follows/{OTHER_USER}"),
                TranslatedDel::Follow {
                    user_id: user,
                    followee_id: other,
                },
            ),
            (
                format!("pubky.app/tags/{tag_id}"),
                TranslatedDel::Tag(owned_key(tag_id)),
            ),
            (
                format!("pubky.app/bookmarks/{bookmark_id}"),
                TranslatedDel::Bookmark(owned_key(bookmark_id)),
            ),
            (
                format!("pubky.app/files/{file_id}"),
                TranslatedDel::File(owned_key(file_id)),
            ),
        ];
        for (path, expected) in cases {
            assert_eq!(translate_del(&target(&path)).unwrap(), expected, "{path}");
        }
    }
}
