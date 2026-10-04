use super::*;

#[test]
fn decide_covers_every_state() {
    assert_eq!(TrustMode::decide(false, false), TrustMode::Off);
    assert_eq!(TrustMode::decide(false, true), TrustMode::Off);
    assert_eq!(TrustMode::decide(true, true), TrustMode::Ranked);
    assert_eq!(TrustMode::decide(true, false), TrustMode::Off);
}

#[test]
fn keys_match_the_key_parts_readers_use() {
    assert_eq!(ranking_key(), "Sorted:Users:SocialGraph");
    let global = RankedSet::global();
    assert_eq!(global.source, "Sorted:Posts:Global:Timeline");
    assert_eq!(global.ranked, "Sorted:Posts:Ranked:Timeline");
    let tag = RankedSet::tag("bitcoin");
    assert_eq!(tag.source, "Sorted:Tags:Global:Post:Timeline:bitcoin");
    assert_eq!(tag.ranked, "Sorted:Tags:Ranked:Post:Timeline:bitcoin");
    // A scan or glob for one family of keys must never match another.
    let families = [global.source, global.ranked, tag.source, tag.ranked];
    for (i, a) in families.iter().enumerate() {
        for (j, b) in families.iter().enumerate() {
            let prefix = b.trim_end_matches("bitcoin");
            assert!(i == j || !a.starts_with(prefix), "{a} starts with {prefix}");
        }
    }
}

/// Live tests against the shared Redis and its fixture ranking. Each test
/// writes only its own labels and authors, and clears them before and
/// after. A full rebuild and the ranking key are shared, so
/// `.config/nextest.toml` runs each of these alone.
mod live {
    use std::collections::BTreeSet;

    use redis::AsyncCommands;

    use pubky_app_specs::PubkyAppPostKind;

    use super::super::rebuild::{rebuild_set, reconcile_page, unlink_keys, RankedRebuildStats};
    use super::super::*;
    use crate::db::kv::{RedisOps, SortOrder};
    use crate::models::post::{KindFilter, PostStream, StreamSource, TrustFilter};
    use crate::types::{DynError, Pagination, StreamSorting};
    use crate::{StackConfig, StackManager};

    type TestResult = Result<(), DynError>;

    const ALICE: &str = "test-ranked-alice";
    const BOB: &str = "test-ranked-bob";
    const CAROL: &str = "test-ranked-carol";
    /// Where a test parks the ranking while it checks behaviour without one.
    const RANKING_ASIDE: &str = "Test:Ranked:RankingAside";
    /// The fixture's wot window (`indexed_at`), used by no other fixture: see
    /// nexus-webapi's tests/stream/post/ranked.rs.
    const WOT_WINDOW_START: f64 = 1650000000014.0;
    const WOT_WINDOW_END: f64 = 1650000000001.0;

    /// Connects and clears what an earlier run may have left behind,
    /// including a ranking it parked and never restored.
    async fn setup(labels: &[&str]) -> TestResult {
        StackManager::setup(&StackConfig::default()).await?;
        let mut conn = get_redis_conn().await?;
        let parked: bool = conn.exists(RANKING_ASIDE).await?;
        if parked {
            restore_ranking().await?;
        }
        cleanup(labels).await
    }

    /// Clears the sets of `labels` and the test authors' ranks.
    async fn cleanup(labels: &[&str]) -> TestResult {
        let mut conn = get_redis_conn().await?;
        let keys = labels.iter().flat_map(|label| {
            let set = RankedSet::tag(label);
            [set.source, set.ranked]
        });
        unlink_keys(&mut conn, keys).await?;
        let authors = [ALICE, BOB, CAROL];
        PostStream::remove_from_index_sorted_set(None, &USER_SOCIAL_GRAPH_KEY_PARTS, &authors)
            .await?;
        Ok(())
    }

    /// Ranks `authors` below every fixture user, so fixture ranks are untouched.
    async fn rank(authors: &[&str]) -> TestResult {
        let entries: Vec<(f64, &str)> = authors.iter().map(|author| (1e9, *author)).collect();
        PostStream::put_index_sorted_set(&USER_SOCIAL_GRAPH_KEY_PARTS, &entries, None, None)
            .await?;
        Ok(())
    }

    async fn zadd(key: &str, entries: &[(f64, &str)]) -> TestResult {
        let mut conn = get_redis_conn().await?;
        let _: () = conn.zadd_multiple(key, entries).await?;
        Ok(())
    }

    async fn members(key: &str) -> Result<Vec<(String, f64)>, DynError> {
        let mut conn = get_redis_conn().await?;
        Ok(conn.zrange_withscores(key, 0, -1).await?)
    }

    async fn exists(key: &str) -> Result<bool, DynError> {
        let mut conn = get_redis_conn().await?;
        Ok(conn.exists(key).await?)
    }

    fn owned(entries: &[(&str, f64)]) -> Vec<(String, f64)> {
        entries.iter().map(|(m, s)| (m.to_string(), *s)).collect()
    }

    /// Moves the ranking aside, so the code under test sees none.
    async fn park_ranking() -> TestResult {
        let mut conn = get_redis_conn().await?;
        let _: () = conn.rename(ranking_key(), RANKING_ASIDE).await?;
        Ok(())
    }

    async fn restore_ranking() -> TestResult {
        let mut conn = get_redis_conn().await?;
        let _: () = conn.rename(RANKING_ASIDE, ranking_key()).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn add_mirrors_ranked_authors_only() -> TestResult {
        let label = "test-ranked-add";
        setup(&[label]).await?;
        rank(&[ALICE]).await?;
        let set = RankedSet::tag(label);

        add(&set, "test-ranked-alice:p1", 10.0).await?;
        add(&set, "test-ranked-bob:p2", 20.0).await?;

        assert_eq!(
            members(&set.source).await?,
            owned(&[("test-ranked-alice:p1", 10.0), ("test-ranked-bob:p2", 20.0)])
        );
        assert_eq!(
            members(&set.ranked).await?,
            owned(&[("test-ranked-alice:p1", 10.0)])
        );

        remove(&set, "test-ranked-alice:p1").await?;
        assert!(!exists(&set.ranked).await?);
        cleanup(&[label]).await
    }

    #[tokio_shared_rt::test(shared)]
    async fn rebuild_keeps_ranked_authors_in_every_set() -> TestResult {
        let labels = [
            "test-ranked-rebuild-rust",
            "test-ranked-rebuild-spam",
            "test-ranked-rebuild-gone",
        ];
        setup(&labels).await?;
        rank(&[ALICE, CAROL]).await?;
        let [rust, spam, gone] = labels.map(RankedSet::tag);
        zadd(
            &rust.source,
            &[(1.0, "test-ranked-alice:p1"), (2.0, "test-ranked-bob:p2")],
        )
        .await?;
        zadd(&spam.source, &[(9.0, "test-ranked-bob:p9")]).await?;
        // Drift the rebuild must correct, a ranked set whose source is gone included.
        zadd(&rust.ranked, &[(2.0, "test-ranked-bob:p2")]).await?;
        zadd(&spam.ranked, &[(9.0, "test-ranked-bob:p9")]).await?;
        zadd(&gone.ranked, &[(2.0, "test-ranked-alice:p2")]).await?;

        let stats = rebuild().await?;

        assert_eq!(
            members(&rust.ranked).await?,
            owned(&[("test-ranked-alice:p1", 1.0)])
        );
        assert!(!exists(&spam.ranked).await?);
        assert!(!exists(&gone.ranked).await?, "orphaned ranked set kept");
        assert!(stats.orphans >= 1);
        assert!(exists(BUILT_AT_KEY).await?);
        assert!(!stats.dropped);
        // The global set is rebuilt from the fixture: only ranked authors.
        let mut conn = get_redis_conn().await?;
        let ranking: BTreeSet<String> = conn.zrange(ranking_key(), 0, -1).await?;
        let ranked_posts: Vec<String> = conn.zrange(RankedSet::global().ranked, 0, -1).await?;
        assert!(!ranked_posts.is_empty());
        let unranked: Vec<&String> = ranked_posts
            .iter()
            .filter(|post| {
                !post
                    .split_once(':')
                    .is_some_and(|(author, _)| ranking.contains(author))
            })
            .collect();
        assert!(unranked.is_empty(), "unranked authors: {unranked:?}");
        cleanup(&labels).await
    }

    /// Across `ZSCAN` pages and a run of equal scores longer than a page,
    /// with a ranked set too big to prune in the first call: posts gone from
    /// the source and an unranked author's posts are removed.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_reconciles_large_sets() -> TestResult {
        let label = "test-ranked-large";
        setup(&[label]).await?;
        rank(&[ALICE, BOB]).await?;
        let set = RankedSet::tag(label);
        let mut posts: Vec<(String, f64)> = Vec::new();
        posts.extend((0..1_500).map(|i| (format!("{ALICE}:P{i:05}"), 1_000.0)));
        posts.extend((0..700).map(|i| (format!("{BOB}:P{i:05}"), 2_000.0 + i as f64)));
        posts.extend((0..900).map(|i| (format!("{CAROL}:P{i:05}"), 500.0 + i as f64)));
        let entries: Vec<(f64, &str)> = posts.iter().map(|(m, s)| (*s, m.as_str())).collect();
        zadd(&set.source, &entries).await?;
        let gone: Vec<String> = (0..600).map(|i| format!("{BOB}:GONE{i:05}")).collect();
        let mut stale: Vec<(f64, &str)> = gone.iter().map(|m| (3_000.0, m.as_str())).collect();
        stale.extend(
            entries
                .iter()
                .filter(|(_, m)| m.starts_with(CAROL))
                .take(50),
        );
        zadd(&set.ranked, &stale).await?;

        let mut conn = get_redis_conn().await?;
        let mut stats = RankedRebuildStats::default();
        rebuild_set(&mut conn, &set, &mut stats).await?;

        let mut expected: Vec<(String, f64)> = posts
            .into_iter()
            .filter(|(m, _)| !m.starts_with(CAROL))
            .collect();
        expected.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        assert_eq!(members(&set.ranked).await?, expected);
        assert_eq!(
            stats.removed, 650,
            "600 posts gone, 50 by an unranked author"
        );
        cleanup(&[label]).await
    }

    /// Writes landing mid-rebuild: a post deleted after its page was
    /// reconciled is not resurrected, and one created meanwhile is kept.
    #[tokio_shared_rt::test(shared)]
    async fn writes_during_a_rebuild_are_kept() -> TestResult {
        let label = "test-ranked-concurrent";
        setup(&[label]).await?;
        rank(&[ALICE]).await?;
        let set = RankedSet::tag(label);
        let posts: Vec<String> = (0..1_200).map(|i| format!("{ALICE}:P{i:05}")).collect();
        let entries: Vec<(f64, &str)> = posts.iter().map(|m| (1.0, m.as_str())).collect();
        zadd(&set.source, &entries).await?;

        let mut conn = get_redis_conn().await?;
        let (mut cursor, ..) = reconcile_page(&mut conn, &set, 0).await?;
        assert_ne!(cursor, 0, "the set spans several pages");
        let reconciled: Vec<String> = conn.zrange(&set.ranked, 0, 0).await?;
        let deleted = reconciled.first().ok_or("nothing reconciled")?;
        remove(&set, deleted).await?;
        add(&set, "test-ranked-alice:NEW", 2.0).await?;
        while cursor != 0 {
            (cursor, ..) = reconcile_page(&mut conn, &set, cursor).await?;
        }

        let source = members(&set.source).await?;
        assert_eq!(members(&set.ranked).await?, source);
        assert!(!source.iter().any(|(m, _)| m == deleted));
        assert!(source.iter().any(|(m, _)| m == "test-ranked-alice:NEW"));
        cleanup(&[label]).await
    }

    /// A set small enough for one page, its ranked copy included, takes one call.
    #[tokio_shared_rt::test(shared)]
    async fn a_small_set_is_reconciled_in_one_call() -> TestResult {
        let label = "test-ranked-small";
        setup(&[label]).await?;
        rank(&[ALICE]).await?;
        let set = RankedSet::tag(label);
        let posts = [(1.0, "test-ranked-alice:p1"), (2.0, "test-ranked-bob:p2")];
        zadd(&set.source, &posts).await?;
        // Stale: a post gone from the source, and an unranked author's post.
        let stale = [(3.0, "test-ranked-alice:gone"), (2.0, "test-ranked-bob:p2")];
        zadd(&set.ranked, &stale).await?;

        let mut conn = get_redis_conn().await?;
        let page = reconcile_page(&mut conn, &set, 0).await?;

        assert_eq!(page, (0, 2, 1, 2, true));
        assert_eq!(
            members(&set.ranked).await?,
            owned(&[("test-ranked-alice:p1", 1.0)])
        );
        cleanup(&[label]).await
    }

    /// Without a ranking every ranked set goes, the ready marker first. The
    /// ranking is parked and restored, and a second rebuild brings the
    /// fixture's ranked sets back before anything is asserted.
    #[tokio_shared_rt::test(shared)]
    async fn rebuild_without_a_ranking_drops_every_ranked_set() -> TestResult {
        let label = "test-ranked-drop";
        setup(&[label]).await?;
        let set = RankedSet::tag(label);
        zadd(&set.ranked, &[(1.0, "test-ranked-alice:p1")]).await?;
        let global = RankedSet::global();
        let mut conn = get_redis_conn().await?;

        park_ranking().await?;
        let dropped = rebuild().await;
        let survivors: redis::RedisResult<usize> = conn
            .exists(&[BUILT_AT_KEY, &global.ranked, &set.ranked])
            .await;
        let source_kept = exists(&global.source).await;
        restore_ranking().await?;
        rebuild().await?;

        assert!(dropped?.dropped);
        assert_eq!(survivors?, 0, "ranked keys survived without a ranking");
        // The source sets are not the rebuild's to touch.
        assert!(source_kept?);
        cleanup(&[label]).await
    }

    /// No lock: two rebuilds running at once still leave every set exact.
    #[tokio_shared_rt::test(shared)]
    async fn overlapping_rebuilds_converge() -> TestResult {
        let label = "test-ranked-overlap";
        setup(&[label]).await?;
        rank(&[ALICE]).await?;
        let set = RankedSet::tag(label);
        let posts: Vec<String> = (0..1_200).map(|i| format!("{ALICE}:P{i:05}")).collect();
        let mut entries: Vec<(f64, &str)> = posts.iter().map(|m| (1.0, m.as_str())).collect();
        entries.push((2.0, "test-ranked-bob:p1"));
        zadd(&set.source, &entries).await?;

        let (first, second) = tokio::join!(rebuild(), rebuild());
        first?;
        second?;

        let ranked: Vec<(String, f64)> = posts.iter().map(|m| (m.clone(), 1.0)).collect();
        assert_eq!(members(&set.ranked).await?, ranked);
        cleanup(&[label]).await
    }

    /// `kind=short` over the fixture's wot window, a Cypher shape. Its hidden
    /// authors' posts (the on-ramp accounts, the spammer) sit among ranked ones.
    async fn wot_window_short(trust_filter: Option<TrustFilter>) -> Result<Vec<String>, DynError> {
        let window = Pagination {
            start: Some(WOT_WINDOW_START),
            end: Some(WOT_WINDOW_END),
            skip: Some(0),
            limit: Some(50),
        };
        let stream = PostStream::get_post_keys(
            StreamSource::All,
            window,
            SortOrder::Descending,
            StreamSorting::Timeline,
            None,
            Some(KindFilter::Kind(PubkyAppPostKind::Short)),
            trust_filter,
        )
        .await?;
        Ok(stream.map(|stream| stream.post_keys).unwrap_or_default())
    }

    /// The marker and the ranking are restored before anything is asserted.
    #[tokio_shared_rt::test(shared)]
    async fn load_reflects_the_ranking_marker_and_viewer() -> TestResult {
        setup(&[]).await?;
        rank(&[ALICE]).await?;
        rebuild().await?;

        let ranked = TrustMode::load(Some(ALICE)).await;
        let anonymous = TrustMode::load(None).await;
        let stranger = TrustMode::load(Some("test-ranked-stranger")).await;

        // Without the marker no shape filters, Cypher included.
        let mut conn = get_redis_conn().await?;
        let built_at: String = conn.get(BUILT_AT_KEY).await?;
        let _: () = conn.del(BUILT_AT_KEY).await?;
        let unbuilt = TrustMode::load(None).await;
        let unbuilt_short = wot_window_short(Some(TrustFilter::default())).await;
        let _: () = conn.set(BUILT_AT_KEY, built_at).await?;
        let built_short = wot_window_short(Some(TrustFilter::default())).await?;
        let unfiltered_short = wot_window_short(None).await?;

        park_ranking().await?;
        let no_ranking = [
            TrustMode::load(Some(ALICE)).await,
            TrustMode::load(None).await,
        ];
        restore_ranking().await?;

        assert_eq!(ranked?, TrustMode::Ranked);
        assert_eq!(anonymous?, TrustMode::Ranked);
        assert_eq!(stranger?, TrustMode::Off);
        assert_eq!(unbuilt?, TrustMode::Off);
        assert_eq!(unbuilt_short?, unfiltered_short);
        assert_ne!(
            built_short, unfiltered_short,
            "the window must hide someone"
        );
        for mode in no_ranking {
            assert_eq!(mode?, TrustMode::Off);
        }
        cleanup(&[]).await
    }
}
