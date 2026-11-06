//! The `/v1` response shapes: the stored models as a `social/v1` client reads them.
//!
//! Each is a plain struct converted from the model `/v0` serves, so serde and utoipa see the
//! `/v1` shape as it is and the compiler lists every field that differs between the versions.
//! What differs so far: `/v1` says `note` and `article` where storage and `/v0` say `short`
//! and `long`, and reads those names, strictly, in the kind filters; `/v1` has no bookmarks,
//! since `social/v1` keeps them under `/priv/`, which Nexus never sees: no `bookmark` on a
//! post view, no `bookmarks` in a user's counts, no `bookmarks` stream source; and `/v1` has
//! no file details, since a `social/v1` file is bytes behind `/v1/static` with no `File`
//! node: no `attachments_metadata` on a post view, no `files` in the bootstrap, and no
//! `include_attachment_metadata` to ask for them with; and `/v1` tags have no `relationship`,
//! since `tag_uri` says whether the viewer tagged the label, and a label a client queries,
//! alone or in a `tags=` list, is folded by the `social/v1` rule. The responses that embed
//! those, the user view and stream, the resource view and envelope, and the bootstrap, are
//! `/v1` types too, so an embedded post, user or tag is shaped like the one its own route
//! serves.

mod bootstrap;
mod notification;
mod post;
mod post_kinds;
mod resource;
mod stream;
mod tag;
mod user;

pub use bootstrap::Bootstrap;
pub use notification::{Notification, NotificationBody};
pub use post::{NoAttachmentMetadata, PostDetails, PostKind, PostStream, PostView, KIND_NAMES};
pub use post_kinds::PostKinds;
pub use resource::{ResourceTagsResponse, ResourceView};
pub use stream::{PostStreamByIdsRequest, StreamSourceKind, SOURCE_NAMES};
pub use tag::{TagDetails, TagLabel, TaggersInfoResponse, Tags};
pub use user::{UserCounts, UserStream, UserView};
