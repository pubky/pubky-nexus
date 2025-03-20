use chrono::{Duration, TimeZone, Utc};
use csv::WriterBuilder;
use neo4rs::query;
use nexus_common::db::get_neo4j_graph;
use std::error::Error;
use std::fs::OpenOptions;
use std::path::Path;
use tokio::time::{interval, Duration as TokioDuration};

/// This structure represents a complete CSV record, containing both overall and daily metrics.
/// The timestamp is maintained as an i64.
#[derive(Debug, serde::Serialize)]
pub struct MetricsRecord {
    pub timestamp: i64,
    // Overall totals (across all time)
    pub overall_total_users: i64,
    pub overall_total_posts: i64,
    pub overall_total_files: i64,
    pub overall_total_replies: i64,
    pub overall_total_reposts: i64,
    pub overall_total_user_tags: i64,
    pub overall_total_post_tags: i64,
    // Daily metrics (for a fully completed day, e.g. yesterday)
    pub daily_new_users: i64,
    pub daily_active_users: i64,
    pub daily_new_posts: i64,
    pub daily_replies: i64,
    pub daily_reposts: i64,
    pub daily_mentions: i64,
    pub daily_new_follows: i64,
    pub daily_new_mutes: i64,
    pub daily_new_bookmarks: i64,
    pub daily_new_post_tags: i64,
    pub daily_new_user_tags: i64,
    pub daily_new_files: i64,
}

/// Structure to hold overall totals (all-time counts).
pub struct OverallTotals {
    pub total_users: i64,
    pub total_posts: i64,
    pub total_files: i64,
    pub total_replies: i64,
    pub total_reposts: i64,
    pub total_user_tags: i64,
    pub total_post_tags: i64,
}

/// Structure to hold daily metrics.
pub struct DailyMetrics {
    pub new_users: i64,
    pub daily_active_users: i64,
    pub new_posts: i64,
    pub replies: i64,
    pub reposts: i64,
    pub mentions: i64,
    pub new_follows: i64,
    pub new_mutes: i64,
    pub new_bookmarks: i64,
    pub new_post_tags: i64,
    pub new_user_tags: i64,
    pub new_files: i64,
}

/// Executes a multi-statement query to collect overall totals from Neo4j.
pub async fn collect_overall_totals() -> Result<OverallTotals, Box<dyn Error>> {
    tracing::debug!("Collecting overall totals from Neo4j...");
    let overall_query = r#"
CALL {
  MATCH (u:User)
  RETURN count(u) AS totalUsers
}
CALL {
  MATCH (p:Post)
  RETURN count(p) AS totalPosts
}
CALL {
  MATCH (f:File)
  RETURN count(f) AS totalFiles
}
CALL {
  MATCH (p:Post)-[:REPLIED]->(:Post)
  RETURN count(p) AS totalReplies
}
CALL {
  MATCH (p:Post)-[:REPOSTED]->(:Post)
  RETURN count(p) AS totalReposts
}
CALL {
  MATCH ()-[r:TAGGED]->(target:User)
  RETURN count(r) AS totalUserTags
}
CALL {
  MATCH ()-[r:TAGGED]->(target:Post)
  RETURN count(r) AS totalPostTags
}
RETURN totalUsers, totalPosts, totalFiles, totalReplies, totalReposts, totalUserTags, totalPostTags;
    "#;

    let neo_query = query(overall_query);
    let graph = get_neo4j_graph()?;
    let mut result = {
        let graph = graph.lock().await;
        graph.execute(neo_query).await?
    };

    if let Some(row) = result.next().await? {
        tracing::debug!("Overall totals collected: {:?}", row);
        Ok(OverallTotals {
            total_users: row.get("totalUsers")?,
            total_posts: row.get("totalPosts")?,
            total_files: row.get("totalFiles")?,
            total_replies: row.get("totalReplies")?,
            total_reposts: row.get("totalReposts")?,
            total_user_tags: row.get("totalUserTags")?,
            total_post_tags: row.get("totalPostTags")?,
        })
    } else {
        Err("No data returned from overall totals query".into())
    }
}

/// Executes a query to collect daily metrics from Neo4j, using a time window defined by `start` and `end` (in milliseconds).
pub async fn collect_daily_metrics(start: i64, end: i64) -> Result<DailyMetrics, Box<dyn Error>> {
    tracing::debug!(
        "Collecting daily metrics from Neo4j for time window {} - {}...",
        start,
        end
    );
    let daily_query = r#"
CALL {
  MATCH (u:User)
  WHERE u.indexed_at >= $start AND u.indexed_at < $end
  RETURN count(u) AS newUsers
}
CALL {
  CALL {
    MATCH (u:User)-[:AUTHORED]->(p:Post)
    WHERE p.indexed_at >= $start AND p.indexed_at < $end
    RETURN DISTINCT u.id AS userId
    UNION
    MATCH (u:User)-[r:FOLLOWS]->()
    WHERE r.indexed_at >= $start AND r.indexed_at < $end
    RETURN DISTINCT u.id AS userId
    UNION
    MATCH (u:User)-[r:MUTED]->()
    WHERE r.indexed_at >= $start AND r.indexed_at < $end
    RETURN DISTINCT u.id AS userId
    UNION
    MATCH (u:User)-[r:BOOKMARKED]->()
    WHERE r.indexed_at >= $start AND r.indexed_at < $end
    RETURN DISTINCT u.id AS userId
    UNION
    MATCH (u:User)-[r:TAGGED]->()
    WHERE r.indexed_at >= $start AND r.indexed_at < $end
    RETURN DISTINCT u.id AS userId
  }
  RETURN count(DISTINCT userId) AS dailyActiveUsers
}
CALL {
  MATCH (p:Post)
  WHERE p.indexed_at >= $start AND p.indexed_at < $end
  RETURN count(p) AS newPosts
}
CALL {
  MATCH (p:Post)-[:REPLIED]->(:Post)
  WHERE p.indexed_at >= $start AND p.indexed_at < $end
  RETURN count(p) AS replies
}
CALL {
  MATCH (p:Post)-[:REPOSTED]->(:Post)
  WHERE p.indexed_at >= $start AND p.indexed_at < $end
  RETURN count(p) AS reposts
}
CALL {
  MATCH (p:Post)-[m:MENTIONED]->(:User)
  WHERE p.indexed_at >= $start AND p.indexed_at < $end
  RETURN count(m) AS mentions
}
CALL {
  MATCH ()-[r:FOLLOWS]->()
  WHERE r.indexed_at >= $start AND r.indexed_at < $end
  RETURN count(r) AS newFollows
}
CALL {
  MATCH ()-[r:MUTED]->()
  WHERE r.indexed_at >= $start AND r.indexed_at < $end
  RETURN count(r) AS newMutes
}
CALL {
  MATCH ()-[r:BOOKMARKED]->()
  WHERE r.indexed_at >= $start AND r.indexed_at < $end
  RETURN count(r) AS newBookmarks
}
CALL {
  MATCH (u:User)-[r:TAGGED]->(p:Post)
  WHERE r.indexed_at >= $start AND r.indexed_at < $end
  RETURN count(r) AS newPostTags
}
CALL {
  MATCH (tagger:User)-[r:TAGGED]->(tagged:User)
  WHERE r.indexed_at >= $start AND r.indexed_at < $end
  RETURN count(r) AS newUserTags
}
CALL {
  MATCH (f:File)
  WHERE f.created_at >= $start AND f.created_at < $end
  RETURN count(f) AS newFiles
}
RETURN newUsers, dailyActiveUsers, newPosts, replies, reposts, mentions, newFollows, newMutes, newBookmarks, newPostTags, newUserTags, newFiles;
    "#;

    // Pass parameters in milliseconds.
    let neo_query = query(daily_query).param("start", start).param("end", end);
    let graph = get_neo4j_graph()?;
    let mut result = {
        let graph = graph.lock().await;
        graph.execute(neo_query).await?
    };

    if let Some(row) = result.next().await? {
        tracing::debug!("Daily metrics collected: {:?}", row);
        Ok(DailyMetrics {
            new_users: row.get("newUsers")?,
            daily_active_users: row.get("dailyActiveUsers")?,
            new_posts: row.get("newPosts")?,
            replies: row.get("replies")?,
            reposts: row.get("reposts")?,
            mentions: row.get("mentions")?,
            new_follows: row.get("newFollows")?,
            new_mutes: row.get("newMutes")?,
            new_bookmarks: row.get("newBookmarks")?,
            new_post_tags: row.get("newPostTags")?,
            new_user_tags: row.get("newUserTags")?,
            new_files: row.get("newFiles")?,
        })
    } else {
        Err("No data returned from daily metrics query".into())
    }
}

/// Writes a CSV record to a file, appending headers only if the file does not exist.
pub fn write_metrics(record: MetricsRecord) -> Result<(), Box<dyn Error>> {
    let file_path = "metrics.csv";
    let file_exists = Path::new(file_path).exists();
    tracing::debug!(
        "Writing metrics to {} (file exists: {})",
        file_path,
        file_exists
    );
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;
    let mut wtr = WriterBuilder::new()
        .has_headers(!file_exists)
        .from_writer(file);
    wtr.serialize(record)?;
    wtr.flush()?;
    tracing::debug!("Metrics successfully written to CSV.");
    Ok(())
}

/// The main loop for the metrics collection service.
/// This function runs periodically (every day, in this example) and writes metrics for a fully completed day.
/// The window for yesterday is computed in milliseconds.
pub async fn run_metrics() -> Result<(), Box<dyn Error>> {
    tracing::debug!("Starting observability metrics collection...");
    let mut ticker = interval(TokioDuration::from_secs(24 * 60 * 60)); // adjust interval as needed

    loop {
        ticker.tick().await;
        let now = Utc::now();
        tracing::debug!("Ticker tick at {}", now);

        // Compute today's midnight (UTC) then derive yesterday's window.
        let today_midnight = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .expect("Failed to compute midnight");
        let yesterday_start = today_midnight - Duration::days(1);
        let yesterday_end = today_midnight - Duration::milliseconds(1);
        // Use milliseconds since your data (indexed_at) is stored in ms.
        let start_of_yesterday = Utc.from_utc_datetime(&yesterday_start).timestamp_millis();
        let end_of_yesterday = Utc.from_utc_datetime(&yesterday_end).timestamp_millis();

        tracing::debug!(
            "Yesterday's window (in milliseconds): {} to {}",
            start_of_yesterday,
            end_of_yesterday
        );

        // Collect overall totals and daily metrics from Neo4j.
        let overall_totals = collect_overall_totals().await?;
        let daily_metrics = collect_daily_metrics(start_of_yesterday, end_of_yesterday).await?;

        // Build the complete metrics record.
        let record = MetricsRecord {
            timestamp: now.timestamp_millis(),
            overall_total_users: overall_totals.total_users,
            overall_total_posts: overall_totals.total_posts,
            overall_total_files: overall_totals.total_files,
            overall_total_replies: overall_totals.total_replies,
            overall_total_reposts: overall_totals.total_reposts,
            overall_total_user_tags: overall_totals.total_user_tags,
            overall_total_post_tags: overall_totals.total_post_tags,
            daily_new_users: daily_metrics.new_users,
            daily_active_users: daily_metrics.daily_active_users,
            daily_new_posts: daily_metrics.new_posts,
            daily_replies: daily_metrics.replies,
            daily_reposts: daily_metrics.reposts,
            daily_mentions: daily_metrics.mentions,
            daily_new_follows: daily_metrics.new_follows,
            daily_new_mutes: daily_metrics.new_mutes,
            daily_new_bookmarks: daily_metrics.new_bookmarks,
            daily_new_post_tags: daily_metrics.new_post_tags,
            daily_new_user_tags: daily_metrics.new_user_tags,
            daily_new_files: daily_metrics.new_files,
        };

        tracing::debug!("Metrics record built: {:?}", record);
        write_metrics(record)?;
        tracing::debug!("Cycle complete. Waiting for the next tick...");
    }
}
