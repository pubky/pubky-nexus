use crate::run_setup;
use crate::streams_benches::LIMIT_20;
use criterion::Criterion;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::post::{PostStream, StreamSource};
use nexus_common::types::{DomainTrust, StreamSorting, WotDepth};
use tokio::runtime::Runtime;

const TAG: &str = "free";

// Real users from the seeded skunk graph. The reach observer is the same as in
// the reach and wot benches, and the bookmarks observer and author match the
// bookmarks and author benches. The endorser observes the `Me` domain stream:
// the reach observer has endorsed no users, so its `Me` stream is empty.
const OBSERVER_ID: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const BOOKMARKS_OBSERVER_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
const AUTHOR_ID: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
const ENDORSER_ID: &str = AUTHOR_ID;
// Not the wot bench's `bitcoin` and `dev` domain tags: no author endorsed with
// those wrote a `bitcoin`-tagged post, so both domain streams would be empty.
// Authors endorsed as `bot` wrote 110, reached through the endorser's own
// endorsements (`Me`) and through the observer's network (depth 2).
const DOMAIN_TAG: &str = "bot";
// Tagged on posts in every source below: 3 in `following`, 113 in `wot_2`.
const REACH_TAG: &str = "bitcoin";

fn wot(depth: u8) -> StreamSource {
    StreamSource::Wot {
        observer_id: OBSERVER_ID.to_string(),
        depth: WotDepth::new(depth).expect("bench depth must be in range"),
    }
}

fn run_tag_stream(
    label: &str,
    description: &str,
    source: StreamSource,
    sorting: StreamSorting,
    tags: Vec<&str>,
    c: &mut Criterion,
) {
    println!("******************************************************************************");
    println!("Benchmarking the post streams with {description} sorting '{sorting:?}'.");
    println!("******************************************************************************");

    run_setup();

    let rt = Runtime::new().unwrap();
    let tags: Vec<String> = tags.into_iter().map(String::from).collect();

    c.bench_function(label, |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                source.clone(),
                LIMIT_20,
                SortOrder::Descending,
                sorting.clone(),
                None,
                Some(tags.clone()),
                None,
            )
            .await
            .unwrap();
            std::hint::black_box(post_stream);
        });
    });
}

/// TAG RELATED POST STREAMS BENCHMARKS
pub fn bench_stream_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::All;
    run_tag_stream(
        "stream_posts_tag_timeline",
        "tag 'free'",
        source,
        StreamSorting::Timeline,
        vec![TAG],
        c,
    );
}

pub fn bench_stream_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::All;
    run_tag_stream(
        "stream_posts_tag_total_engagement",
        "tag 'free'",
        source,
        StreamSorting::TotalEngagement,
        vec![TAG],
        c,
    );
}

// A single tag on `All` (the benches above) is served from that tag's sorted
// set. Two or more tags are not, so filtering by several tags is its own case.
pub fn bench_stream_multi_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::All;
    run_tag_stream(
        "stream_posts_multi_tag_timeline",
        "tags 'bitcoin' and 'fees'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG, "fees"],
        c,
    );
}

pub fn bench_stream_multi_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::All;
    run_tag_stream(
        "stream_posts_multi_tag_total_engagement",
        "tags 'bitcoin' and 'fees'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG, "fees"],
        c,
    );
}

pub fn bench_stream_following_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::Following {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_following_tag_timeline",
        "reach 'Following' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_following_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::Following {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_following_tag_total_engagement",
        "reach 'Following' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_followers_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::Followers {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_followers_tag_timeline",
        "reach 'Followers' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_followers_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::Followers {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_followers_tag_total_engagement",
        "reach 'Followers' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_friends_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::Friends {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_friends_tag_timeline",
        "reach 'Friends' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_friends_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::Friends {
        observer_id: OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_friends_tag_total_engagement",
        "reach 'Friends' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth1_tag_timeline(c: &mut Criterion) {
    let source = wot(1);
    run_tag_stream(
        "stream_posts_wot_depth1_tag_timeline",
        "source 'Wot' depth 1 and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth1_tag_total_engagement(c: &mut Criterion) {
    let source = wot(1);
    run_tag_stream(
        "stream_posts_wot_depth1_tag_total_engagement",
        "source 'Wot' depth 1 and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth2_tag_timeline(c: &mut Criterion) {
    let source = wot(2);
    run_tag_stream(
        "stream_posts_wot_depth2_tag_timeline",
        "source 'Wot' depth 2 and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth2_tag_total_engagement(c: &mut Criterion) {
    let source = wot(2);
    run_tag_stream(
        "stream_posts_wot_depth2_tag_total_engagement",
        "source 'Wot' depth 2 and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth3_tag_timeline(c: &mut Criterion) {
    let source = wot(3);
    run_tag_stream(
        "stream_posts_wot_depth3_tag_timeline",
        "source 'Wot' depth 3 and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_depth3_tag_total_engagement(c: &mut Criterion) {
    let source = wot(3);
    run_tag_stream(
        "stream_posts_wot_depth3_tag_total_engagement",
        "source 'Wot' depth 3 and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_bookmarks_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::Bookmarks {
        observer_id: BOOKMARKS_OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_bookmarks_tag_timeline",
        "source 'Bookmarks' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_bookmarks_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::Bookmarks {
        observer_id: BOOKMARKS_OBSERVER_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_bookmarks_tag_total_engagement",
        "source 'Bookmarks' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_domain_me_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::WotDomain {
        observer_id: ENDORSER_ID.to_string(),
        trust: DomainTrust::Me,
        domain_tags: vec![DOMAIN_TAG.to_string()],
    };
    run_tag_stream(
        "stream_posts_wot_domain_me_tag_timeline",
        "source 'WotDomain' trust 'Me' with domain tag 'bot' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_domain_me_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::WotDomain {
        observer_id: ENDORSER_ID.to_string(),
        trust: DomainTrust::Me,
        domain_tags: vec![DOMAIN_TAG.to_string()],
    };
    run_tag_stream(
        "stream_posts_wot_domain_me_tag_total_engagement",
        "source 'WotDomain' trust 'Me' with domain tag 'bot' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_domain_depth2_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::WotDomain {
        observer_id: OBSERVER_ID.to_string(),
        trust: DomainTrust::Network(WotDepth::default()),
        domain_tags: vec![DOMAIN_TAG.to_string()],
    };
    run_tag_stream(
        "stream_posts_wot_domain_depth2_tag_timeline",
        "source 'WotDomain' depth 2 with domain tag 'bot' and tag 'bitcoin'",
        source,
        StreamSorting::Timeline,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_wot_domain_depth2_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::WotDomain {
        observer_id: OBSERVER_ID.to_string(),
        trust: DomainTrust::Network(WotDepth::default()),
        domain_tags: vec![DOMAIN_TAG.to_string()],
    };
    run_tag_stream(
        "stream_posts_wot_domain_depth2_tag_total_engagement",
        "source 'WotDomain' depth 2 with domain tag 'bot' and tag 'bitcoin'",
        source,
        StreamSorting::TotalEngagement,
        vec![REACH_TAG],
        c,
    );
}

pub fn bench_stream_author_tag_timeline(c: &mut Criterion) {
    let source = StreamSource::Author {
        author_id: AUTHOR_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_author_tag_timeline",
        "source 'Author' and tag 'pkarr'",
        source,
        StreamSorting::Timeline,
        vec!["pkarr"],
        c,
    );
}

pub fn bench_stream_author_tag_total_engagement(c: &mut Criterion) {
    let source = StreamSource::Author {
        author_id: AUTHOR_ID.to_string(),
    };
    run_tag_stream(
        "stream_posts_author_tag_total_engagement",
        "source 'Author' and tag 'pkarr'",
        source,
        StreamSorting::TotalEngagement,
        vec!["pkarr"],
        c,
    );
}
