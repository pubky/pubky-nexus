use chrono::{Duration, NaiveDate, TimeZone, Utc};
use csv::WriterBuilder;
use neo4rs::query;
use nexus_common::db::get_neo4j_graph;
use std::error::Error;
use std::fs::OpenOptions;
use std::path::Path;

// Begining of time of Pubky App production homeserver.
const EPOCH_YEAR: i32 = 2025;
const EPOCH_MONTH: u32 = 03;
const EPOCH_DAY: u32 = 12;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MetricsRecord {
    pub date: String,
    // Overall totals (across all time)
    pub overall_total_users: i64,
    pub overall_total_posts: i64,
    pub overall_total_files: i64,
    pub overall_total_replies: i64,
    pub overall_total_reposts: i64,
    pub overall_total_user_tags: i64,
    pub overall_total_post_tags: i64,
    // Daily metrics
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
    // Additional metrics: weekly & monthly active users and churned users.
    pub active_users_weekly: i64,
    pub active_users_monthly: i64,
    pub churned_users_weekly: i64,
    pub churned_users_monthly: i64,
}

pub struct OverallTotals {
    pub total_users: i64,
    pub total_posts: i64,
    pub total_files: i64,
    pub total_replies: i64,
    pub total_reposts: i64,
    pub total_user_tags: i64,
    pub total_post_tags: i64,
}

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

pub async fn collect_daily_metrics(start: i64, end: i64) -> Result<DailyMetrics, Box<dyn Error>> {
    tracing::debug!("Collecting daily metrics from {} to {}...", start, end);
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

pub async fn collect_active_users(start: i64, end: i64) -> Result<i64, Box<dyn Error>> {
    let active_users_query = r#"
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
RETURN count(DISTINCT userId) AS activeUsers;
    "#;
    let neo_query = query(active_users_query)
        .param("start", start)
        .param("end", end);
    let graph = get_neo4j_graph()?;
    let mut result = {
        let graph = graph.lock().await;
        graph.execute(neo_query).await?
    };
    if let Some(row) = result.next().await? {
        tracing::debug!("Active users for window {} - {}: {:?}", start, end, row);
        Ok(row.get("activeUsers")?)
    } else {
        Err("No data returned from active users query".into())
    }
}

pub fn write_metrics(record: MetricsRecord) -> Result<(), Box<dyn Error>> {
    let file_path = "metrics.csv";
    if Path::new(file_path).exists() {
        let mut rdr = csv::Reader::from_path(file_path)?;
        let mut records: Vec<MetricsRecord> = rdr.deserialize().collect::<Result<_, _>>()?;
        if let Some(last_date) = records.last().map(|r| r.date.clone()) {
            if last_date == record.date {
                records.pop();
                records.push(record);
                let file = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(file_path)?;
                let mut wtr = WriterBuilder::new().has_headers(true).from_writer(file);
                for rec in records {
                    wtr.serialize(rec)?;
                }
                wtr.flush()?;
                tracing::debug!("Replaced metrics for date {}.", last_date);
                return Ok(());
            }
        }
        let file = OpenOptions::new().append(true).open(file_path)?;
        let mut wtr = WriterBuilder::new().has_headers(false).from_writer(file);
        wtr.serialize(record)?;
        wtr.flush()?;
        tracing::debug!("Appended new metrics record.");
        Ok(())
    } else {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(file_path)?;
        let mut wtr = WriterBuilder::new().has_headers(true).from_writer(file);
        wtr.serialize(record)?;
        wtr.flush()?;
        tracing::debug!("Created new metrics file with the first record.");
        Ok(())
    }
}

// --- Helper Functions to simplify time computations and per-day processing ---

fn get_day_timestamps(date: NaiveDate) -> (i64, i64) {
    let start_of_day = date
        .and_hms_opt(0, 0, 0)
        .expect("Failed to compute start of day");
    let end_of_day = date
        .and_hms_milli_opt(23, 59, 59, 999)
        .expect("Failed to compute end of day");
    (
        Utc.from_utc_datetime(&start_of_day).timestamp_millis(),
        Utc.from_utc_datetime(&end_of_day).timestamp_millis(),
    )
}

fn get_window_start(date: NaiveDate, days_offset: i64) -> i64 {
    let window_date = date - Duration::days(days_offset);
    let window_start = window_date
        .and_hms_opt(0, 0, 0)
        .expect("Failed to compute window start");
    Utc.from_utc_datetime(&window_start).timestamp_millis()
}

async fn process_day(date: NaiveDate) -> Result<MetricsRecord, Box<dyn Error>> {
    let (start_ts, end_ts) = get_day_timestamps(date);
    let weekly_start_ts = get_window_start(date, 6);
    let monthly_start_ts = get_window_start(date, 29);
    let record_date = Utc
        .from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
        .to_rfc3339();

    let overall_totals = collect_overall_totals().await?;
    let daily_metrics = collect_daily_metrics(start_ts, end_ts).await?;
    let weekly_active_users = collect_active_users(weekly_start_ts, end_ts).await?;
    let monthly_active_users = collect_active_users(monthly_start_ts, end_ts).await?;

    let churned_users_weekly = overall_totals.total_users - weekly_active_users;
    let churned_users_monthly = overall_totals.total_users - monthly_active_users;

    Ok(MetricsRecord {
        date: record_date,
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
        active_users_weekly: weekly_active_users,
        active_users_monthly: monthly_active_users,
        churned_users_weekly,
        churned_users_monthly,
    })
}

// --- Main loop: Process missing days from a start date until yesterday ---

pub async fn run_metrics() -> Result<(), Box<dyn Error>> {
    tracing::debug!("Starting observability metrics collection...");

    let today_midnight = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    let target_date = today_midnight - Duration::days(1);

    let file_path = "metrics.csv";
    let start_date = if Path::new(file_path).exists() {
        let mut rdr = csv::Reader::from_path(file_path)?;
        let records: Vec<MetricsRecord> = rdr.deserialize().collect::<Result<_, _>>()?;
        if let Some(last_record) = records.last() {
            let last_date = chrono::DateTime::parse_from_rfc3339(&last_record.date)?
                .with_timezone(&Utc)
                .date_naive();
            last_date + Duration::days(1)
        } else {
            NaiveDate::from_ymd_opt(EPOCH_YEAR, EPOCH_MONTH, EPOCH_DAY).unwrap()
        }
    } else {
        NaiveDate::from_ymd_opt(EPOCH_YEAR, EPOCH_MONTH, EPOCH_DAY).unwrap()
    };

    let mut current_date = start_date;
    while current_date <= target_date.into() {
        let record = process_day(current_date).await?;
        tracing::debug!("Metrics record for {} built: {:?}", record.date, record);
        write_metrics(record)?;
        current_date = current_date + Duration::days(1);
    }
    tracing::debug!("All missing dates processed. Metrics up-to-date.");
    Ok(())
}
