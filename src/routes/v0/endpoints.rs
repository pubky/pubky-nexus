use const_format::concatcp;

// Version routes
const VERSION_ROUTE: &str = "/v0";

// Info routes
pub const INFO_ROUTE: &str = concatcp!(VERSION_ROUTE, "/info");

// -- USER endpoints --
const USER_PREFIX: &str = concatcp!(VERSION_ROUTE, "/user");
pub const USER_ROUTE: &str = concatcp!(USER_PREFIX, "/{user_id}");
pub const RELATIONSHIP_ROUTE: &str = concatcp!(USER_ROUTE, "/relationship/{viewer_id}");
pub const USER_COUNTS_ROUTE: &str = concatcp!(USER_ROUTE, "/counts");
pub const USER_DETAILS_ROUTE: &str = concatcp!(USER_ROUTE, "/details");
pub const USER_TAGS_ROUTE: &str = concatcp!(USER_ROUTE, "/tags");
pub const USER_TAGGERS_ROUTE: &str = concatcp!(USER_ROUTE, "/taggers/{label}");
pub const USER_FOLLOWERS_ROUTE: &str = concatcp!(USER_ROUTE, "/followers");
pub const USER_FOLLOWING_ROUTE: &str = concatcp!(USER_ROUTE, "/following");
pub const USER_FRIENDS_ROUTE: &str = concatcp!(USER_ROUTE, "/friends");
pub const USER_MUTED_ROUTE: &str = concatcp!(USER_ROUTE, "/muted");

// -- POST endpoints --
const POST_PREFIX: &str = concatcp!(VERSION_ROUTE, "/post");
pub const POST_ROUTE: &str = concatcp!(POST_PREFIX, "/{author_id}/{post_id}");
pub const POST_RELATIONSHIPS_ROUTE: &str = concatcp!(POST_ROUTE, "/relationships");
pub const POST_BOOKMARK_ROUTE: &str = concatcp!(POST_ROUTE, "/bookmark");
pub const POST_COUNTS_ROUTE: &str = concatcp!(POST_ROUTE, "/counts");
pub const POST_DETAILS_ROUTE: &str = concatcp!(POST_ROUTE, "/details");
pub const POST_TAGS_ROUTE: &str = concatcp!(POST_ROUTE, "/tags");
pub const POST_TAGGERS_ROUTE: &str = concatcp!(POST_ROUTE, "/taggers/{label}");

// -- THREAD endpoints --
const THREAD_PREFIX: &str = concatcp!(VERSION_ROUTE, "/thread");
pub const THREAD_ROUTE: &str = concatcp!(THREAD_PREFIX, "/{author_id}/{post_id}");

// -- STREAM endpoints --
const STREAM_PREFIX: &str = concatcp!(VERSION_ROUTE, "/stream");
// STREAM of UserView objects
pub const STREAM_USERS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/users");
pub const STREAM_USERS_USERNAME_SEARCH_ROUTE: &str = concatcp!(STREAM_USERS_ROUTE, "/username");
pub const STREAM_USERS_BY_IDS_ROUTE: &str = concatcp!(STREAM_USERS_ROUTE, "/by_ids");
// STREAM of PostView objects
pub const STREAM_POSTS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/posts");
// STREAM of Tags for posts
pub const STREAM_TAGS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/tags");
pub const STREAM_TAGS_GLOBAL_ROUTE: &str = concatcp!(STREAM_TAGS_ROUTE, "/global");
pub const STREAM_TAGS_REACH_ROUTE: &str =
    concatcp!(STREAM_TAGS_ROUTE, "/reached/{user_id}/{reach}");

// -- SEARCH endpoints --
const SEARCH_PREFIX: &str = concatcp!(VERSION_ROUTE, "/search");
pub const SEARCH_USERS_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/users");
pub const SEARCH_TAGS_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/tags/{label}");

// TAG endpoints
// Axum routes
const TAG_PREFIX: &str = concatcp!(VERSION_ROUTE, "/tags");
pub const HOT_TAGS_ROUTE: &str = concatcp!(TAG_PREFIX, "/hot");
pub const HOT_TAGS_BY_REACH_ROUTE: &str = concatcp!(HOT_TAGS_ROUTE, "/{user_id}/{reach}");
pub const TAG_TAGGERS_ROUTE: &str = concatcp!(TAG_PREFIX, "/tag/{label}/taggers");

// FILE endpoints
// Axum routes
const FILE_PREFIX: &str = concatcp!(VERSION_ROUTE, "/files");
pub const FILE_LIST_ROUTE: &str = concatcp!(FILE_PREFIX, "/by-ids");
pub const FILE_ROUTE: &str = concatcp!(FILE_PREFIX, "/file/{file_id}");

// Notification route
pub const NOTIFICATION_ROUTE: &str = concatcp!(USER_ROUTE, "/notifications");
