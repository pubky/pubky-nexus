use criterion::{criterion_group, criterion_main, Criterion};
use nexus_common::models::event::{author_of_line, EventLine};
use setup::run_setup;
use std::collections::HashSet;
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
                EventLine::get_from_index(None, 500).await.unwrap();
            let mut current_result = total_result.clone();

            while !current_result.is_empty() {
                let (next_result, next_cursor) =
                    EventLine::get_from_index(Some(cursor), 500).await.unwrap();
                cursor = next_cursor;
                total_result.extend(next_result.iter().cloned());
                current_result = next_result;
            }

            std::hint::black_box(total_result);
        });
    });
}

fn bench_events_filtered(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the events filtered by author.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    let (first_page, _) = rt.block_on(EventLine::get_from_index(None, 1)).unwrap();
    let log_len = rt.block_on(EventLine::log_state()).unwrap().len;
    println!("Event log length: {log_len}");

    // Worst case per request: an author with no lines scans the whole budget.
    let absent = HashSet::from(["absent-author".to_string()]);
    c.bench_function("events_filtered_full_scan", |b| {
        b.to_async(&rt).iter(|| async {
            let page = EventLine::get_filtered_from_index(None, 500, &absent)
                .await
                .unwrap();
            std::hint::black_box(page);
        });
    });

    // Paging one real author (the author of the first line) to the end of the log.
    let Some(author) = first_page.first().and_then(|l| author_of_line(l)) else {
        println!("Event log is empty: skipping events_filtered_one_author");
        return;
    };
    let one = HashSet::from([author.to_string()]);
    c.bench_function("events_filtered_one_author", |b| {
        b.to_async(&rt).iter(|| async {
            let mut cursor = 0;
            let mut lines = Vec::new();
            loop {
                let page = EventLine::get_filtered_from_index(Some(cursor), 500, &one)
                    .await
                    .unwrap();
                lines.extend(page.lines);
                if page.next_cursor == cursor {
                    break;
                }
                cursor = page.next_cursor;
            }
            std::hint::black_box(lines);
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
    targets = bench_events, bench_events_filtered,
}

criterion_main!(benches);
