// Global influencer timeframe fixture.
//
// The global `today` / `this_month` influencer streams rank users by
// engagement inside a now-relative window (last 24h / 30d). Fixed
// timestamps fall out of that window as the clock moves, so the five users
// these streams assert are anchored to load time instead: phh5aq, pcckx7,
// otn147 and omynbj are active ~1h ago (inside both windows); oh8ku6 is
// active ~10d ago (inside this_month, outside today). Each authors anchor
// posts whose AUTHORED edge carries the recent timestamp while the post node
// stays old and gets no engagement. The global influencer ranking keys on
// the AUTHORED timestamp, so it counts these posts; every post stream keys on
// the post timestamp (timelines) or its engagement score, so the posts sort
// below the recent / high-engagement entries those streams return and never
// surface. They are top-level posts that reply to nothing, so no existing
// post's engagement count changes either.
//
// Loaded after skunk.cypher: the followers and post authors all exist there.

// In-window followers (two existing users) give each ranked user a non-zero
// sqrt(followers) factor.
MATCH (u1:User {id: "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o"}), (u2:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000001"}]->(u2);
MATCH (u1:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}), (u2:User {id: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000002"}]->(u2);
MATCH (u1:User {id: "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o"}), (u2:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000003"}]->(u2);
MATCH (u1:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}), (u2:User {id: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000004"}]->(u2);
MATCH (u1:User {id: "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o"}), (u2:User {id: "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000005"}]->(u2);
MATCH (u1:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}), (u2:User {id: "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000006"}]->(u2);
MATCH (u1:User {id: "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o"}), (u2:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000007"}]->(u2);
MATCH (u1:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}), (u2:User {id: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 3600000, id: "1FXW000000008"}]->(u2);
MATCH (u1:User {id: "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o"}), (u2:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 864000000, id: "1FXW000000009"}]->(u2);
MATCH (u1:User {id: "1t5fyryjggt6kfha18jcstnz3wfyqh9fycra5sg8hr5qhmot5rky"}), (u2:User {id: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo"}) MERGE (u1)-[:FOLLOWS {indexed_at: timestamp() - 864000000, id: "1FXW000000010"}]->(u2);

// Recent authored (hidden reply) posts give each ranked user a non-zero
// (tags + posts) factor inside the window. The post stays old; only the
// AUTHORED edge is recent.
UNWIND [
  {pid: "1FXP000000001", a: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco", at: timestamp() - 3600000},
  {pid: "1FXP000000002", a: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco", at: timestamp() - 3600000},
  {pid: "1FXP000000003", a: "phh5aqdfwkmydr1d6b48xa3tcbiipy8wpcmougyed7otitx69kco", at: timestamp() - 3600000},
  {pid: "1FXP000000004", a: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy", at: timestamp() - 3600000},
  {pid: "1FXP000000005", a: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy", at: timestamp() - 3600000},
  {pid: "1FXP000000006", a: "pcckx7sercfy1u8rrr8cc4gkdnce93f6jarngcdsfu5enty51aiy", at: timestamp() - 3600000},
  {pid: "1FXP000000007", a: "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio", at: timestamp() - 3600000},
  {pid: "1FXP000000008", a: "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio", at: timestamp() - 3600000},
  {pid: "1FXP000000009", a: "otn147ixg3i4sorqupuzptnx9gtiku4y77i8fdo35m7yug1d8zio", at: timestamp() - 3600000},
  {pid: "1FXP000000010", a: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy", at: timestamp() - 3600000},
  {pid: "1FXP000000011", a: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy", at: timestamp() - 3600000},
  {pid: "1FXP000000012", a: "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy", at: timestamp() - 3600000},
  {pid: "1FXP000000013", a: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo", at: timestamp() - 864000000},
  {pid: "1FXP000000014", a: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo", at: timestamp() - 864000000},
  {pid: "1FXP000000015", a: "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo", at: timestamp() - 864000000}
] AS row
MATCH (au:User {id: row.a})
MERGE (p:Post {id: row.pid}) SET p.content = "influencer timeframe anchor", p.kind = "short", p.indexed_at = 1650000000000
MERGE (au)-[:AUTHORED {indexed_at: row.at}]->(p);
