use std::fmt;
use std::str::FromStr;

use crate::Error;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use utoipa::ToSchema;

pub const MIN_POST_SEARCH_QUERY_LEN: usize = 2;
pub const MAX_POST_SEARCH_QUERY_LEN: usize = 200;

/// Post content search query (2–200 characters).
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "bitcoin")]
pub struct PostSearchQuery(pub String);

impl FromStr for PostSearchQuery {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::validate(s)?;
        Ok(PostSearchQuery(s.to_owned()))
    }
}

impl fmt::Display for PostSearchQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PostSearchQuery {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(q: &str) -> Result<(), Error> {
        let len = q.chars().count();
        if len < MIN_POST_SEARCH_QUERY_LEN {
            return Err(Error::invalid_input(format!(
                "Search query must be at least {MIN_POST_SEARCH_QUERY_LEN} characters"
            )));
        }
        if len > MAX_POST_SEARCH_QUERY_LEN {
            return Err(Error::invalid_input(format!(
                "Search query exceeds maximum length of {MAX_POST_SEARCH_QUERY_LEN} characters"
            )));
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for PostSearchQuery {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl TryFrom<String> for PostSearchQuery {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate(&s)?;
        Ok(PostSearchQuery(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(PostSearchQuery::try_from("".to_string()).is_err());
    }

    #[test]
    fn rejects_one_char() {
        assert!(PostSearchQuery::try_from("a".to_string()).is_err());
    }

    #[test]
    fn accepts_min_length() {
        assert!(PostSearchQuery::try_from("ab".to_string()).is_ok());
    }

    #[test]
    fn rejects_over_limit() {
        assert!(PostSearchQuery::try_from("a".repeat(201)).is_err());
    }

    #[test]
    fn accepts_max_length() {
        assert!(PostSearchQuery::try_from("a".repeat(200)).is_ok());
    }

    #[test]
    fn accepts_normal_query() {
        let q = PostSearchQuery::try_from("bitcoin price".to_string()).unwrap();
        assert_eq!(q.0, "bitcoin price");
    }
}
