use nexus_webapi::routes::v0::endpoints;

pub mod author;
pub mod author_replies;
pub mod bookmarks;
pub mod kind;
pub mod post_keys;
pub mod post_replies;
pub mod posts;
pub mod reach;
pub mod tags;
pub mod utils;

pub const ROOT_PATH: &str = endpoints::STREAM_POSTS_ROUTE;
pub const KEYS_ROOT_PATH: &str = endpoints::STREAM_POST_KEYS_ROUTE;

// Aldert
pub const USER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
// Eixample user from test/posts.cypher
pub const VIEWER_ID: &str = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";
pub const AMSTERDAM: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
pub const BOGOTA: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";

pub const TAG_LABEL_1: &str = "bitcoin";
pub const TAG_LABEL_2: &str = "opensource";
pub const TAG_LABEL_3: &str = "dev";
const TAG_LABEL_4: &str = "free";

pub const POST_A: &str = "00000039YD99Y";
pub const POST_B: &str = "00000039YD9B2";
pub const POST_C: &str = "00000039YD9BM";
pub const POST_D: &str = "00000039YD9C0";
pub const POST_E: &str = "00000039YD9CE";
pub const POST_F: &str = "00000039YD9CY";
pub const POST_G: &str = "00000039YD9DA";
pub const POST_H: &str = "00000039YD9DP";
