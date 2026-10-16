//! Ids from docker/test-graph/mocks/search-reach.cypher, the fixture behind the
//! reach-filtered search tests. See its header for the follow topology.

pub const OBS: &str = "wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy";
pub const FRIEND: &str = "x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay";
pub const FOLLOWED: &str = "xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy";
pub const FOLLOWER: &str = "xu1n8qam7zjwpg4qtormzjezszs6k9m9hqdp9gsktkzw5dboijcy";
pub const D2: &str = "xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo";
pub const STRANGER: &str = "y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo";
/// A valid Pubky id that no fixture creates.
pub const UNKNOWN_USER: &str = "w8phaw75htdp4pkchuicp76yn1ycwhaixaeqw6zhfubg641ogn8y";

pub const POST_TAG: &str = "reachpost";
pub const USER_TAG: &str = "reachuser";
pub const USER_TAG_2: &str = "reachuser2";
pub const CONTENT_TERM: &str = "zyqwombat";

pub const POST_OBS: &str = "SRCHPOSTOBS01";
pub const POST_FRIEND: &str = "SRCHPOSTFRI01";
pub const POST_FRIEND_REPLY: &str = "SRCHPOSTFRIR1";
pub const POST_FOLLOWED: &str = "SRCHPOSTFOL01";
pub const POST_FOLLOWER: &str = "SRCHPOSTFLR01";
pub const POST_D2: &str = "SRCHPOSTD2001";
pub const POST_STRANGER: &str = "SRCHPOSTSTR01";

/// `author_id:post_id`, the key search endpoints return.
pub fn post_key(author_id: &str, post_id: &str) -> String {
    format!("{author_id}:{post_id}")
}
