use chrono::{DateTime, Utc};
use nexus_common::db::fetch_all_rows_from_graph;
use nexus_common::models::error::ModelResult;
use nexus_common::types::DynError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::neo4j::queries::{read_trust_scores, trust_report_user_details};

// Per-process counter so two reports in the same second get distinct names.
static REPORT_SEQ: AtomicU64 = AtomicU64::new(0);

/// Reads the `trust` scores written by the last recompute, highest first.
/// Used by the CSV report and callers needing scores back after a
/// [`super::TrustRankEngine::compute`].
pub async fn read_scores() -> ModelResult<Vec<(String, f64)>> {
    let rows = fetch_all_rows_from_graph(read_trust_scores()).await?;
    let mut scores = Vec::with_capacity(rows.len());
    for row in rows {
        let user_id: String = row.get("user_id")?;
        let score: f64 = row.get("score")?;
        scores.push((user_id, score));
    }
    Ok(scores)
}

#[derive(Default, Clone)]
struct UserReportRow {
    name: String,
    following: i64,
    followers: i64,
    friends: i64,
    posts: i64,
    replies: i64,
    collections: i64,
    bookmarks: i64,
    tagged: i64,
    tags: i64,
    unique_tags: i64,
}

/// Writes a CSV report joining trust scores with user details and counts, for
/// investigating who ranked where and why. Read straight from the graph (not
/// Redis), so it reflects current state even where the cache is stale or empty.
///
/// Columns: id, name, score, followers, following, friends, posts, replies,
/// tagged, tags, unique_tags, bookmarks, collections. `scores` must be
/// highest-first (as [`super::read_scores`] returns); rows preserve that order.
pub async fn write_csv(path: &Path, scores: &[(String, f64)]) -> Result<(), DynError> {
    let ids: Vec<String> = scores.iter().map(|(id, _)| id.clone()).collect();

    let rows = fetch_all_rows_from_graph(trust_report_user_details(&ids)).await?;

    let mut details: HashMap<String, UserReportRow> = HashMap::with_capacity(rows.len());
    for row in rows {
        let id: String = row.get("id")?;
        let name: Option<String> = row.get("name").unwrap_or(None);
        details.insert(
            id,
            UserReportRow {
                name: name.unwrap_or_default(),
                following: row.get("following").unwrap_or_default(),
                followers: row.get("followers").unwrap_or_default(),
                friends: row.get("friends").unwrap_or_default(),
                posts: row.get("posts").unwrap_or_default(),
                replies: row.get("replies").unwrap_or_default(),
                collections: row.get("collections").unwrap_or_default(),
                bookmarks: row.get("bookmarks").unwrap_or_default(),
                tagged: row.get("tagged").unwrap_or_default(),
                tags: row.get("tags").unwrap_or_default(),
                unique_tags: row.get("unique_tags").unwrap_or_default(),
            },
        );
    }

    let mut csv = String::from(
        "id,name,score,followers,following,friends,posts,replies,tagged,tags,unique_tags,bookmarks,collections\n",
    );
    for (id, score) in scores {
        // Every requested id yields a row (OPTIONAL MATCH), but default missing.
        let d = details.get(id).cloned().unwrap_or_default();

        csv.push_str(&csv_field(id));
        csv.push(',');
        csv.push_str(&csv_field(&d.name));
        csv.push(',');
        csv.push_str(&format!(
            "{score},{},{},{},{},{},{},{},{},{},{}\n",
            d.followers,
            d.following,
            d.friends,
            d.posts,
            d.replies,
            d.tagged,
            d.tags,
            d.unique_tags,
            d.bookmarks,
            d.collections,
        ));
    }

    // Create the target dir so a write into a missing dir doesn't fail.
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent).await?;
        }
    }
    tokio::fs::write(path, csv).await?;
    Ok(())
}

/// Writes a scheduled-run report into `dir` as `trust-report-<UTC timestamp>.csv`
/// (columns per [`write_csv`]), creating `dir` if missing. Returns the path.
pub async fn write_timestamped_csv(
    dir: &Path,
    scores: &[(String, f64)],
) -> Result<PathBuf, DynError> {
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join(report_file_name(Utc::now()));
    write_csv(&path, scores).await?;
    Ok(path)
}

// Second-precision timestamp stays readable/sortable; the per-process counter
// makes the name unique so two runs in the same second can't overwrite it.
fn report_file_name(now: DateTime<Utc>) -> String {
    format!(
        "trust-report-{}-{:06}.csv",
        now.format("%Y%m%dT%H%M%SZ"),
        REPORT_SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

/// Quotes a CSV field per RFC 4180 and neutralizes spreadsheet formula
/// injection: a leading trigger char (`= + - @` etc.) gets an `'` prefix.
/// Tradeoff: a legit name starting with `-` gains an apostrophe — fine here.
fn csv_field(s: &str) -> String {
    let s = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Filenames are timestamped, filesystem-safe (no `:` or spaces), and the
    /// per-process counter makes two same-second runs distinct.
    #[test]
    fn test_report_file_name() {
        let ts = DateTime::parse_from_rfc3339("2026-07-08T03:00:05Z")
            .unwrap()
            .with_timezone(&Utc);

        let first = report_file_name(ts);
        assert!(first.starts_with("trust-report-20260708T030005Z-"));
        assert!(first.ends_with(".csv"));

        // The counter segment is numeric.
        let seq = first
            .strip_prefix("trust-report-20260708T030005Z-")
            .and_then(|s| s.strip_suffix(".csv"))
            .expect("name has the expected prefix/suffix");
        assert!(seq.chars().all(|c| c.is_ascii_digit()));

        // Same timestamp, second run → different name (no overwrite).
        assert_ne!(first, report_file_name(ts));
    }

    /// Formula trigger chars get a leading `'` so they don't execute on open.
    #[test]
    fn csv_field_neutralizes_formula_triggers() {
        assert_eq!(csv_field("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(csv_field("+1"), "'+1");
        assert_eq!(csv_field("@foo"), "'@foo");
    }

    /// Neutralization composes with RFC 4180 quoting.
    #[test]
    fn csv_field_neutralization_composes_with_quoting() {
        assert_eq!(csv_field("=a,b"), "\"'=a,b\"");
    }

    /// Fields without a trigger char are unchanged.
    #[test]
    fn csv_field_leaves_safe_fields_untouched() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a\"b"), "\"a\"\"b\"");
    }
}
