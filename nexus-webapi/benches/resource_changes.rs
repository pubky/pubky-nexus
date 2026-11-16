use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::models::event::resource_changes::ResourceChange;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

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

fn bench_resource_changes(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the resource changes feed.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    let (_, _, length) = rt.block_on(ResourceChange::get_from_index(0, 0)).unwrap();
    assert!(
        length > 0,
        "seed the feed first: the resource changes seed job, or a watcher run"
    );
    println!("The feed holds {length} lines.");

    let mut group = c.benchmark_group("resource_changes");
    // The route's maximum page and its default
    for limit in [1000, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(limit), &limit, |b, &limit| {
            b.to_async(&rt).iter(|| read_feed(limit));
        });
    }
    group.finish();
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
