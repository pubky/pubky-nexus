use criterion::{criterion_group, criterion_main, Criterion};
use nexus_common::models::event::Event;
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

fn bench_events(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the events function.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("events", |b| {
        b.to_async(&rt).iter(|| async {
            let (mut total_result, mut cursor) =
                Event::get_events_from_redis(None, 500).await.unwrap();
            let mut current_result = total_result.clone();

            while !current_result.is_empty() {
                let (next_result, next_cursor) = Event::get_events_from_redis(Some(cursor), 500)
                    .await
                    .unwrap();
                cursor = next_cursor;
                total_result.extend(next_result.iter().cloned());
                current_result = next_result;
            }

            std::hint::black_box(total_result);
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
    targets = bench_events,
}

criterion_main!(benches);
