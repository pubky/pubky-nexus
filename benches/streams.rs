use criterion::{criterion_group, criterion_main, Criterion};
use std::time::Duration;
use streams_benches::{author, bookmarks, reach, sorting, tag, user};

mod setup;
mod streams_benches;
pub use setup::run_setup;

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(100)
        .warm_up_time(Duration::new(1, 0))
}

criterion_group! {
    name = streams;
    config = configure_criterion();
    targets = bookmarks::bench_stream_bookmarks_timeline,
              bookmarks::bench_stream_bookmarks_total_engagement,
              author::bench_stream_author_timeline,
              author::bench_stream_author_total_engagement,
              author::bench_stream_author_replies_timeline,
              reach::bench_stream_followers_timeline,
              reach::bench_stream_following_timeline,
              reach::bench_stream_friends_timeline,
              reach::bench_stream_followers_total_engagement,
              reach::bench_stream_following_total_engagement,
              reach::bench_stream_friends_total_engagement,
              sorting::bench_stream_all_timeline,
              sorting::bench_stream_all_total_engagement,
              tag::bench_stream_tag_timeline,
              tag::bench_stream_tag_total_engagement,
              user::bench_stream_users_by_username_search,
              user::bench_stream_pioneers,
              user::bench_stream_following,
              user::bench_stream_most_followed,
}

criterion_main!(streams);
