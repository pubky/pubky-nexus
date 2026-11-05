//! The API version as a type: what `/v0` and `/v1` answer differently, and nothing else.
//!
//! [`ApiVersion`] has one marker type per mount, [`V0`] and [`V1`], and an associated type for
//! every response shape that differs between them. A handler whose shape differs is one generic
//! body returning the version's type; the router builder is generic too and runs once per
//! version, so a route is declared once and a fix lands in both versions at once.
//!
//! utoipa's path annotation does not take a generic function: the path struct it generates has
//! no type parameter, so a `V::PostView` body does not resolve (checked against utoipa 5.5).
//! [`versioned_handlers!`] therefore writes the annotation once and emits a thin wrapper per
//! version that calls the shared body, with that version's type as the documented 200 body. The
//! version mounts and documents the wrappers through [`ApiVersion`]'s route and document
//! functions, so what a version serves and what it documents are read in one place.

use axum::routing::MethodRouter;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::PostDetails;
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};

use super::endpoints::{V0_PREFIX, V1_PREFIX};
use super::{notification, post, stream};
use crate::models::{v1, PostStreamDetailed, PostViewDetailed};
use crate::routes::AppState;

/// One API version: its mount prefix, the response shapes that are its own, and the annotated
/// handlers that serve and document them.
///
/// Every associated type converts from the model the shared handler body produces, so a body is
/// written once and ends in `V::Shape::from(model)`.
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

    /// This version's handler of each route whose response is one of the types above. The
    /// generic router builders mount these, so the route declaration stays single.
    fn post_view() -> MethodRouter<AppState>;
    fn post_details() -> MethodRouter<AppState>;
    fn stream_posts() -> MethodRouter<AppState>;
    fn stream_posts_by_ids() -> MethodRouter<AppState>;
    fn list_notifications() -> MethodRouter<AppState>;

    /// The document of those routes with this version's schemas. The version's document is
    /// built on it, so these schemas win over a same-named one a shared route nests.
    fn docs() -> utoipa::openapi::OpenApi;
}

/// The frozen API: fixes only, every new feature lands in v1.
pub struct V0;

/// The API the `social/v1` clients read.
pub struct V1;

impl ApiVersion for V0 {
    const PREFIX: &'static str = V0_PREFIX;

    type PostView = PostViewDetailed;
    type PostDetails = PostDetails;
    type PostStream = PostStreamDetailed;
    type Notification = Notification;

    fn post_view() -> MethodRouter<AppState> {
        axum::routing::get(post::view::post_view_handler_v0)
    }
    fn post_details() -> MethodRouter<AppState> {
        axum::routing::get(post::details::post_details_handler_v0)
    }
    fn stream_posts() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_posts_handler_v0)
    }
    fn stream_posts_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::posts::stream_posts_by_ids_handler_v0)
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

    fn post_view() -> MethodRouter<AppState> {
        axum::routing::get(post::view::post_view_handler_v1)
    }
    fn post_details() -> MethodRouter<AppState> {
        axum::routing::get(post::details::post_details_handler_v1)
    }
    fn stream_posts() -> MethodRouter<AppState> {
        axum::routing::get(stream::posts::stream_posts_handler_v1)
    }
    fn stream_posts_by_ids() -> MethodRouter<AppState> {
        axum::routing::post(stream::posts::stream_posts_by_ids_handler_v1)
    }
    fn list_notifications() -> MethodRouter<AppState> {
        axum::routing::get(notification::list::list_notifications_handler_v1)
    }

    fn docs() -> utoipa::openapi::OpenApi {
        V1Doc::openapi()
    }
}

/// Each version's shapes by name, for the annotations [`versioned_handlers!`] writes: utoipa
/// takes a body as a type path and reads the schema's name from the type at runtime, so an
/// alias of the associated type documents exactly what the handler serializes.
pub mod shapes {
    macro_rules! shapes_of {
        ($version:ty) => {
            pub type PostView = <$version as super::super::ApiVersion>::PostView;
            pub type PostDetails = <$version as super::super::ApiVersion>::PostDetails;
            pub type PostStream = <$version as super::super::ApiVersion>::PostStream;
            pub type Notification = <$version as super::super::ApiVersion>::Notification;
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
///     post_view_handler_v0 / post_view_handler_v1 => post_view(path: Path<PostPath>) -> PostView;
///     #[utoipa::path(get, path = POST_ROUTE, operation_id = "post_view_handler", params(...))]
///     responses(
///         (status = 200, description = "Post"),
///         (status = 400, description = "Invalid parameters"),
///     )
/// }
/// ```
///
/// `post_view` is the generic body, `async fn post_view<V: ApiVersion>(path: Path<PostPath>)
/// -> Result<impl IntoResponse>`, and `PostView` the [`ApiVersion`] type it returns; `-> [T]`
/// documents a list of `T`. The 200 response gets `body = ` that version's type, the rest of
/// the annotation is written once. Give it an `operation_id`: utoipa would otherwise take the
/// wrapper's name, and the two versions of one route are one operation to a client.
macro_rules! versioned_handlers {
    (
        $(#[$doc:meta])*
        $v0:ident / $v1:ident => $body:ident ( $($arg:ident : $ty:ty),* $(,)? ) -> $shape:tt;
        #[utoipa::path( $($attr:tt)* )]
        responses( (status = 200, $($ok:tt)*), $($responses:tt)* )
    ) => {
        $crate::routes::v0::version::versioned_handlers!(@one $(#[$doc])* $v0, V0, v0, $body ( $($arg : $ty),* ) -> $shape; [$($attr)*] [$($ok)*] [$($responses)*]);
        $crate::routes::v0::version::versioned_handlers!(@one $(#[$doc])* $v1, V1, v1, $body ( $($arg : $ty),* ) -> $shape; [$($attr)*] [$($ok)*] [$($responses)*]);
    };
    // `-> [T]` documents `Vec<T>`, `-> T` documents `T`, each as the version's alias in `shapes`.
    (@one $(#[$doc:meta])* $name:ident, $version:ident, $shapes:ident, $body:ident ( $($arg:ident : $ty:ty),* ) -> [$shape:ident]; $($rest:tt)*) => {
        $crate::routes::v0::version::versioned_handlers!(@emit $(#[$doc])* $name, $version, $body ( $($arg : $ty),* ) -> [Vec<$crate::routes::v0::version::shapes::$shapes::$shape>]; $($rest)*);
    };
    (@one $(#[$doc:meta])* $name:ident, $version:ident, $shapes:ident, $body:ident ( $($arg:ident : $ty:ty),* ) -> $shape:ident; $($rest:tt)*) => {
        $crate::routes::v0::version::versioned_handlers!(@emit $(#[$doc])* $name, $version, $body ( $($arg : $ty),* ) -> [$crate::routes::v0::version::shapes::$shapes::$shape]; $($rest)*);
    };
    (@emit $(#[$doc:meta])* $name:ident, $version:ident, $body:ident ( $($arg:ident : $ty:ty),* ) -> [$($response:tt)*]; [$($attr:tt)*] [$($ok:tt)*] [$($responses:tt)*]) => {
        $(#[$doc])*
        #[utoipa::path($($attr)*, responses((status = 200, body = $($response)*, $($ok)*), $($responses)*))]
        pub async fn $name($($arg: $ty),*) -> $crate::Result<axum::response::Response> {
            $body::<$crate::routes::v0::version::$version>($($arg),*)
                .await
                .map(axum::response::IntoResponse::into_response)
        }
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
        stream::posts::stream_posts_by_ids_handler_v0,
        notification::list::list_notifications_handler_v0,
    ),
    components(schemas(
        PostViewDetailed,
        PostDetails,
        PostStreamDetailed,
        Notification,
        nexus_common::models::notification::NotificationBody,
        nexus_common::models::post::PostKind,
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
        stream::posts::stream_posts_by_ids_handler_v1,
        notification::list::list_notifications_handler_v1,
    ),
    components(schemas(
        v1::PostView,
        v1::PostDetails,
        v1::PostStream,
        v1::Notification,
        v1::NotificationBody,
        v1::PostKind,
    ))
)]
struct V1Doc;
