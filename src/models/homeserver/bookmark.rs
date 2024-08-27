use serde::{Deserialize, Serialize};

/// Represents raw homeserver bookmark with id
/// URI: /pub/pubky.app/bookmarks/:URI_BOOKMARKED_OBJECT
///
/// Example URI:
///
/// `/pub/pubky.app/bookmarks/pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pub/pubky.app/posts/2ZKGWXZ44J300/cool`
///
#[derive(Serialize, Deserialize, Default)]
pub struct HomeserverBookmark {
    pub created_at: i64,
}
