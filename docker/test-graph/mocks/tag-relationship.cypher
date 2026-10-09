// Fixture for the viewer's tag on tag details and taggers.
// Isolated: its own users, post, resource and labels, so no other test's counts change.
// The viewer's tags live in another app's folder (mapky), so their stored `uri`
// can't be rebuilt from ids.
:param trel_viewer => 'xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o';
:param trel_tagger => 'ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o';
:param trel_author => 'zxtmja5wju4pi5s681i1xrr957tz1n6o5g6489erm7usiok1jmto';
:param trel_tagged => 'xcnswkwriypnzekfuwed6h5xecinwjj46x3ao7ffppy61z6hafiy';
// Only the tests that make the index disagree with the graph read this user: they write to its Redis keys
:param trel_drift => 'uq1swy84jaw6pmqi399oqcf6izs86peamftbfhcxjhzywu97x8wo';

MERGE (u:User {id: $trel_viewer}) SET u.name = "TagRelViewer", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_tagger}) SET u.name = "TagRelTagger", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_author}) SET u.name = "TagRelAuthor", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_tagged}) SET u.name = "TagRelTagged", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_drift}) SET u.name = "TagRelDrift", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";

MERGE (p:Post {id: "2ZRT8G4VXQ0M0"}) SET p.content = "Tag relationship fixture post", p.kind = "short", p.indexed_at = 1724134096000;
MATCH (u:User {id: $trel_author}), (p:Post {id: "2ZRT8G4VXQ0M0"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://zxtmja5wju4pi5s681i1xrr957tz1n6o5g6489erm7usiok1jmto/pub/pubky.app/posts/2ZRT8G4VXQ0M0";

// Fixture resource id: not derived from the uri; only the resource tag routes read it.
MERGE (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000001"}) SET r.uri = "https://example.com/tag-relationship", r.scheme = "https", r.indexed_at = 1724134097000;

// Post: the viewer and the tagger share `trel-shared`; only the tagger has `trel-other`.
MATCH (u:User {id: $trel_viewer}), (p:Post {id: "2ZRT8G4VXQ0M0"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELPOSTV01", indexed_at: 1724134098000, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELPOSTV01"}]->(p);
MATCH (u:User {id: $trel_tagger}), (p:Post {id: "2ZRT8G4VXQ0M0"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELPOSTT01", indexed_at: 1724134098001, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELPOSTT01"}]->(p);
MATCH (u:User {id: $trel_tagger}), (p:Post {id: "2ZRT8G4VXQ0M0"}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELPOSTT02", indexed_at: 1724134098002, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELPOSTT02"}]->(p);

// User: same shape.
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_tagged}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELUSERV01", indexed_at: 1724134098003, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELUSERV01"}]->(t);
MATCH (u:User {id: $trel_tagger}), (t:User {id: $trel_tagged}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELUSERT01", indexed_at: 1724134098004, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELUSERT01"}]->(t);
MATCH (u:User {id: $trel_tagger}), (t:User {id: $trel_tagged}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELUSERT02", indexed_at: 1724134098005, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELUSERT02"}]->(t);

// Drift user: the viewer's own tag, so the test reads a stored address next to the faked one.
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_drift}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELDRIFTV01", indexed_at: 1724134098009, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELDRIFTV01"}]->(t);
// Two more of the viewer's tags, for the WoT test that makes the global taggers sets disagree with the graph.
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_drift}) MERGE (u)-[:TAGGED {label: "trel-wot-ruled-out", id: "TRELDRIFTV02", indexed_at: 1724134098023, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELDRIFTV02"}]->(t);
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_drift}) MERGE (u)-[:TAGGED {label: "trel-wot-unindexed", id: "TRELDRIFTV03", indexed_at: 1724134098024, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELDRIFTV03"}]->(t);

// Resource: same shape.
MATCH (u:User {id: $trel_viewer}), (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000001"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELRESV01", indexed_at: 1724134098006, app: "mapky", uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELRESV01"}]->(r);
MATCH (u:User {id: $trel_tagger}), (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000001"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELREST01", indexed_at: 1724134098007, app: "mapky", uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/mapky/tags/TRELREST01"}]->(r);
MATCH (u:User {id: $trel_tagger}), (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000001"}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELREST02", indexed_at: 1724134098008, app: "mapky", uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/mapky/tags/TRELREST02"}]->(r);

// Cache-miss targets: the cache-miss tests delete their Redis tag keys, so no other test reads
// them. The resource streams do list the resource: see the nextest group `resource-tag-index`.
:param trel_miss_user => 'twtytiiy6girjjoep9amicagowbde5m7n3dcpjdeuehzstca13oo';
// Its own author: an author stream of the shared fixture author would read the miss post's tags
:param trel_miss_author => 'xxthkxjtb16wto9b7sud74dmsetqi54ewiisxaeadt6bhy6j9zmo';
MERGE (u:User {id: $trel_miss_user}) SET u.name = "TagRelMiss", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_miss_author}) SET u.name = "TagRelMissAuthor", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (p:Post {id: "2ZRT8G4VXQ0N0"}) SET p.content = "Tag relationship cache-miss fixture post", p.kind = "short", p.indexed_at = 1724134096001;
MATCH (u:User {id: $trel_miss_author}), (p:Post {id: "2ZRT8G4VXQ0N0"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://xxthkxjtb16wto9b7sud74dmsetqi54ewiisxaeadt6bhy6j9zmo/pub/pubky.app/posts/2ZRT8G4VXQ0N0";
MERGE (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000002"}) SET r.uri = "https://example.com/tag-relationship-miss", r.scheme = "https", r.indexed_at = 1724134097001;

MATCH (u:User {id: $trel_viewer}), (p:Post {id: "2ZRT8G4VXQ0N0"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELMISSPV01", indexed_at: 1724134098010, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSPV01"}]->(p);
MATCH (u:User {id: $trel_tagger}), (p:Post {id: "2ZRT8G4VXQ0N0"}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELMISSPT01", indexed_at: 1724134098011, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELMISSPT01"}]->(p);
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_miss_user}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELMISSUV01", indexed_at: 1724134098012, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSUV01"}]->(t);
MATCH (u:User {id: $trel_tagger}), (t:User {id: $trel_miss_user}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELMISSUT01", indexed_at: 1724134098013, uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/pubky.app/tags/TRELMISSUT01"}]->(t);
MATCH (u:User {id: $trel_viewer}), (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000002"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELMISSRV01", indexed_at: 1724134098014, app: "mapky", uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELMISSRV01"}]->(r);
MATCH (u:User {id: $trel_tagger}), (r:Resource {id: "7a6e1c0de5f0a11ce0ffee0000000002"}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELMISSRT01", indexed_at: 1724134098015, app: "mapky", uri: "pubky://ww9hfpxrbq6hg8b34gg57irnhfk1iz9b8px9o7biy1d1dz1xep8o/pub/mapky/tags/TRELMISSRT01"}]->(r);

// Web of Trust targets: the viewer follows a WoT tagger who tags them, so their labels show
// with `depth`. Only the WoT tests read them; the user's WoT cache keys are its own.
// User ids sort after "p": the today/this-month influencer caches keep 100 users by id among
// equal scores, and a lower id pushes out a user the influencer tests expect.
:param trel_wot_tagger => 'xfidtk4ikwqrrojyog6ufdn6i4je67meubho31r95j4e7c4zarxo';
:param trel_wot_user => 'ri3o8565ke5ngbknd9yg1g8gcxuuwk51owjwdioyqsxzqppy3dby';
MERGE (u:User {id: $trel_wot_tagger}) SET u.name = "TagRelWotTagger", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MERGE (u:User {id: $trel_wot_user}) SET u.name = "TagRelWotUser", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000, u.links = "[]";
MATCH (u1:User {id: $trel_viewer}), (u2:User {id: $trel_wot_tagger}) MERGE (u1)-[:FOLLOWS {indexed_at: 1724134098016, id: "TRELWOTFOLLOW1"}]->(u2);
MERGE (p:Post {id: "2ZRT8G4VXQ0Q0"}) SET p.content = "Tag relationship WoT fixture post", p.kind = "short", p.indexed_at = 1724134096002;
MATCH (u:User {id: $trel_author}), (p:Post {id: "2ZRT8G4VXQ0Q0"}) MERGE (u)-[:AUTHORED]->(p) SET p.uri = "pubky://zxtmja5wju4pi5s681i1xrr957tz1n6o5g6489erm7usiok1jmto/pub/pubky.app/posts/2ZRT8G4VXQ0Q0";

MATCH (u:User {id: $trel_viewer}), (p:Post {id: "2ZRT8G4VXQ0Q0"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELWOTPV01", indexed_at: 1724134098017, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELWOTPV01"}]->(p);
MATCH (u:User {id: $trel_wot_tagger}), (p:Post {id: "2ZRT8G4VXQ0Q0"}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELWOTPT01", indexed_at: 1724134098018, uri: "pubky://xfidtk4ikwqrrojyog6ufdn6i4je67meubho31r95j4e7c4zarxo/pub/pubky.app/tags/TRELWOTPT01"}]->(p);
MATCH (u:User {id: $trel_wot_tagger}), (p:Post {id: "2ZRT8G4VXQ0Q0"}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELWOTPT02", indexed_at: 1724134098019, uri: "pubky://xfidtk4ikwqrrojyog6ufdn6i4je67meubho31r95j4e7c4zarxo/pub/pubky.app/tags/TRELWOTPT02"}]->(p);
MATCH (u:User {id: $trel_viewer}), (t:User {id: $trel_wot_user}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELWOTUV01", indexed_at: 1724134098020, uri: "pubky://xcb38btb3ioaoxtinmwtmizfum1akeg5yajju67o6rrcxq47n76o/pub/mapky/tags/TRELWOTUV01"}]->(t);
MATCH (u:User {id: $trel_wot_tagger}), (t:User {id: $trel_wot_user}) MERGE (u)-[:TAGGED {label: "trel-shared", id: "TRELWOTUT01", indexed_at: 1724134098021, uri: "pubky://xfidtk4ikwqrrojyog6ufdn6i4je67meubho31r95j4e7c4zarxo/pub/pubky.app/tags/TRELWOTUT01"}]->(t);
MATCH (u:User {id: $trel_wot_tagger}), (t:User {id: $trel_wot_user}) MERGE (u)-[:TAGGED {label: "trel-other", id: "TRELWOTUT02", indexed_at: 1724134098022, uri: "pubky://xfidtk4ikwqrrojyog6ufdn6i4je67meubho31r95j4e7c4zarxo/pub/pubky.app/tags/TRELWOTUT02"}]->(t);
