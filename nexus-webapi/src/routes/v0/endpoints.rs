//! The API's route paths, relative to a version prefix.
//!
//! The router is built once from these paths and mounted under every prefix in
//! [`VERSION_PREFIXES`], so a route exists under `/v0` and `/v1` from one declaration. A
//! test or client that spells out a full path puts the prefix in front, as
//! `concatcp!(V0_PREFIX, POST_ROUTE)`.

use const_format::concatcp;

/// The frozen API: fixes only, every new feature lands in v1.
pub const V0_PREFIX: &str = "/v0";
/// The API the `social/v1` clients read.
pub const V1_PREFIX: &str = "/v1";
/// Every prefix the API is mounted under.
pub const VERSION_PREFIXES: [&str; 2] = [V0_PREFIX, V1_PREFIX];

// Info routes
pub const INFO_ROUTE: &str = "/info";

// -- USER endpoints --
const USER_PREFIX: &str = "/user";
pub const USER_ROUTE: &str = concatcp!(USER_PREFIX, "/{user_id}");
pub const RELATIONSHIP_ROUTE: &str = concatcp!(USER_ROUTE, "/relationship/{viewer_id}");
pub const USER_COUNTS_ROUTE: &str = concatcp!(USER_ROUTE, "/counts");
pub const USER_DETAILS_ROUTE: &str = concatcp!(USER_ROUTE, "/details");
pub const USER_TAGS_ROUTE: &str = concatcp!(USER_ROUTE, "/tags");
pub const USER_TAGGERS_ROUTE: &str = concatcp!(USER_ROUTE, "/taggers/{label}");
pub const USER_FOLLOWERS_ROUTE: &str = concatcp!(USER_ROUTE, "/followers");
pub const USER_FOLLOWING_ROUTE: &str = concatcp!(USER_ROUTE, "/following");
pub const USER_FRIENDS_ROUTE: &str = concatcp!(USER_ROUTE, "/friends");

// -- POST endpoints --
pub const POST_PREFIX: &str = "/post";
pub const POST_ROUTE: &str = concatcp!(POST_PREFIX, "/{author_id}/{post_id}");
pub const POST_RELATIONSHIPS_ROUTE: &str = concatcp!(POST_ROUTE, "/relationships");
pub const POST_BOOKMARK_ROUTE: &str = concatcp!(POST_ROUTE, "/bookmark");
pub const POST_COUNTS_ROUTE: &str = concatcp!(POST_ROUTE, "/counts");
pub const POST_DETAILS_ROUTE: &str = concatcp!(POST_ROUTE, "/details");
pub const POST_TAGS_ROUTE: &str = concatcp!(POST_ROUTE, "/tags");
pub const POST_TAGGERS_ROUTE: &str = concatcp!(POST_ROUTE, "/taggers/{label}");

// -- STREAM endpoints --
const STREAM_PREFIX: &str = "/stream";
// STREAM of UserView objects
pub const STREAM_USERS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/users");
pub const STREAM_USER_IDS_ROUTE: &str = concatcp!(STREAM_USERS_ROUTE, "/ids");
pub const STREAM_USERS_USERNAME_SEARCH_ROUTE: &str = concatcp!(STREAM_USERS_ROUTE, "/username");
pub const STREAM_USERS_BY_IDS_ROUTE: &str = concatcp!(STREAM_USERS_ROUTE, "/by_ids");
// STREAM of PostView objects
pub const STREAM_POSTS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/posts");
pub const STREAM_POSTS_BY_IDS_ROUTE: &str = concatcp!(STREAM_POSTS_ROUTE, "/by_ids");
pub const STREAM_POST_KEYS_ROUTE: &str = concatcp!(STREAM_POSTS_ROUTE, "/keys");
// STREAM of Tags for posts
pub const STREAM_TAGS_ROUTE: &str = concatcp!(STREAM_PREFIX, "/tags");
pub const STREAM_TAGS_GLOBAL_ROUTE: &str = concatcp!(STREAM_TAGS_ROUTE, "/global");
pub const STREAM_TAGS_REACH_ROUTE: &str =
    concatcp!(STREAM_TAGS_ROUTE, "/reached/{user_id}/{reach}");

// -- SEARCH endpoints --
const SEARCH_PREFIX: &str = "/search";
const SEARCH_USERS_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/users");
pub const SEARCH_USERS_BY_NAME_ROUTE: &str = concatcp!(SEARCH_USERS_ROUTE, "/by_name/{prefix}");
pub const SEARCH_USERS_BY_ID_ROUTE: &str = concatcp!(SEARCH_USERS_ROUTE, "/by_id/{prefix}");
pub const SEARCH_USERS_BY_TAGS_ROUTE: &str = concatcp!(SEARCH_USERS_ROUTE, "/by_tags");
pub const SEARCH_POSTS_BY_TAG_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/posts/by_tag/{tag}");
pub const SEARCH_POSTS_BY_CONTENT_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/posts/by_content");
pub const SEARCH_TAGS_BY_PREFIX_ROUTE: &str = concatcp!(SEARCH_PREFIX, "/tags/by_prefix/{prefix}");

// -- TAG endpoints --
const TAG_PREFIX: &str = "/tags";
pub const TAGS_HOT_ROUTE: &str = concatcp!(TAG_PREFIX, "/hot");
pub const TAG_TAGGERS_ROUTE: &str = concatcp!(TAG_PREFIX, "/taggers/{label}");
pub const TAG_ROUTE: &str = concatcp!(TAG_PREFIX, "/{tagger_id}/{tag_id}");

// -- FILE endpoints --
const FILE_PREFIX: &str = "/files";
pub const FILE_LIST_ROUTE: &str = concatcp!(FILE_PREFIX, "/by_ids");
pub const FILE_ROUTE: &str = concatcp!(FILE_PREFIX, "/file/{file_id}");

// -- NOTIFICATION endpoints -
pub const NOTIFICATION_ROUTE: &str = concatcp!(USER_ROUTE, "/notifications");

// -- BOOTSTRAP endpoints -
pub const BOOTSTRAP_ROUTE: &str = "/bootstrap/{user_id}";
pub const INGEST_USER_ROUTE: &str = "/ingest/{user_id}";

// -- RESOURCE endpoints --
const RESOURCE_PREFIX: &str = "/resource";
pub const RESOURCE_TAGS_ROUTE: &str = concatcp!(RESOURCE_PREFIX, "/{resource_id}/tags");
pub const RESOURCE_TAGGERS_ROUTE: &str =
    concatcp!(RESOURCE_PREFIX, "/{resource_id}/tags/{label}/taggers");
pub const RESOURCE_BY_URI_ROUTE: &str = concatcp!(RESOURCE_PREFIX, "/by-uri");
/// Served under `/v1` only (`V1::own_routes`).
pub const RESOURCE_REPLIES_ROUTE: &str = concatcp!(RESOURCE_PREFIX, "/{resource_id}/replies");
/// Served under `/v1` only (`V1::own_routes`).
pub const RESOURCE_EMBEDS_ROUTE: &str = concatcp!(RESOURCE_PREFIX, "/{resource_id}/embeds");

// STREAM of ResourceView objects
pub const STREAM_RESOURCES_ROUTE: &str = concatcp!(STREAM_PREFIX, "/resources");
/// Served under `/v1` only (`V1::own_routes`).
pub const STREAM_RESOURCES_BY_IDS_ROUTE: &str = concatcp!(STREAM_RESOURCES_ROUTE, "/by_ids");
pub const STREAM_RESOURCE_IDS_ROUTE: &str = concatcp!(STREAM_RESOURCES_ROUTE, "/ids");

// -- EVENTS endpoints
pub const EVENTS_ROUTE: &str = "/events";

// -- RESOURCE CHANGES endpoints, served under `/v1` only (`V1::own_routes`)
pub const RESOURCE_CHANGES_ROUTE: &str = "/resources/changes";
