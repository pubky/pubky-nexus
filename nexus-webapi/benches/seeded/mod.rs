//! Data a bench seeds for itself, for routes the shared mock graph holds nothing for, and
//! removes again before its binary exits. The mock fixtures are left as they are, so no other
//! test or bench sees this data.
//!
//! Every id is run-scoped: fresh user keys, and canonical post ids counted up from the run's
//! start, so a crashed run cannot collide with the fixtures or with a later run. [`Seed::finish`]
//! removes every node, edge, Redis key and Redis member the run wrote, then checks that the
//! graph's node and edge counts and the counts of persistent Redis keys, all and per touched
//! prefix, are what they were before seeding, and panics if one is not. Keys with a TTL (cached
//! counts) may expire meanwhile, so they are left out of the check, and `DBSIZE` is printed
//! beside it. The counts do not see a changed value of a key that was there before (a string, a
//! JSON document, a hash field's value); the seeds write none.
//!
//! Run it on the bench stack alone, as every bench runs on the mock: nothing else may write to
//! or read from the databases meanwhile. A key created while the bench ran that names none of
//! the run's users loses the run's members; if it is still there then and is not one of the
//! [`SHARED`] keys, it is reported and left, and the check then fails.

use deadpool_redis::redis;
use nexus_common::db::{exec_single_row, fetch_row_from_graph, get_redis_conn, graph::Query};
use nexus_common::models::user::Epoch;
use pubky_social_specs::PubkyId;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};

/// The key prefixes the seeds write under, counted before seeding and after cleanup.
const PREFIXES: [&str; 4] = [
    "Post:*",
    "Sorted:Posts:*",
    "Event:ResourceChanges*",
    "Notification:*",
];

/// Keys naming no user that the seeds create when they are absent: the resource changes feed
/// and its epoch. When present, only the run's lines are taken out of the feed again.
const SHARED: [&str; 2] = ["Event:ResourceChanges", "Event:ResourceChanges:Epoch"];

/// What the proof compares: graph nodes and edges, and the persistent Redis keys, all (`*`) and
/// per prefix.
#[derive(Debug, PartialEq, Eq)]
pub struct Counts {
    nodes: i64,
    edges: i64,
    persistent: BTreeMap<&'static str, usize>,
}

impl Counts {
    async fn read() -> Self {
        let count = |cypher: &'static str| async move {
            fetch_row_from_graph(Query::new("bench_seed_count", cypher))
                .await
                .unwrap()
                .unwrap()
                .get::<i64>("n")
                .unwrap()
        };
        let keys = scan("*").await;
        let mut conn = get_redis_conn().await.unwrap();
        let mut ttls: Vec<i64> = Vec::with_capacity(keys.len());
        for chunk in keys.chunks(1000) {
            let mut pipe = redis::pipe();
            for key in chunk {
                pipe.cmd("PTTL").arg(key);
            }
            ttls.extend(pipe.query_async::<Vec<i64>>(&mut conn).await.unwrap());
        }
        let persistent: Vec<&String> = keys
            .iter()
            .zip(ttls)
            .filter(|(_, ttl)| *ttl == -1)
            .map(|(key, _)| key)
            .collect();
        let mut counts = BTreeMap::from([("*", persistent.len())]);
        for prefix in PREFIXES {
            let start = prefix.trim_end_matches('*');
            counts.insert(
                prefix,
                persistent
                    .iter()
                    .filter(|key| key.starts_with(start))
                    .count(),
            );
        }
        Self {
            nodes: count("MATCH (n) RETURN count(n) AS n").await,
            edges: count("MATCH ()-[r]->() RETURN count(r) AS n").await,
            persistent: counts,
        }
    }
}

/// `DBSIZE`, printed beside the counts.
async fn dbsize() -> usize {
    let mut conn = get_redis_conn().await.unwrap();
    redis::cmd("DBSIZE").query_async(&mut conn).await.unwrap()
}

/// Every key matching `pattern`.
async fn scan(pattern: &str) -> Vec<String> {
    let mut conn = get_redis_conn().await.unwrap();
    let (mut cursor, mut keys) = (0u64, Vec::new());
    loop {
        let (next, page): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(1000)
            .query_async(&mut conn)
            .await
            .unwrap();
        keys.extend(page);
        if next == 0 {
            return keys;
        }
        cursor = next;
    }
}

/// One run's users, its post ids and the counts taken before it wrote anything. Keep it where
/// the cleanup finds it before writing anything ([`Seed::write_users`]), so a seed that fails
/// halfway is removed too.
pub struct Seed {
    pub users: Vec<String>,
    before: Counts,
    dbsize_before: usize,
    keys_before: HashSet<String>,
    next_id: AtomicI64,
}

impl Seed {
    /// Takes the counts and draws `users` fresh user keys. Writes nothing.
    pub async fn new(users: usize) -> Self {
        let before = Counts::read().await;
        let dbsize_before = dbsize().await;
        let keys_before = scan("*").await.into_iter().collect();
        Self {
            users: (0..users)
                .map(|_| pubky::Keypair::random().public_key().to_z32())
                .collect(),
            before,
            dbsize_before,
            keys_before,
            next_id: AtomicI64::new(chrono::Utc::now().timestamp_micros()),
        }
    }

    /// Writes the users as the watcher does for a `social/v1` user whose profile it has not
    /// indexed: a stub at epoch `v1`.
    pub async fn write_users(&self) {
        for user in &self.users {
            Epoch::set_v1_in_graph(&PubkyId::try_from(user.as_str()).unwrap())
                .await
                .unwrap();
        }
    }

    /// A canonical timestamp post id (13 Crockford characters, the dangling bit clear), counted up
    /// from the run's start.
    pub fn post_id(&self) -> String {
        const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
        let bits = (self.next_id.fetch_add(1, Ordering::Relaxed) as u128) << 1;
        (0..13)
            .map(|i| CROCKFORD[((bits >> (5 * (12 - i))) & 31) as usize] as char)
            .collect()
    }

    /// Removes what the run wrote and proves it. `graph` runs first, with `$users` bound to the
    /// run's users, for the nodes that are not the users' posts (Resources, event series). Then
    /// the users and their posts go, every Redis key naming a user, the [`SHARED`] keys the run
    /// created, and every member, element or field naming a user in the other keys. A key the
    /// run created that is still there then is reported.
    pub async fn finish(&self, graph: &[&str]) {
        for cypher in graph {
            exec_single_row(
                Query::new("bench_seed_cleanup", *cypher).param("users", self.users.clone()),
            )
            .await
            .unwrap();
        }
        exec_single_row(
            Query::new(
                "bench_seed_cleanup_users",
                "MATCH (u:User) WHERE u.id IN $users
                 OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p, u",
            )
            .param("users", self.users.clone()),
        )
        .await
        .unwrap();
        let names = |value: &str| self.users.iter().any(|user| value.contains(user.as_str()));
        let mut conn = get_redis_conn().await.unwrap();
        let (mut keys, mut members, mut unknown) = (0, 0, Vec::new());
        for key in scan("*").await {
            let created = !self.keys_before.contains(&key);
            if names(&key) || (created && SHARED.contains(&key.as_str())) {
                let _: () = redis::cmd("DEL")
                    .arg(&key)
                    .query_async(&mut conn)
                    .await
                    .unwrap();
                keys += 1;
                continue;
            }
            let kind: String = redis::cmd("TYPE")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap();
            let (read, remove) = match kind.as_str() {
                "zset" => ("ZRANGE", "ZREM"),
                "set" => ("SMEMBERS", "SREM"),
                "hash" => ("HKEYS", "HDEL"),
                "list" => ("LRANGE", "LREM"),
                _ => {
                    if created {
                        unknown.push(key);
                    }
                    continue;
                }
            };
            let mut read_cmd = redis::cmd(read);
            read_cmd.arg(&key);
            if matches!(kind.as_str(), "zset" | "list") {
                read_cmd.arg(0).arg(-1);
            }
            let values: Vec<String> = read_cmd.query_async(&mut conn).await.unwrap();
            for value in values.into_iter().filter(|value| names(value)) {
                let mut remove_cmd = redis::cmd(remove);
                remove_cmd.arg(&key);
                if kind == "list" {
                    remove_cmd.arg(0);
                }
                let _: () = remove_cmd.arg(value).query_async(&mut conn).await.unwrap();
                members += 1;
            }
            let exists: bool = redis::cmd("EXISTS")
                .arg(&key)
                .query_async(&mut conn)
                .await
                .unwrap();
            if created && exists {
                unknown.push(key);
            }
        }
        let after = Counts::read().await;
        println!(
            "Seed cleanup: {} users, {keys} Redis keys and {members} members removed.\n  before {:?}, DBSIZE {}\n  after  {:?}, DBSIZE {}",
            self.users.len(),
            self.before,
            self.dbsize_before,
            after,
            dbsize().await
        );
        assert!(
            unknown.is_empty(),
            "keys written meanwhile, left: {unknown:?}"
        );
        assert_eq!(self.before, after, "the cleanup left seeded data behind");
    }
}
