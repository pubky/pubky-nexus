// Resource nodes for universal tag testing
// Uses existing user params from posts.cypher (amsterdam, bogota)
:param amsterdam => 'emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy';
:param bogota => 'ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny';

// Resource IDs are computed as hex(BLAKE3(normalized_uri)[0..16])
// https://example.com/article -> 450a72e3da164bfc3ac5f4056f9e5c7c
// pubky://somepk/pub/eventky.app/events/E001 -> fb4155a2295ff3a8a8fe02e28229c021
// https://example.com/video -> e23f778c4f2a84606f350e4df1a918e9

// Resource 1: External HTTPS article tagged by 2 users from mapky
MERGE (r1:Resource {id: "450a72e3da164bfc3ac5f4056f9e5c7c"})
SET r1.uri = "https://example.com/article", r1.scheme = "https", r1.indexed_at = 1724134095000;

// Resource 2: Internal-unknown eventky event tagged by 1 user from eventky
MERGE (r2:Resource {id: "fb4155a2295ff3a8a8fe02e28229c021"})
SET r2.uri = "pubky://somepk/pub/eventky.app/events/E001", r2.scheme = "pubky", r2.indexed_at = 1724234095000;

// Resource 3: External HTTPS video tagged by 1 user from mapky
MERGE (r3:Resource {id: "e23f778c4f2a84606f350e4df1a918e9"})
SET r3.uri = "https://example.com/video", r3.scheme = "https", r3.indexed_at = 1724334095000;

// Tags on Resource 1 (article): 2 users tag "bitcoin" from mapky, 1 tags "interesting" from eventky
MATCH (u:User {id: $amsterdam}), (r:Resource {id: "450a72e3da164bfc3ac5f4056f9e5c7c"})
MERGE (u)-[:TAGGED {label: "bitcoin", id: "RES_TAG_001", indexed_at: 1724544095000, app: "mapky"}]->(r);

MATCH (u:User {id: $bogota}), (r:Resource {id: "450a72e3da164bfc3ac5f4056f9e5c7c"})
MERGE (u)-[:TAGGED {label: "bitcoin", id: "RES_TAG_002", indexed_at: 1724544095001, app: "mapky"}]->(r);

MATCH (u:User {id: $amsterdam}), (r:Resource {id: "450a72e3da164bfc3ac5f4056f9e5c7c"})
MERGE (u)-[:TAGGED {label: "interesting", id: "RES_TAG_003", indexed_at: 1724544095002, app: "eventky"}]->(r);

// Tags on Resource 2 (eventky event): 1 user tags "calendar" from eventky
MATCH (u:User {id: $bogota}), (r:Resource {id: "fb4155a2295ff3a8a8fe02e28229c021"})
MERGE (u)-[:TAGGED {label: "calendar", id: "RES_TAG_004", indexed_at: 1724544095003, app: "eventky"}]->(r);

// Tags on Resource 3 (video): 1 user tags "bitcoin" from mapky
MATCH (u:User {id: $amsterdam}), (r:Resource {id: "e23f778c4f2a84606f350e4df1a918e9"})
MERGE (u)-[:TAGGED {label: "bitcoin", id: "RES_TAG_005", indexed_at: 1724544095004, app: "mapky"}]->(r);
