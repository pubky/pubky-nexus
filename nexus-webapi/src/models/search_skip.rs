use crate::Error;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use utoipa::ToSchema;

/// Pagination skip for search endpoints (0–MAX). Use as `SearchSkip<1000>`.
#[derive(Debug, ToSchema)]
#[schema(value_type = u64, example = 0)]
pub struct SearchSkip<const MAX: usize>(pub usize);

impl<const MAX: usize> SearchSkip<MAX> {
    pub fn value(&self) -> usize {
        self.0
    }

    fn validate(n: usize) -> Result<(), Error> {
        if n > MAX {
            return Err(Error::invalid_input(format!(
                "skip exceeds maximum of {MAX}"
            )));
        }
        Ok(())
    }
}

impl<'de, const MAX: usize> Deserialize<'de> for SearchSkip<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let n = u64::deserialize(deserializer)?;
        let n = usize::try_from(n).map_err(|_| de::Error::custom("skip out of range"))?;
        Self::validate(n).map_err(de::Error::custom)?;
        Ok(SearchSkip(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Skip1k = SearchSkip<1000>;

    fn deser(s: &str) -> Result<Skip1k, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn accepts_zero() {
        assert_eq!(deser("0").unwrap().value(), 0);
    }

    #[test]
    fn accepts_max() {
        assert_eq!(deser("1000").unwrap().value(), 1000);
    }

    #[test]
    fn rejects_above_max() {
        assert!(deser("1001").is_err());
    }

    #[test]
    fn different_max_is_enforced() {
        assert!(serde_json::from_str::<SearchSkip<500>>("501").is_err());
        assert_eq!(
            serde_json::from_str::<SearchSkip<500>>("500")
                .unwrap()
                .value(),
            500
        );
    }
}
