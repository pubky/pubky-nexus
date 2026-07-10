// Web of Trust test fixture. Kept inert against the global timeline/engagement/
// hot-tags/influencer suites: post indexed_at far below every window start, zero
// engagement on stream posts, the mod-bot post is a reply, tag labels unique with
// year-2008 timestamps. Topology is in the MERGE blocks below (O = observer;
// S = spammer with no inbound follows; M = mod bot; BTC*/ART1 = domain-tag targets).
// Fixture user ids deliberately sort high so they stay out of the zero-score,
// id-ordered window the global-influencer suite asserts over (lower ids would evict
// the users it expects).

// Set up parameters (session-based)
:param o_obs => 'y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao';
:param d1 => 'qjftuwjog819ki1wktuy5tndebce36bmxxwtjjm3z1fr97jk9yuo';
:param d1b => 't5ixbtatg4tq5q5ixg16qqrg1bmem75ksg6cweuftuydwzw91pzy';
:param d2 => 'smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y';
:param spammer => 'qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o';
:param modbot => 'qsfngw6xm9kk7yp99xustjfj8mu9auufkixas5f8goeujuxt45ao';
:param btc1 => 'tfqnfppxtr8xei6n3zrfa1b7mmc6gqn41szgutgcccea33pqf3yo';
:param btc2 => 'uuxjusor98rw4xdo3k3shsgdqwi844si14aaxfcnjxyczjt5eqxy';
:param btc3 => 'qwzn6jx1gm1ziptn41dxonqy1rpuumwggdq1hu6zc334qep3kjho';
:param btc4 => 'z5eect18reuccuwuq78da8k5re3y8si346n3bah45gad6t6b1zby';
:param btc5 => 'wbhcz1gfz14jc4qjg74auyo5bwxd4gc3y84ic18iro17yi4bgz3y';
:param artist1 => 'w153s1dr9rw6t8s3nd1de6pqquuprb37dwrnwh3nk85jt9ys9k7o';

:param bitcoiner_tag => 'bitcoiner';
:param btcdev_tag => 'btc-dev';
:param artist_tag => 'artist';
:param nudity_tag => 'nudity';

// ##############################
// ##### Create users ###########
// ##############################
MERGE (u:User {id: $o_obs}) SET u.name = "wot_observer", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $d1}) SET u.name = "wot_d1", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $d1b}) SET u.name = "wot_d1b", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $d2}) SET u.name = "wot_d2", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $spammer}) SET u.name = "wot_spammer", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $modbot}) SET u.name = "wot_modbot", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $btc1}) SET u.name = "wot_btc1", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $btc2}) SET u.name = "wot_btc2", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $btc3}) SET u.name = "wot_btc3", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $btc4}) SET u.name = "wot_btc4", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $btc5}) SET u.name = "wot_btc5", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";
MERGE (u:User {id: $artist1}) SET u.name = "wot_artist1", u.bio = "", u.status = "undefined", u.indexed_at = 1650000000000, u.links = "[]";

// ##############################
// ##### Create follows #########
// ##############################
MATCH (u1:User {id: $o_obs}), (u2:User {id: $d1}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000001, id: "WOTFOLLOW0001"}]->(u2);
MATCH (u1:User {id: $o_obs}), (u2:User {id: $d1b}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000002, id: "WOTFOLLOW0002"}]->(u2);
MATCH (u1:User {id: $o_obs}), (u2:User {id: $modbot}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000003, id: "WOTFOLLOW0003"}]->(u2);
MATCH (u1:User {id: $d1}), (u2:User {id: $d2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000004, id: "WOTFOLLOW0004"}]->(u2);
MATCH (u1:User {id: $d1b}), (u2:User {id: $d2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000005, id: "WOTFOLLOW0005"}]->(u2);
// Cycle back to the observer: D1 follows O, so O is reachable as an author via
// O->D1->O at depth 2. The stream must still exclude the observer's own posts.
MATCH (u1:User {id: $d1}), (u2:User {id: $o_obs}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230000000006, id: "WOTFOLLOW0006"}]->(u2);

// ##############################
// ##### Create parent posts ####
// ##############################
MERGE (p:Post {id: "WOTPOSTO00001"}) SET p.content = "observer own post", p.kind = "short", p.indexed_at = 1650000000001;
MATCH (u:User {id: $o_obs}), (p:Post {id: "WOTPOSTO00001"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTD10002"}) SET p.content = "d1 post", p.kind = "short", p.indexed_at = 1650000000002;
MATCH (u:User {id: $d1}), (p:Post {id: "WOTPOSTD10002"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTD1B003"}) SET p.content = "d1b post", p.kind = "short", p.indexed_at = 1650000000003;
MATCH (u:User {id: $d1b}), (p:Post {id: "WOTPOSTD1B003"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTD20004"}) SET p.content = "d2 post", p.kind = "short", p.indexed_at = 1650000000004;
MATCH (u:User {id: $d2}), (p:Post {id: "WOTPOSTD20004"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTS00006"}) SET p.content = "spammer post", p.kind = "short", p.indexed_at = 1650000000006;
MATCH (u:User {id: $spammer}), (p:Post {id: "WOTPOSTS00006"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTBTC1A"}) SET p.content = "btc1 post", p.kind = "short", p.indexed_at = 1650000000007;
MATCH (u:User {id: $btc1}), (p:Post {id: "WOTPOSTBTC1A"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTBTC2A"}) SET p.content = "btc2 post", p.kind = "short", p.indexed_at = 1650000000008;
MATCH (u:User {id: $btc2}), (p:Post {id: "WOTPOSTBTC2A"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTBTC3A"}) SET p.content = "btc3 post", p.kind = "short", p.indexed_at = 1650000000009;
MATCH (u:User {id: $btc3}), (p:Post {id: "WOTPOSTBTC3A"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTBTC4A"}) SET p.content = "btc4 post", p.kind = "short", p.indexed_at = 1650000000010;
MATCH (u:User {id: $btc4}), (p:Post {id: "WOTPOSTBTC4A"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTBTC5A"}) SET p.content = "btc5 post", p.kind = "short", p.indexed_at = 1650000000011;
MATCH (u:User {id: $btc5}), (p:Post {id: "WOTPOSTBTC5A"}) MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post {id: "WOTPOSTART1A"}) SET p.content = "artist1 post", p.kind = "short", p.indexed_at = 1650000000012;
MATCH (u:User {id: $artist1}), (p:Post {id: "WOTPOSTART1A"}) MERGE (u)-[:AUTHORED]->(p);

// ##############################
// ##### Mod-bot reply post #####
// ##############################
// Reply by D2 to D1's post; excluded from parent streams. Mod bot tags it nudity.
MERGE (p:Post {id: "WOTPOSTREPLY1"}) SET p.content = "reply needing moderation", p.kind = "short", p.indexed_at = 1650000000005;
MATCH (u:User {id: $d2}), (p:Post {id: "WOTPOSTREPLY1"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (parent:Post {id: "WOTPOSTD10002"}), (reply:Post {id: "WOTPOSTREPLY1"}) MERGE (reply)-[:REPLIED]->(parent);
MATCH (u:User {id: $modbot}), (p:Post {id: "WOTPOSTREPLY1"}) MERGE (u)-[:TAGGED {label: $nudity_tag, id: "WOTTAGNUDITY1", indexed_at: 1224534095000}]->(p);

// WoT post-tag limit/pagination fixture: a deep reply (reply-to-a-reply, so it is
// excluded from parent streams AND from the engagement index, keeping its tags out
// of the global streams) tagged by WoT members D1, D1B ('wotreview') and M
// ('wotflag'). Unique labels, year-2008 timestamps, so it stays inert elsewhere.
MERGE (p:Post {id: "WOTPOSTTAGS01"}) SET p.content = "wot tag limit fixture", p.kind = "short", p.indexed_at = 1650000000007;
MATCH (u:User {id: $d2}), (p:Post {id: "WOTPOSTTAGS01"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (parent:Post {id: "WOTPOSTREPLY1"}), (reply:Post {id: "WOTPOSTTAGS01"}) MERGE (reply)-[:REPLIED]->(parent);
MATCH (u:User {id: $d1}), (p:Post {id: "WOTPOSTTAGS01"}) MERGE (u)-[:TAGGED {label: "wotreview", id: "WOTTAGREV0001", indexed_at: 1224534095800}]->(p);
MATCH (u:User {id: $d1b}), (p:Post {id: "WOTPOSTTAGS01"}) MERGE (u)-[:TAGGED {label: "wotreview", id: "WOTTAGREV0002", indexed_at: 1224534095900}]->(p);
MATCH (u:User {id: $modbot}), (p:Post {id: "WOTPOSTTAGS01"}) MERGE (u)-[:TAGGED {label: "wotflag", id: "WOTTAGFLAG001", indexed_at: 1224534096000}]->(p);

// ##############################
// ##### Domain user->user tags #
// ##############################
// Endorsed by O's WoT (bitcoiner): D1->BTC1, D1->BTC2, D1B->BTC3, D1B->BTC1 (dedup)
MATCH (from:User {id: $d1}), (to:User {id: $btc1}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGBTC0001", indexed_at: 1224534095100}]->(to);
MATCH (from:User {id: $d1}), (to:User {id: $btc2}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGBTC0002", indexed_at: 1224534095200}]->(to);
MATCH (from:User {id: $d1b}), (to:User {id: $btc3}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGBTC0003", indexed_at: 1224534095300}]->(to);
MATCH (from:User {id: $d1b}), (to:User {id: $btc1}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGBTC0004", indexed_at: 1224534095400}]->(to);
// Endorsed via btc-dev by depth-2 rater D2 -> BTC4
MATCH (from:User {id: $d2}), (to:User {id: $btc4}) MERGE (from)-[:TAGGED {label: $btcdev_tag, id: "WOTTAGBTC0005", indexed_at: 1224534095500}]->(to);
// Out of WoT: SPAMMER endorses BTC5 (bitcoiner) and ARTIST1 (artist) -> not visible to O.
// These double as SPAMMER's own (depth-0 "Me") domain endorsements: a wot_domain
// stream for SPAMMER at depth=0 surfaces BTC5/ARTIST1's posts (its follow-network is
// empty, so only its own TAGGED edges count).
MATCH (from:User {id: $spammer}), (to:User {id: $btc5}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGBTC0006", indexed_at: 1224534095600}]->(to);
MATCH (from:User {id: $spammer}), (to:User {id: $artist1}) MERGE (from)-[:TAGGED {label: $artist_tag, id: "WOTTAGART0001", indexed_at: 1224534095700}]->(to);
// SPAMMER self-tags as bitcoiner: its own post must still be excluded from its
// own depth-0 feed (self-exclusion applies to the "Me" trust set too).
MATCH (from:User {id: $spammer}), (to:User {id: $spammer}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGSELF002", indexed_at: 1224534096300}]->(to);
// Self-endorsement: D1 (in O's WoT) tags the OBSERVER as bitcoiner. O's own posts
// must still be excluded from O's wot_domain feed (self-exclusion, like `wot`).
MATCH (from:User {id: $d1}), (to:User {id: $o_obs}) MERGE (from)-[:TAGGED {label: $bitcoiner_tag, id: "WOTTAGSELF001", indexed_at: 1224534096100}]->(to);

// ##################################
// ##### WoT post-tag pagination ####
// ##################################
// A reply authored OUTSIDE O's trust network (spammer), so it stays out of O's
// `wot` stream and — being a reply with year-2008 tag timestamps — out of the
// global timeline/engagement/hot-tags assertions. O's WoT applies five `wmtag*`
// labels (one tagger each) plus the mod bot's flag `wmtagflag`, which sorts last
// by (tagger count, label). The old limit_tags=5 paginated the flag out of the
// WoT tag view; it must now be returned by default.
MERGE (p:Post {id: "WOTPOSTMODF01"}) SET p.content = "heavily flagged reply", p.kind = "short", p.indexed_at = 1650000000013;
MATCH (u:User {id: $spammer}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (parent:Post {id: "WOTPOSTTAGS01"}), (reply:Post {id: "WOTPOSTMODF01"}) MERGE (reply)-[:REPLIED]->(parent);
MATCH (u:User {id: $d1}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtag1", id: "WOTTAGMOD0001", indexed_at: 1224534096200}]->(p);
MATCH (u:User {id: $d1}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtag2", id: "WOTTAGMOD0002", indexed_at: 1224534096201}]->(p);
MATCH (u:User {id: $d1b}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtag3", id: "WOTTAGMOD0003", indexed_at: 1224534096202}]->(p);
MATCH (u:User {id: $d1b}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtag4", id: "WOTTAGMOD0004", indexed_at: 1224534096203}]->(p);
MATCH (u:User {id: $d2}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtag5", id: "WOTTAGMOD0005", indexed_at: 1224534096204}]->(p);
MATCH (u:User {id: $modbot}), (p:Post {id: "WOTPOSTMODF01"}) MERGE (u)-[:TAGGED {label: "wmtagflag", id: "WOTTAGMOD0006", indexed_at: 1224534096205}]->(p);
