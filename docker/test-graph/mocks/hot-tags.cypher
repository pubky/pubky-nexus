// Generate user keys
:param user1 => 'pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo';
:param user2 => 'qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy';
:param user3 => 'r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo';
:param user4 => 'r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y';
:param user5 => 'tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo';
:param user6 => '6xejaazm58f5dca3aj6o4is3k55wxy86hyxtd1pu5h897cfq76yy';

:param post1 => '0032GZQ335NEJ';
:param post2 => '0032GZQ338BMP';
:param post3 => '0032BZ0T19R70';
:param post4 => '0032BZ3YFDG2G';
:param post5 => '0032DR2GJYAAG';

:param post6a => '0032H5K9X31W5'
:param post6b => '0032MV7YTGK38'
:param post6c => '0032QX1N7RD2F'
:param post6d => '0032JK8WTTVJ9'
:param post6e => '0032C2M8B9Y6R'

:param tag1 => 'tag1';
:param tag2 => 'tag2';
:param tag3 => 'tag3';
:param today => 'today';
:param month => 'month';
:param all => 'all';

// Create users
MERGE (u:User {id: $user1}) SET u.name = "HotTag:User1", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $user2}) SET u.name = "HotTag:User2", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $user3}) SET u.name = "HotTag:User3", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $user4}) SET u.name = "HotTag:User4", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $user5}) SET u.name = "HotTag:User5", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";
MERGE (u:User {id: $user6}) SET u.name = "Active:User6:Post", u.bio = "", u.status = "undefined", u.indexed_at = 1724134095000 , u.links = "[{\"url\":\"\",\"title\":\"website\"},{\"url\":\"\",\"title\":\"email\"},{\"url\":\"\",\"title\":\"x\"},{\"url\":\"\",\"title\":\"telegram\"}]";

// Create the posts
MERGE (p:Post { id: $post1 }) SET p.content = "This is a test post", p.kind = "short", p.indexed_at = 1733380839000;
MATCH (u:User { id: $user1 }), (p:Post {id: $post1})
MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post { id: $post2 }) SET p.content = "This is a second test post", p.kind = "short", p.indexed_at = 1733380849000;
MATCH (u:User { id: $user2 }), (p:Post {id: $post2})
MERGE (u)-[:AUTHORED]->(p);

// Today
WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRVBRB30", indexed_at: today_millis }]->(p);

WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRVBRAR8", indexed_at: today_millis }]->(p);

WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRSZTRA8", indexed_at: today_millis }]->(p);

WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label: $tag2, id: "0032GZRSZTV00", indexed_at: today_millis }]->(p);

WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label: $tag2, id: "0032GZS4DWEGM", indexed_at: today_millis }]->(p);

// This month
WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZR6TQTSG", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag2, id: "0032GZRVBRC3M", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag3, id: "0032GZR6TQTT6", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag3, id: "0032GZR6TQV4P", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag3, id: "0032GZS4DWF62", indexed_at: this_month_millis }]->(p);

// All time (outside this month period)
WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag3, id: "0032GZRG46YPY", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag2, id: "0032GZRG46ZH0", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRG46ZH6", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRG46ZH6", indexed_at: all_time_millis }]->(p);

// ###################################################
// ##### Lets start connecting all the users #########
// ###################################################
// u1 user folling users
MATCH (u1:User {id: $user1}), (u2:User {id: $user2}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441131100, id: "GR3K85JG5EZK1"}]->(u2);
MATCH (u1:User {id: $user1}), (u2:User {id: $user3}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441141101, id: "GR3K85JG5EZK2"}]->(u2);
MATCH (u1:User {id: $user1}), (u2:User {id: $user4}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441151102, id: "GR3K85JG5EZK3"}]->(u2);
MATCH (u1:User {id: $user1}), (u2:User {id: $user5}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441161103, id: "GR3K85JG5EZK4"}]->(u2);
MATCH (u1:User {id: $user1}), (u2:User {id: $user6}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441161103, id: "GR3K88JG5EZK5"}]->(u2);

// u4 user followers
MATCH (u1:User {id: $user2}), (u2:User {id: $user4}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441171104, id: "GR3K85JG5EZK5"}]->(u2);
MATCH (u1:User {id: $user3}), (u2:User {id: $user4}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441181105, id: "GR3K85JG5EZK6"}]->(u2);
MATCH (u1:User {id: $user5}), (u2:User {id: $user4}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441191106, id: "GR3K85JG5EZK7"}]->(u2);

// u1 and u5 are friends
MATCH (u1:User {id: $user5}), (u2:User {id: $user1}) MERGE (u1)-[:FOLLOWS {indexed_at: 1737441201107, id: "GR3K85JG5EZK8"}]->(u2);

// publish more posts
MERGE (p:Post { id: $post3 }) SET p.content = "Tag me!", p.kind = "short", p.indexed_at = 1737441201107108;
MATCH (u:User { id: $user1 }), (p:Post {id: $post3})
MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post { id: $post4 }) SET p.content = "Who wants to follow me", p.kind = "short", p.indexed_at = 1737441201104109;
MATCH (u:User { id: $user1 }), (p:Post {id: $post4})
MERGE (u)-[:AUTHORED]->(p);

MERGE (p:Post { id: $post5 }) SET p.content = "Hello friends!?!?", p.kind = "short", p.indexed_at = 1737441201105109;
MATCH (u:User { id: $user5 }), (p:Post {id: $post5})
MERGE (u)-[:AUTHORED]->(p);

// ** u6 user creates lot of post in that month
WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MERGE (p:Post { id: $post6a }) SET p.content = "Post A", p.kind = "short", p.indexed_at = this_month_millis;
MATCH (u:User { id: $user6 }), (p:Post {id: $post6a})
MERGE (u)-[:AUTHORED]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MERGE (p:Post { id: $post6b }) SET p.content = "Post A", p.kind = "short", p.indexed_at = this_month_millis;
MATCH (u:User { id: $user6 }), (p:Post {id: $post6b})
MERGE (u)-[:AUTHORED]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MERGE (p:Post { id: $post6c }) SET p.content = "Post A", p.kind = "short", p.indexed_at = this_month_millis;
MATCH (u:User { id: $user6 }), (p:Post {id: $post6c})
MERGE (u)-[:AUTHORED]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MERGE (p:Post { id: $post6d }) SET p.content = "Post A", p.kind = "short", p.indexed_at = this_month_millis;
MATCH (u:User { id: $user6 }), (p:Post {id: $post6d})
MERGE (u)-[:AUTHORED]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MERGE (p:Post { id: $post6e }) SET p.content = "Post A", p.kind = "short", p.indexed_at = this_month_millis;
MATCH (u:User { id: $user6 }), (p:Post {id: $post6e})
MERGE (u)-[:AUTHORED]->(p);

// #### Add tags to Post 3. reach=following&timeframe=x ####
// Today timeframe
WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $today, id: "2B6X94BZQVS9QQCMF69JX92H3C", indexed_at: today_millis }]->(p);

WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $today, id: "2B6X94BZQVS9QQCMF69JX92H3D", indexed_at: today_millis }]->(p);

// Month tags timeframe
WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $month, id: "2B6X94BZQVS9QQCMF69JX92H3H", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $month, id: "2B6X94BZQVS9QQCMF69JX92H3I", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $month, id: "2B6X94BZQVS9QQCMF69JX92H3J", indexed_at: this_month_millis }]->(p);

// All tags timeframe
WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $all, id: "2B6X94BZQVS9QQCMF69JX92H3L", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $all, id: "2B6X94BZQVS9QQCMF69JX92H3M", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $all, id: "2B6X94BZQVS9QQCMF69JX92H3N", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post3})
MERGE (u)-[:TAGGED { label: $all, id: "2B6X94BZQVS9QQCMF69JX92H3O", indexed_at: all_time_millis }]->(p);

// #### Add tags to Post 4. reach=followers&timeframe=x ####
// Today timeframe
WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $today, id: "53EYM1KZ1JGCHFXWG8RP0J67K0", indexed_at: today_millis }]->(p);

// Month timeframe
WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $month, id: "53EYM1KZ1JGCHFXWG8RP0J67K3", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $month, id: "53EYM1KZ1JGCHFXWG8RP0J67K4", indexed_at: this_month_millis }]->(p);

// All timeframe
WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $all, id: "53EYM1KZ1JGCHFXWG8RP0J67K6", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $all, id: "53EYM1KZ1JGCHFXWG8RP0J67K7", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post4})
MERGE (u)-[:TAGGED { label: $all, id: "53EYM1KZ1JGCHFXWG8RP0J67K8", indexed_at: all_time_millis }]->(p);

// #### Add tags to Post 5. reach=friends&timeframe=x ####
// Today timeframe
WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post5})
MERGE (u)-[:TAGGED { label: $today, id: "53EYM1KZ1JGCHFXWG8RP0J67J0", indexed_at: today_millis }]->(p);

// Month timeframe
WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post5})
MERGE (u)-[:TAGGED { label: $month, id: "53EYM1KZ1JGCHFXWG8RP0J67J2", indexed_at: this_month_millis }]->(p)

// All timeframe
WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post5})
MERGE (u)-[:TAGGED { label: $all, id: "53EYM1KZ1JGCHFXWG8RP0J67J5", indexed_at: all_time_millis }]->(p);