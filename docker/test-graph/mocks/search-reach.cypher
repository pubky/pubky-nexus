// Reach-filtered search fixture (search/posts/by_tag, search/users/by_tags,
// search/posts/by_content with `user_id` + `reach`). Kept inert against the global
// suites the same way wot.cypher is: post indexed_at far below every window start,
// unique tag labels with year-2008 timestamps, and user ids that sort high.
//
// Topology (OBS = observer):
//   OBS <-> FRIEND        mutual follow: following, follower, friend, wot_1
//   OBS  -> FOLLOWED      following only, wot_1
//   FOLLOWER -> OBS       follower only
//   FOLLOWED -> D2        D2 is reachable only at wot depth 2
//   OBS <-> LURKER        in every reach of OBS, but authored nothing
//   STRANGER              outside every reach
// TAGGER1 and TAGGER2 only tag; nobody follows them.

:param obs => 'wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy';
:param friend => 'x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay';
:param followed => 'xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy';
:param follower => 'xu1n8qam7zjwpg4qtormzjezszs6k9m9hqdp9gsktkzw5dboijcy';
:param d2 => 'xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo';
:param stranger => 'y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo';
:param lurker => 'zy8gjbx3xoi4j7cgudcajwxg3y6ybc7if8zwiznn4mfy84t5yjco';
:param tagger1 => 'yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy';
:param tagger2 => 'yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo';

:param post_tag => 'reachpost';
:param user_tag => 'reachuser';
:param user_tag_2 => 'reachuser2';

// ##############################
// ##### Create users ###########
// ##############################
MERGE (u:User {id: $obs}) SET u.name = "reach_obs", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy/pub/pubky.app/profile.json";
MERGE (u:User {id: $friend}) SET u.name = "reach_friend", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay/pub/pubky.app/profile.json";
MERGE (u:User {id: $followed}) SET u.name = "reach_followed", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy/pub/pubky.app/profile.json";
MERGE (u:User {id: $follower}) SET u.name = "reach_follower", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://xu1n8qam7zjwpg4qtormzjezszs6k9m9hqdp9gsktkzw5dboijcy/pub/pubky.app/profile.json";
MERGE (u:User {id: $d2}) SET u.name = "reach_d2", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo/pub/pubky.app/profile.json";
MERGE (u:User {id: $stranger}) SET u.name = "reach_stranger", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo/pub/pubky.app/profile.json";
MERGE (u:User {id: $lurker}) SET u.name = "reach_lurker", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://zy8gjbx3xoi4j7cgudcajwxg3y6ybc7if8zwiznn4mfy84t5yjco/pub/pubky.app/profile.json";
MERGE (u:User {id: $tagger1}) SET u.name = "reach_tagger1", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/profile.json";
MERGE (u:User {id: $tagger2}) SET u.name = "reach_tagger2", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]", u.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/profile.json";

// ##############################
// ##### Create follows #########
// ##############################
MATCH (u1:User {id: $obs}), (u2:User {id: $friend}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001001, id: "SRCHFOLLOW001"}]->(u2);
MATCH (u1:User {id: $friend}), (u2:User {id: $obs}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001002, id: "SRCHFOLLOW002"}]->(u2);
MATCH (u1:User {id: $obs}), (u2:User {id: $followed}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001003, id: "SRCHFOLLOW003"}]->(u2);
MATCH (u1:User {id: $follower}), (u2:User {id: $obs}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001004, id: "SRCHFOLLOW004"}]->(u2);
MATCH (u1:User {id: $followed}), (u2:User {id: $d2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001005, id: "SRCHFOLLOW005"}]->(u2);
// LURKER is in every reach of OBS and authors nothing, so a reach listing that
// keeps users without posts shows it.
MATCH (u1:User {id: $obs}), (u2:User {id: $lurker}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001006, id: "SRCHFOLLOW006"}]->(u2);
MATCH (u1:User {id: $lurker}), (u2:User {id: $obs}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000001007, id: "SRCHFOLLOW007"}]->(u2);

// ##############################
// ##### Create posts ###########
// ##############################
// One `reachpost`-tagged parent post per user; all of them mention `zyqwombat`
// for the content search.
MERGE (p:Post {id: "SRCHPOSTOBS01"}) SET p.content = "zyqwombat by the observer", p.kind = "short", p.indexed_at = 1600000000001;
MATCH (u:User {id: $obs}), (p:Post {id: "SRCHPOSTOBS01"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy/pub/pubky.app/posts/SRCHPOSTOBS01";
MERGE (p:Post {id: "SRCHPOSTFRI01"}) SET p.content = "zyqwombat by the friend", p.kind = "short", p.indexed_at = 1600000000002;
MATCH (u:User {id: $friend}), (p:Post {id: "SRCHPOSTFRI01"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay/pub/pubky.app/posts/SRCHPOSTFRI01";
MERGE (p:Post {id: "SRCHPOSTFOL01"}) SET p.content = "zyqwombat by the followed", p.kind = "short", p.indexed_at = 1600000000003;
MATCH (u:User {id: $followed}), (p:Post {id: "SRCHPOSTFOL01"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy/pub/pubky.app/posts/SRCHPOSTFOL01";
MERGE (p:Post {id: "SRCHPOSTFLR01"}) SET p.content = "zyqwombat by the follower", p.kind = "short", p.indexed_at = 1600000000004;
MATCH (u:User {id: $follower}), (p:Post {id: "SRCHPOSTFLR01"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xu1n8qam7zjwpg4qtormzjezszs6k9m9hqdp9gsktkzw5dboijcy/pub/pubky.app/posts/SRCHPOSTFLR01";
MERGE (p:Post {id: "SRCHPOSTD2001"}) SET p.content = "zyqwombat by the second degree", p.kind = "short", p.indexed_at = 1600000000005;
MATCH (u:User {id: $d2}), (p:Post {id: "SRCHPOSTD2001"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo/pub/pubky.app/posts/SRCHPOSTD2001";
MERGE (p:Post {id: "SRCHPOSTSTR01"}) SET p.content = "zyqwombat by the stranger", p.kind = "short", p.indexed_at = 1600000000006;
MATCH (u:User {id: $stranger}), (p:Post {id: "SRCHPOSTSTR01"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo/pub/pubky.app/posts/SRCHPOSTSTR01";

// Out-of-reach matches that outrank every in-reach one (the term repeats), so an
// unscoped first page is all stranger posts.
MERGE (p:Post {id: "SRCHPOSTSTR02"}) SET p.content = "zyqwombat zyqwombat zyqwombat", p.kind = "short", p.indexed_at = 1600000000007;
MATCH (u:User {id: $stranger}), (p:Post {id: "SRCHPOSTSTR02"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo/pub/pubky.app/posts/SRCHPOSTSTR02";
MERGE (p:Post {id: "SRCHPOSTSTR03"}) SET p.content = "zyqwombat zyqwombat zyqwombat zyqwombat", p.kind = "short", p.indexed_at = 1600000000008;
MATCH (u:User {id: $stranger}), (p:Post {id: "SRCHPOSTSTR03"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo/pub/pubky.app/posts/SRCHPOSTSTR03";
MERGE (p:Post {id: "SRCHPOSTSTR04"}) SET p.content = "zyqwombat zyqwombat zyqwombat zyqwombat zyqwombat", p.kind = "short", p.indexed_at = 1600000000009;
MATCH (u:User {id: $stranger}), (p:Post {id: "SRCHPOSTSTR04"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo/pub/pubky.app/posts/SRCHPOSTSTR04";

// A tagged reply by the friend: indexed by the tag search, but reach-filtered
// results are parent posts only.
MERGE (p:Post {id: "SRCHPOSTFRIR1"}) SET p.content = "friend reply", p.kind = "short", p.indexed_at = 1600000000010;
MATCH (u:User {id: $friend}), (p:Post {id: "SRCHPOSTFRIR1"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay/pub/pubky.app/posts/SRCHPOSTFRIR1";
MATCH (parent:Post {id: "SRCHPOSTOBS01"}), (reply:Post {id: "SRCHPOSTFRIR1"}) MERGE (reply)-[:REPLIED]->(parent);

// ##############################
// ##### Tag posts ##############
// ##############################
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTOBS01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00001", indexed_at: 1224534097001}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00001";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTFRI01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00002", indexed_at: 1224534097002}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00002";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTFOL01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00003", indexed_at: 1224534097003}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00003";
// Second tagger: FOLLOWED's post leads the engagement ranking.
MATCH (u:User {id: $tagger2}), (p:Post {id: "SRCHPOSTFOL01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00004", indexed_at: 1224534097004}]->(p) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGP00004";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTFLR01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00005", indexed_at: 1224534097005}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00005";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTD2001"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00006", indexed_at: 1224534097006}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00006";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTSTR01"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00007", indexed_at: 1224534097007}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00007";
MATCH (u:User {id: $tagger1}), (p:Post {id: "SRCHPOSTFRIR1"}) MERGE (u)-[rel:TAGGED {label: $post_tag, id: "SRCHTAGP00008", indexed_at: 1224534097008}]->(p) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGP00008";

// ##############################
// ##### Tag user profiles ######
// ##############################
// reachuser scores: OBS 1, FRIEND 2, FOLLOWED 1, FOLLOWER 2, D2 1, STRANGER 2
MATCH (u:User {id: $tagger1}), (t:User {id: $obs}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00001", indexed_at: 1224534098001}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00001";
MATCH (u:User {id: $tagger1}), (t:User {id: $friend}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00002", indexed_at: 1224534098002}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00002";
MATCH (u:User {id: $tagger2}), (t:User {id: $friend}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00003", indexed_at: 1224534098003}]->(t) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGU00003";
MATCH (u:User {id: $tagger1}), (t:User {id: $followed}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00004", indexed_at: 1224534098004}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00004";
MATCH (u:User {id: $tagger1}), (t:User {id: $follower}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00005", indexed_at: 1224534098005}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00005";
MATCH (u:User {id: $tagger2}), (t:User {id: $follower}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00006", indexed_at: 1224534098006}]->(t) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGU00006";
MATCH (u:User {id: $tagger1}), (t:User {id: $d2}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00007", indexed_at: 1224534098007}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00007";
MATCH (u:User {id: $tagger1}), (t:User {id: $stranger}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00008", indexed_at: 1224534098008}]->(t) SET rel.uri = "pubky://yotutfhyeze8486ga9rrz31nnx4h58mqiy3oqmdnn8wbkem9t5xy/pub/pubky.app/tags/SRCHTAGU00008";
MATCH (u:User {id: $tagger2}), (t:User {id: $stranger}) MERGE (u)-[rel:TAGGED {label: $user_tag, id: "SRCHTAGU00009", indexed_at: 1224534098009}]->(t) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGU00009";
// reachuser2 scores: FOLLOWED 1, STRANGER 1
MATCH (u:User {id: $tagger2}), (t:User {id: $followed}) MERGE (u)-[rel:TAGGED {label: $user_tag_2, id: "SRCHTAGU00010", indexed_at: 1224534098010}]->(t) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGU00010";
MATCH (u:User {id: $tagger2}), (t:User {id: $stranger}) MERGE (u)-[rel:TAGGED {label: $user_tag_2, id: "SRCHTAGU00011", indexed_at: 1224534098011}]->(t) SET rel.uri = "pubky://yuquj9be39per9efya4b59139y8y6f3cf3imwduxsqw8q7uocdpo/pub/pubky.app/tags/SRCHTAGU00011";
