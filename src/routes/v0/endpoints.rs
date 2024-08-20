use const_format::concatcp;

// Version routes
const VERSION_ROUTE: &str = "/v0";

// Info routes
pub const INFO_ROUTE: &str = concatcp!(VERSION_ROUTE, "/info");

// User routes
const USER_PREFIX: &str = concatcp!(VERSION_ROUTE, "/user");
pub const USER_ROUTE: &str = concatcp!(USER_PREFIX, "/:user_id");
pub const RELATIONSHIP_ROUTE: &str = concatcp!(USER_ROUTE, "/relationship/:viewer_id");
pub const USER_COUNTS_ROUTE: &str = concatcp!(USER_ROUTE, "/counts");
pub const USER_DETAILS_ROUTE: &str = concatcp!(USER_ROUTE, "/details");
pub const USER_TAGS_ROUTE: &str = concatcp!(USER_ROUTE, "/tags");
pub const USER_FOLLOWERS_ROUTE: &str = concatcp!(USER_ROUTE, "/followers");
pub const USER_FOLLOWING_ROUTE: &str = concatcp!(USER_ROUTE, "/following");
pub const USER_FRIENDS_ROUTE: &str = concatcp!(USER_ROUTE, "/friends");

// Post routes
const POST_PREFIX: &str = concatcp!(VERSION_ROUTE, "/post");
pub const POST_ROUTE: &str = concatcp!(POST_PREFIX, "/:author_id/:post_id");
pub const POST_RELATIONSHIPS_ROUTE: &str = concatcp!(POST_ROUTE, "/relationships");
pub const POST_BOOKMARK_ROUTE: &str = concatcp!(POST_ROUTE, "/bookmark");
pub const POST_COUNTS_ROUTE: &str = concatcp!(POST_ROUTE, "/counts");
pub const POST_DETAILS_ROUTE: &str = concatcp!(POST_ROUTE, "/details");
pub const POST_TAGS_ROUTE: &str = concatcp!(POST_ROUTE, "/tags");

// Stream routes
const STREAM_PREFIX: &str = concatcp!(VERSION_ROUTE, "/stream");
pub const STREAM_USERS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/users");
pub const STREAM_USERS_MOSTFOLLOWED_ROUTE: &str = concatcp!(STREAM_PREFIX, "/users/most-followed");
pub const STREAM_POSTS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/posts");
pub const STREAM_POSTS_USER_ROUTE: &str = concatcp!(STREAM_POSTS_ROUTE, "/user/:user_id");
pub const STREAM_POSTS_REACH_ROUTE: &str = concatcp!(STREAM_POSTS_ROUTE, "/reach");
pub const STREAM_POSTS_BOOKMARKED_ROUTE: &str =
    concatcp!(STREAM_POSTS_ROUTE, "/bookmarks/:user_id");
pub const STREAM_TAGS_ROUTE: &str  = concatcp!(STREAM_PREFIX, "/tags");
pub const STREAM_TAGS_GLOBAL_ROUTE: &str  = concatcp!(STREAM_TAGS_ROUTE, "/global");
pub const STREAM_TAGS_REACH_ROUTE: &str  = concatcp!(STREAM_TAGS_ROUTE, "/reached/:user_id/:reach");
