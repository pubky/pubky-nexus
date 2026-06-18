use crate::Error;
use serde::de::{self, Deserializer};
use serde::Deserialize;

/// Pagination limit with compile-time default and maximum.
///
/// Deserializes from a string (query params are always strings).
/// Rejects 0 and values above MAX with a 400 error.
/// Absent query params resolve to DEFAULT via `Option::unwrap_or_default`.
#[derive(Debug, Clone)]
pub struct BoundedLimit<const DEFAULT: usize, const MAX: usize>(pub usize);

impl<const DEFAULT: usize, const MAX: usize> utoipa::PartialSchema for BoundedLimit<DEFAULT, MAX> {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        use utoipa::openapi::schema::{ObjectBuilder, SchemaType, Type};
        ObjectBuilder::new()
            .schema_type(SchemaType::new(Type::Integer))
            .minimum(Some(1usize))
            .maximum(Some(MAX))
            .default(Some(serde_json::json!(DEFAULT)))
            .into()
    }
}

impl<const DEFAULT: usize, const MAX: usize> utoipa::ToSchema for BoundedLimit<DEFAULT, MAX> {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Owned(format!("BoundedLimit_{}_{}", DEFAULT, MAX))
    }
}

impl<const DEFAULT: usize, const MAX: usize> BoundedLimit<DEFAULT, MAX> {
    pub fn value(&self) -> usize {
        self.0
    }

    fn validate(n: usize) -> Result<(), Error> {
        if n == 0 {
            return Err(Error::invalid_input("limit must be at least 1"));
        }
        if n > MAX {
            return Err(Error::invalid_input(format!(
                "limit exceeds maximum of {MAX}"
            )));
        }
        Ok(())
    }
}

impl<const DEFAULT: usize, const MAX: usize> Default for BoundedLimit<DEFAULT, MAX> {
    fn default() -> Self {
        BoundedLimit(DEFAULT)
    }
}

impl<'de, const DEFAULT: usize, const MAX: usize> Deserialize<'de> for BoundedLimit<DEFAULT, MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl de::Visitor<'_> for V {
            type Value = usize;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string containing a positive integer")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<usize, E> {
                let n: u64 = v.parse().map_err(de::Error::custom)?;
                usize::try_from(n).map_err(|_| de::Error::custom("limit out of range"))
            }
        }
        let n = deserializer.deserialize_str(V)?;
        Self::validate(n).map_err(de::Error::custom)?;
        Ok(BoundedLimit(n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Limit50_200 = BoundedLimit<50, 200>;

    fn deser(s: &str) -> Result<Limit50_200, serde_json::Error> {
        serde_json::from_str(s)
    }

    #[test]
    fn accepts_valid() {
        assert_eq!(deser("\"100\"").unwrap().value(), 100);
    }

    #[test]
    fn accepts_max() {
        assert_eq!(deser("\"200\"").unwrap().value(), 200);
    }

    #[test]
    fn accepts_one() {
        assert_eq!(deser("\"1\"").unwrap().value(), 1);
    }

    #[test]
    fn rejects_zero() {
        assert!(deser("\"0\"").is_err());
    }

    #[test]
    fn rejects_above_max() {
        assert!(deser("\"201\"").is_err());
    }

    #[test]
    fn default_is_compile_time_param() {
        assert_eq!(Limit50_200::default().value(), 50);
    }

    #[test]
    fn different_bounds_enforced() {
        assert!(serde_json::from_str::<BoundedLimit<5, 50>>("\"51\"").is_err());
        assert_eq!(
            serde_json::from_str::<BoundedLimit<5, 50>>("\"50\"")
                .unwrap()
                .value(),
            50
        );
        assert_eq!(BoundedLimit::<5, 50>::default().value(), 5);
    }
}
