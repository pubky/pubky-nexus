use crate::db::config::FT_SEARCH_TIMEOUT_MS;
use crate::db::get_redis_conn;
use crate::db::kv::error::{RedisError, RedisResult};
use std::sync::OnceLock;
use tracing::warn;

static FT_SEARCH_TIMEOUT: OnceLock<usize> = OnceLock::new();

/// Sets the configured FT.SEARCH timeout (ms). Called once during stack setup.
pub(crate) fn set_ft_search_timeout_ms(ms: usize) {
    let _ = FT_SEARCH_TIMEOUT.set(ms);
}

fn ft_search_timeout_ms() -> usize {
    *FT_SEARCH_TIMEOUT.get().unwrap_or(&FT_SEARCH_TIMEOUT_MS)
}

/// Creates the post content index: $.content TEXT + $.author TAG CASESENSITIVE + $.kind TAG CASESENSITIVE.
/// NOOFFSETS/NOHL kept; NOFIELDS dropped to allow field-targeted queries.
/// Idempotent: short-circuits on "already exists".
pub(crate) async fn ft_create_post_content_index(prefix: &str) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;

    let result = deadpool_redis::redis::cmd("FT.CREATE")
        .arg("postContentIdx")
        .arg("ON")
        .arg("JSON")
        .arg("PREFIX")
        .arg("1")
        .arg(prefix)
        .arg("NOOFFSETS")
        .arg("NOHL")
        .arg("SCHEMA")
        .arg("$.content")
        .arg("AS")
        .arg("content")
        .arg("TEXT")
        .arg("$.author")
        .arg("AS")
        .arg("author")
        .arg("TAG")
        .arg("CASESENSITIVE")
        .arg("$.kind")
        .arg("AS")
        .arg("kind")
        .arg("TAG")
        .arg("CASESENSITIVE")
        .query_async::<()>(&mut conn)
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("already exists") => Ok(()),
        Err(e) => Err(RedisError::CommandFailed(e.to_string().into())),
    }
}

/// Drops the post content index without deleting the underlying documents.
/// Idempotent: swallows "Unknown index name" so repeated calls are safe.
pub(crate) async fn drop_post_content_index() -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;

    let result = deadpool_redis::redis::cmd("FT.DROPINDEX")
        .arg("postContentIdx")
        .query_async::<()>(&mut conn)
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("Unknown index name") => Ok(()),
        Err(e) => Err(RedisError::CommandFailed(e.to_string().into())),
    }
}

/// Assembles the RediSearch query string from a content fragment, optional author
/// filter, and optional kind filter.
///
/// * The **content** half runs through `sanitize_query` → `fuzzy_token` as before.
/// * The **author** half is assembled raw as `@author:{<id>}` and must NOT pass
///   through `sanitize_query` (which would turn `@` and `{` into spaces) or
///   `fuzzy_token` (which would %-escape the author id).
/// * The **kind** half is assembled raw as `@kind:{<kind>}` — the kind value is
///   the serde-serialized enum variant (e.g. "short", "long").
///
/// Returns `None` when the content half is empty — author/kind filters alone
/// must not degenerate into listing endpoints.
fn build_ft_query(content: &str, author: Option<&str>, kind: Option<&str>) -> Option<String> {
    let sanitized = sanitize_query(content);
    if sanitized.is_empty() {
        return None;
    }

    let fuzzy = sanitized
        .split_whitespace()
        .map(fuzzy_token)
        .collect::<Vec<_>>()
        .join(" ");

    let mut parts = Vec::with_capacity(3);
    if let Some(a) = author {
        parts.push(format!("@author:{{{a}}}"));
    }
    if let Some(k) = kind {
        parts.push(format!("@kind:{{{k}}}"));
    }
    parts.push(fuzzy);

    Some(parts.join(" "))
}

/// Full-text search returning `(redis_key, score)` pairs ordered by relevance.
/// Keys are returned as-is (including any Redis prefix); the caller strips the prefix.
///
/// When `author` is `Some`, results are scoped to posts by that author.
/// When `kind` is `Some`, results are further filtered to that post kind.
pub(crate) async fn ft_search_scored(
    index_name: &str,
    query: &str,
    author: Option<&str>,
    kind: Option<&str>,
    skip: usize,
    limit: usize,
) -> RedisResult<Vec<(String, f64)>> {
    let ft_query = match build_ft_query(query, author, kind) {
        Some(q) => q,
        None => return Ok(vec![]),
    };

    let mut conn = get_redis_conn().await?;

    let raw: deadpool_redis::redis::Value = deadpool_redis::redis::cmd("FT.SEARCH")
        .arg(index_name)
        .arg(&ft_query)
        .arg("NOCONTENT")
        .arg("WITHSCORES")
        .arg("LIMIT")
        .arg(skip)
        .arg(limit)
        .arg("TIMEOUT")
        .arg(ft_search_timeout_ms())
        .query_async(&mut conn)
        .await
        .map_err(|e| RedisError::CommandFailed(e.to_string().into()))?;

    parse_ft_search_response(raw)
}

fn token_fuzzy_distance(token: &str) -> usize {
    match token.chars().count() {
        n if n <= 3 => 0,
        n if n <= 8 => 1,
        _ => 2,
    }
}

fn fuzzy_token(token: &str) -> String {
    let distance = token_fuzzy_distance(token);
    if distance == 0 {
        token.to_string()
    } else {
        let pct = "%".repeat(distance);
        format!("{pct}{token}{pct}")
    }
}

fn sanitize_query(query: &str) -> String {
    // Replace non-alphanumeric chars with spaces so "web3.0" → "web3 0" and "e-mail" → "e mail",
    // matching RediSearch's punctuation-as-separator tokenization of indexed content.
    query
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parses the raw RESP2 reply from `FT.SEARCH … NOCONTENT WITHSCORES`.
/// Format: `[total_count, key1, score1, key2, score2, ...]`
fn parse_ft_search_response(raw: deadpool_redis::redis::Value) -> RedisResult<Vec<(String, f64)>> {
    let array = match raw {
        deadpool_redis::redis::Value::Array(a) => a,
        _ => {
            return Err(RedisError::CommandFailed(
                "unexpected FT.SEARCH reply shape (RESP3 may be active)".into(),
            ));
        }
    };

    // array[0] is the total count; real entries start at index 1
    if array.len() < 3 {
        return Ok(vec![]);
    }

    let mut results = Vec::new();
    let mut i = 1usize;

    while i + 1 < array.len() {
        let key = match &array[i] {
            deadpool_redis::redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
            deadpool_redis::redis::Value::SimpleString(s) => s.clone(),
            _ => {
                warn!("Unexpected FT.SEARCH key value at index {i}, truncating results");
                break;
            }
        };

        let score: f64 = match &array[i + 1] {
            deadpool_redis::redis::Value::BulkString(b) => {
                String::from_utf8_lossy(b).parse().unwrap_or(0.0)
            }
            deadpool_redis::redis::Value::SimpleString(s) => s.parse().unwrap_or(0.0),
            deadpool_redis::redis::Value::Double(d) => *d,
            _ => 0.0,
        };

        results.push((key, score));
        i += 2;
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::{build_ft_query, sanitize_query};

    #[test]
    fn punctuation_becomes_separator_not_glue() {
        // RediSearch tokenizes "web3.0" as ["web3", "0"] and "e-mail" as ["e", "mail"].
        // sanitize_query must produce the same split so query tokens align with indexed tokens.
        assert_eq!(sanitize_query("web3.0"), "web3 0");
        assert_eq!(sanitize_query("e-mail"), "e mail");
    }

    #[test]
    fn whitespace_is_normalized() {
        assert_eq!(sanitize_query("  hello   world  "), "hello world");
    }

    #[test]
    fn alphanumeric_passthrough() {
        assert_eq!(sanitize_query("bitcoin price"), "bitcoin price");
    }

    #[test]
    fn only_punctuation_becomes_empty() {
        assert_eq!(sanitize_query("!!!"), "");
    }

    // -----------------------------------------------------------------------
    // build_ft_query: brace escaping and sanitize-bypass correctness tests
    // -----------------------------------------------------------------------

    #[test]
    fn author_none_produces_content_only() {
        let q = build_ft_query("hello world", None, None);
        // Two tokens ≥ 5 chars → fuzzy distance 1 → %hello% %world%
        assert_eq!(q.as_deref(), Some("%hello% %world%"));
    }

    #[test]
    fn author_some_appends_braced_tag_filter() {
        let q = build_ft_query("hello", Some("user123"), None);
        // @author:{user123} must use double-{{ }} to produce single braces.
        // "hello" (5 chars) → %hello%
        assert_eq!(q.as_deref(), Some("@author:{user123} %hello%"));
    }

    #[test]
    fn author_only_empty_content_returns_none() {
        // Scoped search with empty content → None, not an author listing.
        // If this returned Some("@author:{alice}"), it would act as an
        // unbounded author-listing endpoint rather than a search.
        let q = build_ft_query("", Some("alice"), None);
        assert!(q.is_none());
    }

    #[test]
    fn author_only_all_punctuation_returns_none() {
        // q=".." passes PostSearchQuery validation (2 chars, 1 term) but
        // sanitize_query strips to empty. Must not become an author listing.
        let q = build_ft_query("..", Some("alice"), None);
        assert!(q.is_none());
    }

    #[test]
    fn both_empty_returns_none() {
        let q = build_ft_query("", None, None);
        assert!(q.is_none());
    }

    #[test]
    fn author_braces_are_not_sanitized() {
        // CRITICAL: if the author clause ever routes through sanitize_query,
        // "@author:{alice}" becomes "author alice" and the query degrades to garbage.
        // This test asserts the braces survive intact.
        let q = build_ft_query("test", Some("alice"), None);
        assert!(
            q.as_deref().map(|s| s.contains('@')).unwrap_or(false),
            "author clause must contain @"
        );
        assert!(
            q.as_deref().map(|s| s.contains('{')).unwrap_or(false),
            "author clause must contain open brace"
        );
        assert!(
            q.as_deref().map(|s| s.contains('}')).unwrap_or(false),
            "author clause must contain close brace"
        );
    }

    #[test]
    fn kind_some_appends_braced_tag_filter() {
        let q = build_ft_query("hello", None, Some("short"));
        assert_eq!(q.as_deref(), Some("@kind:{short} %hello%"));
    }

    #[test]
    fn author_and_kind_both_set() {
        let q = build_ft_query("hello", Some("user123"), Some("long"));
        assert_eq!(q.as_deref(), Some("@author:{user123} @kind:{long} %hello%"));
    }

    #[test]
    fn kind_only_empty_content_returns_none() {
        let q = build_ft_query("", None, Some("short"));
        assert!(q.is_none());
    }
}
