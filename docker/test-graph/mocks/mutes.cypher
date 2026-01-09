// Set up some parameters
:param alice => 'db6w580d5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy';
:param bob => '58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qag0';
:param charlie => '5f4e800ogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to';
:param dave => 'rz6oe4yda9em9b4m7ymt8gym3r9g5gfa51su3rgdj9oszyz787n5';

// Enable Neo4j Parallel Runtime before inserting data
CYPHER runtime=parallel

// ################################
// ##### Create Users #############
// ################################
MERGE (u:User {id: $alice}) SET u.name = "Alice", u.indexed_at = 1724134095000;
MERGE (u:User {id: $bob}) SET u.name = "Bob", u.indexed_at = 1724134095000;
MERGE (u:User {id: $charlie}) SET u.name = "Charlie", u.indexed_at = 1724134095000;
MERGE (u:User {id: $dave}) SET u.name = "Dave", u.indexed_at = 1724134095000;

// ########################################
// ##### Create MUTED relationships #######
// ########################################
MATCH (from:User {id: $alice}), (to:User {id: $bob})
MERGE (from)-[:MUTED {indexed_at: 1724534095000}]->(to);

MATCH (from:User {id: $alice}), (to:User {id: $charlie})
MERGE (from)-[:MUTED {indexed_at: 1724534100000}]->(to);

MATCH (from:User {id: $alice}), (to:User {id: $dave})
MERGE (from)-[:MUTED {indexed_at: 1724534110000}]->(to);

MATCH (from:User {id: $charlie}), (to:User {id: $alice})
MERGE (from)-[:MUTED {indexed_at: 1724534120000}]->(to);

MATCH (from:User {id: $dave}), (to:User {id: $bob})
MERGE (from)-[:MUTED {indexed_at: 1724534130000}]->(to);