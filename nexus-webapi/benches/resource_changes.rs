use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use deadpool_redis::redis;
use nexus_common::db::get_redis_conn;
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeOp, RESOURCE_CHANGES_KEY,
};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

/// Lines the bench appends to the feed for itself: three pages at the route's maximum, so the
/// page sizes read different amounts of work, whatever the feed held before.
const SEEDED_LINES: usize = 3000;

/// Reads the whole resource changes feed from cursor 0 in pages of `limit`, as a consumer
/// catching up does through `GET /v1/resources/changes`: one script call and up to `limit`
/// JSON parses per page.
async fn read_feed(limit: usize) -> usize {
    let mut cursor = 0;
    let mut read = 0;
    loop {
        let (page, _, _) = ResourceChange::get_from_index(cursor, limit).await.unwrap();
        if page.lines.is_empty() {
            return read;
        }
        read += page.lines.len();
        cursor = page.next_cursor;
        std::hint::black_box(page);
    }
}

/// What the cleanup must give back: the number of Redis keys, and the feed's length and
/// its first and last lines.
#[derive(Debug, PartialEq, Eq)]
struct FeedState {
    keys: usize,
    length: usize,
    ends: Vec<String>,
}

async fn feed_state() -> FeedState {
    let mut conn = get_redis_conn().await.unwrap();
    let (keys, length, first, last): (usize, usize, Vec<String>, Vec<String>) = redis::pipe()
        .cmd("DBSIZE")
        .cmd("LLEN")
        .arg(RESOURCE_CHANGES_KEY)
        .cmd("LRANGE")
        .arg(RESOURCE_CHANGES_KEY)
        .arg(0)
        .arg(0)
        .cmd("LRANGE")
        .arg(RESOURCE_CHANGES_KEY)
        .arg(-1)
        .arg(-1)
        .query_async(&mut conn)
        .await
        .unwrap();
    FeedState {
        keys,
        length,
        ends: [first, last].concat(),
    }
}

/// The bench's own feed lines, appended once and taken out again when it is dropped (also when
/// a bench panics), by a run-scoped author: a fresh mock holds no feed, and the bench must not
/// depend on one. Nothing else may write to the feed meanwhile, as for every bench on the mock.
struct SeededFeed<'a> {
    rt: &'a Runtime,
    before: FeedState,
    lines: Vec<String>,
}

impl<'a> SeededFeed<'a> {
    fn seed(rt: &'a Runtime) -> Self {
        let (before, lines) = rt.block_on(Self::append());
        Self { rt, before, lines }
    }

    async fn append() -> (FeedState, Vec<String>) {
        let before = feed_state().await;
        let author = pubky::Keypair::random().public_key().to_z32();
        let lines: Vec<String> = (0..SEEDED_LINES)
            .map(|i| {
                let uri = format!("https://example.com/bench/{author}/{i}");
                let op = if i % 4 == 3 {
                    ResourceChangeOp::Del
                } else {
                    ResourceChangeOp::Put
                };
                let line = ResourceChange::tag(
                    op,
                    nexus_common::universal_tag::normalize::resource_id(&uri),
                    uri,
                    author.clone(),
                    "bench".into(),
                    &format!("pubky://{author}/pub/pubky.app/tags/BENCH{i:08}"),
                    i as i64,
                )
                .unwrap();
                serde_json::to_string(&line).unwrap()
            })
            .collect();
        // Appended without the epoch: a fresh mock has none, and the bench writes no other key
        let mut conn = get_redis_conn().await.unwrap();
        redis::cmd("RPUSH")
            .arg(RESOURCE_CHANGES_KEY)
            .arg(&lines)
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
        (before, lines)
    }

    async fn remove(&self) {
        let mut conn = get_redis_conn().await.unwrap();
        let start = self.before.length;
        let tail: Vec<String> = redis::cmd("LRANGE")
            .arg(RESOURCE_CHANGES_KEY)
            .arg(start)
            .arg(-1)
            .query_async(&mut conn)
            .await
            .unwrap();
        if tail == self.lines {
            if start == 0 {
                redis::cmd("DEL")
                    .arg(RESOURCE_CHANGES_KEY)
                    .query_async::<()>(&mut conn)
                    .await
                    .unwrap();
            } else {
                redis::cmd("LTRIM")
                    .arg(RESOURCE_CHANGES_KEY)
                    .arg(0)
                    .arg(start - 1)
                    .query_async::<()>(&mut conn)
                    .await
                    .unwrap();
            }
        } else {
            // Something else appended meanwhile: take out the bench's lines one by one
            for line in &self.lines {
                redis::cmd("LREM")
                    .arg(RESOURCE_CHANGES_KEY)
                    .arg(1)
                    .arg(line)
                    .query_async::<()>(&mut conn)
                    .await
                    .unwrap();
            }
        }
        let after = feed_state().await;
        println!(
            "Feed before seeding {:?}, after cleanup {after:?}",
            self.before
        );
        assert_eq!(
            after, self.before,
            "the bench must leave the feed as it found it"
        );
    }
}

impl Drop for SeededFeed<'_> {
    fn drop(&mut self) {
        self.rt.block_on(self.remove());
    }
}

fn bench_resource_changes(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the resource changes feed.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    let seeded = SeededFeed::seed(&rt);
    let (_, _, length) = rt.block_on(ResourceChange::get_from_index(0, 0)).unwrap();
    println!(
        "The feed holds {length} lines, {} of them seeded by the bench.",
        seeded.lines.len()
    );

    let mut group = c.benchmark_group("resource_changes");
    // The route's maximum page and its default
    for limit in [1000, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(limit), &limit, |b, &limit| {
            b.to_async(&rt).iter(|| read_feed(limit));
        });
    }
    group.finish();
    drop(seeded);
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(10, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_resource_changes,
}

criterion_main!(benches);
