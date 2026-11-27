use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use deadpool_redis::redis;
use nexus_common::db::get_redis_conn;
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeOp, RESOURCE_CHANGES_KEY,
};
use setup::run_setup;
use std::sync::LazyLock;
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

/// The feed's length and its first and last lines: what the cleanup gives back when nothing
/// else touched the feed meanwhile. The number of Redis keys is logged, not compared: other
/// keys may come or expire during a run, and the bench owns only its feed lines.
#[derive(Debug, PartialEq, Eq)]
struct FeedState {
    length: usize,
    ends: Vec<String>,
}

async fn feed_state() -> Result<(FeedState, usize), String> {
    let mut conn = get_redis_conn().await.map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;
    let state = FeedState {
        length,
        ends: [first, last].concat(),
    };
    Ok((state, keys))
}

/// The bench's own feed lines, appended once and taken out again when it is dropped (also when
/// a bench panics), by a run-scoped author: a fresh mock holds no feed, and the bench must not
/// depend on one. When nothing else appended meanwhile, the feed is cut back to what it was;
/// otherwise each bench line is removed and the others stay. Either way no bench line is left.
struct SeededFeed<'a> {
    rt: &'a Runtime,
    before: FeedState,
    keys_before: usize,
    lines: Vec<String>,
}

impl<'a> SeededFeed<'a> {
    fn seed(rt: &'a Runtime) -> Self {
        let (before, keys_before, lines) = rt.block_on(Self::append());
        Self {
            rt,
            before,
            keys_before,
            lines,
        }
    }

    async fn append() -> (FeedState, usize, Vec<String>) {
        let (before, keys_before) = feed_state().await.unwrap();
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
        (before, keys_before, lines)
    }

    /// Takes the bench's lines out and checks what the bench owns: none of its lines is left
    /// in the feed, and, when nothing else appended meanwhile, the feed is what it was.
    async fn remove(&self) -> Result<(), String> {
        // The feed's tail after the length it had is exactly the bench's lines: cut it back to
        // that length (or delete it, if it had none), and return the feed's state right after
        // the cut. Compared, cut and read in one script, so a line appended meanwhile is never
        // cut with them and cannot fail the check of the cut.
        static CUT: LazyLock<redis::Script> = LazyLock::new(|| {
            redis::Script::new(
                r"
            local start = tonumber(ARGV[1])
            local tail = redis.call('LRANGE', KEYS[1], start, -1)
            if #tail ~= #ARGV - 1 then return {0, 0, {}, {}} end
            for i = 1, #tail do
                if tail[i] ~= ARGV[i + 1] then return {0, 0, {}, {}} end
            end
            if start == 0 then
                redis.call('DEL', KEYS[1])
            else
                redis.call('LTRIM', KEYS[1], 0, start - 1)
            end
            return {1, redis.call('LLEN', KEYS[1]), redis.call('LRANGE', KEYS[1], 0, 0),
                redis.call('LRANGE', KEYS[1], -1, -1)}
        ",
            )
        });
        let mut conn = get_redis_conn().await.map_err(|e| e.to_string())?;
        let (cut, length, first, last): (bool, usize, Vec<String>, Vec<String>) = CUT
            .key(RESOURCE_CHANGES_KEY)
            .arg(self.before.length)
            .arg(&self.lines)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        let after_cut = FeedState {
            length,
            ends: [first, last].concat(),
        };
        if !cut {
            // Something else wrote to the feed meanwhile: take out the bench's lines one by one
            for line in &self.lines {
                redis::cmd("LREM")
                    .arg(RESOURCE_CHANGES_KEY)
                    .arg(1)
                    .arg(line)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
        let feed: Vec<String> = redis::cmd("LRANGE")
            .arg(RESOURCE_CHANGES_KEY)
            .arg(0)
            .arg(-1)
            .query_async(&mut conn)
            .await
            .map_err(|e| e.to_string())?;
        let own: std::collections::HashSet<&String> = self.lines.iter().collect();
        let left = feed.iter().filter(|line| own.contains(line)).count();
        let (after, keys_after) = feed_state().await?;
        println!(
            "Feed before seeding {:?} ({} Redis keys), after cleanup {after:?} ({keys_after} keys)",
            self.before, self.keys_before
        );
        if left > 0 {
            return Err(format!("{left} bench lines left in the feed"));
        }
        if cut && after_cut != self.before {
            return Err(format!(
                "the feed is not what it was before seeding: {after_cut:?} != {:?}",
                self.before
            ));
        }
        Ok(())
    }
}

impl Drop for SeededFeed<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.rt.block_on(self.remove()) {
            // A panic while already unwinding would abort the bench: report instead.
            if std::thread::panicking() {
                eprintln!("The bench's feed cleanup failed while unwinding: {e}");
            } else {
                panic!("The bench must take its lines out of the feed: {e}");
            }
        }
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
