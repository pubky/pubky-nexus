use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use nexus_common::models::bootstrap::{Bootstrap, ViewType};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_bootstrap_user(c: &mut Criterion) {
    println!("***************************************************************");
    println!("Test the performance of retrieving the user’s Bootstrap view upon sign-in");
    println!("***************************************************************");

    run_setup();

    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("user_bootstrap_handler", user_id),
        &user_id,
        |b, &id| {
            b.to_async(&rt).iter(|| async {
                let user = Bootstrap::get_by_id(id, ViewType::Full).await.unwrap();
                std::hint::black_box(user);
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
    targets = bench_bootstrap_user
}

criterion_main!(benches);
