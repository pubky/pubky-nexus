//! Reach-scoped content search (`search/posts/by_content` with `user_id` +
//! `reach`) against a large synthetic dataset, so the curve reflects a big
//! follow graph and a big `postContentIdx` rather than the mock data.
//!
//! The dataset is seeded before the benches and removed afterwards. Synthetic
//! users carry valid, deterministic Pubky ids and the name `benchreach`, and
//! their post ids start with `BENCH`. Sizes come from the environment:
//!
//! | variable                       | default | meaning                              |
//! |--------------------------------|---------|--------------------------------------|
//! | `BENCH_REACH_USERS`            | 20000   | synthetic users in the graph         |
//! | `BENCH_REACH_AVG_FOLLOWS`      | 30      | mean out-degree of a regular user    |
//! | `BENCH_REACH_POSTS_PER_USER`   | 10      | mean posts per user (skewed)         |
//! | `BENCH_REACH_KEEP`             | unset   | `1` keeps the dataset for a re-run   |
//!
//! Run with `cargo bench -p nexus-webapi --bench search_reach`. Seeding
//! writes to the configured Neo4j and Redis; don't point it at production.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use deadpool_redis::redis::{self, AsyncCommands};
use nexus_common::{
    db::{get_neo4j_graph, get_redis_conn, graph::Query, kv::AuthorFilter},
    models::{
        follow::reach::reach_authors,
        post::search::{PostsByContentSearch, MAX_REACH_AUTHORS_FT},
    },
    types::{StreamReach, WotDepth},
};
use pubky_app_specs::PubkyId;
use serde_json::json;
use setup::run_setup;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

mod setup;

/// Name of every synthetic user, which is how cleanup finds them in the graph.
const BENCH_NAME: &str = "benchreach";
const SEED_MARKER_KEY: &str = "Bench:SearchReach:Seeded";
/// In roughly 30% of posts.
const COMMON_TERM: &str = "benchcommon";
/// In roughly 0.1% of posts.
const RARE_TERM: &str = "benchrare";
const TERMS: [(&str, &str); 2] = [("common", COMMON_TERM), ("rare", RARE_TERM)];
const PAGE: usize = 20;

/// Observers with a fixed out-degree, so reach sizes are predictable. They are
/// the first users; every other user draws a random out-degree.
const OBSERVERS: [(&str, usize); 3] = [("small", 20), ("medium", 200), ("large", 2_000)];

#[derive(Debug, Clone, Copy, PartialEq)]
struct Config {
    users: usize,
    avg_follows: usize,
    posts_per_user: usize,
    keep: bool,
}

impl Config {
    fn from_env() -> Self {
        let var = |name: &str, default: usize| {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Config {
            users: var("BENCH_REACH_USERS", 20_000).max(OBSERVERS[2].1 + 1),
            avg_follows: var("BENCH_REACH_AVG_FOLLOWS", 30),
            posts_per_user: var("BENCH_REACH_POSTS_PER_USER", 10),
            keep: std::env::var("BENCH_REACH_KEEP").as_deref() == Ok("1"),
        }
    }

    fn signature(&self) -> String {
        format!(
            "users={} avg_follows={} posts_per_user={}",
            self.users, self.avg_follows, self.posts_per_user
        )
    }
}

/// Deterministic, valid Pubky id of synthetic user `i`.
fn pubky_id(i: usize) -> PubkyId {
    let mut secret = [0u8; 32];
    secret[..BENCH_NAME.len()].copy_from_slice(BENCH_NAME.as_bytes());
    secret[24..].copy_from_slice(&(i as u64).to_le_bytes());
    PubkyId::from(pubky::Keypair::from_secret(&secret))
}

/// Ids of the configured users, derived once.
static USER_IDS: OnceLock<Vec<PubkyId>> = OnceLock::new();

fn user_ids() -> &'static [PubkyId] {
    USER_IDS.get().expect("user ids are derived before use")
}

fn user_id(i: usize) -> String {
    user_ids()[i].to_string()
}

fn observer_id(name: &str) -> String {
    let index = OBSERVERS
        .iter()
        .position(|(n, _)| *n == name)
        .expect("known observer");
    user_id(index)
}

/// Deterministic xorshift, so every run seeds the same dataset.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

// ── Dataset lifecycle ─────────────────────────────────────────────────────────

static DATASET: OnceLock<Config> = OnceLock::new();

/// Seeds the dataset once per process and registers its cleanup.
fn dataset(rt: &Runtime) -> Config {
    *DATASET.get_or_init(|| {
        run_setup();
        let config = Config::from_env();
        USER_IDS.get_or_init(|| (0..config.users).map(pubky_id).collect());
        rt.block_on(async {
            let mut conn = get_redis_conn().await.unwrap();
            let seeded: Option<String> = conn.get(SEED_MARKER_KEY).await.unwrap();
            if seeded.as_deref() == Some(config.signature().as_str()) {
                println!("Reusing the seeded dataset ({})", config.signature());
            } else {
                cleanup().await;
                seed(&config).await;
                let _: () = conn.set(SEED_MARKER_KEY, config.signature()).await.unwrap();
            }
            print_reach_sizes().await;
        });
        config
    })
}

fn post_id(user: usize, n: usize) -> String {
    format!("BENCH{user:07}{n:05}")
}

/// Posts per user, heavily skewed (u³ sampling) around the configured mean,
/// so trimming a reach to the most prolific authors has something to pick.
fn post_counts(config: &Config) -> Vec<usize> {
    let mut rng = Rng(0xbead_bead_bead_bead);
    (0..config.users)
        .map(|_| (rng.unit().powi(3) * 4.0 * config.posts_per_user as f64) as usize)
        .collect()
}

async fn seed(config: &Config) {
    println!("Seeding {}", config.signature());
    let counts = post_counts(config);
    let started = Instant::now();
    seed_graph(config, &counts).await;
    println!("  graph seeded in {:?}", started.elapsed());
    let started = Instant::now();
    seed_posts(&counts).await;
    println!("  posts indexed in {:?}", started.elapsed());
}

async fn seed_graph(config: &Config, counts: &[usize]) {
    let graph = get_neo4j_graph().unwrap();

    for chunk in (0..config.users).collect::<Vec<_>>().chunks(10_000) {
        let ids: Vec<String> = chunk.iter().map(|&i| user_id(i)).collect();
        let query = Query::new(
            "bench_seed_users",
            "UNWIND $ids AS id CREATE (:User {id: id, name: $name, bio: '', status: 'undefined', indexed_at: 0, links: '[]'})",
        )
        .param("ids", ids)
        .param("name", BENCH_NAME);
        graph.run(query).await.unwrap();
    }

    // Targets are skewed towards low indices (u² sampling), so a few users are
    // followed by many, as in a real network
    let mut rng = Rng(0x5eed_5eed_5eed_5eed);
    let mut batch: Vec<Vec<String>> = Vec::with_capacity(20_000);
    let mut edges = 0usize;
    for follower in 0..config.users {
        let degree = match OBSERVERS.get(follower) {
            Some((_, degree)) => *degree,
            None => rng.below(2 * config.avg_follows + 1),
        };
        let mut targets = std::collections::HashSet::with_capacity(degree);
        while targets.len() < degree {
            let target = if targets.len() % 2 == 0 {
                (rng.unit().powi(2) * config.users as f64) as usize
            } else {
                rng.below(config.users)
            };
            if target != follower {
                targets.insert(target);
            }
        }
        for target in targets {
            batch.push(vec![user_id(follower), user_id(target)]);
        }
        if batch.len() >= 20_000 || follower + 1 == config.users {
            edges += batch.len();
            let query = Query::new(
                "bench_seed_follows",
                "UNWIND $pairs AS pair
                 MATCH (a:User {id: pair[0]}), (b:User {id: pair[1]})
                 CREATE (a)-[:FOLLOWS {indexed_at: 0, id: 'bench'}]->(b)",
            )
            .param("pairs", std::mem::take(&mut batch));
            graph.run(query).await.unwrap();
        }
    }
    println!("  {} users, {edges} follows", config.users);

    // Only the AUTHORED degree matters to the graph side (reach ordering)
    let mut batch: Vec<Vec<String>> = Vec::with_capacity(20_000);
    for (user, &count) in counts.iter().enumerate() {
        for n in 0..count {
            batch.push(vec![user_id(user), post_id(user, n)]);
        }
        if batch.len() >= 20_000 || user + 1 == counts.len() {
            let query = Query::new(
                "bench_seed_posts",
                "UNWIND $pairs AS pair
                 MATCH (u:User {id: pair[0]})
                 CREATE (u)-[:AUTHORED]->(:Post {id: pair[1], content: '', kind: 'short', indexed_at: 0})",
            )
            .param("pairs", std::mem::take(&mut batch));
            graph.run(query).await.unwrap();
        }
    }
}

async fn seed_posts(counts: &[usize]) {
    let mut conn = get_redis_conn().await.unwrap();
    let mut rng = Rng(0xc0ff_ee00_c0ff_ee00);
    let mut pipe = redis::pipe();
    let mut pending = 0;
    for (user, &count) in counts.iter().enumerate() {
        let author = user_id(user);
        for n in 0..count {
            let post_id = post_id(user, n);
            // Zipf-like filler vocabulary, plus the two query terms
            let mut words: Vec<String> = (0..8)
                .map(|_| format!("w{}", (rng.unit().powi(3) * 5_000.0) as usize))
                .collect();
            if rng.unit() < 0.3 {
                words.push(COMMON_TERM.to_string());
            }
            if rng.unit() < 0.001 {
                words.push(RARE_TERM.to_string());
            }
            let doc = json!({
                "content": words.join(" "),
                "id": post_id,
                "indexed_at": 0,
                "author": author,
                "kind": "short",
                "uri": format!("pubky://{author}/pub/pubky.app/posts/{post_id}"),
                "attachments": null,
            });
            pipe.cmd("JSON.SET")
                .arg(format!("Post:Details:{author}:{post_id}"))
                .arg("$")
                .arg(doc.to_string())
                .ignore();
            pending += 1;
            if pending == 2_000 {
                let _: () = pipe.query_async(&mut conn).await.unwrap();
                pipe = redis::pipe();
                pending = 0;
            }
        }
    }
    if pending > 0 {
        let _: () = pipe.query_async(&mut conn).await.unwrap();
    }
    println!("  {} posts", counts.iter().sum::<usize>());
}

/// Removes the synthetic dataset, including one seeded with a different size.
async fn cleanup() {
    let started = Instant::now();
    let mut conn = get_redis_conn().await.unwrap();

    // Post keys are found by their post id
    let mut cursor = 0u64;
    loop {
        let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("Post:Details:*:BENCH*")
            .arg("COUNT")
            .arg(10_000)
            .query_async(&mut conn)
            .await
            .unwrap();
        if !keys.is_empty() {
            let _: () = conn.unlink(keys).await.unwrap();
        }
        if next == 0 {
            break;
        }
        cursor = next;
    }
    let _: () = conn.del(SEED_MARKER_KEY).await.unwrap();

    let graph = get_neo4j_graph().unwrap();
    for (label, cypher) in [
        (
            "bench_cleanup_posts",
            "MATCH (u:User {name: $name})-[:AUTHORED]->(p:Post)
             CALL { WITH p DETACH DELETE p } IN TRANSACTIONS OF 10000 ROWS",
        ),
        // Relationships go first, in bounded batches: detaching a followed hub
        // together with its batch exceeds Neo4j's transaction memory pool
        (
            "bench_cleanup_follows",
            "MATCH (u:User {name: $name})-[f:FOLLOWS]->()
             CALL { WITH f DELETE f } IN TRANSACTIONS OF 20000 ROWS",
        ),
        (
            "bench_cleanup_users",
            "MATCH (u:User {name: $name})
             CALL { WITH u DETACH DELETE u } IN TRANSACTIONS OF 5000 ROWS",
        ),
    ] {
        let query = Query::new(label, cypher).param("name", BENCH_NAME);
        graph.run(query).await.unwrap();
    }
    println!("Synthetic dataset removed in {:?}", started.elapsed());
}

fn reach_cases() -> Vec<(String, StreamReach)> {
    let wot = |d| StreamReach::Wot(WotDepth::new(d).unwrap());
    vec![
        ("following".into(), StreamReach::Following),
        ("followers".into(), StreamReach::Followers),
        ("friends".into(), StreamReach::Friends),
        ("wot_1".into(), wot(1)),
        ("wot_2".into(), wot(2)),
        ("wot_3".into(), wot(3)),
    ]
}

/// Prints how many authors each benched reach resolves to (uncapped), so the
/// timings can be read against the reach size.
async fn print_reach_sizes() {
    println!("Reach sizes before trimming to {MAX_REACH_AUTHORS_FT}:");
    for (observer, _) in OBSERVERS {
        let id = observer_id(observer);
        let mut sizes = Vec::new();
        for (name, reach) in reach_cases() {
            let size = reach_authors(&id, &reach, usize::MAX - 1)
                .await
                .unwrap()
                .author_ids
                .len();
            sizes.push(format!("{name}={size}"));
        }
        println!("  {observer:<6} {}", sizes.join(" "));
    }
}

// ── Benchmarks ────────────────────────────────────────────────────────────────

/// Reach resolution alone: the graph query that picks the `MAX_REACH_AUTHORS_FT`
/// most prolific authors in reach.
fn bench_resolve_reach(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    dataset(&rt);

    let mut group = c.benchmark_group("search_reach/resolve");
    for (observer, _) in OBSERVERS {
        let id = observer_id(observer);
        for (name, reach) in reach_cases() {
            group.bench_function(BenchmarkId::new(name, observer), |b| {
                b.to_async(&rt).iter(|| async {
                    let ids = reach_authors(&id, &reach, MAX_REACH_AUTHORS_FT)
                        .await
                        .unwrap();
                    std::hint::black_box(ids);
                });
            });
        }
    }
    group.finish();
}

/// FT.SEARCH scoped to N seeded authors, next to the unscoped and
/// single-author baselines, on the large index.
fn bench_ft_author_set(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = dataset(&rt);

    // The most prolific authors first, as a trimmed reach would send them
    let counts = post_counts(&config);
    let mut by_posts: Vec<usize> = (0..config.users).collect();
    by_posts.sort_by_key(|&user| std::cmp::Reverse(counts[user]));
    let authors: Vec<PubkyId> = by_posts
        .into_iter()
        .take(10_000)
        .map(|user| user_ids()[user].clone())
        .collect();

    for (term_name, term) in TERMS {
        let mut group = c.benchmark_group(format!("search_reach/ft/{term_name}"));
        group.bench_function("unscoped", |b| {
            b.to_async(&rt).iter(|| async {
                let r = PostsByContentSearch::search(term, None, None, 0, PAGE)
                    .await
                    .unwrap();
                std::hint::black_box(r);
            });
        });
        group.bench_function("one_author", |b| {
            b.to_async(&rt).iter(|| async {
                let r = PostsByContentSearch::search(
                    term,
                    Some(AuthorFilter::One(&authors[0])),
                    None,
                    0,
                    PAGE,
                )
                .await
                .unwrap();
                std::hint::black_box(r);
            });
        });
        for size in [10, 100, 500, 1_000, 2_500, 5_000, 10_000] {
            let ids = &authors[..size.min(authors.len())];
            group.bench_function(BenchmarkId::new("authors", size), |b| {
                b.to_async(&rt).iter(|| async {
                    let r = PostsByContentSearch::search(
                        term,
                        Some(AuthorFilter::AnyOf(ids)),
                        None,
                        0,
                        PAGE,
                    )
                    .await
                    .unwrap();
                    std::hint::black_box(r);
                });
            });
        }
        group.finish();
    }
}

/// What the handler does: resolve the (trimmed) reach, then search within it.
fn bench_end_to_end(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    dataset(&rt);

    for (term_name, term) in TERMS {
        let mut group = c.benchmark_group(format!("search_reach/end_to_end/{term_name}"));
        for (observer, _) in OBSERVERS {
            let id = observer_id(observer);
            for (name, reach) in reach_cases() {
                group.bench_function(BenchmarkId::new(&name, observer), |b| {
                    b.to_async(&rt).iter(|| async {
                        let ids = reach_authors(&id, &reach, MAX_REACH_AUTHORS_FT)
                            .await
                            .unwrap()
                            .author_ids;
                        let r = PostsByContentSearch::search(
                            term,
                            Some(AuthorFilter::AnyOf(&ids)),
                            None,
                            0,
                            PAGE,
                        )
                        .await
                        .unwrap();
                        std::hint::black_box(r);
                    });
                });
            }
        }
        group.finish();
    }
}

/// Runs last: drops the dataset unless `BENCH_REACH_KEEP=1`.
fn teardown(_: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let config = dataset(&rt);
    if config.keep {
        println!("BENCH_REACH_KEEP=1: leaving the synthetic dataset in place");
    } else {
        rt.block_on(cleanup());
    }
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(3, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_resolve_reach,
              bench_ft_author_set,
              bench_end_to_end,
              teardown
}

criterion_main!(benches);
