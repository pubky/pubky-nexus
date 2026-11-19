// ##############################
// ##### Discovery stub #########
// ##############################
// A user only key discovery knows, as the ingestor writes them: named after
// their key, with no `uri`, as only a profile write stores one. The rankings
// and the user search leave stubs out, and trust.cypher leaves them unscored.
MERGE (u:User {id: "xph5h5cw3ycbaww3ewokeoq3w4fkk36qa5cmxjbm4onsbeqxi9my"}) SET u.name = "xph5h5cw3ycbaww3ewokeoq3w4fkk36qa5cmxjbm4onsbeqxi9my", u.links = "null", u.indexed_at = 1724134095000, u.deleted = false;
