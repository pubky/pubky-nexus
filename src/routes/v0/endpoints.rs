use const_format::concatcp;

// Info routes
pub const INFO_ROUTE: &str = "/v0/info";

// Profile routes
const PROFILE_PREFIX: &str = "/v0/profile";
pub const PROFILE_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id");
pub const RELATIONSHIP_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/relationship/:viewer_id");
pub const PROFILE_COUNTS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/counts");
pub const PROFILE_DETAILS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/details");
pub const PROFILE_TAGS_ROUTE: &str = concatcp!(PROFILE_PREFIX, "/:user_id/tags");
