/// Raw Pubky.App schemas as stored on homeserver.
mod bookmark;
mod follow;
mod post;
mod tag;
pub mod traits;
mod user;

pub use bookmark::PubkyAppBookmark;
pub use follow::PubkyAppFollow;
pub use post::{PostKind, PubkyAppPost};
pub use tag::PubkyAppTag;
pub use user::{PubkyAppUser, UserLink};
