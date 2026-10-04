// ##############################
// ##### Trust ranking ##########
// ##############################
// Runs last. Every fixture user is reachable from the seed set, as on a live
// network, so `source=all` serves their posts. The score sits between the wot
// endorsers (wot.cypher: D1 0.4, D2 0.2, D1B 0.1), which keeps D1 at the top
// of the ranking and D2, D1B below the `established` cut (user/views.rs). The
// wot on-ramp accounts stay unranked: stream/post/ranked.rs hides their
// posts. recommended.cypher's deleted account is scored on purpose, as a
// recompute scores a followed deleted account: ranked.rs checks that every
// path still hides it. Only wot.cypher tags with the starter-pack labels, so
// these scores do not touch stream/user/starter_pack.rs.
// A GDS recompute rewrites `trust` for every user, so run the nexusd suite last and reseed.
MATCH (u:User)
WHERE u.trust IS NULL AND NOT u.id IN [
  'y6apowjmcg8rocmd9jirg95fyf3yykwuhqxozzts4mjipk4n7iao', // observer
  'qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o', // spammer
  'qsfngw6xm9kk7yp99xustjfj8mu9auufkixas5f8goeujuxt45ao', // modbot
  'tfqnfppxtr8xei6n3zrfa1b7mmc6gqn41szgutgcccea33pqf3yo', // btc1
  'uuxjusor98rw4xdo3k3shsgdqwi844si14aaxfcnjxyczjt5eqxy', // btc2
  'qwzn6jx1gm1ziptn41dxonqy1rpuumwggdq1hu6zc334qep3kjho', // btc3
  'z5eect18reuccuwuq78da8k5re3y8si346n3bah45gad6t6b1zby', // btc4
  'wbhcz1gfz14jc4qjg74auyo5bwxd4gc3y84ic18iro17yi4bgz3y', // btc5
  'w153s1dr9rw6t8s3nd1de6pqquuprb37dwrnwh3nk85jt9ys9k7o', // artist1
  'z4e8s17cou9qmuwen8p1556jzhf1wktmzo6ijsfnri9c4hnrdfty'  // deleted_user
]
SET u.trust = 0.3;
