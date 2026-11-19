use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{ResourceSorting, ResourceStream};
use tokio::runtime::Runtime;

// Fixture data (docker/test-graph/mocks/resources.cypher) uses this; a
// synthetic graph should make it its most popular label so the same target
// stays meaningful at scale.
const TAG: &str = "bitcoin";

/// RESOURCE STREAM BENCHMARKS (served from the graph)
fn bench_resource_keys(
    c: &mut Criterion,
    name: &str,
    tag: Option<&'static str>,
    sorting: ResourceSorting,
) {
    println!("******************************************************************************");
    println!("Benchmarking the resource stream '{name}'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function(name, |b| {
        b.to_async(&rt).iter(|| async {
            let tags = tag.map(|t| vec![t.to_string()]);

            let keys = ResourceStream::get_resource_keys(
                LIMIT_20,
                SortOrder::Descending,
                &sorting,
                tags.as_deref(),
            )
            .await
            .unwrap();
            std::hint::black_box(keys);
        });
    });
}

pub fn bench_stream_resources_all_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_all_timeline",
        None,
        ResourceSorting::Timeline,
    );
}

pub fn bench_stream_resources_tag_timeline(c: &mut Criterion) {
    bench_resource_keys(
        c,
        "stream_resources_tag_timeline",
        Some(TAG),
        ResourceSorting::Timeline,
    );
}
