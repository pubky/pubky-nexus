use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::user::UserSearch;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_user_search(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user search.");
    println!("***************************************");

    run_setup();

    let username = "a"; // Will match the anonymous users
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("user_search", username),
        &username,
        |b, &username| {
            b.to_async(&rt).iter(|| async {
                let result = UserSearch::get_by_name(username, None, Some(40))
                    .await
                    .unwrap();
                criterion::black_box(result);
            });
        },
    );
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = benches;
    config = configure_criterion();
    targets =   bench_user_search
}

criterion_main!(benches);
