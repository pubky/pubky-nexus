// Recommended users fixture (stream/users with source `recommended`). Kept inert
// against the global suites the same way wot.cypher is: post indexed_at far below
// every window start, no tags, and user ids that sort high.
//
// Topology (OBS = observer), everyone but HOP and SHORT has 5 posts (DELETED has a
// sixth, a post tombstone):
//   OBS <-> HOP           follow cycle: OBS reaches itself at depth 2
//   OBS  -> FOLLOWED      directly followed, also reachable at depth 2 over HOP
//   HOP  -> FOLLOWED
//   HOP  -> SHORT         depth 2, 4 posts: one short of the threshold
//   HOP  -> D2            depth 2, reached over two paths (HOP and FOLLOWED)
//   FOLLOWED -> D2
//   HOP  -> DELETED       depth 2, flagged as deleted
//   D2   -> D3            depth 3
//   D3   -> D4            depth 4, out of range
// Only D2 and D3 are recommended to OBS.

:param obs => 'w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao';
:param hop => 'wtqrgw3s7w4qgxwhychgho191aqotmoejcdp4enctkrcthn3y37o';
:param followed => 'xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy';
:param short => 'xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory';
:param d2 => 'ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio';
:param d3 => 'ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o';
:param d4 => 'yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy';
:param deleted => 'z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to';

// ##############################
// ##### Create users ###########
// ##############################
MERGE (u:User {id: $obs}) SET u.name = "recommended_obs", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/profile.json";
MERGE (u:User {id: $hop}) SET u.name = "recommended_hop", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://wtqrgw3s7w4qgxwhychgho191aqotmoejcdp4enctkrcthn3y37o/pub/pubky.app/profile.json";
MERGE (u:User {id: $followed}) SET u.name = "recommended_followed", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/profile.json";
MERGE (u:User {id: $short}) SET u.name = "recommended_short", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory/pub/pubky.app/profile.json";
MERGE (u:User {id: $d2}) SET u.name = "recommended_d2", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/profile.json";
MERGE (u:User {id: $d3}) SET u.name = "recommended_d3", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/profile.json";
MERGE (u:User {id: $d4}) SET u.name = "recommended_d4", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/profile.json";
MERGE (u:User {id: $deleted}) SET u.name = "[DELETED]", u.deleted = true, u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/profile.json";

// ##############################
// ##### Create follows #########
// ##############################
MATCH (u1:User {id: $obs}), (u2:User {id: $hop}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002001, id: "RECFOLLOW0001"}]->(u2);
MATCH (u1:User {id: $hop}), (u2:User {id: $obs}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002002, id: "RECFOLLOW0002"}]->(u2);
MATCH (u1:User {id: $obs}), (u2:User {id: $followed}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002003, id: "RECFOLLOW0003"}]->(u2);
MATCH (u1:User {id: $hop}), (u2:User {id: $followed}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002004, id: "RECFOLLOW0004"}]->(u2);
MATCH (u1:User {id: $hop}), (u2:User {id: $short}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002005, id: "RECFOLLOW0005"}]->(u2);
MATCH (u1:User {id: $hop}), (u2:User {id: $d2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002006, id: "RECFOLLOW0006"}]->(u2);
MATCH (u1:User {id: $followed}), (u2:User {id: $d2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002007, id: "RECFOLLOW0007"}]->(u2);
MATCH (u1:User {id: $hop}), (u2:User {id: $deleted}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002008, id: "RECFOLLOW0008"}]->(u2);
MATCH (u1:User {id: $d2}), (u2:User {id: $d3}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002009, id: "RECFOLLOW0009"}]->(u2);
MATCH (u1:User {id: $d3}), (u2:User {id: $d4}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000002010, id: "RECFOLLOW0010"}]->(u2);

// ##############################
// ##### Create posts ###########
// ##############################
MERGE (p:Post {id: "RECPOSTOBS001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001001;
MATCH (u:User {id: $obs}), (p:Post {id: "RECPOSTOBS001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/posts/RECPOSTOBS001";
MERGE (p:Post {id: "RECPOSTOBS002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001002;
MATCH (u:User {id: $obs}), (p:Post {id: "RECPOSTOBS002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/posts/RECPOSTOBS002";
MERGE (p:Post {id: "RECPOSTOBS003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001003;
MATCH (u:User {id: $obs}), (p:Post {id: "RECPOSTOBS003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/posts/RECPOSTOBS003";
MERGE (p:Post {id: "RECPOSTOBS004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001004;
MATCH (u:User {id: $obs}), (p:Post {id: "RECPOSTOBS004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/posts/RECPOSTOBS004";
MERGE (p:Post {id: "RECPOSTOBS005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001005;
MATCH (u:User {id: $obs}), (p:Post {id: "RECPOSTOBS005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://w39631pxk8epa77ztwmrw5qxgwp1nimeqtahw31d7bnzzrtg94ao/pub/pubky.app/posts/RECPOSTOBS005";
MERGE (p:Post {id: "RECPOSTFOL001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001006;
MATCH (u:User {id: $followed}), (p:Post {id: "RECPOSTFOL001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/posts/RECPOSTFOL001";
MERGE (p:Post {id: "RECPOSTFOL002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001007;
MATCH (u:User {id: $followed}), (p:Post {id: "RECPOSTFOL002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/posts/RECPOSTFOL002";
MERGE (p:Post {id: "RECPOSTFOL003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001008;
MATCH (u:User {id: $followed}), (p:Post {id: "RECPOSTFOL003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/posts/RECPOSTFOL003";
MERGE (p:Post {id: "RECPOSTFOL004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001009;
MATCH (u:User {id: $followed}), (p:Post {id: "RECPOSTFOL004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/posts/RECPOSTFOL004";
MERGE (p:Post {id: "RECPOSTFOL005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001010;
MATCH (u:User {id: $followed}), (p:Post {id: "RECPOSTFOL005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xmnoqzjw956gpboe3ne95feqbk9yf6wbem347ukayxq1jdi1jmxy/pub/pubky.app/posts/RECPOSTFOL005";
// One post short of the threshold
MERGE (p:Post {id: "RECPOSTSHO001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001011;
MATCH (u:User {id: $short}), (p:Post {id: "RECPOSTSHO001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory/pub/pubky.app/posts/RECPOSTSHO001";
MERGE (p:Post {id: "RECPOSTSHO002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001012;
MATCH (u:User {id: $short}), (p:Post {id: "RECPOSTSHO002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory/pub/pubky.app/posts/RECPOSTSHO002";
MERGE (p:Post {id: "RECPOSTSHO003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001013;
MATCH (u:User {id: $short}), (p:Post {id: "RECPOSTSHO003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory/pub/pubky.app/posts/RECPOSTSHO003";
MERGE (p:Post {id: "RECPOSTSHO004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001014;
MATCH (u:User {id: $short}), (p:Post {id: "RECPOSTSHO004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xos98pobdo9wm4qoiq5rgkrdjmfysn8q5wk7g4yb8yy59kufnory/pub/pubky.app/posts/RECPOSTSHO004";
MERGE (p:Post {id: "RECPOSTD2X001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001015;
MATCH (u:User {id: $d2}), (p:Post {id: "RECPOSTD2X001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/posts/RECPOSTD2X001";
MERGE (p:Post {id: "RECPOSTD2X002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001016;
MATCH (u:User {id: $d2}), (p:Post {id: "RECPOSTD2X002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/posts/RECPOSTD2X002";
MERGE (p:Post {id: "RECPOSTD2X003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001017;
MATCH (u:User {id: $d2}), (p:Post {id: "RECPOSTD2X003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/posts/RECPOSTD2X003";
MERGE (p:Post {id: "RECPOSTD2X004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001018;
MATCH (u:User {id: $d2}), (p:Post {id: "RECPOSTD2X004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/posts/RECPOSTD2X004";
MERGE (p:Post {id: "RECPOSTD2X005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001019;
MATCH (u:User {id: $d2}), (p:Post {id: "RECPOSTD2X005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ya4cjt3j6b58hqf4sujfuj954pymkd6j8ukrbb8pqud5dyguigio/pub/pubky.app/posts/RECPOSTD2X005";
MERGE (p:Post {id: "RECPOSTD3X001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001020;
MATCH (u:User {id: $d3}), (p:Post {id: "RECPOSTD3X001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/posts/RECPOSTD3X001";
MERGE (p:Post {id: "RECPOSTD3X002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001021;
MATCH (u:User {id: $d3}), (p:Post {id: "RECPOSTD3X002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/posts/RECPOSTD3X002";
MERGE (p:Post {id: "RECPOSTD3X003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001022;
MATCH (u:User {id: $d3}), (p:Post {id: "RECPOSTD3X003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/posts/RECPOSTD3X003";
MERGE (p:Post {id: "RECPOSTD3X004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001023;
MATCH (u:User {id: $d3}), (p:Post {id: "RECPOSTD3X004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/posts/RECPOSTD3X004";
MERGE (p:Post {id: "RECPOSTD3X005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001024;
MATCH (u:User {id: $d3}), (p:Post {id: "RECPOSTD3X005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://ywpwmrrndzumk68oszuieigct5bap68oobbdkou8ks1djxyh3w5o/pub/pubky.app/posts/RECPOSTD3X005";
MERGE (p:Post {id: "RECPOSTD4X001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001025;
MATCH (u:User {id: $d4}), (p:Post {id: "RECPOSTD4X001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/posts/RECPOSTD4X001";
MERGE (p:Post {id: "RECPOSTD4X002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001026;
MATCH (u:User {id: $d4}), (p:Post {id: "RECPOSTD4X002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/posts/RECPOSTD4X002";
MERGE (p:Post {id: "RECPOSTD4X003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001027;
MATCH (u:User {id: $d4}), (p:Post {id: "RECPOSTD4X003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/posts/RECPOSTD4X003";
MERGE (p:Post {id: "RECPOSTD4X004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001028;
MATCH (u:User {id: $d4}), (p:Post {id: "RECPOSTD4X004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/posts/RECPOSTD4X004";
MERGE (p:Post {id: "RECPOSTD4X005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001029;
MATCH (u:User {id: $d4}), (p:Post {id: "RECPOSTD4X005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://yx8ztwc65s1djkoy518n5p6m67kjbcj1wmmjn7jrhzb1yx9mzwcy/pub/pubky.app/posts/RECPOSTD4X005";
MERGE (p:Post {id: "RECPOSTDEL001"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001030;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL001";
MERGE (p:Post {id: "RECPOSTDEL002"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001031;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL002"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL002";
MERGE (p:Post {id: "RECPOSTDEL003"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001032;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL003"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL003";
MERGE (p:Post {id: "RECPOSTDEL004"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001033;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL004"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL004";
MERGE (p:Post {id: "RECPOSTDEL005"}) SET p.content = "recommended fixture entry", p.kind = "short", p.indexed_at = 1600000001034;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL005"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL005";
// A post tombstone for the post /details and view tests: the shape `post::del` leaves
// when a deleted post still has relationships. A sixth post, so DELETED keeps five live
// ones if tombstones ever stop counting toward the threshold.
MERGE (p:Post {id: "RECPOSTDEL006"}) SET p.content = "", p.kind = "short", p.deleted = true, p.indexed_at = 1600000001035;
MATCH (u:User {id: $deleted}), (p:Post {id: "RECPOSTDEL006"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://z48xiqc4mcfiicwi1ewgn7swkhuem86wbkyoudt9yqidcxadt4to/pub/pubky.app/posts/RECPOSTDEL006";
