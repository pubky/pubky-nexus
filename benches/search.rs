use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use pubky_nexus::models::{post::PostStreamSorting, tag::search::TagSearch, user::UserSearch};
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

fn bench_tag_search_by_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the tag search by timeline");
    println!("***************************************");

    run_setup();

    let label = "free";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("tag_search_by_timeline", label),
        &label,
        |b, &label| {
            b.to_async(&rt).iter(|| async {
                let result = TagSearch::get_by_label(label, None, None, None, None)
                    .await
                    .unwrap();
                criterion::black_box(result);
            });
        },
    );
}

fn bench_tag_search_by_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the tag search by engagement");
    println!("***************************************");

    run_setup();

    let label = "free";
    let rt = Runtime::new().unwrap();

    c.bench_with_input(
        BenchmarkId::new("tag_search_by_engagement", label),
        &label,
        |b, &label| {
            b.to_async(&rt).iter(|| async {
                let result = TagSearch::get_by_label(label, Some(PostStreamSorting::TotalEngagement), None, None, None)
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
    targets =   bench_user_search,
                bench_tag_search_by_timeline,
                bench_tag_search_by_engagement
}

criterion_main!(benches);
