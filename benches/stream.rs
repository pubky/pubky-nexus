use criterion::{criterion_group, criterion_main, Criterion};
use pubky_nexus::models::post::{PostStream, PostStreamSorting, ViewerStreamSource};
use pubky_nexus::models::user::{UserStream, UserStreamType};
use setup::run_setup;
use std::time::Duration;
use tokio::runtime::Runtime;

mod setup;

/// POST STREAM BENCHMARKS

fn bench_stream_followers_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::Timeline,
                ViewerStreamSource::Following,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_following_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::Timeline,
                ViewerStreamSource::Followers,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_friends_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::Timeline,
                ViewerStreamSource::Friends,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_all_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                None,
                PostStreamSorting::Timeline,
                ViewerStreamSource::All,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_bookmarks_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'Timeline'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::Timeline,
                ViewerStreamSource::Bookmarks,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_followers_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Followers' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_followers_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Followers,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_following_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Following' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_following_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Following,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_friends_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Friends' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_friends_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Friends,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_all_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'All' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_all_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                None,
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::All,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_bookmarks_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with reach 'Bookmarks' sorting 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let viewer_id = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_bookmarks_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                Some(viewer_id.to_string()),
                None,
                PostStreamSorting::TotalEngagement,
                ViewerStreamSource::Bookmarks,
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_author_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams for author_id sorted by 'Timeline'.");
    println!("***************************************");

    run_setup();

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                Some(author_id.to_string()), // Filter by author_id
                PostStreamSorting::Timeline, // Sort by timeline
                ViewerStreamSource::All, // No reach filter, as we are only interested in the author
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_author_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams for author_id sorted by 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let author_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_author_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                Some(author_id.to_string()),        // Filter by author_id
                PostStreamSorting::TotalEngagement, // Sort by total engagement
                ViewerStreamSource::All, // No reach filter, as we are only interested in the author
                None,
                None,
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_tag_timeline(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'Timeline'.");
    println!("***************************************");

    run_setup();

    let tag_label = "free"; // Tag to filter by
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_timeline", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                None,                              // No author_id filter
                PostStreamSorting::Timeline,       // Sort by timeline
                ViewerStreamSource::All,           // No reach filtering
                Some(vec![tag_label.to_string()]), // Filter by tag label
                Some(0),
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

fn bench_stream_tag_total_engagement(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the post streams with tag 'free' sorted by 'TotalEngagement'.");
    println!("***************************************");

    run_setup();

    let tag_label = "free"; // Tag to filter by
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_posts_tag_total_engagement", |b| {
        b.to_async(&rt).iter(|| async {
            let post_stream = PostStream::get_posts(
                None,
                None,                               // No author_id filter
                PostStreamSorting::TotalEngagement, // Sort by total engagement
                ViewerStreamSource::All,            // No reach filtering
                Some(vec![tag_label.to_string()]),  // Filter by tag label
                Some(0),
                Some(20),
            )
            .await
            .unwrap();
            criterion::black_box(post_stream);
        });
    });
}

/// USER STREAMS BENCHMARKS

fn bench_stream_most_followed(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for most followed users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_most_followed", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream =
                UserStream::get_by_id("", None, None, Some(20), UserStreamType::MostFollowed)
                    .await
                    .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_pioneers(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams for pioneer users.");
    println!("***************************************");

    run_setup();

    let rt = Runtime::new().unwrap();

    c.bench_function("stream_pioneers", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream =
                UserStream::get_by_id("", None, None, Some(20), UserStreamType::MostFollowed)
                    .await
                    .unwrap();
            criterion::black_box(user_stream);
        });
    });
}

fn bench_stream_users_by_username_search(c: &mut Criterion) {
    println!("***************************************");
    println!("Benchmarking the user streams by username search.");
    println!("***************************************");

    run_setup();

    let username = "An"; // Match all anonymous profiles
    let rt = Runtime::new().unwrap();

    c.bench_function("stream_users_by_username_search", |b| {
        b.to_async(&rt).iter(|| async {
            let user_stream = UserStream::get_from_username_search(
                username,
                None,
                None,
                Some(40), // Limit to 40 results
            )
            .await
            .unwrap();
            criterion::black_box(user_stream);
        });
    });
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
    targets = bench_stream_followers_timeline,
              bench_stream_following_timeline,
              bench_stream_friends_timeline,
              bench_stream_all_timeline,
              bench_stream_bookmarks_timeline,
              bench_stream_followers_total_engagement,
              bench_stream_following_total_engagement,
              bench_stream_friends_total_engagement,
              bench_stream_all_total_engagement,
              bench_stream_bookmarks_total_engagement,
              bench_stream_author_timeline,
              bench_stream_author_total_engagement,
              bench_stream_tag_timeline,
              bench_stream_tag_total_engagement,
              bench_stream_most_followed,
              bench_stream_pioneers,
              bench_stream_users_by_username_search
}

criterion_main!(benches);
