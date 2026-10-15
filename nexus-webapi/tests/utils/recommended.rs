//! Ids from docker/test-graph/mocks/recommended.cypher, the fixture behind the
//! recommended users tests. See its header for the follow topology.

use nexus_common::models::user::CACHE_USER_RECOMMENDED_KEY_PARTS;

pub const OBS: &str = "w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao";
pub const HOP: &str = "wtqrgw3s7w4qgxwhychgho191aqotmoejcdp4enctkrcthn3y37o";
pub const FOLLOWED: &str = "xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy";
pub const SHORT: &str = "xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory";
pub const D2: &str = "ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio";
pub const D3: &str = "ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o";
pub const D4: &str = "yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy";
pub const DELETED: &str = "z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to";
/// A post tombstone by DELETED: `deleted = true`, content cleared.
pub const TOMBSTONE_POST: &str = "RECPOSTDEL006";

/// The Redis key caching the recommendations of `user_id`.
pub fn recommended_cache_key(user_id: &str) -> String {
    format!("{}:{user_id}", CACHE_USER_RECOMMENDED_KEY_PARTS.join(":"))
}
