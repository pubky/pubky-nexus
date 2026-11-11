//! The API version as a type: what `/v0` and `/v1` answer differently, and nothing else.
//!
//! [`ApiVersion`] has one marker type per mount, [`V0`] and [`V1`], and an associated type for
//! every shape that differs between them: the response shapes, and the filters a client writes
//! differently. A handler whose shape differs is one generic body over the version's types;
//! the router builder is generic too and runs once per version, so a route is declared once and
//! a fix lands in both versions at once. A route only one version serves is listed with that
//! version ([`ApiVersion::own_routes`]), never behind an `if` in a shared builder, and a route
//! served outside the version's mount but documented with it is listed there too
//! ([`ApiVersion::absolute_docs`]).
//!
//! utoipa's path annotation does not take a generic function: the path struct it generates has
//! no type parameter, so a `V::PostView` body does not resolve (checked against utoipa 5.5).
//! [`versioned_handlers!`] therefore writes the annotation once and emits a thin wrapper per
//! version that calls the shared body, each in a module where `shapes` names that version's
//! types, so the one annotation documents each version's own body, parameters and query. The
//! version mounts and documents the wrappers through [`ApiVersion`]'s route and document
//! functions, so what a version serves and what it documents are read in one place.

use std::fmt;
use std::ops::Deref;

use axum::routing::MethodRouter;
use axum::Router;
use const_format::concatcp;
use nexus_common::models::bootstrap::{AttachmentFiles, Bootstrap};
use nexus_common::models::file::{FileDetails, FileUrls};
use nexus_common::models::notification::Notification;
use nexus_common::models::post::{Bookmark, PostDetails, PostKind};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::tag::search::PrefixFold;
use nexus_common::models::tag::traits::taggers::TaggersTuple;
use nexus_common::models::tag::TagDetails;
use nexus_common::models::user::{UserCounts, UserStream, UserView};
use serde::de::DeserializeOwned;
use serde::Serialize;
use utoipa::{IntoParams, OpenApi, ToSchema};

use super::endpoints::{
    POST_BOOKMARK_ROUTE, RESOURCE_EMBEDS_ROUTE, RESOURCE_REPLIES_ROUTE,
    STREAM_RESOURCES_BY_IDS_ROUTE, USER_REACH_SET_ROUTE, V0_PREFIX, V1_PREFIX,
};
use super::stream::posts::{PostStreamByIdsRequest, StreamSourceKind};
use super::types::TaggersInfoResponse;
use super::{
    bootstrap, file, notification, post, resource, resource_changes, search, stream, tag, user,
};
use crate::models::{
    v1, IncludeAttachmentMetadata, KindToken, PostStreamDetailed, PostViewDetailed,
    StoredKindToken, StoredPostKinds, TagLabel, TagLabels, Tags, V0Shape,
};
use crate::routes::AppState;

/// One API version: its mount prefix, the shapes that are its own, and the annotated handlers
/// that serve and document them.
///
/// Every response type converts from the model the shared handler body produces, so a body is
/// written once and ends in `V::Shape::from(model)`; every request type converts into the
/// stored model the body queries with, so it starts in `query.kind.into()`.
pub trait ApiVersion: Send + Sync + 'static {
    /// The mount prefix, `/v0` or `/v1`.
    const PREFIX: &'static str;
    /// The version's standing, appended to its OpenAPI document's description so a client
    /// reads it next to the routes: `/v0` is frozen (fixes only), `/v1` takes every new feature.
    const STANDING: &'static str;

    /// A post with its counts, tags and relationships, `GET /post/{author_id}/{post_id}`.
    /// `/v1` has no `bookmark` and no `attachments_metadata`.
    type PostView: Serialize + ToSchema + From<PostViewDetailed> + Send;
    /// A post's own fields, `GET /post/{author_id}/{post_id}/details`.
    type PostDetails: Serialize + ToSchema + From<PostDetails> + Send;
    /// A list of post views, `GET /stream/posts` and `POST /stream/posts/by_ids`.
    type PostStream: Serialize + ToSchema + From<PostStreamDetailed> + Send;
    /// One notification, listed by `GET /user/{user_id}/notifications`.
    type Notification: Serialize + ToSchema + From<Notification> + Send;
    /// A user's counts, `GET /user/{user_id}/counts`. `/v1` has no `bookmarks`.
    type UserCounts: Serialize + ToSchema + From<UserCounts> + Send;
    /// A user with their counts, tags and relationship, `GET /user/{user_id}`; it embeds
    /// [`Self::UserCounts`].
    type UserView: Serialize + ToSchema + From<UserView> + Send;
    /// A list of user views, `GET /stream/users`, `GET /stream/users/username` and
    /// `POST /stream/users/by_ids`.
    type UserStream: Serialize + ToSchema + From<UserStream> + Send;
    /// The sign-in payload, `GET /bootstrap/{user_id}`; it embeds [`Self::UserStream`],
    /// [`Self::PostStream`]'s views and [`Self::Notification`]s. `/v1` has no `files`.
    type Bootstrap: Serialize + ToSchema + From<Bootstrap> + Send;
    /// A label with its taggers, listed by `GET /user/{user_id}/tags`, `GET
    /// /post/{author_id}/{post_id}/tags` and the resource tag routes, and embedded in
    /// [`Self::PostView`], [`Self::UserView`] and [`Self::ResourceView`]. `/v1` has no
    /// `relationship`: `tag_uri` says whether the viewer tagged the label.
    type TagDetails: Serialize + ToSchema + From<TagDetails> + Send;
    /// The taggers of one label, `GET /user/{user_id}/taggers/{label}`, `GET
    /// /post/{author_id}/{post_id}/taggers/{label}` and `GET
    /// /resource/{resource_id}/tags/{label}/taggers`. `/v1` has no `relationship`.
    type TaggersInfoResponse: Serialize + ToSchema + From<TaggersTuple> + Send;
    /// A resource with a page of its tags, listed by `GET /stream/resources`; it embeds
    /// [`Self::TagDetails`].
    type ResourceView: Serialize + ToSchema + From<ResourceView> + Send;
    /// The envelope of `GET /resource/{resource_id}/tags` and `GET /resource/by-uri`; it embeds
    /// [`Self::TagDetails`].
    type ResourceTagsResponse: Serialize + ToSchema + From<ResourceView> + Send;

    /// A tag label as a client writes it in a path, on the taggers routes, the post search by
    /// tag and the tag search by prefix. `/v0` folds and validates it by the frozen `pubky.app`
    /// rule, `/v1` by the `social/v1` rule: the frozen trim, the ASCII-only lowercase and the
    /// spec crate's validator, with no fallback to the `/v0` fold on a miss.
    type TagLabel: DeserializeOwned
        + ToSchema
        + Deref<Target = str>
        + fmt::Display
        + fmt::Debug
        + Send;
    /// A comma-separated list of labels as a client writes it in `tags=` on the post, resource
    /// and user streams and the user search by tags, and in `domain_tags=` on the post stream:
    /// one to five of [`Self::TagLabel`], each folded and validated by the version's rule.
    type Tags: DeserializeOwned
        + ToSchema
        + TagLabels
        + Deref<Target = Vec<Self::TagLabel>>
        + fmt::Debug
        + Send;
    /// What the tag search does with a `prefix` after the version's label fold. `/v0`
    /// lowercases it again with the Unicode rule, as it always has; `/v1` passes it as its
    /// fold left it, so a prefix with a non-ASCII capital, which that fold keeps, finds the
    /// labels stored with one.
    const TAG_PREFIX_FOLD: PrefixFold;

    /// Whether the bootstrap's `files` are read: `/v0` serves the list, `/v1` has no file
    /// details and drops it, so it skips the read rather than fetch what it throws away.
    const ATTACHMENT_FILES: AttachmentFiles;

    /// Whether a post's `attachments_metadata` is fetched, as a client asks in
    /// `include_attachment_metadata=` on the post view and the post stream. `/v0` reads a
    /// flag and documents the parameter; `/v1` has no file details, reads nothing under that
    /// name, as it reads any unknown parameter, and documents no parameter.
    type IncludeAttachmentMetadata: DeserializeOwned
        + IntoParams
        + Into<bool>
        + Default
        + Copy
        + fmt::Debug
        + Send;
    /// The body of `POST /stream/posts/by_ids`, converted into the one the shared body reads.
    /// `/v0`'s carries `include_attachment_metadata`; `/v1`'s has no such field.
    type PostStreamByIdsRequest: DeserializeOwned + ToSchema + Into<PostStreamByIdsRequest> + Send;

    /// The source of a post stream as a client writes it in `source=`. `/v0` reads every
    /// source, `/v1` every source but `bookmarks`, which is an unknown name there.
    type StreamSource: DeserializeOwned
        + ToSchema
        + Into<StreamSourceKind>
        + Default
        + Clone
        + fmt::Debug
        + Send;
    /// The names [`Self::StreamSource`] takes, as the parameter description lists them.
    const SOURCE_NAMES: &'static str;
    /// The line of the stream descriptions that says what `source=bookmarks` needs: present on
    /// `/v0`, empty on `/v1`, which has no such source.
    const BOOKMARKS_SOURCE_LINE: &'static str;
    /// What `locked=true` does with replies: left out, except on `/v0`'s bookmark stream.
    const LOCKED_REPLIES_RULE: &'static str;

    /// A post kind as a client writes it in `kind=`, on the post streams and the content
    /// search. `/v0` reads the stored names and any other name as `Unknown`; `/v1` reads the
    /// `social/v1` names and `unknown`, and rejects every other name.
    type PostKind: DeserializeOwned + ToSchema + Into<PostKind> + Clone + fmt::Debug + Send;
    /// The kinds a client lists in `exclude_kinds=` on the post streams. Strict in both
    /// versions; `unknown` is a name only on `/v1`.
    type PostKinds: DeserializeOwned + ToSchema + Into<Vec<PostKind>> + Clone + fmt::Debug + Send;

    /// The names [`Self::PostKind`] and [`Self::PostKinds`] take, as the parameter
    /// descriptions list them.
    const KIND_NAMES: &'static str;
    /// What `kind=` does with any other name, appended to the names: nothing to say on `/v0`,
    /// which reads it as `unknown`; a 400 on `/v1`.
    const KIND_RULE: &'static str;
    /// How many kinds `exclude_kinds=` takes.
    const EXCLUDE_KINDS_RANGE: &'static str;
    /// What `exclude_kinds=` does with a post whose kind is missing or unrecognised.
    const MISSING_KIND_RULE: &'static str;

    /// This version's handler of each route whose response or query is one of the types above.
    /// The generic router builders mount these, so the route declaration stays single.
    fn post_view() -> MethodRouter<AppState>;
    fn post_details() -> MethodRouter<AppState>;
    fn stream_posts() -> MethodRouter<AppState>;
    fn stream_post_keys() -> MethodRouter<AppState>;
    fn stream_posts_by_ids() -> MethodRouter<AppState>;
    fn search_posts_by_content() -> MethodRouter<AppState>;
    fn list_notifications() -> MethodRouter<AppState>;
    fn user_counts() -> MethodRouter<AppState>;
    fn user_view() -> MethodRouter<AppState>;
    fn stream_users() -> MethodRouter<AppState>;
    fn stream_user_ids() -> MethodRouter<AppState>;
    fn stream_username_search() -> MethodRouter<AppState>;
    fn stream_users_by_ids() -> MethodRouter<AppState>;
    fn search_users_by_tags() -> MethodRouter<AppState>;
    fn bootstrap() -> MethodRouter<AppState>;
    fn user_tags() -> MethodRouter<AppState>;
    fn user_taggers() -> MethodRouter<AppState>;
    fn post_tags() -> MethodRouter<AppState>;
    fn post_taggers() -> MethodRouter<AppState>;
    fn tag_taggers() -> MethodRouter<AppState>;
    fn resource_tags() -> MethodRouter<AppState>;
    fn resource_by_uri() -> MethodRouter<AppState>;
    fn resource_taggers() -> MethodRouter<AppState>;
    fn search_posts_by_tag() -> MethodRouter<AppState>;
    fn search_tags_by_prefix() -> MethodRouter<AppState>;
    fn stream_resources() -> MethodRouter<AppState>;
    fn stream_resource_ids() -> MethodRouter<AppState>;

    /// The routes only this version serves, as `(expensive, default)` buckets mounted next to
    /// the shared ones: `/v0` keeps the bookmark route until R1 and the file details routes
    /// for as long as it lives; `/v1` has the routes added since `/v0` froze, which answer in
    /// the `/v1` shapes only. A route leaves a version by moving from the
    /// shared builders into the other version's list here, and its path into that version's
    /// [`Self::docs`], so the compiler and the path parity test both see it.
    fn own_routes() -> (Router<AppState>, Router<AppState>);

    /// The document of those routes with this version's schemas. The version's document is
    /// built on it, so these schemas win over a same-named one a shared route nests.
    fn docs() -> utoipa::openapi::OpenApi;

    /// The routes this version documents from outside its mount, at the absolute paths they
    /// are served at, merged into the version's document after the nesting that prefixes the
    /// rest. `/v1` documents the two `/v1/static` routes the static router serves, so a v1
    /// client reads one document; `/v0` documents none, the `/static` routes go with the
    /// static document.
    fn absolute_docs() -> utoipa::openapi::OpenApi;

    /// The version's whole document as served, once nested and merged. `/v0` is frozen, so
    /// the shared schemas lose what the stored models gained since
    /// ([`freeze_docs`](crate::models::v0_frozen::freeze_docs)); `/v1` serves its own as built.
    fn finish_docs(docs: utoipa::openapi::OpenApi) -> utoipa::openapi::OpenApi;
}

/// The frozen API: fixes only, every new feature lands in v1.
#[derive(Debug)]
pub struct V0;

/// The API the `social/v1` clients read.
#[derive(Debug)]
pub struct V1;

impl ApiVersion for V0 {
    const PREFIX: &'static str = V0_PREFIX;
    const STANDING: &'static str =
        "`/v0` is frozen: fixes only, no new routes or fields. Every new feature lands in `/v1`.";

    type PostView = V0Shape<PostViewDetailed>;
    type PostDetails = PostDetails;
    type PostStream = V0Shape<PostStreamDetailed>;
    type Notification = Notification;
    type UserCounts = UserCounts;
    type UserView = UserView;
    type UserStream = UserStream;
    type Bootstrap = V0Shape<Bootstrap>;
    type TagDetails = TagDetails;
    type TaggersInfoResponse = TaggersInfoResponse;
    type ResourceView = ResourceView;
    type ResourceTagsResponse = resource::ResourceTagsResponse;

    type TagLabel = TagLabel;
    type Tags = Tags<TagLabel>;
    const TAG_PREFIX_FOLD: PrefixFold = PrefixFold::Lowercase;

    const ATTACHMENT_FILES: AttachmentFiles = AttachmentFiles::Fetch;
    type IncludeAttachmentMetadata = IncludeAttachmentMetadata;
    type PostStreamByIdsRequest = PostStreamByIdsRequest;

    type StreamSource = StreamSourceKind;
    const SOURCE_NAMES: &'static str = "following, followers, friends, bookmarks, post_replies, author, author_replies, collection, post_collections, wot, wot_domain, all";
    const BOOKMARKS_SOURCE_LINE: &'static str = "- *bookmarks*: Requires **observer_id**.\n";
    const LOCKED_REPLIES_RULE: &'static str = "Replies are left out, except on `source=bookmarks`.";

    type PostKind = PostKind;
    type PostKinds = StoredPostKinds;

    const KIND_NAMES: &'static str = "short, long, image, video, link, file, collection";
    const KIND_RULE: &'static str = "";
    const EXCLUDE_KINDS_RANGE: &'static str = concatcp!("1-", StoredKindToken::MAX);
    const MISSING_KIND_RULE: &'static str =
        "Posts with a missing or unrecognized kind are never excluded.";

    fn post_view() -> MethodRouter<AppState> {
        axum::routing::get(post::view::post_view_handler_v0)
    }
    fn post_details() -> MethodRouter<AppState> {
        axum::routing::get(post::details::post_details_handler_v0)
    }
    fn stream_posts() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_posts_handler_v0)
    }
    fn stream_post_keys() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_post_keys_handler_v0)
    }
    fn stream_posts_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::posts::stream_posts_by_ids_handler_v0)
    }
    fn search_posts_by_content() -> MethodRouter<AppState> {
        axum::routing::get(search::posts::search_posts_by_content_handler_v0)
    }
    fn list_notifications() -> MethodRouter<AppState> {
        axum::routing::get(notification::list::list_notifications_handler_v0)
    }
    fn user_counts() -> MethodRouter<AppState> {
        axum::routing::get(user::counts::user_counts_handler_v0)
    }
    fn user_view() -> MethodRouter<AppState> {
        axum::routing::get(user::view::user_view_handler_v0)
    }
    fn stream_users() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_users_handler_v0)
    }
    fn stream_user_ids() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_user_ids_handler_v0)
    }
    fn stream_username_search() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_username_search_handler_v0)
    }
    fn stream_users_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::users::stream_users_by_ids_handler_v0)
    }
    fn search_users_by_tags() -> MethodRouter<AppState> {
        axum::routing::get(search::users::search_users_by_tags_handler_v0)
    }
    fn bootstrap() -> MethodRouter<AppState> {
        axum::routing::get(bootstrap::bootstrap_handler_v0)
    }
    fn user_tags() -> MethodRouter<AppState> {
        axum::routing::get(user::tags::user_tags_handler_v0)
    }
    fn user_taggers() -> MethodRouter<AppState> {
        axum::routing::get(user::tags::user_taggers_handler_v0)
    }
    fn post_tags() -> MethodRouter<AppState> {
        axum::routing::get(post::tags::post_tags_handler_v0)
    }
    fn post_taggers() -> MethodRouter<AppState> {
        axum::routing::get(post::tags::post_taggers_handler_v0)
    }
    fn tag_taggers() -> MethodRouter<AppState> {
        axum::routing::get(tag::global::tag_taggers_handler_v0)
    }
    fn resource_tags() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_tags_handler_v0)
    }
    fn resource_by_uri() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_by_uri_handler_v0)
    }
    fn resource_taggers() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_taggers_handler_v0)
    }
    fn search_posts_by_tag() -> MethodRouter<AppState> {
        axum::routing::get(search::posts::search_posts_by_tag_handler_v0)
    }
    fn search_tags_by_prefix() -> MethodRouter<AppState> {
        axum::routing::get(search::tags::search_tags_by_prefix_handler_v0)
    }
    fn stream_resources() -> MethodRouter<AppState> {
        axum::routing::get(stream::resources::stream_resources_handler_v0)
    }
    fn stream_resource_ids() -> MethodRouter<AppState> {
        axum::routing::get(stream::resources::stream_resource_ids_handler_v0)
    }

    /// The file details routes, and the bookmark route until R1.
    fn own_routes() -> (Router<AppState>, Router<AppState>) {
        let default = file::routes().route(
            POST_BOOKMARK_ROUTE,
            axum::routing::get(post::bookmark::post_bookmark_handler),
        );
        (file::expensive_routes(), default)
    }

    fn docs() -> utoipa::openapi::OpenApi {
        V0Doc::openapi()
    }

    /// None: the `/static` routes are the static document's.
    fn absolute_docs() -> utoipa::openapi::OpenApi {
        utoipa::openapi::OpenApi::default()
    }

    fn finish_docs(docs: utoipa::openapi::OpenApi) -> utoipa::openapi::OpenApi {
        crate::models::v0_frozen::freeze_docs(docs)
    }
}

impl ApiVersion for V1 {
    const PREFIX: &'static str = V1_PREFIX;
    const STANDING: &'static str =
        "`/v1` is the current API, the one the `social/v1` clients read: every new feature lands here.";

    type PostView = v1::PostView;
    type PostDetails = v1::PostDetails;
    type PostStream = v1::PostStream;
    type Notification = v1::Notification;
    type UserCounts = v1::UserCounts;
    type UserView = v1::UserView;
    type UserStream = v1::UserStream;
    type Bootstrap = v1::Bootstrap;
    type TagDetails = v1::TagDetails;
    type TaggersInfoResponse = v1::TaggersInfoResponse;
    type ResourceView = v1::ResourceView;
    type ResourceTagsResponse = v1::ResourceTagsResponse;

    type TagLabel = v1::TagLabel;
    type Tags = v1::Tags;
    const TAG_PREFIX_FOLD: PrefixFold = PrefixFold::AsGiven;

    const ATTACHMENT_FILES: AttachmentFiles = AttachmentFiles::Skip;
    type IncludeAttachmentMetadata = v1::NoAttachmentMetadata;
    type PostStreamByIdsRequest = v1::PostStreamByIdsRequest;

    type StreamSource = v1::StreamSourceKind;
    const SOURCE_NAMES: &'static str = v1::SOURCE_NAMES;
    const BOOKMARKS_SOURCE_LINE: &'static str = "";
    const LOCKED_REPLIES_RULE: &'static str = "Replies are left out.";

    type PostKind = v1::PostKind;
    type PostKinds = v1::PostKinds;

    const KIND_NAMES: &'static str = v1::KIND_NAMES;
    const KIND_RULE: &'static str =
        "; any other name, `short` and `long` included, is rejected with 400";
    const EXCLUDE_KINDS_RANGE: &'static str = concatcp!("1-", v1::V1KindToken::MAX);
    const MISSING_KIND_RULE: &'static str = "Posts with a missing kind are never excluded; `unknown` excludes the posts whose kind the spec reader didn't recognise.";

    fn post_view() -> MethodRouter<AppState> {
        axum::routing::get(post::view::post_view_handler_v1)
    }
    fn post_details() -> MethodRouter<AppState> {
        axum::routing::get(post::details::post_details_handler_v1)
    }
    fn stream_posts() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_posts_handler_v1)
    }
    fn stream_post_keys() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_post_keys_handler_v1)
    }
    fn stream_posts_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::posts::stream_posts_by_ids_handler_v1)
    }
    fn search_posts_by_content() -> MethodRouter<AppState> {
        axum::routing::get(search::posts::search_posts_by_content_handler_v1)
    }
    fn list_notifications() -> MethodRouter<AppState> {
        axum::routing::get(notification::list::list_notifications_handler_v1)
    }
    fn user_counts() -> MethodRouter<AppState> {
        axum::routing::get(user::counts::user_counts_handler_v1)
    }
    fn user_view() -> MethodRouter<AppState> {
        axum::routing::get(user::view::user_view_handler_v1)
    }
    fn stream_users() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_users_handler_v1)
    }
    fn stream_user_ids() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_user_ids_handler_v1)
    }
    fn stream_username_search() -> MethodRouter<AppState> {
        axum::routing::get(stream::users::stream_username_search_handler_v1)
    }
    fn stream_users_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::users::stream_users_by_ids_handler_v1)
    }
    fn search_users_by_tags() -> MethodRouter<AppState> {
        axum::routing::get(search::users::search_users_by_tags_handler_v1)
    }
    fn bootstrap() -> MethodRouter<AppState> {
        axum::routing::get(bootstrap::bootstrap_handler_v1)
    }
    fn user_tags() -> MethodRouter<AppState> {
        axum::routing::get(user::tags::user_tags_handler_v1)
    }
    fn user_taggers() -> MethodRouter<AppState> {
        axum::routing::get(user::tags::user_taggers_handler_v1)
    }
    fn post_tags() -> MethodRouter<AppState> {
        axum::routing::get(post::tags::post_tags_handler_v1)
    }
    fn post_taggers() -> MethodRouter<AppState> {
        axum::routing::get(post::tags::post_taggers_handler_v1)
    }
    fn tag_taggers() -> MethodRouter<AppState> {
        axum::routing::get(tag::global::tag_taggers_handler_v1)
    }
    fn resource_tags() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_tags_handler_v1)
    }
    fn resource_by_uri() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_by_uri_handler_v1)
    }
    fn resource_taggers() -> MethodRouter<AppState> {
        axum::routing::get(resource::resource_taggers_handler_v1)
    }
    fn search_posts_by_tag() -> MethodRouter<AppState> {
        axum::routing::get(search::posts::search_posts_by_tag_handler_v1)
    }
    fn search_tags_by_prefix() -> MethodRouter<AppState> {
        axum::routing::get(search::tags::search_tags_by_prefix_handler_v1)
    }
    fn stream_resources() -> MethodRouter<AppState> {
        axum::routing::get(stream::resources::stream_resources_handler_v1)
    }
    fn stream_resource_ids() -> MethodRouter<AppState> {
        axum::routing::get(stream::resources::stream_resource_ids_handler_v1)
    }

    /// The routes added since `/v0` froze: the resource views by IDs or URIs, the resource
    /// changes feed, the posts replying to or embedding a resource and a user's reach set. No
    /// bookmark or file details route at all.
    fn own_routes() -> (Router<AppState>, Router<AppState>) {
        let expensive = Router::new().route(
            STREAM_RESOURCES_BY_IDS_ROUTE,
            axum::routing::post(stream::resources::stream_resources_by_ids_handler),
        );
        let default = resource_changes::routes()
            .route(
                RESOURCE_REPLIES_ROUTE,
                axum::routing::get(resource::references::replies),
            )
            .route(
                RESOURCE_EMBEDS_ROUTE,
                axum::routing::get(resource::references::embeds),
            )
            .route(
                USER_REACH_SET_ROUTE,
                axum::routing::get(user::reach_set::user_reach_set_handler),
            );
        (expensive, default)
    }

    fn docs() -> utoipa::openapi::OpenApi {
        V1Doc::openapi()
    }

    /// The two `/v1/static` routes, served by the static router.
    fn absolute_docs() -> utoipa::openapi::OpenApi {
        crate::routes::r#static::V1StaticApiDoc::openapi()
    }

    fn finish_docs(docs: utoipa::openapi::OpenApi) -> utoipa::openapi::OpenApi {
        docs
    }
}

/// Each version's shapes by name, for the wrappers [`versioned_handlers!`] writes: a wrapper
/// is emitted in a module where `shapes` is its version's module here, so one annotation and
/// one signature spell `shapes::PostView` or `Query<shapes::PostStreamQuery>` and each
/// version documents and parses its own. utoipa reads a schema's name from the type at
/// runtime, so an alias of the associated type documents exactly what the handler serializes.
pub mod shapes {
    macro_rules! shapes_of {
        ($version:ty) => {
            use super::super::ApiVersion;

            pub type PostView = <$version as ApiVersion>::PostView;
            pub type PostDetails = <$version as ApiVersion>::PostDetails;
            pub type PostStream = <$version as ApiVersion>::PostStream;
            pub type Notification = <$version as ApiVersion>::Notification;
            pub type UserCounts = <$version as ApiVersion>::UserCounts;
            pub type UserView = <$version as ApiVersion>::UserView;
            pub type UserStream = <$version as ApiVersion>::UserStream;
            pub type Bootstrap = <$version as ApiVersion>::Bootstrap;
            pub type TagDetails = <$version as ApiVersion>::TagDetails;
            pub type TaggersInfoResponse = <$version as ApiVersion>::TaggersInfoResponse;
            pub type ResourceView = <$version as ApiVersion>::ResourceView;
            pub type ResourceTagsResponse = <$version as ApiVersion>::ResourceTagsResponse;
            pub type TagLabel = <$version as ApiVersion>::TagLabel;
            pub type Tags = <$version as ApiVersion>::Tags;
            pub type StreamSource = <$version as ApiVersion>::StreamSource;
            pub type PostKind = <$version as ApiVersion>::PostKind;
            pub type PostKinds = <$version as ApiVersion>::PostKinds;
            pub type IncludeAttachmentMetadata =
                <$version as ApiVersion>::IncludeAttachmentMetadata;
            pub type PostStreamByIdsRequest = <$version as ApiVersion>::PostStreamByIdsRequest;

            /// The post view query with `include_attachment_metadata` as this version reads it.
            pub type PostViewQuery = super::super::post::view::PostViewQuery<$version>;
            /// The post stream query with `kind` and `exclude_kinds` in this version's names,
            /// `tags` and `domain_tags` folded as this version folds a label, and
            /// `include_attachment_metadata` as this version reads it.
            pub type PostStreamQuery = super::super::stream::posts::PostStreamQuery<$version>;
            /// The resource stream, user stream and user search queries with `tags` folded as
            /// this version folds a label.
            pub type ResourceStreamQuery =
                super::super::stream::resources::ResourceStreamQuery<$version>;
            pub type UserStreamQuery = super::super::stream::users::UserStreamQuery<$version>;
            pub type SearchUsersByTagsQuery =
                super::super::search::users::SearchUsersByTagsQuery<$version>;
            /// The content search query with `kind` in this version's names.
            pub type ContentSearchQuery =
                super::super::search::posts::SearchPostsByContentQuery<$version>;
            /// The taggers paths with `label` as this version folds it.
            pub type UserTaggersPath = super::super::user::tags::UserTaggersPath<$version>;
            pub type PostTaggersPath = super::super::post::tags::PostTaggersPath<$version>;
            pub type ResourceTaggersPath = super::super::resource::ResourceTaggersPath<$version>;

            pub const PREFIX: &str = <$version as ApiVersion>::PREFIX;
            pub const SOURCE_NAMES: &str = <$version as ApiVersion>::SOURCE_NAMES;
            pub const BOOKMARKS_SOURCE_LINE: &str = <$version as ApiVersion>::BOOKMARKS_SOURCE_LINE;
            pub const LOCKED_REPLIES_RULE: &str = <$version as ApiVersion>::LOCKED_REPLIES_RULE;
            pub const KIND_NAMES: &str = <$version as ApiVersion>::KIND_NAMES;
            pub const KIND_RULE: &str = <$version as ApiVersion>::KIND_RULE;
            pub const EXCLUDE_KINDS_RANGE: &str = <$version as ApiVersion>::EXCLUDE_KINDS_RANGE;
            pub const MISSING_KIND_RULE: &str = <$version as ApiVersion>::MISSING_KIND_RULE;
        };
    }

    pub mod v0 {
        shapes_of!(super::super::V0);
    }

    pub mod v1 {
        shapes_of!(super::super::V1);
    }
}

/// Two annotated handlers over one shared body, one per version.
///
/// ```ignore
/// versioned_handlers! {
///     /// Doc comment, which utoipa takes as the summary.
///     post_view_handler_v0 / post_view_handler_v1 => post_view(path: Path<PostPath>) -> shapes::PostView;
///     #[utoipa::path(get, path = POST_ROUTE, operation_id = "post_view_handler", params(...))]
///     responses(
///         (status = 200, description = "Post"),
///         (status = 400, description = "Invalid parameters"),
///     )
/// }
/// ```
///
/// `post_view` is the generic body, `async fn post_view<V: ApiVersion>(path: Path<PostPath>)
/// -> Result<impl IntoResponse>`, and `shapes::PostView` the type it returns, named through
/// [`shapes`]: each wrapper lives in a module of its own where `shapes` is its version's module,
/// so the same tokens name `V0`'s type in one wrapper and `V1`'s in the other, in the response,
/// the argument types and the parameter types and descriptions alike. The parent module's
/// names are in scope there too, so a shared type is written as it is: `-> PostKeyStream`,
/// `-> Vec<shapes::Notification>`. The 200 response gets `body = ` that type, the rest of the
/// annotation is written once. Give it an `operation_id`: utoipa would otherwise take the
/// wrapper's name, and the two versions of one route are one operation to a client.
macro_rules! versioned_handlers {
    (
        $(#[$doc:meta])*
        $v0:ident / $v1:ident => $body:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $response:ty;
        #[utoipa::path( $($attr:tt)* )]
        responses( (status = 200, $($ok:tt)*), $($responses:tt)* )
    ) => {
        $crate::routes::v0::version::versioned_handlers!(@one $(#[$doc])* $v0, V0, v0, $body ( $($arg : $ty),* ) -> $response; [$($attr)*] [$($ok)*] [$($responses)*]);
        $crate::routes::v0::version::versioned_handlers!(@one $(#[$doc])* $v1, V1, v1, $body ( $($arg : $ty),* ) -> $response; [$($attr)*] [$($ok)*] [$($responses)*]);
    };
    // The wrapper and the path struct utoipa derives for it, in a module named after the
    // wrapper and re-exported whole, so `module::wrapper_v0` reads as before.
    (@one $(#[$doc:meta])* $name:ident, $version:ident, $shapes:ident, $body:ident ( $($arg:ident : $ty:ty),* ) -> $response:ty; [$($attr:tt)*] [$($ok:tt)*] [$($responses:tt)*]) => {
        #[doc(hidden)]
        pub mod $name {
            use super::*;
            use $crate::routes::v0::version::shapes::$shapes as shapes;

            $(#[$doc])*
            #[utoipa::path($($attr)*, responses((status = 200, body = $response, $($ok)*), $($responses)*))]
            pub async fn $name($($arg: $ty),*) -> $crate::Result<axum::response::Response> {
                $body::<$crate::routes::v0::version::$version>($($arg),*)
                    .await
                    .map(axum::response::IntoResponse::into_response)
            }
        }
        pub use $name::*;
    };
}
pub(crate) use versioned_handlers;

/// The `/v0` routes whose shape, label parameter, label list or several of those are the
/// version's, with the stored models' schemas, and the bookmark and file details routes `/v0`
/// alone serves.
#[derive(OpenApi)]
#[openapi(
    paths(
        post::view::post_view_handler_v0,
        post::details::post_details_handler_v0,
        post::bookmark::post_bookmark_handler,
        file::details::file_details_handler,
        file::list::file_details_by_uris_handler,
        stream::posts::stream_posts_handler_v0,
        stream::posts::stream_post_keys_handler_v0,
        stream::posts::stream_posts_by_ids_handler_v0,
        search::posts::search_posts_by_content_handler_v0,
        notification::list::list_notifications_handler_v0,
        user::counts::user_counts_handler_v0,
        user::view::user_view_handler_v0,
        stream::users::stream_users_handler_v0,
        stream::users::stream_user_ids_handler_v0,
        stream::users::stream_username_search_handler_v0,
        stream::users::stream_users_by_ids_handler_v0,
        search::users::search_users_by_tags_handler_v0,
        bootstrap::bootstrap_handler_v0,
        user::tags::user_tags_handler_v0,
        user::tags::user_taggers_handler_v0,
        post::tags::post_tags_handler_v0,
        post::tags::post_taggers_handler_v0,
        tag::global::tag_taggers_handler_v0,
        resource::resource_tags_handler_v0,
        resource::resource_by_uri_handler_v0,
        resource::resource_taggers_handler_v0,
        search::posts::search_posts_by_tag_handler_v0,
        search::tags::search_tags_by_prefix_handler_v0,
        stream::resources::stream_resources_handler_v0,
        stream::resources::stream_resource_ids_handler_v0,
    ),
    components(schemas(
        PostViewDetailed,
        PostDetails,
        PostStreamDetailed,
        PostStreamByIdsRequest,
        Bookmark,
        FileDetails,
        FileUrls,
        file::list::FilesByIdsBody,
        Notification,
        nexus_common::models::notification::NotificationBody,
        UserCounts,
        UserView,
        UserStream,
        Bootstrap,
        StreamSourceKind,
        PostKind,
        shapes::v0::PostKinds,
        TagDetails,
        TaggersInfoResponse,
        ResourceView,
        resource::ResourceTagsResponse,
        TagLabel,
        shapes::v0::Tags,
    ))
)]
struct V0Doc;

/// The same routes on `/v1`, the bookmark and file details routes left out, with the
/// `social/v1` schemas and the `social/v1` label rule, and the routes `/v1` alone serves.
#[derive(OpenApi)]
#[openapi(
    paths(
        post::view::post_view_handler_v1,
        post::details::post_details_handler_v1,
        stream::posts::stream_posts_handler_v1,
        stream::posts::stream_post_keys_handler_v1,
        stream::posts::stream_posts_by_ids_handler_v1,
        search::posts::search_posts_by_content_handler_v1,
        notification::list::list_notifications_handler_v1,
        user::counts::user_counts_handler_v1,
        user::view::user_view_handler_v1,
        stream::users::stream_users_handler_v1,
        stream::users::stream_user_ids_handler_v1,
        stream::users::stream_username_search_handler_v1,
        stream::users::stream_users_by_ids_handler_v1,
        search::users::search_users_by_tags_handler_v1,
        bootstrap::bootstrap_handler_v1,
        user::tags::user_tags_handler_v1,
        user::tags::user_taggers_handler_v1,
        post::tags::post_tags_handler_v1,
        post::tags::post_taggers_handler_v1,
        tag::global::tag_taggers_handler_v1,
        resource::resource_tags_handler_v1,
        resource::resource_by_uri_handler_v1,
        resource::resource_taggers_handler_v1,
        search::posts::search_posts_by_tag_handler_v1,
        search::tags::search_tags_by_prefix_handler_v1,
        stream::resources::stream_resources_handler_v1,
        stream::resources::stream_resource_ids_handler_v1,
        stream::resources::stream_resources_by_ids_handler,
        resource_changes::resource_changes_handler,
        resource::references::replies,
        resource::references::embeds,
        user::reach_set::user_reach_set_handler,
    ),
    components(schemas(
        v1::PostView,
        v1::PostDetails,
        v1::PostStream,
        v1::PostStreamByIdsRequest,
        v1::Notification,
        v1::NotificationBody,
        v1::UserCounts,
        v1::UserView,
        v1::UserStream,
        v1::Bootstrap,
        v1::StreamSourceKind,
        v1::PostKind,
        v1::PostKinds,
        v1::TagDetails,
        v1::TaggersInfoResponse,
        v1::ResourceView,
        v1::ResourceTagsResponse,
        v1::TagLabel,
        v1::Tags,
        stream::resources::ResourceStreamByIdsRequest,
        nexus_common::models::follow::reach::ReachSet,
    ))
)]
struct V1Doc;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::test_helpers::enum_values;

    /// Each version's names are the ones its `PostKind` schema lists, in that order: a kind
    /// added to the stored enum without a name here fails this test, not the document.
    #[test]
    fn kind_names_match_each_versions_schema() {
        // `/v0` lists the stored names a strict filter parses: every kind but `unknown`.
        let mut v0 = enum_values::<PostKind>();
        assert_eq!(v0.pop().as_deref(), Some("unknown"));
        assert_eq!(V0::KIND_NAMES.split(", ").collect::<Vec<_>>(), v0);
        // `/v1` lists `unknown` too, since its filters take it.
        assert_eq!(
            V1::KIND_NAMES.split(", ").collect::<Vec<_>>(),
            enum_values::<v1::PostKind>()
        );
    }
}
