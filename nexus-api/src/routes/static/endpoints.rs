use const_format::concatcp;

pub const STATIC_ROUTE: &str = "/static";
pub const FILES_PREFIX: &str = concatcp!(STATIC_ROUTE, "/files");
pub const STATIC_FILES_ROUTE: &str = concatcp!(FILES_PREFIX, "/{owner_id}/{file_id}/{variant}");
pub const LEGACY_STATIC_FILES_ROUTE: &str = concatcp!(FILES_PREFIX, "/{owner_id}/{file_id}");
pub const USER_AVATAR_ROUTE: &str = concatcp!(STATIC_ROUTE, "/avatar/{user_id}");
