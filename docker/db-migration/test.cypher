// Set up some parameters
// This parameters are session-based
:param peter => 'db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy';
:param arst => '5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to';
:param nakamoto => '58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy';
:param Wobly => 'rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny';
:param anonymous => 'mwsnc3qzej8hks6motdeyj8ag7gzaf3ft5emcjzk9wn5erxg968y';

:param pubky_tag => 'pubky';
:param hike_tag => 'hike';
:param cycle_tag => 'cycle';
:param dev_tag => 'privacy';
:param p2p => 'p2p';
:param pkarr => 'pkarr';
:param satoshi => 'satoshi';

// ##############################
// ##### User related tags ######
// ##############################
MATCH (from:User {id: $peter}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "1ABC23DE45F67G", indexed_at: 1724534095}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "8KLMNO2PQR345S", indexed_at: 1724534130}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "7UVWXY8ZABCDE9", indexed_at: 1724534175}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "1AB2345CDEFG67", indexed_at: 1724534200}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "2HIJK345LMNOPQR", indexed_at: 1724534205}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "3STUV456WXYZ678", indexed_at: 1724534210}]->(to);
MATCH (from:User {id: $arst}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "7BCDE8FGH9IJ01", indexed_at: 1724534125}]->(from);
MATCH (from:User {id: $arst}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "6LMNOP4QRST567", indexed_at: 1724534170}]->(from);

MATCH (from:User {id: $anonymous}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "0DEFG3HIJKL45N", indexed_at: 1724534140}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "2HIJK3LM4NOPQR", indexed_at: 1724534100}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9QRST3UVWXY45Z", indexed_at: 1724534185}]->(to);MATCH (from:User {id: $anonymous}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "6TUVW789YZ01234", indexed_at: 1724534225}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "7CDEF890GHIJ123", indexed_at: 1724534230}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "8LMNOP901QRST45", indexed_at: 1724534235}]->(to);
MATCH (from:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9UVWXY0123ZAB56", indexed_at: 1724534240}]->(from);
MATCH (from:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "3STUV4WX5YZ678", indexed_at: 1724534105}]->(from);

MATCH (from:User {id: $arst}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "4ABCD5EFG6H78I", indexed_at: 1724534110}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $p2p, id: "1OPQRS6TUVW89A", indexed_at: 1724534145}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "8FGHI0JKLMN12P", indexed_at: 1724534180}]->(to);
MATCH (from:User {id: $arst}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "0BCDE1234FGHI67", indexed_at: 1724534245}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "1JKLM4567NOPQ89", indexed_at: 1724534250}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $pkarr, id: "2RSTU8901VWXY12", indexed_at: 1724534255}]->(to);
MATCH (from:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "3ABCD2345EFGH34", indexed_at: 1724534260}]->(from);
MATCH (from:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "0ABCDE6FGHI78J", indexed_at: 1724534190}]->(from);

MATCH (from:User {id: $arst}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $pkarr, id: "2XYZ01ABCDE34G", indexed_at: 1724534150}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $p2p, id: "5JKLMN6OPQR78S", indexed_at: 1724534115}]->(to);
MATCH (from:User {id: $anonymous}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "5BCDE9FGHIJK23", indexed_at: 1724534165}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $p2p, id: "9JKLM1NOPQR23ST", indexed_at: 1724534290}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $pkarr, id: "0UVWXY2ZABC34DE", indexed_at: 1724534295}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "1FGHIJ5KLMNOP67", indexed_at: 1724534300}]->(to);
MATCH (from:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $satoshi, id: "0ABCDE6FGHI78J", indexed_at: 1724534190}]->(from);

MATCH (from:User {id: $anonymous}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pkarr, id: "6TUVWX7YZ0123A", indexed_at: 1724534120}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "4QRST8UVWXY01Z", indexed_at: 1724534160}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "2QRSTU8VWXYZ90A", indexed_at: 1724534305}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "3BCDEF1GHIJK23L", indexed_at: 1724534310}]->(to);
MATCH (from:User {id: $arst}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "4MNOPQ5RSTUVW78", indexed_at: 1724534315}]->(to);
MATCH (from:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "3FGHI5JKLM67OP", indexed_at: 1724534155}]->(from);
MATCH (from:User {id: $peter}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9TUVW6XYZA78BC", indexed_at: 1724534135}]->(from);
