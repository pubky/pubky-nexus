use const_format::concatcp;

// Version routes
const VERSION_ROUTE: &str = "/v0";

// Info routes
pub const INFO_ROUTE: &str = concatcp!(VERSION_ROUTE, "/info");

// Profile routes
const PROFILE_PREFIX: &str = concatcp!(VERSION_ROUTE, "/profile");
pub const PROFILE_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id");
pub const RELATIONSHIP_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/relationship/:viewer_id");
pub const PROFILE_COUNTS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/counts");
pub const PROFILE_DETAILS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/details");
pub const PROFILE_TAGS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/tags");

// Post routes
const POST_PREFIX: &str = concatcp!(VERSION_ROUTE, "/post");
pub const POST_ROUTE: &str = concatcp!(POST_PREFIX, "/:author_id/:post_id");
