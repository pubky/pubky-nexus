// Set up some parameters
:param amsterdam => 'emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy';
:param bogota => 'ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny';
:param cairo => 'f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o';
:param detroit => '7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o';
:param eixample => '8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy';

:param api_tag => 'api';
:param encryption_tag => 'encryption';
:param opensource_tag => 'opensource';

// ##############################
// ##### Create users ###########
// ##############################

MERGE (u:User {id: $amsterdam}) SET u.name = "Amsterdam", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $bogota}) SET u.name = "Bogota", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $cairo}) SET u.name = "Cairo", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $detroit}) SET u.name = "Detroit", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $eixample}) SET u.name = "Eixample", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";

// ###############################
// ##### Posts related tags ######
// ###############################
MERGE (p:Post {id: "V8N1P3L9J4X0"}) SET p.content = "API security is crucial for ensuring user privacy and data protection", p.kind = "Short", p.indexed_at = 1709308315917;
MATCH (u:User {id: $amsterdam}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "6JDN8B3W4F5M", indexed_at: 1724544095}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "7T0R9X6A2E1L", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "8G7Q5N4W1Z3P", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "9B4M8L0X2K7F", indexed_at: 1724334095}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "A5S6P9V3Q0T", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "B7X8N2F4W1J9", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "C1L2P4M7Q8R", indexed_at: 1724334095}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D3W5K8X2J9N", indexed_at: 1724334095}]->(p);


MERGE (p2:Post {id: "3NFG9K0L5QH4"}) SET p2.content = "Open-source software promotes transparency and enhances user trust in digital privacy", p2.kind = "Short", p2.indexed_at = 1719308316921;
MATCH (u2:User {id: $amsterdam}), (p2:Post {id: "3NFG9K0L5QH4"}) MERGE (u2)-[:AUTHORED]->(p2);
MATCH (u:User {id: $bogota}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label:$opensource_tag, id: "E4F6M1P0Q2R7", indexed_at: 1724544095}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "F8N2W3X9J4L", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "G5P7Q8L0X1D", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "H9X2M4F7N3J", indexed_at: 1724134096}]->(p);

MERGE (p3:Post {id: "A5D6P9V3Q0T"}) SET p3.content = "Encryption standards are vital for securing communications and preserving user privacy", p3.kind = "Short", p3.indexed_at = 1729308318234;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "A5D6P9V3Q0T"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "J0L8Q1P5W4K", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "K2N7W9F4X1D", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "L3P8X0J2Q7M", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "M4W1N9F2X8J", indexed_at: 1724134092}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:BOOKMARKED {id: "A9G7F2L4Q1W3", indexed_at: 1721764200}]->(p);

MERGE (p3:Post {id: "C3L7W0F9Q4K8"}) SET p3.content = "Open-source projects often lead the way in implementing cutting-edge privacy tech innovations", p3.kind = "Short", p3.indexed_at = 1693824823456;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "C3L7W0F9Q4K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D2X5P9M1J7L0", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "E4N8Q1W2F6J3", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "K1P6Q9M2X4J8"}) SET p3.content = "Open-source code supports transparency", p3.kind = "Short", p3.indexed_at = 1693824190123;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "K1P6Q9M2X4J8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "P8M1X4L2Q9J7", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "Q9N2W5F0J8K1", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "L3W5N0F8Q2J7"}) SET p3.content = "Open-source solutions build trusty", p3.kind = "Short", p3.indexed_at = 1693823567890;
MATCH (u3:User {id: $detroit}), (p3:Post {id: "L3W5N0F8Q2J7"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "R1L3X7P2F9J8", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "S4M0Q2N6F1J9", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "M4X1P9L2J6K8"}) SET p3.content = "Open-source enables security auditing", p3.kind = "Short", p3.indexed_at = 1693822934567;
MATCH (u3:User {id: $detroit}), (p3:Post {id: "M4X1P9L2J6K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "T5X8L1P3J4N2", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "U6Q7F0M2X1J8", indexed_at: 1724134092}]->(p);

MERGE (p3:Post {id: "N7Q2F5W8J0L3"}) SET p3.content = "Open-source drives innovation forward", p3.kind = "Short", p3.indexed_at = 1693822331234;
MATCH (u3:User {id: $cairo}), (p3:Post {id: "N7Q2F5W8J0L3"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "V8N1P3L9J4X0", indexed_at: 1724134080}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "J4N8Q1W2F6J3", indexed_at: 1724134092}]->(p);

// ##################################
// ##### Posts related replies ######
// ##################################
MERGE (p1:Post {id: "1A1P4D8C9K0F"}) SET p1.content = "whop whop", p1.kind = "Short", p1.indexed_at = 1719477230000;
MATCH (amst:User {id: $amsterdam}), (p1:Post {id: "1A1P4D8C9K0F"}) MERGE (amst)-[:AUTHORED]->(p1);

MERGE (p2:Post {id: "2B9XKZG3T4L6"}) SET p2.content = "we did it!", p2.kind = "Short", p2.indexed_at = 1719477230015;
MATCH (bog:User {id: $bogota}), (p2:Post {id: "2B9XKZG3T4L6"}) MERGE (bog)-[:AUTHORED]->(p2);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p2:Post {id: "2B9XKZG3T4L6"}) MERGE (p2)-[:REPLIED]->(p1);

MERGE (p3:Post {id: "3M6WQ8F5P9R2"}) SET p3.content = "keep building", p3.kind = "Short", p3.indexed_at = 1719477230019;
MATCH (cai:User {id: $cairo}), (p3:Post {id: "3M6WQ8F5P9R2"}) MERGE (cai)-[:AUTHORED]->(p3);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p3:Post {id: "3M6WQ8F5P9R2"}) MERGE (p3)-[:REPLIED]->(p1);

MERGE (p4:Post {id: "4T7ZV0C8K5B1"}) SET p4.content = "yiaaaah", p4.kind = "Short", p4.indexed_at = 1719477230030;
MATCH (det:User {id: $detroit}), (p4:Post {id: "4T7ZV0C8K5B1"}) MERGE (det)-[:AUTHORED]->(p4);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p4:Post {id: "4T7ZV0C8K5B1"}) MERGE (p4)-[:REPLIED]->(p1);

MERGE (p5:Post {id: "5F8YQJ1L2D3E"}) SET p5.content = "finally", p5.kind = "Short", p5.indexed_at = 1719477230088;
MATCH (eix:User {id: $eixample}), (p5:Post {id: "5F8YQJ1L2D3E"}) MERGE (eix)-[:AUTHORED]->(p5);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p5:Post {id: "5F8YQJ1L2D3E"}) MERGE (p5)-[:REPLIED]->(p1);

MERGE (p6:Post {id: "6G3ZB9X0H7M4"}) SET p6.content = "we enjoy the path", p6.kind = "Short", p6.indexed_at = 1719477230101;
MATCH (eix:User {id: $eixample}), (p6:Post {id: "6G3ZB9X0H7M4"}) MERGE (eix)-[:AUTHORED]->(p6);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p6:Post {id: "6G3ZB9X0H7M4"}) MERGE (p6)-[:REPLIED]->(p1);

MERGE (p7:Post {id: "7N8K0Y1C3T2Q"}) SET p7.content = "what is next?", p7.kind = "Short", p7.indexed_at = 1719477230208;
MATCH (cai:User {id: $cairo}), (p7:Post {id: "7N8K0Y1C3T2Q"}) MERGE (cai)-[:AUTHORED]->(p7);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p7:Post {id: "7N8K0Y1C3T2Q"}) MERGE (p7)-[:REPLIED]->(p1);