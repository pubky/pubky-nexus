use crate::db::get_redis_conn;
use crate::db::kv::error::{RedisError, RedisResult};
use tracing::warn;

/// Creates a RediSearch JSON index with a single TEXT field.
/// Idempotent: treats "Index already exists" as success so concurrent callers are safe.
pub(crate) async fn ft_create_json_text_index(
    index_name: &str,
    prefix: &str,
    field_path: &str,
    field_alias: &str,
) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;

    let result = deadpool_redis::redis::cmd("FT.CREATE")
        .arg(index_name)
        .arg("ON")
        .arg("JSON")
        .arg("PREFIX")
        .arg("1")
        .arg(prefix)
        .arg("SCHEMA")
        .arg(field_path)
        .arg("AS")
        .arg(field_alias)
        .arg("TEXT")
        .query_async::<()>(&mut conn)
        .await;

    match result {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("Index already exists") => Ok(()),
        Err(e) => Err(RedisError::CommandFailed(e.to_string().into())),
    }
}

/// Full-text search returning `(redis_key, score)` pairs ordered by relevance.
/// Keys are returned as-is (including any Redis prefix); the caller strips the prefix.
pub(crate) async fn ft_search_scored(
    index_name: &str,
    query: &str,
    skip: usize,
    limit: usize,
) -> RedisResult<Vec<(String, f64)>> {
    let sanitized = sanitize_query(query);
    if sanitized.is_empty() {
        return Ok(vec![]);
    }

    let ft_query = sanitized
        .split_whitespace()
        .map(fuzzy_token)
        .collect::<Vec<_>>()
        .join(" ");

    let mut conn = get_redis_conn().await?;

    let raw: deadpool_redis::redis::Value = deadpool_redis::redis::cmd("FT.SEARCH")
        .arg(index_name)
        .arg(&ft_query)
        .arg("NOCONTENT")
        .arg("WITHSCORES")
        .arg("LIMIT")
        .arg(skip)
        .arg(limit)
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
            warn!("Unexpected FT.SEARCH response shape, expected Array (RESP2); got a different type — RESP3 may be active");
            return Ok(vec![]);
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
    use super::sanitize_query;

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
}
