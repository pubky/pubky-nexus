//! Reach-scoped content search (`search/posts/by_content` with `user_id` +
//! `reach`) against the data the configured Neo4j and Redis already hold.
//!
//! The bench only reads: it seeds nothing and removes nothing. The defaults
//! point at the mock data (`cargo run -p nexusd -- db mock`); to read the curve
//! on a bigger follow graph and `postContentIdx`, load that dataset first and
//! name what to measure through the environment:
//!
//! | variable                | default               | meaning                        |
//! |-------------------------|-----------------------|--------------------------------|
//! | `BENCH_REACH_OBSERVERS` | two mock users        | comma-separated observer ids   |
//! | `BENCH_REACH_TERMS`     | `zyqwombat,privacy`   | comma-separated search terms   |
//!
//! Run with `cargo bench -p nexus-webapi --bench search_reach`.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::{
    db::kv::AuthorFilter,
    models::{
        follow::reach::reach_authors,
        post::search::{PostsByContentSearch, MAX_REACH_AUTHORS_FT},
    },
    types::{StreamReach, WotDepth},
};
use pubky_app_specs::PubkyId;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

/// The most connected user of the mock graph, whose reach also fills the
/// author sets, and the observer of the reach fixture (`search-reach.cypher`).
const DEFAULT_OBSERVERS: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro,\
wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy";
/// The reach fixture's term and one from the mock posts.
const DEFAULT_TERMS: &str = "zyqwombat,privacy";
const PAGE: usize = 20;
/// Author-set sizes of the FT.SEARCH curve.
const AUTHOR_SET_SIZES: [usize; 7] = [10, 100, 500, 1_000, 2_500, 5_000, 10_000];

/// The comma-separated values of `name`, or of `default` when it is unset.
fn env_list(name: &str, default: &str) -> Vec<String> {
    let raw = std::env::var(name).unwrap_or_else(|_| default.to_string());
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn observers() -> Vec<String> {
    env_list("BENCH_REACH_OBSERVERS", DEFAULT_OBSERVERS)
}

fn terms() -> Vec<String> {
    env_list("BENCH_REACH_TERMS", DEFAULT_TERMS)
}

/// Short, readable benchmark label for an observer id.
fn label(observer: &str) -> &str {
    observer.get(..8).unwrap_or(observer)
}

/// Deterministic, valid Pubky id that exists only in memory: it is a value of
/// the author filter, never written anywhere.
fn unstored_author(i: usize) -> PubkyId {
    let mut secret = [0u8; 32];
    secret[..10].copy_from_slice(b"benchreach");
    secret[24..].copy_from_slice(&(i as u64).to_le_bytes());
    PubkyId::from(pubky::Keypair::from_secret(&secret))
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

/// Prints how many authors each benched reach resolves to, so the timings can
/// be read against the reach size. A reach that fails to resolve is reported
/// and does not stop the run.
async fn print_reach_sizes(observers: &[String]) {
    println!("Reach sizes, counted up to {MAX_REACH_AUTHORS_FT}:");
    for observer in observers {
        let mut sizes = Vec::new();
        for (name, reach) in reach_cases() {
            match reach_authors(observer, &reach, MAX_REACH_AUTHORS_FT).await {
                Ok(authors) if authors.met_limit => sizes.push(format!("{name}=over")),
                Ok(authors) => sizes.push(format!("{name}={}", authors.author_ids.len())),
                Err(e) => sizes.push(format!("{name}=failed ({e})")),
            }
        }
        println!("  {:<8} {}", label(observer), sizes.join(" "));
    }
}

/// `size` authors for the FT.SEARCH curve: the stored authors the widest reach
/// of `observer` resolves to, the most prolific first, topped up with ids that
/// match no post, so the author list has the benched length on any dataset.
async fn author_set(observer: &str, size: usize) -> Vec<PubkyId> {
    let widest = StreamReach::Wot(WotDepth::new(3).unwrap());
    let mut authors = match reach_authors(observer, &widest, size).await {
        Ok(authors) => authors.author_ids,
        Err(e) => {
            println!("No stored authors in the author sets: {e}");
            Vec::new()
        }
    };
    println!("Author sets hold {} stored authors", authors.len());
    authors.extend((authors.len()..size).map(unstored_author));
    authors
}

// ── Benchmarks ────────────────────────────────────────────────────────────────

/// Reach resolution alone: the graph query that picks the `MAX_REACH_AUTHORS_FT`
/// most prolific authors in reach.
fn bench_resolve_reach(c: &mut Criterion) {
    run_setup();
    let rt = Runtime::new().unwrap();
    let observers = observers();
    rt.block_on(print_reach_sizes(&observers));

    let mut group = c.benchmark_group("search_reach/resolve");
    for observer in &observers {
        for (name, reach) in reach_cases() {
            group.bench_function(BenchmarkId::new(name, label(observer)), |b| {
                b.to_async(&rt).iter(|| async {
                    let ids = reach_authors(observer, &reach, MAX_REACH_AUTHORS_FT)
                        .await
                        .unwrap();
                    std::hint::black_box(ids);
                });
            });
        }
    }
    group.finish();
}

/// FT.SEARCH scoped to N authors, next to the unscoped and single-author
/// baselines.
fn bench_ft_author_set(c: &mut Criterion) {
    run_setup();
    let rt = Runtime::new().unwrap();
    let Some(observer) = observers().into_iter().next() else {
        println!("No observer to take the author sets from");
        return;
    };
    let largest = AUTHOR_SET_SIZES.into_iter().max().unwrap_or_default();
    let authors = rt.block_on(author_set(&observer, largest));

    for term in terms() {
        let term = term.as_str();
        let mut group = c.benchmark_group(format!("search_reach/ft/{term}"));
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
        for size in AUTHOR_SET_SIZES {
            let ids = &authors[..size];
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
    run_setup();
    let rt = Runtime::new().unwrap();
    let observers = observers();

    for term in terms() {
        let term = term.as_str();
        let mut group = c.benchmark_group(format!("search_reach/end_to_end/{term}"));
        for observer in &observers {
            for (name, reach) in reach_cases() {
                group.bench_function(BenchmarkId::new(&name, label(observer)), |b| {
                    b.to_async(&rt).iter(|| async {
                        let ids = reach_authors(observer, &reach, MAX_REACH_AUTHORS_FT)
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
              bench_end_to_end
}

criterion_main!(benches);
