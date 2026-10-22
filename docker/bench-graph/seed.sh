#!/usr/bin/env bash
#
# Seed a synthetic graph for the query performance audit.
#
# Everything created here carries an extra :Bench label and an `audit_*` id
# prefix, so cleanup is a single `MATCH (n:Bench) DETACH DELETE n` and the
# audit never touches real data. Idempotent: re-running re-seeds from scratch.
#
# Scale is a multiplier M applied to every base dimension. The subject user and
# the hot post are deliberately extreme on EVERY axis, so a query pointed at them
# sees its worst case. Named scales: small=1, large=10, xl=500 (~100k, the #935
# whale). A raw integer also works (e.g. `./seed.sh 3`).
#
# Usage:
#   ./seed.sh small | large | xl | <int>     seed at that multiplier
#   ./seed.sh clean                            remove all :Bench data and exit
#   FOLLOWERS=5000 ./seed.sh 3                 override a single dimension
#
# Deterministic ids the dumper references: audit_subject, audit_post_1,
# audit_viewer, audit_resource, audit_label, audit_hs, audit_subject/audit_file_1,
# audit_bm_1, audit_giventag_p1, audit_hp_tagger_1/audit_hpt_1.
set -euo pipefail

ARG="${1:-small}"
CYPHER() { docker exec -i neo4j cypher-shell -u neo4j -p "${NEO4J_PASSWORD:-12345678}" --format plain "$@"; }
clean() { CYPHER >/dev/null <<'EOF'
MATCH (n:Bench) DETACH DELETE n;
EOF
}
if [[ "$ARG" == "clean" ]]; then echo "cleaning :Bench data..."; clean; echo "done."; exit 0; fi

case "$ARG" in
  small) M=1 ;;
  large) M=10 ;;
  xl)    M=500 ;;
  *) M="$ARG" ;;
esac
[[ "$M" =~ ^[0-9]+$ && "$M" -ge 1 ]] || { echo "scale must be small|large|xl|<positive int>, got '$ARG'" >&2; exit 1; }

# --- base dimensions (M=1); large = 10x makes growth factors readable ---
: "${POSTS:=$((200 * M))}"          # posts authored by the subject
: "${FOLLOWERS:=$((200 * M))}"      # users following the subject
: "${FOLLOWING:=$((100 * M))}"      # users the subject follows (half follow back = friends)
: "${RECV_TAGS:=$((200 * M))}"      # tags the subject receives (the #935 row multiplier)
: "${HOT_TAGS:=$((50 * M))}"        # tags/replies/reposts on audit_post_1
: "${HOT_MENTIONS:=$((50 * M))}"    # users mentioned by audit_post_1 (post_relationships COLLECT)
: "${BOOKMARKS:=$((50 * M))}"       # subject's own bookmarks AND bookmarks received by the hot post
: "${GIVEN_TAGS:=$((50 * M))}"      # tags the subject assigns (user_tags / tagged count)
: "${ACTIVE:=$((30 * M))}"          # followers who author+tag (feed reach/hot-tag/influencer queries)
: "${WOT_MID:=$((20 * M))}"         # connectors in the viewer's trust network (WoT volume)
: "${HOMESERVERS:=$((5 * M))}"      # homeserver nodes (get_all_homeservers)
WOT_FANOUT=10
LABELS=20
TS=1735689600000

echo "seeding M=$M posts=$POSTS followers=$FOLLOWERS recv_tags=$RECV_TAGS hot_tags=$HOT_TAGS bookmarks=$BOOKMARKS given_tags=$GIVEN_TAGS wot_mid=$WOT_MID"
clean

CYPHER >/dev/null <<EOF
CREATE (s:User:Bench {id:'audit_subject', name:'audit_subject', indexed_at:$TS});
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$POSTS) AS i
CREATE (s)-[:AUTHORED {indexed_at:$TS}]->(p:Post:Bench {id:'audit_post_'+toString(i), indexed_at:$TS, content:'c'+toString(i), kind: CASE WHEN i % 50 = 0 THEN 'collection' END});

// hot post: many received tags, replies, reposts, and outgoing mentions
MATCH (hp:Post:Bench {id:'audit_post_1'})
UNWIND range(1,$HOT_TAGS) AS i
CREATE (u:User:Bench {id:'audit_hp_tagger_'+toString(i), name:'n', indexed_at:$TS})-[:TAGGED {id:'audit_hpt_'+toString(i), label:'lbl_'+toString(i % $LABELS), indexed_at:$TS}]->(hp)
CREATE (u)-[:AUTHORED {indexed_at:$TS}]->(rp:Post:Bench {id:'audit_reply_'+toString(i), indexed_at:$TS, content:'r'})-[:REPLIED]->(hp)
CREATE (u)-[:AUTHORED {indexed_at:$TS}]->(rt:Post:Bench {id:'audit_repost_'+toString(i), indexed_at:$TS, content:'x'})-[:REPOSTED]->(hp);
MATCH (hp:Post:Bench {id:'audit_post_1'})
UNWIND range(1,$HOT_MENTIONS) AS i
CREATE (hp)-[:MENTIONED]->(:User:Bench {id:'audit_mentioned_'+toString(i), name:'n', indexed_at:$TS});

// subject receives RECV_TAGS user-tags (the #935 axis), LABELS distinct
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

// FOLLOWERS follow the subject; ACTIVE of them author+tag with audit_label
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$FOLLOWERS) AS i
CREATE (fr:User:Bench {id:'audit_follower_'+toString(i), name:'n', indexed_at:$TS})-[:FOLLOWS {indexed_at:$TS}]->(s);
MATCH (fr:User:Bench)-[:FOLLOWS]->(s:User:Bench {id:'audit_subject'})
WHERE fr.id STARTS WITH 'audit_follower_' AND toInteger(split(fr.id,'_')[2]) <= $ACTIVE
CREATE (fr)-[:AUTHORED {indexed_at:$TS}]->(ap:Post:Bench {id:'audit_apost_'+split(fr.id,'_')[2], indexed_at:$TS, content:'a'})
CREATE (fr)-[:TAGGED {id:'audit_at_'+split(fr.id,'_')[2], label:'audit_label', indexed_at:$TS}]->(ap);

// subject ASSIGNS GIVEN_TAGS tags (posts + a few users), and BOOKMARKS posts
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$GIVEN_TAGS) AS i
MATCH (p:Post:Bench {id:'audit_post_'+toString(i)})
CREATE (s)-[:TAGGED {id:'audit_giventag_p'+toString(i), label:'mine_'+toString(i % $LABELS)}]->(p);
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$GIVEN_TAGS) AS i
MATCH (f:User:Bench {id:'audit_followee_'+toString(((i-1) % ($FOLLOWING/2)) + 1)})
CREATE (s)-[:TAGGED {id:'audit_giventag_u'+toString(i), label:'mine'}]->(f);
MATCH (s:User:Bench {id:'audit_subject'})
UNWIND range(1,$BOOKMARKS) AS i
MATCH (p:Post:Bench {id:'audit_post_'+toString(i)})
CREATE (s)-[:BOOKMARKED {id:'audit_bm_'+toString(i), indexed_at:$TS}]->(p);

// BOOKMARKS fresh users bookmark the hot post (get_post_bookmarks scales)
MATCH (hp:Post:Bench {id:'audit_post_1'})
UNWIND range(1,$BOOKMARKS) AS i
CREATE (:User:Bench {id:'audit_hp_bookmarker_'+toString(i), name:'n', indexed_at:$TS})-[:BOOKMARKED {id:'audit_hpbm_'+toString(i), indexed_at:$TS}]->(hp);

// viewer with a dense, scaling depth-2 trust network reaching subject+hot-post taggers
CREATE (v:User:Bench {id:'audit_viewer', name:'audit_viewer', indexed_at:$TS});
MATCH (v:User:Bench {id:'audit_viewer'})
UNWIND range(1,$WOT_MID) AS i
CREATE (v)-[:FOLLOWS {indexed_at:$TS}]->(:User:Bench {id:'audit_wotmid_'+toString(i), name:'n', indexed_at:$TS});
MATCH (mid:User:Bench) WHERE mid.id STARTS WITH 'audit_wotmid_'
WITH mid, toInteger(split(mid.id,'_')[2]) AS k
UNWIND range(1,$WOT_FANOUT) AS j
MATCH (rt:User:Bench {id:'audit_rtagger_'+toString(((k-1)*$WOT_FANOUT + j - 1) % $RECV_TAGS + 1)})
MATCH (hpt:User:Bench {id:'audit_hp_tagger_'+toString(((k-1)*$WOT_FANOUT + j - 1) % $HOT_TAGS + 1)})
CREATE (mid)-[:FOLLOWS {indexed_at:$TS}]->(rt)
CREATE (mid)-[:FOLLOWS {indexed_at:$TS}]->(hpt);
MATCH (v:User:Bench {id:'audit_viewer'}), (hp:Post:Bench {id:'audit_post_1'})
CREATE (v)-[:TAGGED {id:'audit_viewer_tag', label:'cool', indexed_at:$TS}]->(hp)
CREATE (v)-[:BOOKMARKED {id:'audit_viewer_bm', indexed_at:$TS}]->(hp);

// a resource tagged by several users
CREATE (r:Resource:Bench {id:'audit_resource', uri:'https://audit.example/r', scheme:'https', indexed_at:$TS});
MATCH (r:Resource:Bench {id:'audit_resource'})
UNWIND range(1, $ACTIVE) AS i
CREATE (ru:User:Bench {id:'audit_rsrc_tagger_'+toString(i), name:'n', indexed_at:$TS})-[:TAGGED {id:'audit_rsrc_t_'+toString(i), label:'res_'+toString(i % $LABELS), indexed_at:$TS}]->(r);

// homeservers (first is audit_hs) and a file
UNWIND range(1,$HOMESERVERS) AS i
CREATE (:Homeserver:Bench {id: CASE WHEN i = 1 THEN 'audit_hs' ELSE 'audit_hs_'+toString(i) END});
CREATE (:File:Bench {owner_id:'audit_subject', id:'audit_file_1', indexed_at:$TS});
EOF

echo -n "seeded :Bench nodes: "; CYPHER <<<'MATCH (n:Bench) RETURN count(n);' | tail -1
