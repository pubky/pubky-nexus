use crate::{run_setup, streams_benches::LIMIT_20};
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{KindFilter, PostStream, StreamSource, TrustFilter};
use nexus_common::types::StreamSorting;
use pubky_app_specs::PubkyAppPostKind;
use tokio::runtime::Runtime;

/// Tagged on fixture posts by ranked authors, so its ranked set is populated.
const TAG: &str = "free";
/// Outside the trust ranking (the wot spammer): served unfiltered after the rank check.
const UNRANKED_VIEWER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";

/// TRUST-FILTERED POST KEY STREAM BENCHMARKS, next to their unfiltered
/// baselines. Bench ids are `stream_post_keys_<case>`; the untagged baseline is
/// `stream_post_keys_all_timeline`.
pub fn bench_stream_post_keys_ranked(c: &mut Criterion) {
    let anonymous = || Some(TrustFilter::default());
    let unranked = || Some(TrustFilter::for_viewer(Some(UNRANKED_VIEWER)));
    let tag = || Some(vec![TAG.to_string()]);
    let short = || Some(KindFilter::Kind(PubkyAppPostKind::Short));
    // (case, tags, kind, trust filter)
    let cases = [
        ("all_timeline_ranked", None, None, anonymous()),
        ("all_timeline_unranked_viewer", None, None, unranked()),
        ("tag_timeline_ranked", tag(), None, anonymous()),
        ("kind_timeline_ranked", None, short(), anonymous()),
        ("tag_timeline", tag(), None, None),
        ("kind_timeline", None, short(), None),
    ];

    run_setup();

    let rt = Runtime::new().unwrap();

    for (case, tags, kind, trust_filter) in cases {
        let id = format!("stream_post_keys_{case}");
        println!("******************************************************************************");
        println!("Benchmarking the post key stream with reach 'All' sorting 'Timeline': {id}.");
        println!("******************************************************************************");

        c.bench_function(&id, |b| {
            b.to_async(&rt).iter(|| async {
                let post_key_stream = PostStream::get_post_keys(
                    StreamSource::All,
                    LIMIT_20,
                    SortOrder::Descending,
                    StreamSorting::Timeline,
                    tags.clone(),
                    kind.clone(),
                    trust_filter.clone(),
                )
                .await
                .unwrap()
                .expect("expected post keys in benchmark");

                std::hint::black_box((&post_key_stream.post_keys, post_key_stream.last_post_score));
            });
        });
    }
}
