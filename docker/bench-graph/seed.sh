#!/usr/bin/env bash
#
# Seed a synthetic graph for the query performance audit.
#
# Everything created here carries an extra :Bench label and an `audit_*` id
# prefix, so cleanup is a single `MATCH (n:Bench) DETACH DELETE n` and the
# audit never touches real data. Idempotent: re-running re-seeds from scratch.
#
# Usage:
#   ./seed.sh small     # quick, ~10x smaller than large
#   ./seed.sh large     # whale-scale, the dimension that exposes O(n*m)
#   ./seed.sh clean      # remove all :Bench data and exit
#
# Override any single dimension via env, e.g.  FOLLOWERS=5000 ./seed.sh large
#
# Deterministic entity ids the dumper (examples/graph_audit) references:
#   audit_subject            the user under test (the "whale")
#   audit_post_1             the subject's hottest post (many tags/replies/reposts)
#   audit_viewer             a viewer with a depth-2 trust path to the subject
#   audit_resource           a tagged resource
#   audit_label              the tag label used across the reach network
set -euo pipefail

SCALE="${1:-small}"
CYPHER() { docker exec -i neo4j cypher-shell -u neo4j -p "${NEO4J_PASSWORD:-12345678}" --format plain "$@"; }

clean() { CYPHER >/dev/null <<'EOF'
MATCH (n:Bench) DETACH DELETE n;
EOF
}

if [[ "$SCALE" == "clean" ]]; then echo "cleaning :Bench data..."; clean; echo "done."; exit 0; fi

# --- dimensions (large = ~10x small; the ratio makes growth factors readable) ---
if [[ "$SCALE" == "large" ]]; then M=10; else M=1; fi
: "${POSTS:=$((200 * M))}"          # posts authored by the subject
: "${FOLLOWERS:=$((200 * M))}"      # users following the subject
: "${FOLLOWING:=$((100 * M))}"      # users the subject follows (half follow back = friends)
: "${RECV_TAGS:=$((200 * M))}"      # tags the subject receives (the #935 row multiplier)
: "${HOT_TAGS:=$((50 * M))}"        # tags/replies/reposts on audit_post_1
: "${ACTIVE:=$((30 * M))}"          # followers who author+tag (feed the reach/hot-tag/influencer queries)
LABELS=20                            # distinct received-tag labels
TS=1735689600000                     # fixed indexed_at (Jan 2025), inside any AllTime window

echo "seeding scale=$SCALE  posts=$POSTS followers=$FOLLOWERS following=$FOLLOWING recv_tags=$RECV_TAGS active=$ACTIVE"
clean

CYPHER >/dev/null <<EOF
// subject + authored posts (audit_post_1 is the designated hot post)
CREATE (s:User:Bench {id:'audit_subject', name:'audit_subject', indexed_at:$TS});
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$POSTS) AS i
CREATE (s)-[:AUTHORED {indexed_at:$TS}]->(p:Post:Bench {id:'audit_post_'+toString(i), indexed_at:$TS, content:'c'+toString(i), kind: CASE WHEN i % 50 = 0 THEN 'collection' END});

// hot post: many received tags (distinct labels), replies and reposts from fresh users
MATCH (hp:Post:Bench {id:'audit_post_1'})
UNWIND range(1,$HOT_TAGS) AS i
CREATE (u:User:Bench {id:'audit_hp_tagger_'+toString(i), name:'n', indexed_at:$TS})-[:TAGGED {id:'audit_hpt_'+toString(i), label:'lbl_'+toString(i % $LABELS), indexed_at:$TS}]->(hp)
CREATE (u)-[:AUTHORED {indexed_at:$TS}]->(rp:Post:Bench {id:'audit_reply_'+toString(i), indexed_at:$TS, content:'r'})-[:REPLIED]->(hp)
CREATE (u)-[:AUTHORED {indexed_at:$TS}]->(rt:Post:Bench {id:'audit_repost_'+toString(i), indexed_at:$TS, content:'x'})-[:REPOSTED]->(hp);

// subject receives RECV_TAGS user-tags (the #935 axis), LABELS distinct, from fresh taggers
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$RECV_TAGS) AS i
CREATE (t:User:Bench {id:'audit_rtagger_'+toString(i), name:'n', indexed_at:$TS})-[:TAGGED {id:'audit_rt_'+toString(i), label:'rl_'+toString(i % $LABELS), indexed_at:$TS}]->(s);

// subject follows FOLLOWING users; first half follow back (friends)
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$FOLLOWING) AS i
CREATE (s)-[:FOLLOWS {indexed_at:$TS}]->(f:User:Bench {id:'audit_followee_'+toString(i), name:'n', indexed_at:$TS});
MATCH (s:User:Bench {id:'audit_subject'})-[:FOLLOWS]->(f:User:Bench)
WHERE toInteger(split(f.id,'_')[2]) <= $FOLLOWING/2
CREATE (f)-[:FOLLOWS {indexed_at:$TS}]->(s);

// FOLLOWERS follow the subject; ACTIVE of them author a post and tag it with audit_label
// (so reach=Followers queries: taggers-by-reach / hot-tags / influencers have data)
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$FOLLOWERS) AS i
CREATE (fr:User:Bench {id:'audit_follower_'+toString(i), name:'n', indexed_at:$TS})-[:FOLLOWS {indexed_at:$TS}]->(s);
MATCH (fr:User:Bench)-[:FOLLOWS]->(s:User:Bench {id:'audit_subject'})
WHERE fr.id STARTS WITH 'audit_follower_' AND toInteger(split(fr.id,'_')[2]) <= $ACTIVE
CREATE (fr)-[:AUTHORED {indexed_at:$TS}]->(ap:Post:Bench {id:'audit_apost_'+split(fr.id,'_')[2], indexed_at:$TS, content:'a'})
CREATE (fr)-[:TAGGED {id:'audit_at_'+split(fr.id,'_')[2], label:'audit_label', indexed_at:$TS}]->(ap);

// subject gives tags (user + post) and bookmarks some posts
MATCH (s:User:Bench {id:'audit_subject'}), (f:User:Bench {id:'audit_followee_1'})
CREATE (s)-[:TAGGED {id:'audit_giventag_u', label:'mine', indexed_at:$TS}]->(f);
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1, 20) AS i
MATCH (p:Post:Bench {id:'audit_post_'+toString(i)})
CREATE (s)-[:TAGGED {id:'audit_giventag_p'+toString(i), label:'mine', indexed_at:$TS}]->(p)
CREATE (s)-[:BOOKMARKED {id:'audit_bm_'+toString(i), indexed_at:$TS}]->(p);

// viewer with a depth-2 trust path to the subject, plus a direct tag on the hot post
MATCH (s:User:Bench {id:'audit_subject'})
CREATE (v:User:Bench {id:'audit_viewer', name:'audit_viewer', indexed_at:$TS});
MATCH (v:User:Bench {id:'audit_viewer'}), (s:User:Bench {id:'audit_subject'})-[:FOLLOWS]->(mid:User:Bench {id:'audit_followee_1'})
CREATE (v)-[:FOLLOWS {indexed_at:$TS}]->(mid);
MATCH (v:User:Bench {id:'audit_viewer'}), (hp:Post:Bench {id:'audit_post_1'})
CREATE (v)-[:TAGGED {id:'audit_viewer_tag', label:'cool', indexed_at:$TS}]->(hp);

// a resource tagged by several users
CREATE (r:Resource:Bench {id:'audit_resource', indexed_at:$TS});
MATCH (r:Resource:Bench {id:'audit_resource'})
UNWIND range(1, $ACTIVE) AS i
CREATE (ru:User:Bench {id:'audit_rsrc_tagger_'+toString(i), name:'n', indexed_at:$TS})-[:TAGGED {id:'audit_rsrc_t_'+toString(i), label:'res_'+toString(i % $LABELS), indexed_at:$TS}]->(r);
EOF

echo -n "seeded :Bench nodes: "; CYPHER <<<'MATCH (n:Bench) RETURN count(n);' | tail -1
