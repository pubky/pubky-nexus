use crate::Error;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use utoipa::ToSchema;

pub const SEARCH_LIMIT_DEFAULT: usize = 20;

/// Pagination limit for search endpoints (1–MAX). Use as `SearchLimit<100>`.
#[derive(Debug, ToSchema)]
#[schema(value_type = u64, example = 20)]
pub struct SearchLimit<const MAX: usize>(pub usize);

impl<const MAX: usize> SearchLimit<MAX> {
    pub fn value(&self) -> usize {
        self.0
    }

    fn validate(n: usize) -> Result<(), Error> {
        if n == 0 {
            return Err(Error::invalid_input("limit must be at least 1"));
        }
        if n > MAX {
            return Err(Error::invalid_input(&format!(
                "limit exceeds maximum of {MAX}"
            )));
        }
        Ok(())
    }
}

impl<'de, const MAX: usize> Deserialize<'de> for SearchLimit<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let n = u64::deserialize(deserializer)?;
        let n = usize::try_from(n).map_err(|_| de::Error::custom("limit out of range"))?;
        Self::validate(n).map_err(de::Error::custom)?;
        Ok(SearchLimit(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Limit100 = SearchLimit<100>;

    fn deser(s: &str) -> Result<Limit100, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn accepts_valid_limit() {
        assert_eq!(deser("20").unwrap().value(), 20);
    }

    #[test]
    fn accepts_max_limit() {
        assert_eq!(deser("100").unwrap().value(), 100);
    }

    #[test]
    fn rejects_zero() {
        assert!(deser("0").is_err());
    }

    #[test]
    fn rejects_above_max() {
        assert!(deser("101").is_err());
    }

    #[test]
    fn different_max_is_enforced() {
        let result = serde_json::from_str::<SearchLimit<50>>("51");
        assert!(result.is_err());
        let result = serde_json::from_str::<SearchLimit<50>>("50");
        assert_eq!(result.unwrap().value(), 50);
    }
}
