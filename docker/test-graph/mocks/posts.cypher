// Set up some parameters
:param amsterdam => 'emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy';
:param bogota => 'ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny';
:param cairo => 'f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o';
:param detroit => '7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o';
:param eixample => '8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy';

:param api_tag => 'api';
:param encryption_tag => 'encryption';
:param opensource_tag => 'opensource';
:param Fk_tag => '4k';
:param pubky_tag => 'pubky';

// ##############################
// ##### Create users ###########
// ##############################

MERGE (u:User {id: $amsterdam}) SET u.name = "Amsterdam", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $bogota}) SET u.name = "Bogota", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $cairo}) SET u.name = "Cairo", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $detroit}) SET u.name = "Detroit", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $eixample}) SET u.name = "Eixample", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";

// #####################################
// ####### Create follows ##############
// #####################################
MATCH (u1:User {id: $amsterdam}), (u2:User {id: $bogota}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475736000, id: "P73K85JG5SZT6"}]->(u2);
MATCH (u1:User {id: $detroit}), (u2:User {id: $bogota}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475730000, id: "P73K85JG5SZT7"}]->(u2);
MATCH (u1:User {id: $amsterdam}), (u2:User {id: $cairo}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475725000, id: "P73K85JG5SZT5"}]->(u2);
MATCH (u1:User {id: $detroit}), (u2:User {id: $cairo}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475740000, id: "P73K85JG5SZS6"}]->(u2);

MATCH (u1:User {id: $detroit}), (u2:User {id: $eixample}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475900000, id: "P73K44JG5SZT7"}]->(u2);
MATCH (u1:User {id: $eixample}), (u2:User {id: $detroit}) MERGE (u1)-[:FOLLOWS {indexed_at: 1730475940000, id: "P73K85JG5DZT7"}]->(u2);

// ###############################
// ##### Posts related tags ######
// ###############################
MERGE (p:Post {id: "V8N1P3L9J4X0"}) SET p.content = "API security is crucial for ensuring user privacy and data protection", p.kind = "short", p.indexed_at = 1709308315917;
MATCH (u:User {id: $amsterdam}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "6JDN8B3W4F5M", indexed_at: 1724544095000}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "7T0R9X6A2E1L", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "8G7Q5N4W1Z3P", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "9B4M8L0X2K7F", indexed_at: 1724334095000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "A5S6P9V3Q0T", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "B7X8N2F4W1J9", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "C1L2P4M7Q8R", indexed_at: 1724334095000}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "V8N1P3L9J4X0"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D3W5K8X2J9N", indexed_at: 1724334095000}]->(p);


MERGE (p2:Post {id: "3NFG9K0L5QH4"}) SET p2.content = "Open-source software promotes transparency and enhances user trust in digital privacy", p2.kind = "short", p2.indexed_at = 1719308316921;
MATCH (u2:User {id: $amsterdam}), (p2:Post {id: "3NFG9K0L5QH4"}) MERGE (u2)-[:AUTHORED]->(p2);
MATCH (u:User {id: $bogota}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label:$opensource_tag, id: "E4F6M1P0Q2R7", indexed_at: 1724544095000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $api_tag, id: "F8N2W3X9J4L", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "G5P7Q8L0X1D", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "3NFG9K0L5QH4"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "H9X2M4F7N3J", indexed_at: 1724134096000}]->(p);

MERGE (p3:Post {id: "A5D6P9V3Q0T"}) SET p3.content = "Encryption standards are vital for securing communications and preserving user privacy", p3.kind = "short", p3.indexed_at = 1729308318234;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "A5D6P9V3Q0T"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "J0L8Q1P5W4K", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "K2N7W9F4X1D", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $cairo}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "L3P8X0J2Q7M", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "M4W1N9F2X8J", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "A5D6P9V3Q0T"}) MERGE (u)-[:BOOKMARKED {id: "A9G7F2L4Q1W3", indexed_at: 1721764200000}]->(p);

MERGE (p3:Post {id: "C3L7W0F9Q4K8"}) SET p3.content = "Open-source projects often lead the way in implementing cutting-edge privacy tech innovations", p3.kind = "short", p3.indexed_at = 1693824823456;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "C3L7W0F9Q4K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "D2X5P9M1J7L0", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "C3L7W0F9Q4K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "E4N8Q1W2F6J3", indexed_at: 1724134092000}]->(p);

MERGE (p3:Post {id: "K1P6Q9M2X4J8"}) SET p3.content = "Open-source code supports transparency", p3.kind = "short", p3.indexed_at = 1693824190123;
MATCH (u3:User {id: $bogota}), (p3:Post {id: "K1P6Q9M2X4J8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "P8M1X4L2Q9J7", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "K1P6Q9M2X4J8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "Q9N2W5F0J8K1", indexed_at: 1724134092000}]->(p);

MERGE (p3:Post {id: "L3W5N0F8Q2J7"}) SET p3.content = "Open-source solutions build trusty", p3.kind = "short", p3.indexed_at = 1693823567890;
MATCH (u3:User {id: $detroit}), (p3:Post {id: "L3W5N0F8Q2J7"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "R1L3X7P2F9J8", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "L3W5N0F8Q2J7"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "S4M0Q2N6F1J9", indexed_at: 1724134092000}]->(p);

MERGE (p3:Post {id: "M4X1P9L2J6K8"}) SET p3.content = "Open-source enables security auditing", p3.kind = "short", p3.indexed_at = 1693822934567;
MATCH (u3:User {id: $detroit}), (p3:Post {id: "M4X1P9L2J6K8"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "T5X8L1P3J4N2", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "M4X1P9L2J6K8"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "U6Q7F0M2X1J8", indexed_at: 1724134092000}]->(p);

MERGE (p3:Post {id: "N7Q2F5W8J0L3"}) SET p3.content = "Open-source drives innovation forward", p3.kind = "short", p3.indexed_at = 1693822331234, p3.attachments = ["pubky://f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o/pub/pubky.app/files/2ZK3A1B2C3D40", "pubky://f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o/pub/pubky.app/files/2ZK3E5F6G7H80"];
MATCH (u3:User {id: $cairo}), (p3:Post {id: "N7Q2F5W8J0L3"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "V8N1P3L9J4X0", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $bogota}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "J4N8Q1W2F6J3", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "V8N1P3L9J45R", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $opensource_tag, id: "J4N8Q1W2F6K8", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "V8N1P3L9J421", indexed_at: 1724134095000}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "N7Q2F5W8J0L3"}) MERGE (u)-[:TAGGED {label: $encryption_tag, id: "J4N8Q1W2F6LJ", indexed_at: 1724134077000}]->(p);

// ##################################
// ##### Posts related replies ######
// ##################################
MERGE (p1:Post {id: "1A1P4D8C9K0F"}) SET p1.content = "whop whop", p1.kind = "short", p1.indexed_at = 1719477230000;
MATCH (amst:User {id: $amsterdam}), (p1:Post {id: "1A1P4D8C9K0F"}) MERGE (amst)-[:AUTHORED]->(p1);

MERGE (p2:Post {id: "2B9XKZG3T4L6"}) SET p2.content = "we did it!", p2.kind = "short", p2.indexed_at = 1719477230015;
MATCH (bog:User {id: $bogota}), (p2:Post {id: "2B9XKZG3T4L6"}) MERGE (bog)-[:AUTHORED]->(p2);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p2:Post {id: "2B9XKZG3T4L6"}) MERGE (p2)-[:REPLIED]->(p1);

MERGE (p3:Post {id: "3M6WQ8F5P9R2"}) SET p3.content = "keep building", p3.kind = "short", p3.indexed_at = 1719477230019;
MATCH (cai:User {id: $cairo}), (p3:Post {id: "3M6WQ8F5P9R2"}) MERGE (cai)-[:AUTHORED]->(p3);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p3:Post {id: "3M6WQ8F5P9R2"}) MERGE (p3)-[:REPLIED]->(p1);

MERGE (p4:Post {id: "4T7ZV0C8K5B1"}) SET p4.content = "yiaaaah", p4.kind = "short", p4.indexed_at = 1719477230030;
MATCH (det:User {id: $detroit}), (p4:Post {id: "4T7ZV0C8K5B1"}) MERGE (det)-[:AUTHORED]->(p4);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p4:Post {id: "4T7ZV0C8K5B1"}) MERGE (p4)-[:REPLIED]->(p1);

MERGE (p5:Post {id: "5F8YQJ1L2D3E"}) SET p5.content = "finally", p5.kind = "short", p5.indexed_at = 1719477230088;
MATCH (eix:User {id: $eixample}), (p5:Post {id: "5F8YQJ1L2D3E"}) MERGE (eix)-[:AUTHORED]->(p5);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p5:Post {id: "5F8YQJ1L2D3E"}) MERGE (p5)-[:REPLIED]->(p1);

MERGE (p6:Post {id: "6G3ZB9X0H7M4"}) SET p6.content = "we enjoy the path", p6.kind = "short", p6.indexed_at = 1719477230101;
MATCH (eix:User {id: $eixample}), (p6:Post {id: "6G3ZB9X0H7M4"}) MERGE (eix)-[:AUTHORED]->(p6);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p6:Post {id: "6G3ZB9X0H7M4"}) MERGE (p6)-[:REPLIED]->(p1);

MERGE (p7:Post {id: "7N8K0Y1C3T2Q"}) SET p7.content = "what is next?", p7.kind = "short", p7.indexed_at = 1719477230208;
MATCH (cai:User {id: $cairo}), (p7:Post {id: "7N8K0Y1C3T2Q"}) MERGE (cai)-[:AUTHORED]->(p7);
MATCH (p1:Post {id: "1A1P4D8C9K0F"}), (p7:Post {id: "7N8K0Y1C3T2Q"}) MERGE (p7)-[:REPLIED]->(p1);


// ##################################
// ########## Posts kinds ###########
// ##################################
// Long posts
MERGE (p:Post {id: "4ZCW1TGL5BKG1"}) SET p.content = "Long post, article A", p.kind = "long", p.indexed_at = 1819477230300;
MATCH (u:User {id: $bogota}), (p:Post {id: "4ZCW1TGL5BKG1"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "4ZCW1TGL5BKG1"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7W9F4XRT", indexed_at: 1724934008000}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "4ZCW1TGL5BKG1"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7Z9F4XKO", indexed_at: 1724664008000}]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG2"}) SET p.content = "Long post, article B", p.kind = "long", p.indexed_at = 1819477230305;
MATCH (u:User {id: $cairo}), (p:Post {id: "4ZCW1TGL5BKG2"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG3"}) SET p.content = "Long post, article C", p.kind = "long", p.indexed_at = 1819477230310;
MATCH (u:User {id: $cairo}), (p:Post {id: "4ZCW1TGL5BKG3"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG4"}) SET p.content = "Long post, article D", p.kind = "long", p.indexed_at = 1819477230320;
MATCH (u:User {id: $bogota}), (p:Post {id: "4ZCW1TGL5BKG4"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG5"}) SET p.content = "Long post, article E", p.kind = "long", p.indexed_at = 1819477230330;
MATCH (u:User {id: $cairo}), (p:Post {id: "4ZCW1TGL5BKG5"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG6"}) SET p.content = "Long post, article F", p.kind = "long", p.indexed_at = 1819477230340;
MATCH (u:User {id: $bogota}), (p:Post {id: "4ZCW1TGL5BKG6"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG7"}) SET p.content = "Long post, article G", p.kind = "long", p.indexed_at = 1819477230350;
MATCH (u:User {id: $eixample}), (p:Post {id: "4ZCW1TGL5BKG7"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "4ZCW1TGL5BKG8"}) SET p.content = "Long post, article H", p.kind = "long", p.indexed_at = 1819477230360;
MATCH (u:User {id: $eixample}), (p:Post {id: "4ZCW1TGL5BKG8"}) MERGE (u)-[:AUTHORED]->(p);
// Image posts
MERGE (p:Post {id: "5YCW1TGL5BKG1"}) SET p.content = "IMAGE post, SVG A", p.kind = "image", p.indexed_at = 1820477299300;
MATCH (u:User {id: $bogota}), (p:Post {id: "5YCW1TGL5BKG1"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "5YCW1TGL5BKG1"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7W9F4DE3", indexed_at: 1724934072000}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "5YCW1TGL5BKG1"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7Z9F4KX0", indexed_at: 1724665008050}]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG2"}) SET p.content = "IMAGE post, SVG B", p.kind = "image", p.indexed_at = 1820477299305;
MATCH (u:User {id: $cairo}), (p:Post {id: "5YCW1TGL5BKG2"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG3"}) SET p.content = "IMAGE post, SVG C", p.kind = "image", p.indexed_at = 1820477299310;
MATCH (u:User {id: $cairo}), (p:Post {id: "5YCW1TGL5BKG3"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "5YCW1TGL5BKG3"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7W9F8KE3", indexed_at: 1724934005020}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "5YCW1TGL5BKG3"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7Z3P4KXC", indexed_at: 1724623009700}]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG4"}) SET p.content = "IMAGE post, SVG D", p.kind = "image", p.indexed_at = 1820477299320;
MATCH (u:User {id: $bogota}), (p:Post {id: "5YCW1TGL5BKG4"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG5"}) SET p.content = "IMAGE post, SVG E", p.kind = "image", p.indexed_at = 1820477299330;
MATCH (u:User {id: $cairo}), (p:Post {id: "5YCW1TGL5BKG5"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG6"}) SET p.content = "IMAGE post, SVG F", p.kind = "image", p.indexed_at = 1820477299340;
MATCH (u:User {id: $bogota}), (p:Post {id: "5YCW1TGL5BKG6"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG7"}) SET p.content = "IMAGE post, SVG G", p.kind = "image", p.indexed_at = 1820477299350;
MATCH (u:User {id: $eixample}), (p:Post {id: "5YCW1TGL5BKG7"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "5YCW1TGL5BKG8"}) SET p.content = "IMAGE post, SVG H", p.kind = "image", p.indexed_at = 1820477299360;
MATCH (u:User {id: $eixample}), (p:Post {id: "5YCW1TGL5BKG8"}) MERGE (u)-[:AUTHORED]->(p);
// Video posts
MERGE (p:Post {id: "MLOW1TGL5BKH1"}) SET p.content = "VIDEO post, mkv A", p.kind = "video", p.indexed_at = 1980477299300;
MATCH (u:User {id: $bogota}), (p:Post {id: "MLOW1TGL5BKH1"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "MLOW1TGL5BKH1"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7W9333E3", indexed_at: 1724934125485}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "MLOW1TGL5BKH1"}) MERGE (u)-[:TAGGED {label: $Fk_tag, id: "K2N7Z3SQKXT", indexed_at: 1724623009842}]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH2"}) SET p.content = "VIDEO post, mkv B", p.kind = "video", p.indexed_at = 1980477299305;
MATCH (u:User {id: $cairo}), (p:Post {id: "MLOW1TGL5BKH2"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH3"}) SET p.content = "VIDEO post, mkv C", p.kind = "video", p.indexed_at = 1980477299310;
MATCH (u:User {id: $cairo}), (p:Post {id: "MLOW1TGL5BKH3"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH4"}) SET p.content = "VIDEO post, mkv D", p.kind = "video", p.indexed_at = 1980477299320;
MATCH (u:User {id: $bogota}), (p:Post {id: "MLOW1TGL5BKH4"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH5"}) SET p.content = "VIDEO post, mkv E", p.kind = "video", p.indexed_at = 1980477299330;
MATCH (u:User {id: $cairo}), (p:Post {id: "MLOW1TGL5BKH5"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH6"}) SET p.content = "VIDEO post, mkv F", p.kind = "video", p.indexed_at = 1980477299340;
MATCH (u:User {id: $bogota}), (p:Post {id: "MLOW1TGL5BKH6"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH7"}) SET p.content = "VIDEO post, mkv G", p.kind = "video", p.indexed_at = 1980477299350;
MATCH (u:User {id: $eixample}), (p:Post {id: "MLOW1TGL5BKH7"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "MLOW1TGL5BKH7"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7W933JI3", indexed_at: 1724934965485}]->(p);
MATCH (u:User {id: $eixample}), (p:Post {id: "MLOW1TGL5BKH7"}) MERGE (u)-[:TAGGED {label: $Fk_tag, id: "K2N7ZEPMKXT", indexed_at: 1724642209842}]->(p);
MERGE (p:Post {id: "MLOW1TGL5BKH8"}) SET p.content = "VIDEO post, mkv H", p.kind = "video", p.indexed_at = 1980477299360;
MATCH (u:User {id: $eixample}), (p:Post {id: "MLOW1TGL5BKH8"}) MERGE (u)-[:AUTHORED]->(p);
// File posts
MERGE (p:Post {id: "GJMW1TGL5BKG1"}) SET p.content = "FILE post, pdf A", p.kind = "file", p.indexed_at = 1980477299302;
MATCH (u:User {id: $bogota}), (p:Post {id: "GJMW1TGL5BKG1"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG2"}) SET p.content = "FILE post, pdf B", p.kind = "file", p.indexed_at = 1980477299307;
MATCH (u:User {id: $cairo}), (p:Post {id: "GJMW1TGL5BKG2"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG3"}) SET p.content = "FILE post, pdf C", p.kind = "file", p.indexed_at = 1980477299312;
MATCH (u:User {id: $cairo}), (p:Post {id: "GJMW1TGL5BKG3"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG4"}) SET p.content = "FILE post, pdf D", p.kind = "file", p.indexed_at = 1980477299322;
MATCH (u:User {id: $bogota}), (p:Post {id: "GJMW1TGL5BKG4"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG5"}) SET p.content = "FILE post, pdf E", p.kind = "file", p.indexed_at = 1980477299332;
MATCH (u:User {id: $cairo}), (p:Post {id: "GJMW1TGL5BKG5"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "GJMW1TGL5BKG5"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7WKK8KEP", indexed_at: 1724934267420}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "GJMW1TGL5BKG5"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7Z3ZZKXT", indexed_at: 1724626114700}]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG6"}) SET p.content = "FILE post, pdf F", p.kind = "file", p.indexed_at = 1980477299342;
MATCH (u:User {id: $bogota}), (p:Post {id: "GJMW1TGL5BKG6"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG7"}) SET p.content = "FILE post, pdf G", p.kind = "file", p.indexed_at = 1980477299352;
MATCH (u:User {id: $eixample}), (p:Post {id: "GJMW1TGL5BKG7"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "GJMW1TGL5BKG8"}) SET p.content = "FILE post, pdf H", p.kind = "file", p.indexed_at = 1980477299362;
MATCH (u:User {id: $eixample}), (p:Post {id: "GJMW1TGL5BKG8"}) MERGE (u)-[:AUTHORED]->(p);
// Link posts
MERGE (p:Post {id: "SIJW1TGL5BKG1"}) SET p.content = "LINK post, pubky A", p.kind = "link", p.indexed_at = 1980477299304;
MATCH (u:User {id: $bogota}), (p:Post {id: "SIJW1TGL5BKG1"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG2"}) SET p.content = "LINK post, pubky B", p.kind = "link", p.indexed_at = 1980477299309;
MATCH (u:User {id: $cairo}), (p:Post {id: "SIJW1TGL5BKG2"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG3"}) SET p.content = "LINK post, pubky C", p.kind = "link", p.indexed_at = 1980477299318;
MATCH (u:User {id: $cairo}), (p:Post {id: "SIJW1TGL5BKG3"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG4"}) SET p.content = "LINK post, pubky D", p.kind = "link", p.indexed_at = 1980477299328;
MATCH (u:User {id: $bogota}), (p:Post {id: "SIJW1TGL5BKG4"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $detroit}), (p:Post {id: "SIJW1TGL5BKG4"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7WKKSWDR", indexed_at: 1724935214520}]->(p);
MATCH (u:User {id: $amsterdam}), (p:Post {id: "SIJW1TGL5BKG4"}) MERGE (u)-[:TAGGED {label: $pubky_tag, id: "K2N7Z3ZZKOO", indexed_at: 1724626669700}]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG5"}) SET p.content = "LINK post, pubky E", p.kind = "link", p.indexed_at = 1980477299338;
MATCH (u:User {id: $cairo}), (p:Post {id: "SIJW1TGL5BKG5"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG6"}) SET p.content = "LINK post, pubky F", p.kind = "link", p.indexed_at = 1980477299348;
MATCH (u:User {id: $bogota}), (p:Post {id: "SIJW1TGL5BKG6"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG7"}) SET p.content = "LINK post, pubky G", p.kind = "link", p.indexed_at = 1980477299358;
MATCH (u:User {id: $eixample}), (p:Post {id: "SIJW1TGL5BKG7"}) MERGE (u)-[:AUTHORED]->(p);
MERGE (p:Post {id: "SIJW1TGL5BKG8"}) SET p.content = "LINK post, pubky H", p.kind = "link", p.indexed_at = 1980477299368;
MATCH (u:User {id: $eixample}), (p:Post {id: "SIJW1TGL5BKG8"}) MERGE (u)-[:AUTHORED]->(p);
// It has a reply
MERGE (p:Post {id: "SIJW1TGL5BKG9"}) SET p.content = "LINK post, pubky H", p.kind = "link", p.indexed_at = 1980477299378;
MATCH (u:User {id: $eixample}), (p:Post {id: "SIJW1TGL5BKG9"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (reply:Post {id: "SIJW1TGL5BKG9" }), (parent:Post {id: "SIJW1TGL5BKG8" }) MERGE (reply)-[:REPLIED]->(parent)