//! The `/v1` response shapes: the stored models as a `social/v1` client reads them.
//!
//! Each is a plain struct converted from the model `/v0` serves, so serde and utoipa see the
//! `/v1` shape as it is and the compiler lists every field that differs between the versions.
//! Only the post kind's name differs so far: `/v1` says `note` and `article` where storage
//! and `/v0` say `short` and `long`.

mod notification;
mod post;

pub use notification::{Notification, NotificationBody};
pub use post::{PostDetails, PostKind, PostStream, PostView};
