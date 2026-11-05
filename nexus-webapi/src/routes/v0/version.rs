//! The API version as a type: what `/v0` and `/v1` answer differently, and nothing else.
//!
//! [`ApiVersion`] has one marker type per mount, [`V0`] and [`V1`], and an associated type for
//! every shape that differs between them: the response shapes, and the two kind filters a
//! client writes. A handler whose shape differs is one generic body over the version's types;
//! the router builder is generic too and runs once per version, so a route is declared once and
//! a fix lands in both versions at once.
//!
//! utoipa's path annotation does not take a generic function: the path struct it generates has
//! no type parameter, so a `V::PostView` body does not resolve (checked against utoipa 5.5).
//! [`versioned_handlers!`] therefore writes the annotation once and emits a thin wrapper per
//! version that calls the shared body, each in a module where `shapes` names that version's
//! types, so the one annotation documents each version's own body, parameters and query. The
//! version mounts and documents the wrappers through [`ApiVersion`]'s route and document
//! functions, so what a version serves and what it documents are read in one place.

use std::fmt;

use axum::routing::MethodRouter;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::{PostDetails, PostKind};
use serde::de::DeserializeOwned;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

use super::endpoints::{V0_PREFIX, V1_PREFIX};
use super::{notification, post, search, stream};
use crate::models::{v1, PostKinds, PostStreamDetailed, PostViewDetailed};
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

    /// A post with its counts, tags and relationships, `GET /post/{author_id}/{post_id}`.
    type PostView: Serialize + ToSchema + From<PostViewDetailed> + Send;
    /// A post's own fields, `GET /post/{author_id}/{post_id}/details`.
    type PostDetails: Serialize + ToSchema + From<PostDetails> + Send;
    /// A list of post views, `GET /stream/posts` and `POST /stream/posts/by_ids`.
    type PostStream: Serialize + ToSchema + From<PostStreamDetailed> + Send;
    /// One notification, listed by `GET /user/{user_id}/notifications`.
    type Notification: Serialize + ToSchema + From<Notification> + Send;

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

    /// The document of those routes with this version's schemas. The version's document is
    /// built on it, so these schemas win over a same-named one a shared route nests.
    fn docs() -> utoipa::openapi::OpenApi;
}

/// The frozen API: fixes only, every new feature lands in v1.
#[derive(Debug)]
pub struct V0;

/// The API the `social/v1` clients read.
#[derive(Debug)]
pub struct V1;

impl ApiVersion for V0 {
    const PREFIX: &'static str = V0_PREFIX;

    type PostView = PostViewDetailed;
    type PostDetails = PostDetails;
    type PostStream = PostStreamDetailed;
    type Notification = Notification;

    type PostKind = PostKind;
    type PostKinds = PostKinds;

    const KIND_NAMES: &'static str = "short, long, image, video, link, file, collection";
    const KIND_RULE: &'static str = "";
    const EXCLUDE_KINDS_RANGE: &'static str = "1-7";
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

    fn docs() -> utoipa::openapi::OpenApi {
        V0Doc::openapi()
    }
}

impl ApiVersion for V1 {
    const PREFIX: &'static str = V1_PREFIX;

    type PostView = v1::PostView;
    type PostDetails = v1::PostDetails;
    type PostStream = v1::PostStream;
    type Notification = v1::Notification;

    type PostKind = v1::PostKind;
    type PostKinds = v1::PostKinds;

    const KIND_NAMES: &'static str = v1::KIND_NAMES;
    const KIND_RULE: &'static str =
        "; any other name, `short` and `long` included, is rejected with 400";
    const EXCLUDE_KINDS_RANGE: &'static str = "1-8";
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

    fn docs() -> utoipa::openapi::OpenApi {
        V1Doc::openapi()
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
            pub type PostKind = <$version as ApiVersion>::PostKind;
            pub type PostKinds = <$version as ApiVersion>::PostKinds;

            /// The post stream query with `kind` and `exclude_kinds` in this version's names.
            pub type PostStreamQuery = super::super::stream::posts::PostStreamQuery<$version>;
            /// The content search query with `kind` in this version's names.
            pub type ContentSearchQuery =
                super::super::search::posts::SearchPostsByContentQuery<$version>;

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

/// The `/v0` routes whose shape is the version's, with the stored models' schemas.
#[derive(OpenApi)]
#[openapi(
    paths(
        post::view::post_view_handler_v0,
        post::details::post_details_handler_v0,
        stream::posts::stream_posts_handler_v0,
        stream::posts::stream_post_keys_handler_v0,
        stream::posts::stream_posts_by_ids_handler_v0,
        search::posts::search_posts_by_content_handler_v0,
        notification::list::list_notifications_handler_v0,
    ),
    components(schemas(
        PostViewDetailed,
        PostDetails,
        PostStreamDetailed,
        Notification,
        nexus_common::models::notification::NotificationBody,
        PostKind,
        PostKinds,
    ))
)]
struct V0Doc;

/// The same routes on `/v1`, with the `social/v1` schemas.
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
    ),
    components(schemas(
        v1::PostView,
        v1::PostDetails,
        v1::PostStream,
        v1::Notification,
        v1::NotificationBody,
        v1::PostKind,
        v1::PostKinds,
    ))
)]
struct V1Doc;

#[cfg(test)]
mod tests {
    use super::*;

    /// Each version's names are the ones its `PostKind` schema lists, in that order: a kind
    /// added to the stored enum without a name here fails this test, not the document.
    #[test]
    fn kind_names_match_each_versions_schema() {
        fn listed<T: utoipa::PartialSchema>() -> Vec<String> {
            let schema = serde_json::to_value(T::schema()).unwrap();
            schema["enum"]
                .as_array()
                .unwrap_or_else(|| panic!("no enum in {schema}"))
                .iter()
                .map(|name| name.as_str().unwrap().to_string())
                .collect()
        }
        // `/v0` lists the stored names a strict filter parses: every kind but `unknown`.
        let mut v0 = listed::<PostKind>();
        assert_eq!(v0.pop().as_deref(), Some("unknown"));
        assert_eq!(V0::KIND_NAMES.split(", ").collect::<Vec<_>>(), v0);
        // `/v1` lists `unknown` too, since its filters take it.
        assert_eq!(
            V1::KIND_NAMES.split(", ").collect::<Vec<_>>(),
            listed::<v1::PostKind>()
        );
    }
}
