//! The social event routes on data the bench seeds itself: the mock graph has no events.
//! `GET /v1/stream/events`, `GET /v1/event/{author}/{id}`, its `/attendees`,
//! `GET /v1/stream/availability` and `GET /v1/availability/{author}`, each through the model
//! functions its handler calls. The posts are seeded through the model calls the watcher makes
//! for a new `social/v1` post, the projection included, and removed again when the benches
//! finish ([`seeded::Seed::finish`]).

use chrono::{NaiveDateTime, TimeDelta, Utc};
use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, BenchmarkId, Criterion};
use nexus_common::db::kv::ScoreAction;
use nexus_common::db::OperationOutcome;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::{
    PostCounts, PostDetails, PostKind, PostRelationships, PostStream,
};
use nexus_common::models::resource::references;
use nexus_common::models::social_event::{
    self, calendar, EventQuery, EventViewOptions, SocialContent,
};
use nexus_common::models::ResourceKey;
use nexus_webapi::models::v1;
use pubky_social_specs::PubkyId;
use serde_json::json;
use setup::run_setup;
use std::future::Future;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;

mod seeded;
mod setup;

/// Users who answer the main event, and the first [`SCHEDULES`] of them post a schedule.
const ATTENDEES: usize = 400;
/// Weekly series by the host, of [`WEEKS`] occurrences each.
const SERIES: usize = 8;
const WEEKS: usize = 50;
/// Schedules for one shared room, and the host's schedules for as many rooms.
const SCHEDULES: usize = 210;
/// The paged routes' default and maximum `limit`, and a later page.
const PAGES: [(usize, usize); 3] = [(0, 20), (0, 200), (200, 200)];

static RT: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
/// Set before anything is written, so the cleanup also runs after a seed that failed halfway.
static SEED: OnceLock<seeded::Seed> = OnceLock::new();
static DATA: OnceLock<Data> = OnceLock::new();

struct Data {
    /// The author of every event.
    host: String,
    /// The main event's id.
    event: String,
    /// The local start of the main event's second occurrence, its recurrence id.
    second: String,
    /// The room every attendee's schedule is for.
    room: String,
}

/// A new post as the watcher's `sync_put_at` writes a `social/v1` one: the post, its counts, the
/// reply parent's counts, score, reply set and notification, the post's relationships and
/// details, its (empty) external references, both again from the graph, then the projection.
async fn put(post: &PostDetails, parent: Option<(&ResourceKey, &str)>, value: &SocialContent) {
    let (author, id) = (&post.author, &post.id);
    let outcome = post
        .put_to_graph(parent.map(|(key, _)| key), None)
        .await
        .unwrap();
    assert!(matches!(outcome, OperationOutcome::CreatedOrDeleted));
    PostCounts::default()
        .put_to_index(author, id, parent.is_some())
        .await
        .unwrap();
    if let Some((key, uri)) = parent {
        let parent_key: &[&str; 2] = &[&key.owner_id, &key.resource_id];
        PostCounts::invalidate(parent_key).await.unwrap();
        PostStream::update_index_score(
            &key.owner_id,
            &key.resource_id,
            ScoreAction::Increment(1.0),
        )
        .await
        .unwrap();
        PostStream::add_to_post_reply_sorted_set(parent_key, author, id, post.indexed_at)
            .await
            .unwrap();
        Notification::new_post_reply(author, uri, &post.uri, &key.owner_id, PostKind::Event)
            .await
            .unwrap();
    }
    PostRelationships {
        replied: parent.map(|(_, uri)| uri.to_string()),
        ..Default::default()
    }
    .put_to_index(author, id)
    .await
    .unwrap();
    let parent_ids = parent.map(|(key, _)| (key.owner_id.to_string(), key.resource_id.clone()));
    post.put_to_index(author, parent_ids, false, None)
        .await
        .unwrap();
    references::sync(post, vec![]).await.unwrap();
    PostRelationships::reindex(author, id).await.unwrap();
    PostDetails::reindex(author, id).await.unwrap();
    social_event::sync_with_parent(
        author,
        id,
        &post.content,
        parent.map(|(_, uri)| uri),
        parent.map(|(key, _)| key),
        value,
    )
    .await
    .unwrap();
}

fn details(seed: &seeded::Seed, author: &str, kind: PostKind, content: String) -> PostDetails {
    let id = seed.post_id();
    PostDetails {
        uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
        author: author.to_string(),
        id,
        content,
        kind,
        indexed_at: Utc::now().timestamp_millis(),
        ..Default::default()
    }
}

/// [`SERIES`] weekly series by the host, each with a capacity and a waitlist; one answer to the
/// first series by each of the [`ATTENDEES`], cycling through the statuses, every tenth also
/// answering its second occurrence; one schedule each by the first [`SCHEDULES`] attendees for
/// one room, and the host's schedules for [`SCHEDULES`] other rooms.
async fn seed(seed: &seeded::Seed) -> Data {
    seed.write_users().await;
    let host = seed.users[0].clone();
    let tomorrow = (Utc::now() + TimeDelta::days(1)).date_naive();
    let mut main = None;
    for n in 0..SERIES {
        let start: NaiveDateTime = tomorrow.and_hms_opt(8 + n as u32, 0, 0).unwrap();
        let content = json!({
            "title": format!("Bench series {n}"),
            "description": "A weekly meetup the social events bench seeds",
            "start": start.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "timeZone": "UTC",
            "duration": "PT1H",
            "recurrenceRules": [{"frequency": "weekly", "count": WEEKS}],
            "attendance": {"policy": "open", "capacity": 300, "waitlist": true},
            "locations": [{"uri": format!("https://example.com/nexus-bench/venue/{host}/{n}")}],
        })
        .to_string();
        let post = details(seed, &host, PostKind::Event, content);
        let value = SocialContent::read(PostKind::Event, &post.content, Utc::now()).unwrap();
        put(&post, None, &value).await;
        if n == 0 {
            main = Some((
                post.id.clone(),
                post.uri.clone(),
                start + TimeDelta::days(7),
            ));
        }
    }
    let (event, event_uri, second) = main.unwrap();
    let second = second.format("%Y-%m-%dT%H:%M:%S").to_string();
    let key = ResourceKey {
        owner_id: PubkyId::try_from(host.as_str()).unwrap(),
        resource_id: event.clone(),
    };
    let statuses = [
        "accepted",
        "accepted",
        "accepted",
        "tentative",
        "declined",
        "requested",
    ];
    for (n, attendee) in seed.users[1..].iter().enumerate() {
        let mut answers = vec![json!({"status": statuses[n % statuses.len()], "guests": n % 3})];
        if n % 10 == 0 {
            answers.push(json!({"status": "declined", "recurrenceId": second}));
        }
        for answer in answers {
            let post = details(seed, attendee, PostKind::Rsvp, answer.to_string());
            let value = SocialContent::read(PostKind::Rsvp, &post.content, Utc::now()).unwrap();
            put(&post, Some((&key, &event_uri)), &value).await;
        }
    }
    let room = |n: usize| format!("https://example.com/nexus-bench/room/{host}/{n}");
    let schedules = seed.users[1..=SCHEDULES]
        .iter()
        .map(|author| (author.clone(), room(0)))
        .chain((1..=SCHEDULES).map(|n| (host.clone(), room(n))));
    for (author, target) in schedules {
        let content = json!({"timeZone": "UTC", "horizonDays": 30, "for": target}).to_string();
        let post = details(seed, &author, PostKind::Availability, content);
        let value = SocialContent::read(PostKind::Availability, &post.content, Utc::now()).unwrap();
        put(&post, None, &value).await;
    }
    Data {
        room: room(0),
        host,
        event,
        second,
    }
}

fn data() -> &'static Data {
    let seed = SEED.get_or_init(|| RT.block_on(seeded::Seed::new(1 + ATTENDEES)));
    DATA.get_or_init(|| RT.block_on(self::seed(seed)))
}

/// `GET /v1/stream/events` without filters: now through the default window.
async fn stream(skip: usize, limit: usize) -> usize {
    let now = Utc::now();
    social_event::stream(EventQuery {
        from: now.timestamp_millis(),
        to: calendar::window_end(now),
        skip,
        limit,
        ..Default::default()
    })
    .await
    .unwrap()
    .len()
}

/// `GET /v1/event/{author}/{id}` with the given attendee, tag and tagger limits, as the `/v1`
/// view. The seeded event has no tags, so the tag limits change nothing here: the post benches
/// measure the tag reads.
async fn view(data: &Data, attendees: usize, tags: usize) -> usize {
    let options = EventViewOptions {
        viewer: None,
        tags,
        taggers: tags,
        attendees,
        occurrences: 50,
    };
    let view = social_event::event_view_with_options(&data.host, &data.event, None, options)
        .await
        .unwrap()
        .unwrap();
    let view = v1::EventView::from(view);
    assert_eq!(view.occurrences.len(), 50);
    view.attendees.len()
}

/// `GET /v1/event/{author}/{id}/attendees`: the event lookup, then a page of attendees.
async fn attendees(data: &Data, recurrence: Option<&str>, skip: usize, limit: usize) -> usize {
    let uri = social_event::event_uri(&data.host, &data.event)
        .await
        .unwrap()
        .unwrap();
    social_event::attendees(&uri, recurrence, None, skip, limit)
        .await
        .unwrap()
        .len()
}

/// `GET /v1/stream/availability?for=` and `GET /v1/availability/{author}`.
async fn availability(author: Option<&str>, target: Option<&str>, limit: usize) -> usize {
    social_event::availability_stream(author, target, 0, limit)
        .await
        .unwrap()
        .len()
}

/// Benches `routine` on the seeded data as `function/parameter`, after checking once that it
/// returns `rows` rows. The seed runs here, so only for a selected bench.
fn bench<F, Fut>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    function: &str,
    parameter: String,
    rows: usize,
    routine: F,
) where
    F: Fn(&'static Data) -> Fut,
    Fut: Future<Output = usize>,
{
    let mut checked = false;
    let id = BenchmarkId::new(function, &parameter);
    group.bench_function(id, |b| {
        let data = data();
        if !checked {
            let got = RT.block_on(routine(data));
            assert_eq!(got, rows, "{function}/{parameter}");
            checked = true;
        }
        b.to_async(&*RT).iter(|| routine(data))
    });
}

fn bench_social_events(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking the social event routes on seeded events, RSVPs and schedules.");
    println!("******************************************************************************");

    run_setup();
    let mut group = c.benchmark_group("social_events");
    for (skip, limit) in PAGES {
        let page = format!("skip{skip}_limit{limit}");
        bench(&mut group, "stream_events", page, limit, move |_| {
            stream(skip, limit)
        });
    }
    for (name, limit, tags) in [("default", 20, 10), ("max", 200, 100)] {
        bench(&mut group, "event_view", name.into(), limit, move |data| {
            view(data, limit, tags)
        });
    }
    for (skip, limit) in PAGES {
        let page = format!("skip{skip}_limit{limit}");
        bench(&mut group, "event_attendees", page, limit, move |data| {
            attendees(data, None, skip, limit)
        });
    }
    let page = "occurrence_skip0_limit20".to_string();
    bench(&mut group, "event_attendees", page, 20, |data| {
        attendees(data, Some(&data.second), 0, 20)
    });
    for limit in [20, 200] {
        let page = format!("skip0_limit{limit}");
        bench(
            &mut group,
            "stream_availability_for",
            page.clone(),
            limit,
            move |data| availability(None, Some(&data.room), limit),
        );
        bench(
            &mut group,
            "author_availability",
            page,
            limit,
            move |data| availability(Some(&data.host), None, limit),
        );
    }
    group.finish();
}

fn configure_criterion() -> Criterion {
    Criterion::default()
        .measurement_time(Duration::new(5, 0))
        .sample_size(20)
        .warm_up_time(Duration::new(1, 0))
        .configure_from_args()
}

/// `criterion_main!` with the cleanup after the benches, also when one or the seed panics (the
/// panic is printed when it happens, and raised again after the cleanup).
fn main() {
    let ran = std::panic::catch_unwind(|| {
        let mut c = configure_criterion();
        bench_social_events(&mut c);
        c.final_summary();
    });
    if let Some(seed) = SEED.get() {
        RT.block_on(seed.finish(&[
            "MATCH (n) WHERE (n:EventSeries OR n:EventOccurrence OR n:EventNotice)
               AND any(u IN $users WHERE coalesce(n.family, n.uri) STARTS WITH 'pubky://' + u + '/')
             DETACH DELETE n",
            "MATCH (r:Resource) WHERE r.uri STARTS WITH 'https://example.com/nexus-bench/venue/'
               AND any(u IN $users WHERE r.uri CONTAINS u) DETACH DELETE r",
        ]));
    }
    if let Err(panic) = ran {
        std::panic::resume_unwind(panic);
    }
}
