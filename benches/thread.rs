use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::{
    models::post::PostThread,
    routes::v0::post::ThreadQuery, types::Pagination,
};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_thread_retrieval(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the thread retrieval.");
    println!("***************************************");

    run_setup();

    let author_id = "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio";
    let post_id = "0RE51NMRZAQG";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("thread_retrieval", post_id),
        &(author_id, post_id),
        |b, &(author_id, post_id)| {
            b.to_async(&rt).iter(|| async {
                let params = ThreadQuery {
                    viewer_id: None,
                    depth: Some(3),
                    pagination: Pagination {
                        skip: Some(0),
                        limit: Some(10),
                        start: None,
                        end: None,
                    },
                };
                let thread = PostThread::get_by_id(author_id, post_id, params)
                    .await
                    .unwrap();
                criterion::black_box(thread);
            });
        },
    );
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(10, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets = bench_thread_retrieval,
}

criterion_main!(benches);
