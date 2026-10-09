use std::collections::BTreeMap;

use super::reconcile::{shared_entries, AuthorPost};
use super::*;
use crate::models::post::search::{TAG_GLOBAL_POST_ENGAGEMENT, TAG_GLOBAL_POST_TIMELINE};
use crate::models::post::{
    POST_REPLIES_PER_POST_KEY_PARTS, POST_TIMELINE_KEY_PARTS, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
use crate::models::user::SocialGraphStatus;

fn ranking_key() -> String {
    SocialGraphStatus::ranking_key()
}

fn timeline_key() -> String {
    sorted_key(&POST_TIMELINE_KEY_PARTS)
}

fn engagement_key() -> String {
    sorted_key(&POST_TOTAL_ENGAGEMENT_KEY_PARTS)
}

fn tag_timeline_key(label: &str) -> String {
    sorted_key(&[&TAG_GLOBAL_POST_TIMELINE[..], &[label]].concat())
}

fn tag_engagement_key(label: &str) -> String {
    sorted_key(&[&TAG_GLOBAL_POST_ENGAGEMENT[..], &[label]].concat())
}

fn replies_key(author_id: &str, post_id: &str) -> String {
    sorted_key(&[&POST_REPLIES_PER_POST_KEY_PARTS[..], &[author_id, post_id]].concat())
}

/// Root posts go to the global sets, replies to their parent's thread unless
/// the thread's owner wrote them or follows their author, and every post to
/// the sets of its labels.
#[test]
fn shared_entries_cover_every_set_an_author_reaches() {
    let post = |id: &str, indexed_at, parent: Option<(&str, &str)>, labels: &[&str], engagement| {
        AuthorPost {
            author: "alice".to_string(),
            id: id.to_string(),
            indexed_at,
            parent: parent.map(|(a, p)| (a.to_string(), p.to_string())),
            followed_by_parent_author: false,
            labels: labels.iter().map(ToString::to_string).collect(),
            engagement,
            mentions: 1,
        }
    };
    let followed = AuthorPost {
        followed_by_parent_author: true,
        ..post("followed", 40, Some(("carol", "c1")), &[], 0)
    };
    let posts = [
        post("root", 10, None, &["x", "y"], 5),
        post("reply", 20, Some(("bob", "b1")), &["x"], 1),
        post("own", 30, Some(("alice", "root")), &["z"], 0),
        followed,
    ];

    let entries = shared_entries(&posts);

    let entry = |score: f64, id: &str| (score, format!("alice:{id}"));
    let expected = BTreeMap::from([
        (timeline_key(), vec![entry(10.0, "root")]),
        (engagement_key(), vec![entry(5.0, "root")]),
        (replies_key("bob", "b1"), vec![entry(20.0, "reply")]),
        (
            tag_timeline_key("x"),
            vec![entry(10.0, "root"), entry(20.0, "reply")],
        ),
        // Engagement plus mentions, whatever the label.
        (
            tag_engagement_key("x"),
            vec![entry(6.0, "root"), entry(2.0, "reply")],
        ),
        (tag_timeline_key("y"), vec![entry(10.0, "root")]),
        (tag_engagement_key("y"), vec![entry(6.0, "root")]),
        (tag_timeline_key("z"), vec![entry(30.0, "own")]),
        (tag_engagement_key("z"), vec![entry(1.0, "own")]),
    ]);
    assert_eq!(entries, expected);
}

/// Live tests against the shared Redis, the fixture graph and its ranking. The
/// gate tests write only their own keys and authors; the reconcile tests move
/// the fixture's spammer in and out and put it back. All of them touch the
/// shared ranking, so `.config/nextest.toml` runs each one alone.
mod live {
    use redis::AsyncCommands;

    use super::super::reconcile::{shared_entries, write_pages, AuthorPost, Write};
    use super::super::*;
    use super::{
        engagement_key, ranking_key, replies_key, tag_engagement_key, tag_timeline_key,
        timeline_key, TAG_GLOBAL_POST_ENGAGEMENT, TAG_GLOBAL_POST_TIMELINE,
    };
    use crate::db::graph::Query;
    use crate::db::queries::get::PostEntries;
    use crate::db::{exec_single_row, fetch_all_rows_from_graph, fetch_key_from_graph, queries};
    use crate::models::bootstrap::{Bootstrap, ViewType};
    use crate::models::post::PostCounts;
    use crate::types::DynError;
    use crate::{StackConfig, StackManager};

    type TestResult = Result<(), DynError>;

    const ALICE: &str = "test-filter-alice";
    const BOB: &str = "test-filter-bob";
    /// Where a test parks the ranking while it checks behaviour without one.
    const RANKING_ASIDE: &str = "Test:TrustFilter:RankingAside";

    /// wot.cypher's spammer: unranked, with a root post and a reply to D2's
    /// post that ranked users tagged.
    const SPAMMER: &str = "qdsygndnk45m9ru5jseg3uxk5xg4usj9hrcraqbzgigapzweaa9o";
    const D2: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";

    /// Every entry the spammer's posts would have in the shared sets, with
    /// the scores a full reindex gives them. The reply has six tags.
    fn spammer_entries() -> Vec<(String, String, f64)> {
        let root = format!("{SPAMMER}:WOTPOSTS00006");
        let reply = format!("{SPAMMER}:WOTPOSTMODF01");
        let mut entries = vec![
            (timeline_key(), root.clone(), 1650000000006.0),
            (engagement_key(), root, 0.0),
            (
                replies_key(D2, "WOTPOSTTAGS01"),
                reply.clone(),
                1650000000013.0,
            ),
        ];
        for label in [
            "wmtag1",
            "wmtag2",
            "wmtag3",
            "wmtag4",
            "wmtag5",
            "wmtagflag",
        ] {
            entries.push((tag_timeline_key(label), reply.clone(), 1650000000013.0));
            entries.push((tag_engagement_key(label), reply.clone(), 6.0));
        }
        entries
    }

    /// The score of each spammer entry, `None` where it is absent.
    async fn spammer_scores() -> Result<Vec<Option<f64>>, DynError> {
        scores(&spammer_entries()).await
    }

    /// The score of each `(key, member, _)` entry, `None` where it is absent.
    async fn scores(entries: &[(String, String, f64)]) -> Result<Vec<Option<f64>>, DynError> {
        let mut conn = get_redis_conn().await?;
        let mut scores = Vec::new();
        for (key, member, _) in entries {
            scores.push(conn.zscore(key, member).await?);
        }
        Ok(scores)
    }

    /// Every entry `author`'s posts have in the shared sets, as `(key, member,
    /// score)`, read from the graph as [`reconcile()`] reads it.
    async fn author_entries(author: &str) -> Result<Vec<(String, String, f64)>, DynError> {
        let query =
            queries::get::post_entries(PostEntries::WrittenBy(author), (i64::MIN, ""), 1_000);
        let rows = fetch_all_rows_from_graph(query).await?;
        let posts = rows
            .iter()
            .map(AuthorPost::from_row)
            .collect::<Result<Vec<_>, _>>()?;
        let entries = shared_entries(&posts)
            .into_iter()
            .flat_map(|(key, entries)| {
                entries
                    .into_iter()
                    .map(move |(score, member)| (key.clone(), member, score))
            })
            .collect();
        Ok(entries)
    }

    fn all_present() -> Vec<Option<f64>> {
        spammer_entries()
            .into_iter()
            .map(|(_, _, score)| Some(score))
            .collect()
    }

    fn all_absent() -> Vec<Option<f64>> {
        vec![None; spammer_entries().len()]
    }

    /// Connects, and puts back what an earlier failed run may have left: a
    /// parked ranking, test authors in the ranking, the spammer ranked, or the
    /// shared sets out of step with the ranking.
    async fn setup(keys: &[&str]) -> TestResult {
        StackManager::setup(&StackConfig::default()).await?;
        let mut conn = get_redis_conn().await?;
        let parked: bool = conn.exists(RANKING_ASIDE).await?;
        if parked {
            let _: () = conn.rename(RANKING_ASIDE, ranking_key()).await?;
        }
        cleanup(keys).await
    }

    async fn cleanup(keys: &[&str]) -> TestResult {
        let mut conn = get_redis_conn().await?;
        if !keys.is_empty() {
            let _: () = conn.unlink(keys).await?;
        }
        let _: () = conn.zrem(ranking_key(), &[ALICE, BOB, SPAMMER]).await?;
        set_spammer_trust("REMOVE u.trust").await?;
        reconcile().await?;
        Ok(())
    }

    /// Applies `change` to the spammer's graph trust, which post counts read.
    async fn set_spammer_trust(change: &str) -> TestResult {
        let cypher = format!("MATCH (u:User {{id: $id}}) {change}");
        exec_single_row(Query::new("test_spammer_trust", &cypher).param("id", SPAMMER)).await?;
        Ok(())
    }

    /// Ranks `user_id` below every fixture user, so fixture ranks are untouched.
    async fn rank(user_id: &str) -> TestResult {
        let mut conn = get_redis_conn().await?;
        let _: () = conn.zadd(ranking_key(), user_id, 1e9).await?;
        Ok(())
    }

    async fn unrank(user_id: &str) -> TestResult {
        let mut conn = get_redis_conn().await?;
        let _: () = conn.zrem(ranking_key(), user_id).await?;
        Ok(())
    }

    async fn members(key: &str) -> Result<Vec<(String, f64)>, DynError> {
        let mut conn = get_redis_conn().await?;
        Ok(conn.zrange_withscores(key, 0, -1).await?)
    }

    fn owned(entries: &[(&str, f64)]) -> Vec<(String, f64)> {
        entries.iter().map(|(m, s)| (m.to_string(), *s)).collect()
    }

    /// The key parts the writers pass for a test tag set, and its key.
    fn tag_set(family: [&'static str; 4], label: &'static str) -> (Vec<&'static str>, String) {
        let parts = [&family[..], &[label]].concat();
        let key = sorted_key(&parts);
        (parts, key)
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

    /// The gate holds across the batches one call is split into, and
    /// `add_always` skips it.
    #[tokio_shared_rt::test(shared)]
    async fn add_keeps_ranked_authors_only() -> TestResult {
        let (parts, key) = tag_set(TAG_GLOBAL_POST_TIMELINE, "test-filter-add");
        setup(&[&key]).await?;
        rank(ALICE).await?;
        let posts: Vec<String> = (0..1_200)
            .map(|i| match i % 2 {
                0 => format!("{ALICE}:P{i:05}"),
                _ => format!("{BOB}:P{i:05}"),
            })
            .collect();
        let entries: Vec<(f64, &str)> = posts.iter().map(|m| (1.0, m.as_str())).collect();

        add(&parts, &entries).await?;
        let gated = members(&key).await?;
        add_always(&parts, &[(3.0, "test-filter-bob:p3")]).await?;
        let forced = members(&key).await?;

        cleanup(&[&key]).await?;
        assert_eq!(gated.len(), 600);
        assert!(gated.iter().all(|(m, _)| m.starts_with(ALICE)));
        assert!(forced.contains(&("test-filter-bob:p3".to_string(), 3.0)));
        Ok(())
    }

    /// Without a ranking the gate takes everyone, as before the filter.
    #[tokio_shared_rt::test(shared)]
    async fn add_takes_everyone_without_a_ranking() -> TestResult {
        let (parts, key) = tag_set(TAG_GLOBAL_POST_TIMELINE, "test-filter-open");
        setup(&[&key]).await?;

        park_ranking().await?;
        let added = add(&parts, &[(1.0, "test-filter-bob:p1")]).await;
        // Cypher-served streams follow the applied copy, not the live ranking.
        let still_applied = is_ranking_applied().await;
        restore_ranking().await?;
        let kept = members(&key).await?;

        cleanup(&[&key]).await?;
        added?;
        assert_eq!(kept, owned(&[("test-filter-bob:p1", 1.0)]));
        assert!(still_applied?);
        Ok(())
    }

    /// An increment moves a member that is there, whoever wrote it, and creates
    /// one only for a ranked author; a decrement never creates one.
    #[tokio_shared_rt::test(shared)]
    async fn incr_never_creates_a_hidden_or_negative_entry() -> TestResult {
        let (parts, key) = tag_set(TAG_GLOBAL_POST_ENGAGEMENT, "test-filter-incr");
        setup(&[&key]).await?;
        rank(ALICE).await?;
        let mut conn = get_redis_conn().await?;
        let _: () = conn.zadd(&key, "test-filter-bob:there", 5.0).await?;

        incr(&parts, &[BOB, "new"], ScoreAction::Increment(1.0)).await?;
        incr(&parts, &[ALICE, "new"], ScoreAction::Increment(1.0)).await?;
        incr(&parts, &[ALICE, "gone"], ScoreAction::Decrement(1.0)).await?;
        incr(&parts, &[BOB, "there"], ScoreAction::Decrement(1.0)).await?;
        let scores = members(&key).await?;

        cleanup(&[&key]).await?;
        assert_eq!(
            scores,
            owned(&[
                ("test-filter-alice:new", 1.0),
                ("test-filter-bob:there", 4.0)
            ])
        );
        Ok(())
    }

    /// Off, the gate takes every author, Cypher-served streams stop filtering,
    /// and the next reconcile writes the hidden authors back and drops the copy.
    /// On again, the next reconcile hides them.
    #[tokio_shared_rt::test(shared)]
    async fn the_switch_turns_the_filter_off_and_back_on() -> TestResult {
        let (parts, key) = tag_set(TAG_GLOBAL_POST_TIMELINE, "test-filter-switch");
        setup(&[&key]).await?;

        set_enabled(false);
        let added = add(&parts, &[(1.0, "test-filter-bob:p1")]).await;
        let counted = incr(&parts, &[BOB, "p2"], ScoreAction::Increment(1.0)).await;
        let opened = reconcile().await;
        let open_scores = spammer_scores().await;
        let open_applied = is_ranking_applied().await;
        set_enabled(true);
        let closed = reconcile().await;
        let closed_scores = spammer_scores().await;
        let written = members(&key).await;

        cleanup(&[&key]).await?;
        added?;
        counted?;
        opened?;
        closed?;
        assert_eq!(
            written?,
            owned(&[("test-filter-bob:p1", 1.0), ("test-filter-bob:p2", 1.0)])
        );
        assert_eq!(open_scores?, all_present());
        assert!(!open_applied?);
        assert_eq!(closed_scores?, all_absent());
        Ok(())
    }

    /// Bootstrap's timeline keeps a user's own posts when the ranking hides
    /// them from everyone else. The author of the newest post on the global
    /// timeline is taken out of the ranking for the test.
    #[tokio_shared_rt::test(shared)]
    async fn bootstrap_keeps_the_users_own_posts() -> TestResult {
        setup(&[]).await?;
        let mut conn = get_redis_conn().await?;
        let newest: Vec<String> = conn.zrevrange(timeline_key(), 0, 0).await?;
        let newest = newest.into_iter().next().ok_or("empty timeline")?;
        let author = newest
            .split_once(':')
            .ok_or("author:post member")?
            .0
            .to_string();
        let rank: Option<f64> = conn.zscore(ranking_key(), &author).await?;
        let rank = rank.ok_or("the fixture ranks the newest post's author")?;

        let _: () = conn.zrem(ranking_key(), &author).await?;
        let hidden = reconcile().await;
        let own = Bootstrap::get_by_id(&author, ViewType::Full).await;
        let other = Bootstrap::get_by_id(D2, ViewType::Full).await;
        let _: () = conn.zadd(ranking_key(), &author, rank).await?;
        let shown = reconcile().await;

        cleanup(&[]).await?;
        hidden?;
        shown?;
        assert!(own?.ids.stream.contains(&newest), "the author's own post");
        assert!(!other?.ids.stream.contains(&newest), "hidden from others");
        Ok(())
    }

    /// An author's entries are written and taken out a page of posts at a time,
    /// every page included: the spammer's two posts in pages of one.
    #[tokio_shared_rt::test(shared)]
    async fn writes_walk_every_page_of_posts() -> TestResult {
        setup(&[]).await?;
        let mut conn = get_redis_conn().await?;

        rank(SPAMMER).await?;
        let shown = write_pages(&mut conn, PostEntries::WrittenBy(SPAMMER), Write::Show, 1).await;
        let shown_scores = spammer_scores().await;
        unrank(SPAMMER).await?;
        let hidden = write_pages(&mut conn, PostEntries::WrittenBy(SPAMMER), Write::Hide, 1).await;
        let hidden_scores = spammer_scores().await;

        cleanup(&[]).await?;
        shown?;
        hidden?;
        assert_eq!(shown_scores?, all_present());
        assert_eq!(hidden_scores?, all_absent());
        Ok(())
    }

    /// A rank change moves every entry of the author's posts in or out at the
    /// next reconcile, at the scores a full reindex gives them, and the copy
    /// follows the ranking.
    #[tokio_shared_rt::test(shared)]
    async fn reconcile_follows_rank_changes() -> TestResult {
        setup(&[]).await?;
        let before = spammer_scores().await?;

        rank(SPAMMER).await?;
        let shown = reconcile().await;
        let ranked = spammer_scores().await;
        unrank(SPAMMER).await?;
        let hidden = reconcile().await;
        let unranked = spammer_scores().await;

        cleanup(&[]).await?;
        shown?;
        hidden?;
        assert_eq!(before, all_absent(), "the fixture hides the spammer");
        assert_eq!(ranked?, all_present());
        assert_eq!(unranked?, all_absent());
        Ok(())
    }

    /// A ranking can gate writes without ever being applied: its first reconcile
    /// removed some authors' posts, then failed before recording the copy. The
    /// next publish applies it before moving to the new ranking, so an author
    /// the new one admits gets every post back.
    #[tokio_shared_rt::test(shared)]
    async fn publish_finishes_a_ranking_never_applied() -> TestResult {
        setup(&[]).await?;
        let entries = author_entries(D2).await?;
        let before = scores(&entries).await?;

        // A ranking without D2 goes live, and its first reconcile removes D2's
        // posts before failing.
        unrank(D2).await?;
        let mut conn = get_redis_conn().await?;
        let _: () = conn.del(applied_key()).await?;
        let failed_run =
            write_pages(&mut conn, PostEntries::WrittenBy(D2), Write::Hide, 1_000).await;
        let hidden = scores(&entries).await;

        // The fixture graph ranks D2, so the published ranking admits them.
        let published = SocialGraphStatus::publish().await;
        let after = scores(&entries).await;

        // Puts D2 back whatever happened above. Its rank only moves when the
        // publish failed, and test_social_graph_status publishes before reading.
        let ranked: Option<f64> = conn.zscore(ranking_key(), D2).await?;
        if ranked.is_none() {
            rank(D2).await?;
        }
        write_pages(&mut conn, PostEntries::WrittenBy(D2), Write::Show, 1_000).await?;
        cleanup(&[]).await?;

        failed_run?;
        published?;
        let present: Vec<Option<f64>> = entries.iter().map(|(_, _, s)| Some(*s)).collect();
        assert!(!entries.is_empty(), "the fixture gives D2 shared entries");
        assert!(before.iter().all(Option::is_some), "the fixture shows D2");
        assert_eq!(hidden?, vec![None; entries.len()]);
        assert_eq!(
            after?, present,
            "the publish should give D2 their posts back"
        );
        Ok(())
    }

    /// The replies D2's post counts, cached by the read.
    async fn d2_post_replies() -> Result<u32, DynError> {
        let counts = PostCounts::get_by_id(D2, "WOTPOSTTAGS01").await?;
        Ok(counts.ok_or("D2's post")?.replies)
    }

    /// A rank change drops the cached counts of the posts the author replied
    /// to: the spammer's reply to D2's post counts only while they're ranked.
    #[tokio_shared_rt::test(shared)]
    async fn rank_changes_recount_replied_posts() -> TestResult {
        setup(&[]).await?;
        let before = d2_post_replies().await;

        set_spammer_trust("SET u.trust = 1e-9").await?;
        rank(SPAMMER).await?;
        let shown = reconcile().await;
        let ranked = d2_post_replies().await;
        set_spammer_trust("REMOVE u.trust").await?;
        unrank(SPAMMER).await?;
        let hidden = reconcile().await;
        let unranked = d2_post_replies().await;

        cleanup(&[]).await?;
        shown?;
        hidden?;
        let before = before?;
        assert_eq!(ranked?, before + 1, "the spammer's reply counts");
        assert_eq!(unranked?, before);
        Ok(())
    }

    /// Once a ranking is applied, post counts leave out unranked users'
    /// replies but keep every tag: of the replies to D2's post the spammer's
    /// doesn't count, while the unranked moderation bot's tag does, and the
    /// engagement score counts both.
    #[tokio_shared_rt::test(shared)]
    async fn counts_leave_out_unranked_replies_only() -> TestResult {
        setup(&[]).await?;
        let graph = PostCounts::get_from_graph(D2, "WOTPOSTTAGS01")
            .await?
            .ok_or("D2's post")?;
        let query = Query::new(
            "test_raw_post_counts",
            "MATCH (p:Post {id: 'WOTPOSTTAGS01'})
             RETURN [COUNT { (p)<-[:REPLIED]-() }, COUNT { (p)<-[:TAGGED]-() }] AS counts",
        );
        let raw: Vec<i64> = fetch_key_from_graph(query, "counts")
            .await?
            .unwrap_or_default();
        assert!(
            is_ranking_applied().await?,
            "the fixture's ranking is applied"
        );
        let (replies, tags) = (raw[0], raw[1]);
        assert_eq!(i64::from(graph.counts.replies), replies - 1);
        assert_eq!(i64::from(graph.counts.tags), tags);
        assert_eq!(i64::from(graph.engagement), replies + tags);
        Ok(())
    }
}
