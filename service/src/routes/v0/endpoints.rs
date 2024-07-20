// Info routes
pub const INFO_ROUTE: &str = "/v0/info";

// Profile routes
// TODO: try using route prefixes: pub const PROFILE_ROUTE: &str = "/v0/profile";
pub const PROFILE_ROUTE: &str = "/v0/profile/:user_id";
pub const RELATIONSHIP_ROUTE: &str = "/v0/profile/:user_id/relationship/:viewer_id";
pub const PROFILE_COUNTS_ROUTE: &str = "/v0/profile/:user_id/counts";
pub const PROFILE_DETAILS_ROUTE: &str = "/v0/profile/:user_id/details";
pub const PROFILE_TAGS_ROUTE: &str = "/v0/profile/:user_id/tags";
