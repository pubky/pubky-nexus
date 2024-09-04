// Set up some parameters
:param alice => 'b8e7f3a4c9d6e2f0a1b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5';
:param bob => 'c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7';
:param charlie => 'd8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e';
:param diana => 'e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f';
:param edward => 'f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a';

:param api_tag => 'api';
:param encryption_tag => 'encryption';
:param opensource_tag => 'opensource';

// ##############################
// ##### Create users ###########
// ##############################

MERGE (u:User {id: $alice}) SET u.name = "alice", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $bob}) SET u.name = "bob", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $charlie}) SET u.name = "charlie", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $diana}) SET u.name = "diana", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $edward}) SET u.name = "edward", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";

// ###############################
// ##### Posts related tags ######
// ###############################
MERGE (p:Post {id: "V8N1P3L9J4X0"}) SET p.content = "API security is crucial for ensuring user privacy and data protection", p.kind = "Short", p.indexed_at = 1709308315917;
MATCH (u:User {id: $alice}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $bob}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "6JDN8B3W4F5M", indexed_at: 1724544095}]->(p);
MATCH (u:User {id: $charlie}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "7T0R9X6A2E1L", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "8G7Q5N4W1Z3P", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $edward}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "9B4M8L0X2K7F", indexed_at: 1724334095}]->(p);
MATCH (u:User {id: $edward}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "A5S6P9V3Q0T", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "B7X8N2F4W1J9", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $charlie}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "C1L2P4M7Q8R", indexed_at: 1724334095}]->(p);
MATCH (u:User {id: $bob}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D3W5K8X2J9N", indexed_at: 1724334095}]->(p);


MERGE (p2:Post {id: "3NFG9K0L5QH4"}) SET p2.content = "Open-source software promotes transparency and enhances user trust in digital privacy", p2.kind = "Short", p2.indexed_at = 1719308316921;
MATCH (u2:User {id: $alice}), (p2:Post {id: "3NFG9K0L5QH4"}) MERGE (u2)-[:AUTHORED]->(p2);
MATCH (u:User {id: $bob}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label:$opensource_tag, id: "E4F6M1P0Q2R7", indexed_at: 1724544095}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "F8N2W3X9J4L", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $charlie}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "G5P7Q8L0X1D", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $edward}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "H9X2M4F7N3J", indexed_at: 1724134096}]->(p);

MERGE (p3:Post {id: "A5D6P9V3Q0T"}) SET p3.content = "Encryption standards are vital for securing communications and preserving user privacy", p3.kind = "Short", p3.indexed_at = 1729308318234;
MATCH (u3:User {id: $bob}), (p3:Post {id: "A5D6P9V3Q0T"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "J0L8Q1P5W4K", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "K2N7W9F4X1D", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $charlie}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "L3P8X0J2Q7M", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $edward}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "M4W1N9F2X8J", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $edward}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:BOOKMARKED {id: "A9G7F2L4Q1W3", indexed_at: 1721764200}]->(p);

MERGE (p3:Post {id: "C3L7W0F9Q4K8"}) SET p3.content = "Open-source projects often lead the way in implementing cutting-edge privacy tech innovations", p3.kind = "Short", p3.indexed_at = 1693824823456;
MATCH (u3:User {id: $bob}), (p3:Post {id: "C3L7W0F9Q4K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D2X5P9M1J7L0", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "E4N8Q1W2F6J3", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "K1P6Q9M2X4J8"}) SET p3.content = "Open-source code supports transparency", p3.kind = "Short", p3.indexed_at = 1693824190123;
MATCH (u3:User {id: $bob}), (p3:Post {id: "K1P6Q9M2X4J8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "P8M1X4L2Q9J7", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $diana}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "Q9N2W5F0J8K1", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "L3W5N0F8Q2J7"}) SET p3.content = "Open-source solutions build trusty", p3.kind = "Short", p3.indexed_at = 1693823567890;
MATCH (u3:User {id: $diana}), (p3:Post {id: "L3W5N0F8Q2J7"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "R1L3X7P2F9J8", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bob}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "S4M0Q2N6F1J9", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "M4X1P9L2J6K8"}) SET p3.content = "Open-source enables security auditing", p3.kind = "Short", p3.indexed_at = 1693822934567;
MATCH (u3:User {id: $diana}), (p3:Post {id: "M4X1P9L2J6K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "T5X8L1P3J4N2", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bob}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "U6Q7F0M2X1J8", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "N7Q2F5W8J0L3"}) SET p3.content = "Open-source drives innovation forward", p3.kind = "Short", p3.indexed_at = 1693822331234;
MATCH (u3:User {id: $charlie}), (p3:Post {id: "N7Q2F5W8J0L3"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $alice}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "V8N1P3L9J4X0", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bob}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "J4N8Q1W2F6J3", indexed_at: 1724134092}]->(p);