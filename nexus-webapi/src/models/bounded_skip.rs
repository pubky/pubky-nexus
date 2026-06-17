use crate::Error;
use serde::de::{self, Deserializer};
use serde::Deserialize;
use utoipa::ToSchema;

/// Pagination skip with compile-time maximum. Absent query params resolve to 0.
///
/// Deserializes from a string (query params are always strings).
/// Rejects values above MAX with a 400 error.
#[derive(Debug, ToSchema)]
#[schema(value_type = u64, example = 0)]
pub struct BoundedSkip<const MAX: usize>(pub usize);

impl<const MAX: usize> BoundedSkip<MAX> {
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

impl<const MAX: usize> Default for BoundedSkip<MAX> {
    fn default() -> Self {
        BoundedSkip(0)
    }
}

impl<'de, const MAX: usize> Deserialize<'de> for BoundedSkip<MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl de::Visitor<'_> for V {
            type Value = usize;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string containing a non-negative integer")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<usize, E> {
                let n: u64 = v.parse().map_err(de::Error::custom)?;
                usize::try_from(n).map_err(|_| de::Error::custom("skip out of range"))
            }
        }
        let n = deserializer.deserialize_str(V)?;
        Self::validate(n).map_err(de::Error::custom)?;
        Ok(BoundedSkip(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Skip10k = BoundedSkip<10_000>;

    fn deser(s: &str) -> Result<Skip10k, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn accepts_zero() {
        assert_eq!(deser("\"0\"").unwrap().value(), 0);
    }

    #[test]
    fn accepts_max() {
        assert_eq!(deser("\"10000\"").unwrap().value(), 10_000);
    }

    #[test]
    fn rejects_above_max() {
        assert!(deser("\"10001\"").is_err());
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Skip10k::default().value(), 0);
    }

    #[test]
    fn different_max_enforced() {
        assert!(serde_json::from_str::<BoundedSkip<500>>("\"501\"").is_err());
        assert_eq!(
            serde_json::from_str::<BoundedSkip<500>>("\"500\"")
                .unwrap()
                .value(),
            500
        );
    }
}
