// Generate user keys
:param user1 => 'pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo';

:param user2 => 'qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy';

:param user3 => 'r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo';

:param user4 => 'r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y';

:param user5 => 'tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo';

:param post1 => '0032GZQ335NEJ';

:param post2 => '0032GZQ338BMP';

:param tag1 => 'tag1';

:param tag2 => 'tag2';

:param tag3 => 'tag3';

:param day => 24 * 60 * 60 * 1000;

:param month => 30 * 24 * 60 * 60 * 1000;

// Create the posts
MERGE (p:Post { id: $post1 }) SET p.content = "This is a test post", p.kind = "short", p.indexed_at = 1733380839000;

MERGE (p:Post { id: $post2 }) SET p.content = "This is a second test post", p.kind = "short", p.indexed_at = 1733380849000;

// athors
MATCH (u:User { id: $user1 }), (p:Post {id: $post1})
MERGE (u)-[:AUTHORED]->(p);

MATCH (u:User { id: $user2 }), (p:Post {id: $post2})
MERGE (u)-[:AUTHORED]->(p);

// Today
WITH datetime().epochMillis AS today_millis
MATCH (u:User { id: $user2 }), (p:Post {id: $post1})
MERGE (u)-[:TAGGED { label:$tag1, id: "0032GZRVBRB30", indexed_at: today_millis }]->(p);

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
MERGE (u)-[:TAGGED { label:$tag1, id: "0032GZR6TQTSG", indexed_at: this_month_millis }]->(p);

WITH datetime({ year: datetime().year, month: datetime().month, day: 1 }).epochMillis AS this_month_millis
MATCH (u:User { id: $user1 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label:$tag2, id: "0032GZRVBRC3M", indexed_at: this_month_millis }]->(p);

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
MERGE (u)-[:TAGGED { label:$tag3, id: "0032GZRG46YPY", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user3 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag2, id: "0032GZRG46ZH0", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user4 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRG46ZH6", indexed_at: all_time_millis }]->(p);

WITH (datetime({ year: datetime().year, month: datetime().month, day: 1 }) - Duration({days: 1})).epochMillis AS all_time_millis
MATCH (u:User { id: $user5 }), (p:Post {id: $post2})
MERGE (u)-[:TAGGED { label: $tag1, id: "0032GZRG46ZH6", indexed_at: all_time_millis }]->(p);
