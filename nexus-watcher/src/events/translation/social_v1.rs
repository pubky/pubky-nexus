//! The `social/v1` translator, over the v1 reader.
//!
//! [`translate_put`] turns a v1 address and its fetched bytes into one variant per PUT handler
//! call; [`translate_del`] turns a v1 address into one variant per DEL handler call. A DEL
//! carries no body, so the keys come from the path alone.
//!
//! A v1 post is one node whatever version its address carries: every version path maps to the
//! post id, the post is stored under its versionless address, and the edit id is never carried.

use crate::errors::EventProcessorError;
use crate::events::inputs::{
    PostInput, PostKind, Reference, ResourceKey, TagInput, UserInput, UserLink,
};
use crate::events::route::resolve_reference;
use crate::events::translation::{TranslatedDel, TranslatedPut};
use pubky_social_specs::{
    epoch_segment, post_uri_builder, ParsedUri, PubkyId, PubkySocialCollectionContent,
    PubkySocialObject, PubkySocialPost, PubkySocialPostKind, PubkySocialTag, PubkySocialUser,
    PubkySocialUserLink, Resource, ValidationCtx,
};

/// Reads `bytes` with the v1 reader for the resource `parsed` names, under the root of the
/// parsed visibility, and pairs the object with the ids from the path. `event_uri` is the
/// event's address, which `parsed` was parsed from; a profile and a tag are stored under it, a
/// post under its versionless address.
///
/// Every error is deterministic and never retried: a resource with no handler is
/// [`EventProcessorError::InvalidEventLine`], a pipeline bug surfacing as a dropped, logged
/// event, since the classifier skips or rejects those first; bytes the reader rejects, the
/// post's reference rules with the event's user as author included, are
/// [`EventProcessorError::SpecValidation`].
pub fn translate_put(
    event_uri: &str,
    parsed: &ParsedUri,
    bytes: &[u8],
) -> Result<TranslatedPut, EventProcessorError> {
    if !has_handler(&parsed.resource) {
        return Err(no_handler(&parsed.resource));
    }
    let ctx = ValidationCtx {
        root: parsed.visibility.root(),
    };
    let object = PubkySocialObject::from_resource(&parsed.resource, bytes, &ctx)
        .map_err(EventProcessorError::SpecValidation)?;
    let user_id = parsed.user_id.clone();
    let uri = event_uri.to_string();
    Ok(match (&parsed.resource, object) {
        (Resource::User, PubkySocialObject::User(user)) => TranslatedPut::User {
            user_id,
            user: UserInput::from_v1(user, uri),
        },
        (Resource::Post { id, .. }, PubkySocialObject::Post(post)) => {
            // The ownership rule needs the author, which the resource alone cannot name.
            post.check_references(&ctx, Some(&user_id))
                .map_err(EventProcessorError::SpecValidation)?;
            TranslatedPut::Post {
                post: PostInput::from_v1(post, &user_id, id),
                key: owned_key(user_id, id),
            }
        }
        (Resource::Follow(followee_id), PubkySocialObject::Follow(_)) => TranslatedPut::Follow {
            user_id,
            followee_id: followee_id.clone(),
        },
        // The existing Resource-tag writer scopes v1 tags to the social namespace.
        (Resource::Tag(tag_id), PubkySocialObject::Tag(tag)) => TranslatedPut::Tag {
            tag: TagInput::from_v1(tag, uri),
            key: owned_key(user_id, tag_id),
            app: Some("social".into()),
        },
        (resource, object) => return Err(unexpected_object(resource, &object)),
    })
}

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
        // The existing Resource-tag writer scopes v1 tags to the social namespace.
        Resource::Tag(tag_id) => TranslatedDel::Tag {
            key: owned_key(user_id, tag_id),
            app: Some("social".into()),
        },
        resource => return Err(no_handler(resource)),
    })
}

/// Whether a v1 resource has a PUT handler: the profile, a stored post version, a follow or a
/// tag, the same set [`translate_del`] takes.
///
/// A versionless post address names no stored file; a mute, bookmark, feed, foreign, unknown or
/// unsupported resource, and one the non-exhaustive `Resource` adds later, has no v1 handler.
fn has_handler(resource: &Resource) -> bool {
    // TODO(specs-migration): N5 translates v1 files.
    matches!(
        resource,
        Resource::User
            | Resource::Post {
                version: Some(_),
                ..
            }
            | Resource::Follow(_)
            | Resource::Tag(_)
    )
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

/// The reader read an object of another kind than the resource names, which its
/// resource-to-object mapping never does; a pipeline bug, dropped and logged like
/// [`no_handler`].
fn unexpected_object(resource: &Resource, object: &PubkySocialObject) -> EventProcessorError {
    let (kind, epoch) = (object.kind().wire_name(), epoch_segment());
    EventProcessorError::InvalidEventLine(format!(
        "Resource `{resource}` of epoch `{epoch}` read as a `{kind}` object"
    ))
}

impl UserInput {
    /// The input for a v1 profile stored under `uri`; a missing or `null` link list is empty.
    /// The preserved unknown members are dropped.
    pub fn from_v1(user: PubkySocialUser, uri: String) -> Self {
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

impl From<PubkySocialUserLink> for UserLink {
    fn from(PubkySocialUserLink { title, url, .. }: PubkySocialUserLink) -> Self {
        Self { title, url }
    }
}

impl PostInput {
    /// The input for a version of `author`'s post `post_id`, stored under the versionless post
    /// address whatever version the event named, with its parent, embed and collection items
    /// resolved; the attachments as their addresses, `alt` and `name` dropped.
    pub fn from_v1(post: PubkySocialPost, author: &PubkyId, post_id: &str) -> Self {
        Self {
            uri: post_uri_builder(author.to_string(), post_id.to_string()),
            collection_items: collection_items(&post),
            parent: post.parent.as_deref().map(resolve_reference),
            embed: post.embed.as_deref().map(resolve_reference),
            content: post.content,
            kind: post.kind.into(),
            attachments: post
                .attachments
                .into_iter()
                .map(|attachment| attachment.uri)
                .collect(),
            lock: post.lock,
        }
    }
}

impl From<PubkySocialPostKind> for PostKind {
    fn from(kind: PubkySocialPostKind) -> Self {
        match kind {
            PubkySocialPostKind::Note => Self::Short,
            PubkySocialPostKind::Article => Self::Long,
            PubkySocialPostKind::Image => Self::Image,
            PubkySocialPostKind::Video => Self::Video,
            PubkySocialPostKind::Link => Self::Link,
            PubkySocialPostKind::File => Self::File,
            PubkySocialPostKind::Collection => Self::Collection,
            // The reader keeps an unrecognised kind's spelling; Nexus drops it. A kind the
            // non-exhaustive enum adds later lands here until a pin bump maps it.
            _ => Self::Unknown,
        }
    }
}

/// The items a collection post's content envelope lists, in order, each as its address with
/// the curator's `note` dropped; none for any other kind.
///
/// The reader has already validated a collection's envelope, so content that doesn't parse
/// only reaches here from a direct conversion, and gives no items.
fn collection_items(post: &PubkySocialPost) -> Vec<Reference> {
    if post.kind != PubkySocialPostKind::Collection {
        return Vec::new();
    }
    serde_json::from_str::<PubkySocialCollectionContent>(&post.content)
        .map(|envelope| {
            envelope
                .items
                .iter()
                .map(|item| resolve_reference(&item.uri))
                .collect()
        })
        .unwrap_or_default()
}

impl TagInput {
    /// The input for a v1 tag stored under `uri`, with its target resolved; `created_at` and
    /// the preserved unknown members are dropped.
    pub fn from_v1(tag: PubkySocialTag, uri: String) -> Self {
        Self {
            uri,
            tagged: resolve_reference(&tag.uri),
            label: tag.label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::inputs::ReferenceTarget;
    use crate::events::translation::legacy_v0;
    use pubky_social_specs::legacy_v0::{
        follow_uri_builder as v0_follow_uri_builder, post_uri_builder as v0_post_uri_builder,
        user_uri_builder as v0_user_uri_builder, ExtendedParsedUri, PubkyAppPostKind,
    };
    use pubky_social_specs::traits::HashId;
    use pubky_social_specs::{
        bookmark_uri_builder, feed_uri_builder, file_uri_builder, follow_uri_builder,
        mute_uri_builder, tag_uri_builder, user_uri_builder, PubkySocialAttachment,
        PubkySocialCollectionItem, PubkySocialFollow, Root,
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

    /// The v1 `address`, parsed as the router does for a `social/v1` event.
    fn target(address: &str) -> ParsedUri {
        ParsedUri::try_from(address).unwrap()
    }

    /// Translates a PUT of `bytes` at the v1 `address`.
    fn put_at(address: &str, bytes: &[u8]) -> Result<TranslatedPut, EventProcessorError> {
        translate_put(address, &target(address), bytes)
    }

    /// Translates a DEL at the v1 `address`.
    fn del_at(address: &str) -> Result<TranslatedDel, EventProcessorError> {
        translate_del(&target(address))
    }

    fn bytes<T: serde::Serialize>(object: &T) -> Vec<u8> {
        serde_json::to_vec(object).unwrap()
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

    /// The other user's post, in its `social/v1` address.
    fn v1_post_uri() -> String {
        post_uri_builder(OTHER_USER.into(), POST_ID.into())
    }

    /// The other user's post, in its `pubky.app` address.
    fn v0_post_uri() -> String {
        v0_post_uri_builder(OTHER_USER.into(), POST_ID.into())
    }

    /// The reference to the other user's post, written as `uri`.
    fn post_reference(uri: String) -> Reference {
        Reference {
            uri,
            target: ReferenceTarget::Post(owned_key(pubky_id(OTHER_USER), POST_ID)),
        }
    }

    /// A public v1 media file of `user`.
    fn file_uri(user: &str, ext: &str) -> String {
        file_uri_builder(user.into(), format!("{HASH_ID}.{ext}"))
    }

    fn attachment(uri: String) -> PubkySocialAttachment {
        PubkySocialAttachment::new(uri, None, None)
    }

    fn note(content: &str) -> PubkySocialPost {
        PubkySocialPost::new(
            content.to_string(),
            PubkySocialPostKind::Note,
            None,
            None,
            vec![],
        )
    }

    fn link(title: &str, url: &str) -> UserLink {
        UserLink {
            title: title.to_string(),
            url: url.to_string(),
        }
    }

    fn user_fixture() -> PubkySocialUser {
        PubkySocialUser::new(
            "Alice".to_string(),
            Some("Indexing the social graph".to_string()),
            Some(file_uri(USER, "png")),
            Some(vec![PubkySocialUserLink::new(
                "site".to_string(),
                "https://example.com/alice".to_string(),
            )]),
            Some("online".to_string()),
        )
    }

    /// Translates a PUT of the v1 profile `bytes` to its user id and input.
    fn translate_user(bytes: &[u8]) -> (PubkyId, UserInput) {
        let translated = put_at(&user_uri_builder(USER.into()), bytes).unwrap();
        let TranslatedPut::User { user_id, user } = translated else {
            panic!("expected User, got {translated:?}");
        };
        (user_id, user)
    }

    /// The links the user details built from `input` store, as JSON.
    fn stored_links(input: UserInput) -> serde_json::Value {
        serde_json::to_value(input.into_details(&pubky_id(USER)).links).unwrap()
    }

    /// Translates a PUT of the post version `bytes` at `address` to its key and input.
    fn translate_post_at(address: &str, bytes: &[u8]) -> (ResourceKey, PostInput) {
        let translated = put_at(address, bytes).unwrap();
        let TranslatedPut::Post { key, post } = translated else {
            panic!("expected Post, got {translated:?}");
        };
        (key, post)
    }

    /// Translates a PUT of the post version `bytes` at the plain version path.
    fn translate_post(bytes: &[u8]) -> (ResourceKey, PostInput) {
        translate_post_at(&version_uri(None), bytes)
    }

    /// The attachments the post details built from `input` store, as JSON.
    fn stored_attachments(input: PostInput) -> serde_json::Value {
        serde_json::to_value(input.into_details(&key(POST_ID)).attachments).unwrap()
    }

    fn collection_post(items: Vec<PubkySocialCollectionItem>) -> PubkySocialPost {
        PubkySocialPost::new_collection(
            "Reading list".to_string(),
            Some("Posts worth a second read".to_string()),
            items,
            None,
            None,
        )
    }

    /// Translates a PUT of `tag` at the path its id names, to its key, input and app.
    fn translate_tag(tag: &PubkySocialTag) -> (ResourceKey, TagInput, Option<String>) {
        let address = tag_uri_builder(USER.into(), tag.create_id());
        let translated = put_at(&address, &bytes(tag)).unwrap();
        let TranslatedPut::Tag { key, tag, app } = translated else {
            panic!("expected Tag, got {translated:?}");
        };
        (key, tag, app)
    }

    // ---- profile ------------------------------------------------------------------------

    #[test]
    fn put_user_translates_to_its_input_with_the_path_user() {
        let user = user_fixture();
        let (user_id, input) = translate_user(&bytes(&user));
        assert_eq!(user_id, pubky_id(USER));
        assert_eq!(
            input,
            UserInput {
                uri: user_uri_builder(USER.into()),
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
        let user = PubkySocialUser::new(
            "Alice".to_string(),
            None,
            None,
            Some(vec![
                PubkySocialUserLink::new(
                    "site".to_string(),
                    "https://example.com/alice".to_string(),
                ),
                PubkySocialUserLink::new(
                    "blog".to_string(),
                    "https://blog.example.com/posts".to_string(),
                ),
            ]),
            None,
        );
        let (_, input) = translate_user(&bytes(&user));
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

    // ---- post ---------------------------------------------------------------------------

    #[test]
    fn put_post_translates_to_its_input_with_the_path_ids() {
        let post = PubkySocialPost::new_with_lock(
            "hello".to_string(),
            PubkySocialPostKind::Image,
            Some(v1_post_uri()),
            None,
            vec![attachment(file_uri(USER, "jpg"))],
            Some(format!("pubky://{OTHER_USER}/pub/app.locks/{POST_ID}.json")),
        );
        let (key, input) = translate_post(&bytes(&post));
        assert_eq!(key, self::key(POST_ID));
        assert_eq!(input.uri, post_uri_builder(USER.into(), POST_ID.into()));
        assert_eq!(input.content, post.content);
        assert_eq!(input.kind, PostKind::Image);
        assert_eq!(input.parent, Some(post_reference(v1_post_uri())));
        assert_eq!(input.embed, None);
        assert_eq!(input.collection_items, Vec::new());
        assert_eq!(input.attachments, vec![file_uri(USER, "jpg")]);
        assert_eq!(input.lock, post.lock);
    }

    /// A plain and a labelled version path are versions of one post: the same key, and the
    /// versionless post address as the stored one; the edit id and the label are not carried.
    #[test]
    fn put_post_at_any_version_path_is_keyed_by_the_post_id_and_stored_versionless() {
        let bytes = bytes(&note("hello"));
        let plain = translate_post_at(&version_uri(None), &bytes);
        let labelled = translate_post_at(&version_uri(Some("hello-world")), &bytes);
        assert_eq!(plain, labelled);
        let (key, input) = plain;
        assert_eq!(key, self::key(POST_ID));
        assert_eq!(input.uri, post_uri_builder(USER.into(), POST_ID.into()));
        assert!(!input.uri.contains(EDIT_ID), "{}", input.uri);
    }

    /// A versionless post address names no stored file, so there is nothing to translate.
    #[test]
    fn put_at_a_versionless_post_address_is_invalid_event_line() {
        let address = post_uri_builder(USER.into(), POST_ID.into());
        let put = put_at(&address, &bytes(&note("hello")));
        assert!(
            matches!(put, Err(EventProcessorError::InvalidEventLine(_))),
            "{put:?}"
        );
    }

    #[test]
    fn put_post_kind_translates_to_its_nexus_kind_and_is_stored_as_before() {
        let media = |kind: PubkySocialPostKind| {
            PubkySocialPost::new(
                String::new(),
                kind,
                None,
                None,
                vec![attachment(file_uri(USER, "jpg"))],
            )
        };
        let cases = [
            (note("hello"), PostKind::Short, PubkyAppPostKind::Short),
            (
                PubkySocialPost::new_article(
                    "Title".to_string(),
                    "body".to_string(),
                    None,
                    None,
                    None,
                    vec![],
                    None,
                ),
                PostKind::Long,
                PubkyAppPostKind::Long,
            ),
            (
                media(PubkySocialPostKind::Image),
                PostKind::Image,
                PubkyAppPostKind::Image,
            ),
            (
                media(PubkySocialPostKind::Video),
                PostKind::Video,
                PubkyAppPostKind::Video,
            ),
            (
                PubkySocialPost::new(
                    "https://example.com/article".to_string(),
                    PubkySocialPostKind::Link,
                    None,
                    None,
                    vec![],
                ),
                PostKind::Link,
                PubkyAppPostKind::Link,
            ),
            (
                media(PubkySocialPostKind::File),
                PostKind::File,
                PubkyAppPostKind::File,
            ),
            (
                collection_post(vec![PubkySocialCollectionItem::new(v1_post_uri(), None)]),
                PostKind::Collection,
                PubkyAppPostKind::Collection,
            ),
        ];
        for (post, nexus_kind, stored_kind) in cases {
            let v1_kind = post.kind.clone();
            let (key, input) = translate_post(&bytes(&post));
            assert_eq!(input.kind, nexus_kind, "{v1_kind:?}");
            assert_eq!(input.into_details(&key).kind, stored_kind, "{v1_kind:?}");
        }
    }

    #[test]
    fn post_with_an_unrecognised_kind_carries_the_unknown_kind() {
        // The reader reads the kind as unknown and then rejects the post, so read it directly.
        let bytes = br#"{"content": "hello", "kind": "hyperverse_post", "attachments": []}"#;
        let put = put_at(&version_uri(None), bytes);
        assert!(
            matches!(put, Err(EventProcessorError::SpecValidation(_))),
            "{put:?}"
        );
        let post: PubkySocialPost = serde_json::from_slice(bytes).unwrap();
        assert_eq!(
            post.kind,
            PubkySocialPostKind::Unknown("hyperverse_post".to_string())
        );
        let input = PostInput::from_v1(post, &pubky_id(USER), POST_ID);
        assert_eq!(input.kind, PostKind::Unknown);
        assert_eq!(
            input.into_details(&key(POST_ID)).kind,
            PubkyAppPostKind::Unknown
        );
    }

    /// A reply to, or a repost of, a post written in either epoch's address links to the one
    /// node that post is.
    #[test]
    fn put_post_parent_and_embed_of_either_epoch_resolve_to_the_same_post() {
        let mut keys = Vec::new();
        for address in [v0_post_uri(), v1_post_uri()] {
            let post = PubkySocialPost::new(
                "hello".to_string(),
                PubkySocialPostKind::Note,
                Some(address.clone()),
                Some(address.clone()),
                vec![],
            );
            let (_, input) = translate_post(&bytes(&post));
            assert_eq!(input.parent, Some(post_reference(address.clone())));
            assert_eq!(input.embed, Some(post_reference(address.clone())));
            let parents = input.parents();
            keys.push((
                parents.replied.map(|p| p.key).unwrap(),
                parents.reposted.map(|p| p.key).unwrap(),
            ));
        }
        assert_eq!(keys[0], keys[1]);
    }

    #[test]
    fn put_post_parent_and_embed_outside_posts_resolve_to_their_targets() {
        let url = "https://example.com/article".to_string();
        let file = file_uri(OTHER_USER, "jpg");
        let post = PubkySocialPost::new(
            "hello".to_string(),
            PubkySocialPostKind::Note,
            Some(url.clone()),
            Some(file.clone()),
            vec![],
        );
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input.parent,
            Some(Reference {
                uri: url,
                target: ReferenceTarget::Other,
            })
        );
        assert_eq!(
            input.embed,
            Some(Reference {
                uri: file,
                target: ReferenceTarget::File(owned_key(
                    pubky_id(OTHER_USER),
                    &format!("{HASH_ID}.jpg")
                )),
            })
        );
    }

    #[test]
    fn put_post_with_no_attachments_carries_an_empty_list_and_stores_one() {
        for written in [
            r#"{"content": "hello", "kind": "note"}"#,
            r#"{"content": "hello", "kind": "note", "attachments": []}"#,
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

    /// The attachments are carried as their addresses, in order; `alt` and `name` are dropped.
    #[test]
    fn put_post_attachments_keep_their_order_and_addresses_on_the_input_and_when_stored() {
        let addresses = vec![file_uri(USER, "jpg"), file_uri(USER, "png")];
        let post = PubkySocialPost::new(
            String::new(),
            PubkySocialPostKind::Image,
            None,
            None,
            vec![
                PubkySocialAttachment::new(
                    addresses[0].clone(),
                    Some("a cat".to_string()),
                    Some("cat.jpg".to_string()),
                ),
                attachment(addresses[1].clone()),
            ],
        );
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(input.attachments, addresses);
        assert_eq!(stored_attachments(input), serde_json::json!(addresses));
    }

    /// The reader's reference rules run with the event's user as author: another user's
    /// private file is refused as an attachment, by the root rule under the public root and by
    /// the ownership rule where the root rule is silent.
    #[test]
    fn put_post_attachment_of_another_users_private_file_is_spec_validation() {
        let private = format!("pubky://{OTHER_USER}/priv/social/v1/files/{HASH_ID}.jpg");
        let post = PubkySocialPost::new(
            String::new(),
            PubkySocialPostKind::Image,
            None,
            None,
            vec![attachment(private)],
        );
        let private_path = PubkySocialPost::create_path_in(Root::Priv, POST_ID, EDIT_ID, None);
        for (address, rule) in [
            (version_uri(None), "must not reference a private object: "),
            (
                format!("pubky://{USER}{private_path}"),
                "must not reference a private object of another user: ",
            ),
        ] {
            let put = put_at(&address, &bytes(&post));
            let Err(EventProcessorError::SpecValidation(reason)) = put else {
                panic!("{address}: expected spec validation, got {put:?}");
            };
            assert!(reason.contains(rule), "{address}: {reason}");
        }
    }

    // ---- collection ---------------------------------------------------------------------

    /// An item names the same post in either epoch's address, with or without a note; the
    /// content stays as written, with the notes in it.
    #[test]
    fn put_collection_post_resolves_its_items_of_either_epoch_to_the_same_references() {
        let post = collection_post(vec![
            PubkySocialCollectionItem::new(v0_post_uri(), Some("the original".to_string())),
            PubkySocialCollectionItem::new(v1_post_uri(), Some("migrated".to_string())),
            PubkySocialCollectionItem::new(v1_post_uri(), None),
        ]);
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input.collection_items,
            vec![
                post_reference(v0_post_uri()),
                post_reference(v1_post_uri()),
                post_reference(v1_post_uri()),
            ]
        );
        assert_eq!(input.kind, PostKind::Collection);
        assert_eq!(input.content, post.content);
        assert!(input.content.contains("migrated"), "{}", input.content);
        assert_eq!(
            input.collected_posts(),
            vec![(pubky_id(OTHER_USER), POST_ID.to_string()); 3]
        );
    }

    #[test]
    fn collection_item_outside_posts_resolves_to_its_target() {
        let url = "https://example.com/article".to_string();
        let user = user_uri_builder(OTHER_USER.into());
        let post = collection_post(vec![
            PubkySocialCollectionItem::new(url.clone(), None),
            PubkySocialCollectionItem::new(user.clone(), None),
            PubkySocialCollectionItem::new(v1_post_uri(), None),
        ]);
        let (_, input) = translate_post(&bytes(&post));
        assert_eq!(
            input.collection_items,
            vec![
                Reference {
                    uri: url,
                    target: ReferenceTarget::Other,
                },
                Reference {
                    uri: user,
                    target: ReferenceTarget::User(pubky_id(OTHER_USER)),
                },
                post_reference(v1_post_uri()),
            ]
        );
        assert_eq!(
            input.collected_posts(),
            vec![(pubky_id(OTHER_USER), POST_ID.to_string())]
        );
    }

    #[test]
    fn non_collection_post_has_no_collection_items() {
        // The content is an envelope, but only a collection's items are read.
        let post = PubkySocialPost {
            kind: PubkySocialPostKind::Note,
            ..collection_post(vec![PubkySocialCollectionItem::new(v1_post_uri(), None)])
        };
        let input = PostInput::from_v1(post, &pubky_id(USER), POST_ID);
        assert_eq!(input.collection_items, Vec::new());
    }

    // ---- follow -------------------------------------------------------------------------

    #[test]
    fn put_follow_translates_with_the_followee_from_the_path() {
        let address = follow_uri_builder(USER.into(), OTHER_USER.into());
        let translated = put_at(&address, &bytes(&PubkySocialFollow::new())).unwrap();
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

    /// The reader validates a follow's bytes before the pair is read from the path.
    #[test]
    fn put_follow_with_invalid_bytes_is_spec_validation() {
        let address = follow_uri_builder(USER.into(), OTHER_USER.into());
        for written in [&b"not json"[..], b"{}"] {
            let put = put_at(&address, written);
            assert!(
                matches!(put, Err(EventProcessorError::SpecValidation(_))),
                "{put:?}"
            );
        }
    }

    // ---- tag ----------------------------------------------------------------------------

    /// A tag on a post, in either epoch's address, keys the tag by its id and the post by its
    /// node, in the social Resource-tag scope.
    #[test]
    fn put_tag_on_a_post_translates_to_the_social_resource_scope() {
        for address in [v0_post_uri(), v1_post_uri()] {
            let tag = PubkySocialTag::new(address.clone(), "nexus".to_string());
            let id = tag.create_id();
            let (key, input, app) = translate_tag(&tag);
            assert_eq!(key, self::key(&id), "{address}");
            assert_eq!(
                input,
                TagInput {
                    uri: tag_uri_builder(USER.into(), id),
                    tagged: post_reference(address.clone()),
                    label: "nexus".to_string(),
                },
                "{address}"
            );
            assert_eq!(app.as_deref(), Some("social"), "{address}");
        }
    }

    #[test]
    fn put_tag_on_a_user_resolves_its_target_to_the_user_in_both_forms() {
        for address in [
            user_uri_builder(OTHER_USER.into()),
            format!("pubky://{OTHER_USER}"),
        ] {
            let tag = PubkySocialTag::new(address.clone(), "nexus".to_string());
            assert_eq!(
                translate_tag(&tag).1.tagged,
                Reference {
                    uri: address.clone(),
                    target: ReferenceTarget::User(pubky_id(OTHER_USER)),
                },
                "{address}"
            );
        }
    }

    #[test]
    fn put_tag_on_a_url_resolves_its_target_to_other() {
        let address = "https://example.com/article".to_string();
        let tag = PubkySocialTag::new(address.clone(), "nexus".to_string());
        assert_eq!(
            translate_tag(&tag).1.tagged,
            Reference {
                uri: address,
                target: ReferenceTarget::Other,
            }
        );
    }

    // ---- error classes ------------------------------------------------------------------

    #[test]
    fn put_invalid_bytes_are_spec_validation() {
        for written in [
            &b"not json"[..],
            // An empty note has neither content, embed nor attachments.
            &bytes(&note("")),
        ] {
            let put = put_at(&version_uri(None), written);
            assert!(
                matches!(put, Err(EventProcessorError::SpecValidation(_))),
                "{put:?}"
            );
        }
    }

    /// A versionless post address names no stored file, and the resources with no v1 handler
    /// yet fail the same way on PUT and DEL: deterministic, never retried, nothing guessed.
    #[test]
    fn unhandled_resources_are_invalid_event_line_for_put_and_del() {
        let profile = bytes(&user_fixture());
        for address in [
            post_uri_builder(USER.into(), POST_ID.into()),
            file_uri(USER, "jpg"),
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
            let put = put_at(&address, &profile);
            let del = del_at(&address);
            assert!(
                matches!(put, Err(EventProcessorError::InvalidEventLine(_))),
                "{address}: {put:?}"
            );
            assert!(
                matches!(del, Err(EventProcessorError::InvalidEventLine(_))),
                "{address}: {del:?}"
            );
        }
    }

    // ---- DEL paths ----------------------------------------------------------------------

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
                    app: Some("social".into()),
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

    /// A v1 DEL of a profile, a post or a follow is the operation the `pubky.app` DEL
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
}
