//! `GET /v1/resource/{resource_id}/replies` and `/embeds` on a Resource the bench seeds itself:
//! the mock graph has no post that references a Resource. The posts are seeded through the model
//! calls the watcher makes for a new post with an external reply or embed, and removed again
//! when the benches finish ([`seeded::Seed::finish`]).

use criterion::{BenchmarkId, Criterion};
use nexus_common::db::OperationOutcome;
use nexus_common::models::event::resource_changes::ResourceChangeKind;
use nexus_common::models::post::{PostCounts, PostDetails, PostKind, PostRelationships};
use nexus_common::models::resource::references;
use nexus_webapi::models::{v1, PostViewDetailed};
use setup::run_setup;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;
use tokio::runtime::Runtime;

mod seeded;
mod setup;

/// Authors of the seeded posts.
const AUTHORS: usize = 8;
/// Replies and, as many again, embeds of the one seeded Resource.
const POSTS: usize = 300;
/// The routes' default and maximum `limit`, and a later full page.
const PAGES: [(usize, usize); 3] = [(0, 20), (0, 100), (200, 100)];

static RT: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
/// Set before anything is written, so the cleanup also runs after a seed that failed halfway.
static SEED: OnceLock<seeded::Seed> = OnceLock::new();
/// The seeded Resource's id.
static DATA: OnceLock<String> = OnceLock::new();

/// One Resource with [`POSTS`] replies and [`POSTS`] embeds by [`AUTHORS`] authors, each post
/// written as the watcher's `sync_put_at` writes a new post: the post, its counts, relationships
/// and details, its references (from the version file it was read from, `{uri}/{id}.json`), then
/// the relationships and details again from the graph.
async fn seed(seed: &seeded::Seed) -> String {
    seed.write_users().await;
    let target = format!(
        "https://example.com/nexus-bench/references/{}",
        seed.users[0]
    );
    let at = chrono::Utc::now().timestamp_millis();
    let mut resource = None;
    for n in 0..2 * POSTS {
        let (author, id) = (&seed.users[n % AUTHORS], seed.post_id());
        let (kind, is_reply) = match n % 2 {
            0 => (ResourceChangeKind::Reply, true),
            _ => (ResourceChangeKind::Embed, false),
        };
        let uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
        let post = PostDetails {
            content: format!("bench post {n} about {target}"),
            id: id.clone(),
            indexed_at: at + n as i64,
            author: author.clone(),
            kind: PostKind::Short,
            uri: uri.clone(),
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        let outcome = post.put_to_graph(None, None).await.unwrap();
        assert!(matches!(outcome, OperationOutcome::CreatedOrDeleted));
        PostCounts::default()
            .put_to_index(author, &id, is_reply)
            .await
            .unwrap();
        PostRelationships::default()
            .put_to_index(author, &id)
            .await
            .unwrap();
        post.put_to_index(author, None, false, None).await.unwrap();
        let file = format!("{uri}/{id}.json");
        let line = references::reference(author, &file, &target, kind).unwrap();
        resource.get_or_insert_with(|| line.resource_id.clone());
        references::sync(&post, vec![line]).await.unwrap();
        PostRelationships::reindex(author, &id).await.unwrap();
        PostDetails::reindex(author, &id).await.unwrap();
    }
    resource.unwrap()
}

fn data() -> &'static str {
    let seed = SEED.get_or_init(|| RT.block_on(seeded::Seed::new(AUTHORS)));
    DATA.get_or_init(|| RT.block_on(self::seed(seed)))
}

/// The handlers' read: the Resource lookup, one page of post keys, then each post's `/v1` view.
async fn read(id: &str, kind: ResourceChangeKind, skip: usize, limit: usize) -> Vec<v1::PostView> {
    assert!(references::exists(id).await.unwrap());
    let keys = references::posts(id, kind, skip, limit).await.unwrap();
    let mut posts = Vec::with_capacity(keys.len());
    for (author, id) in keys {
        if let Some(post) = PostViewDetailed::get_by_id(&author, &id, None, None, None, false)
            .await
            .unwrap()
        {
            posts.push(v1::PostView::from(post));
        }
    }
    posts
}

fn bench_resource_references(c: &mut Criterion) {
    println!("******************************************************************************");
    println!("Benchmarking a resource's replies and embeds on {POSTS} seeded posts of each.");
    println!("******************************************************************************");

    run_setup();
    let mut group = c.benchmark_group("resource_references");
    for (name, kind) in [
        ("replies", ResourceChangeKind::Reply),
        ("embeds", ResourceChangeKind::Embed),
    ] {
        for (skip, limit) in PAGES {
            // Seeded and checked in the closure, which runs only for a selected bench, once per
            // sample batch: the page is checked the first time.
            let mut checked = false;
            group.bench_with_input(
                BenchmarkId::new(name, format!("skip{skip}_limit{limit}")),
                &(skip, limit),
                |b, &(skip, limit)| {
                    let id = data();
                    if !checked {
                        let page = RT.block_on(read(id, kind, skip, limit));
                        assert_eq!(page.len(), limit, "{name} skip={skip} limit={limit}");
                        checked = true;
                    }
                    b.to_async(&*RT).iter(|| read(id, kind, skip, limit))
                },
            );
        }
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
        bench_resource_references(&mut c);
        c.final_summary();
    });
    if let Some(seed) = SEED.get() {
        RT.block_on(seed.finish(&[
            "MATCH (r:Resource) WHERE r.uri STARTS WITH 'https://example.com/nexus-bench/references/'
               AND any(u IN $users WHERE r.uri ENDS WITH u) DETACH DELETE r",
        ]));
    }
    if let Err(panic) = ran {
        std::panic::resume_unwind(panic);
    }
}
