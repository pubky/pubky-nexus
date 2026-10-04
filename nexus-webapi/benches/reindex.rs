use criterion::{criterion_group, criterion_main, Criterion};
use nexus_common::db::reindex;
use nexus_common::models::post::PostStream;
use nexus_webapi::mock::MockDb;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_reindex(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the reindex function.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("reindex", |b| {
        b.to_async(&rt).iter(|| async {
            MockDb::drop_cache().await;
            reindex::sync().await;
        });
    });
}

/// The ranked timeline sets alone, as the trust job rebuilds them after every
/// ranking publish. `reindex::sync` ends with the same rebuild.
fn bench_ranked_rebuild(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking a full rebuild of the ranked timeline sets.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("ranked_rebuild", |b| {
        b.to_async(&rt).iter(|| async {
            PostStream::rebuild_ranked_sets().await.unwrap();
        });
    });
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(40, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_reindex, bench_ranked_rebuild,
}

criterion_main!(benches);
