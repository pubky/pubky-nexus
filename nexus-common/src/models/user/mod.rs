mod counts;
mod details;
//mod id;
mod influencers;
mod muted;
mod relationship;
mod search;
mod stream;
mod tags;
mod view;

pub use counts::UserCounts;
pub use details::UserDetails;
//pub use id::PubkyId;
pub use influencers::Influencers;
pub use muted::Muted;
pub use relationship::Relationship;
pub use search::{UserSearch, USER_NAME_KEY_PARTS};
pub use stream::{
    UserIdStream, UserStream, UserStreamInput, UserStreamSource, USER_INFLUENCERS_KEY_PARTS,
    USER_MOSTFOLLOWED_KEY_PARTS,
};
pub use tags::ProfileTag;
pub use tags::UserTags;
pub use view::UserView;

/// Sentinel value used to mark deleted users in the system.
/// When a user with relationships is deleted, their name field is set to this value
/// instead of fully removing their profile data.
pub const USER_DELETED_SENTINEL: &str = "[DELETED]";
