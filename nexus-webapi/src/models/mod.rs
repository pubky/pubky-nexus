pub mod info;
pub mod post;

pub mod bounded_vec;
pub mod global_post_id;
pub mod post_id;
pub mod pubky_id;
pub mod resource_id;
pub mod tag_label;
pub mod user_id_prefix;
pub mod username_prefix;

pub use global_post_id::{GlobalPostId, GlobalPostIds};
pub use info::ServerInfo;
pub use post::{PostStreamDetailed, PostViewDetailed};
pub use post_id::{PostId, PostIds};
pub use pubky_id::PubkyId;
pub use resource_id::ResourceId;
pub use tag_label::TagLabel;
crate::define_bounded_vec!(
    name: Tags,
    element_type: String,
    min: 1,
    max: 5,
    serialize_as: comma_separated_string,
);
crate::define_bounded_vec!(
    name: UserIds,
    element_type: PubkyId,
    min: 1,
    max: 100,
    serialize_as: json_array,
);
pub use user_id_prefix::UserIdPrefix;
pub use username_prefix::UsernamePrefix;
