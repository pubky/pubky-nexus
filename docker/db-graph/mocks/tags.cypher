// Set up some parameters
// This parameters are session-based
:param peter => 'db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy';
:param arst => '5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to';
:param nakamoto => '58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy';
:param Wobly => 'rz6oe4yda9em9b4m7ymttgym3r9g5gfa51su3rgdj9oszyz787ny';
:param anonymous => 'mwsnc3qzej8hks6motdeyj8ag7gzaf3ft5emcjzk9wn5erxg968y';

:param epictto => 'bbkdkhm97pytrb785rdpornkjpcxi331hpq446ckn6rhb4abiguy';
:param aurelio => 'c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o';
:param zenon => 'cjoodgkwaf1bwepoe8m6zsp8guobh5wdwmqqnk496jcd175jjwey';
:param senek => 'cuimec4ngawamq8wa6fjzki6boxmwqcm11x6g7ontufrjwgdaxqo';
:param crispo => 'eroud7pzna7aacy5ob6ziekmm3sjg3m8hkpafcdjnwbmxambzyuy';
:param patro => 'f9rxf5hu1isngupfe6ff741bh7uqjxjwokqc4u3eribzmi89bcxy';
:param zeus => 'frnx4hncm9a94cqbxoudfa6eo58b477d4wuab1zmexw4j9icwmqy';


:param pubky_tag => 'pubky';
:param hike_tag => 'hike';
:param cycle_tag => 'cycle';
:param dev_tag => 'privacy';
:param p2p => 'p2p';
:param pkarr => 'pkarr';
:param satoshi => 'satoshi';
:param now => 'now';
:param athens => 'athens';


//:param go_post_tag => '🔥';
:param go_post_tag => 'lg';
:param privacy_post_tag => 'privacy';
:param free_post_tag => 'free';
:param human_right_post_tag => 'humanright';
:param defend_post_tag => 'defend';

// ##############################
// ##### Create users ###########
// ##############################

MERGE (u:User {id: $peter}) SET u.name = "peter", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $arst}) SET u.name = "arst", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $nakamoto}) SET u.name = "nakamoto", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $Wobly}) SET u.name = "Wobly", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $anonymous}) SET u.name = "anonymous", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $epictto}) SET u.name = "epictto", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $aurelio}) SET u.name = "aurelio", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $zenon}) SET u.name = "zenon", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $senek}) SET u.name = "senek", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $crispo}) SET u.name = "crispo", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $patro}) SET u.name = "patro", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $zeus}) SET u.name = "zeus", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";


// ##############################
// ##### User related tags ######
// ##############################
MATCH (from:User {id: $peter}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "1ABC23DE45F67G", indexed_at: 1724534095000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "8KLMNO2PQR345S", indexed_at: 1724534130000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "7UVWXY8ZABCDE9", indexed_at: 1724534175000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "1AB2345CDEFG67", indexed_at: 1724534200000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "2HIJK345LMNOPQR", indexed_at: 1724534205000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $arst}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "3STUV456WXYZ678", indexed_at: 1724534210000}]->(to);
MATCH (from:User {id: $arst}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "7BCDE8FGH9IJ01", indexed_at: 1724534125000}]->(from);
MATCH (from:User {id: $arst}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "6LMNOP4QRST567", indexed_at: 1724534170000}]->(from);

MATCH (from:User {id: $anonymous}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "0DEFG3HIJKL45N", indexed_at: 1724534140000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "2HIJK3LM4NOPQR", indexed_at: 1724534100000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9QRST3UVWXY45Z", indexed_at: 1724534185000}]->(to);
MATCH (from:User {id: $anonymous}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "6TUVW789YZ01234", indexed_at: 1724534225000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "7CDEF890GHIJ123", indexed_at: 1724534230000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "8LMNOP901QRST45", indexed_at: 1724534235000}]->(to);
MATCH (from:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9UVWXY0123ZAB56", indexed_at: 1724534240000}]->(from);
MATCH (from:User {id: $Wobly}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "3STUV4WX5YZ678", indexed_at: 1724534105000}]->(from);

MATCH (from:User {id: $arst}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "4ABCD5EFG6H78I", indexed_at: 1724534110000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $p2p, id: "1OPQRS6TUVW89A", indexed_at: 1724534145000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "8FGHI0JKLMN12P", indexed_at: 1724534180000}]->(to);
MATCH (from:User {id: $arst}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "0BCDE1234FGHI67", indexed_at: 1724534245000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "1JKLM4567NOPQ89", indexed_at: 1724534250000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $pkarr, id: "2RSTU8901VWXY12", indexed_at: 1724534255000}]->(to);
MATCH (from:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "3ABCD2345EFGH34", indexed_at: 1724534260000}]->(from);
MATCH (from:User {id: $anonymous}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "0ABCDE6FGHI78J", indexed_at: 1724534190000}]->(from);

MATCH (from:User {id: $arst}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $pkarr, id: "2XYZ01ABCDE34G", indexed_at: 1724534150000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $p2p, id: "5JKLMN6OPQR78S", indexed_at: 1724534115000}]->(to);
MATCH (from:User {id: $anonymous}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "5BCDE9FGHIJK23", indexed_at: 1724534165000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $p2p, id: "9JKLM1NOPQR23ST", indexed_at: 1724534290000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $pkarr, id: "0UVWXY2ZABC34DE", indexed_at: 1724534295000}]->(to);
MATCH (from:User {id: $peter}), (to:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "1FGHIJ5KLMNOP67", indexed_at: 1724534300000}]->(to);
MATCH (from:User {id: $nakamoto}) MERGE (from)-[:TAGGED {label: $satoshi, id: "0ABCDE6FGHI78J", indexed_at: 1724534190000}]->(from);

MATCH (from:User {id: $anonymous}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pkarr, id: "6TUVWX7YZ0123A", indexed_at: 1724534120000}]->(to);
MATCH (from:User {id: $Wobly}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $hike_tag, id: "4QRST8UVWXY01Z", indexed_at: 1724534160000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $cycle_tag, id: "2QRSTU8VWXYZ90A", indexed_at: 1724534305000}]->(to);
MATCH (from:User {id: $nakamoto}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "3BCDEF1GHIJK23L", indexed_at: 1724534310000}]->(to);
MATCH (from:User {id: $arst}), (to:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "4MNOPQ5RSTUVW78", indexed_at: 1724534315000}]->(to);
MATCH (from:User {id: $peter}) MERGE (from)-[:TAGGED {label: $pubky_tag, id: "3FGHI5JKLM67OP", indexed_at: 1724534155000}]->(from);
MATCH (from:User {id: $peter}) MERGE (from)-[:TAGGED {label: $dev_tag, id: "9TUVW6XYZA78BC", indexed_at: 1724534135000}]->(from);

// ###############################
// ##### Posts related tags ######
// ###############################
MERGE (p:Post {id: "HC3T5CEPBPHQ"}) SET p.content = "Privacy is a Human Right", p.kind = "short", p.indexed_at = 1719308315917;
MATCH (u:User {id: $peter}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:AUTHORED]->(p);
MATCH (u:User {id: $arst}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $go_post_tag, id: "0RDVNJ0XR560", indexed_at: 1724544095000}]->(p);
MATCH (u:User {id: $nakamoto}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $go_post_tag, id: "S02JBP48173F", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $Wobly}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $go_post_tag, id: "1TDVFKFBB48G", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $anonymous}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $go_post_tag, id: "2VDW8YBDZJ02", indexed_at: 1724334095000}]->(p);
MATCH (u:User {id: $nakamoto}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "ERDW8ZSNPR4G", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $Wobly}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "FTDW8ZSQNL9Y", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $anonymous}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "GRDW8ZSRMPC1", indexed_at: 1724334095000}]->(p);
MATCH (u:User {id: $arst}), (p:Post {id: "HC3T5CEPBPHQ"}) MERGE (u)-[:TAGGED {label: $defend_post_tag, id: "HTDX9ZSTNQ47", indexed_at: 1724334095000}]->(p);


MERGE (p2:Post {id: "1TDV7XBCF4M1"}) SET p2.content = "Freedom of speech is essential for democracy", p2.kind = "short", p2.indexed_at = 1719308316921;
MATCH (u2:User {id: $anonymous}), (p2:Post {id: "1TDV7XBCF4M1"}) MERGE (u2)-[:AUTHORED]->(p2);
MATCH (u:User {id: $arst}), (p:Post {id: "1TDV7XBCF4M1"}) MERGE (u)-[:TAGGED {label:$privacy_post_tag, id: "1RDV7ZX9BX93", indexed_at: 1724544095000}]->(p);
MATCH (u:User {id: $nakamoto}), (p:Post {id: "1TDV7XBCF4M1"}) MERGE (u)-[:TAGGED {label: $privacy_post_tag, id: "2TDW8YKH56JN", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $Wobly}), (p:Post {id: "1TDV7XBCF4M1"}) MERGE (u)-[:TAGGED {label: $human_right_post_tag, id: "3VDX9ZM7C4P1", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $Wobly}), (p:Post {id: "1TDV7XBCF4M1"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "3VDX9ZM7C4P5", indexed_at: 1724134096000}]->(p);

MERGE (p3:Post {id: "2VDW8YBDZJ02"}) SET p3.content = "Decentralization is key to preserving freedom", p3.kind = "short", p3.indexed_at = 1719308318234;
MATCH (u3:User {id: $anonymous}), (p3:Post {id: "2VDW8YBDZJ02"}) MERGE (u3)-[:AUTHORED]->(p3);
MATCH (u:User {id: $nakamoto}), (p:Post {id: "2VDW8YBDZJ02"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "4SDW8YPKDL2M", indexed_at: 1724134080000}]->(p);
MATCH (u:User {id: $Wobly}), (p:Post {id: "2VDW8YBDZJ02"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "6TDW8ZMHJF29", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $peter}), (p:Post {id: "2VDW8YBDZJ02"}) MERGE (u)-[:TAGGED {label: $free_post_tag, id: "7VDW8ZRJKN3Y", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $peter}), (p:Post {id: "2VDW8YBDZJ02"}) MERGE (u)-[:TAGGED {label: $human_right_post_tag, id: "8SDX8YQMKP19", indexed_at: 1724134092000}]->(p);
MATCH (u:User {id: $peter}), (p:Post {id: "2VDW8YBDZJ02"}) MERGE (u)-[:BOOKMARKED {id: "2Z9PFGC3WWWT0", indexed_at: 1721764200000}]->(p);

// ###############################
// ##### WoT related tags ######
// ###############################
MATCH (u1:User {id: $epictto}), (u2:User {id: $zenon}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736154, id: "GR3K85JG5EST1"}]->(u2);
MATCH (u1:User {id: $zenon}), (u2:User {id: $senek}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736485, id: "GR3K85JG5EST2"}]->(u2);
MATCH (u1:User {id: $senek}), (u2:User {id: $crispo}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736785, id: "GR3K85JG5EST3"}]->(u2);
MATCH (u1:User {id: $crispo}), (u2:User {id: $patro}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736359, id: "GR3K85JG5EST4"}]->(u2);
MATCH (u1:User {id: $senek}), (u2:User {id: $epictto}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736999, id: "GR3K85JG5EST5"}]->(u2);
MATCH (u1:User {id: $senek}), (u2:User {id: $zeus}) MERGE (u1)-[:FOLLOWS {indexed_at: 1230475736990, id: "GR3K85JG5ET11"}]->(u2);

MATCH (from:User {id: $zenon}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $now, id: "GR3K85JG5EST6", indexed_at: 1224534095400}]->(to);
MATCH (from:User {id: $senek}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $now, id: "GR3K85JG5EST7", indexed_at: 1224534095300}]->(to);
MATCH (from:User {id: $crispo}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $now, id: "GR3K85JG5EST8", indexed_at: 1224534095200}]->(to);
MATCH (from:User {id: $patro}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $now, id: "GR3K85JG5EST9", indexed_at: 1224534095100}]->(to);
MATCH (from:User {id: $zeus}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $now, id: "GR3K85JG5ET10", indexed_at: 1224534095000}]->(to);
MATCH (from:User {id: $zenon}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $athens, id: "GR3K85JG5ET11", indexed_at: 1224534094400}]->(to);
MATCH (from:User {id: $senek}), (to:User {id: $aurelio}) MERGE (from)-[:TAGGED {label: $athens, id: "GR3K85JG5ET12", indexed_at: 1224534094300}]->(to);