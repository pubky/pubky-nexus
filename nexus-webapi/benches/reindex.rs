use criterion::{criterion_group, criterion_main, Criterion};
use nexus_common::db::reindex;
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

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(40, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_reindex,
}

criterion_main!(benches);
