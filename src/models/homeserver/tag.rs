use serde::{Deserialize, Serialize};

/// Represents raw homeserver tag with id
/// URI: /pub/pubky.app/tags/:URI_TAGGED_OBJECT/:label
///
/// Example URI:
///
/// `/pub/pubky.app/tags/pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy/pub/pubky.app/posts/2ZKGWXZ44J300/cool`
///
#[derive(Serialize, Deserialize, Default)]
pub struct HomeserverTag {
    pub created_at: i64,
}
