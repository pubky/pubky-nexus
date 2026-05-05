use std::fmt::Display;

use serde::de::{self, Deserializer};
use serde::Deserialize;

/// Deserialize a comma-separated string into `Vec<T>`, enforcing `MIN..=MAX` length.
///
/// Tokens are trimmed and empty tokens are skipped before `T::try_from(String)` runs.
pub fn deserialize_csv<'de, T, D, const MIN: usize, const MAX: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<String>,
    <T as TryFrom<String>>::Error: Display,
{
    let s = String::deserialize(deserializer)?;
    let items: Vec<T> = s
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| T::try_from(t.to_string()).map_err(de::Error::custom))
        .collect::<Result<_, _>>()?;
    if MIN > 0 && items.is_empty() {
        return Err(de::Error::custom(format!("At least {MIN} item(s) required")));
    }
    if items.len() > MAX {
        return Err(de::Error::custom(format!("Maximum {MAX} items allowed")));
    }
    Ok(items)
}

/// Deserialize a JSON array into `Vec<T>`, validating `MIN..=MAX` length
/// before deserializing individual elements (fail-fast).
pub fn deserialize_json_array<'de, T, D, const MIN: usize, const MAX: usize>(
    deserializer: D,
) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: for<'des> Deserialize<'des>,
{
    let arr = match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Array(arr) => arr,
        _ => return Err(de::Error::custom("Expected an array")),
    };
    if MIN > 0 && arr.is_empty() {
        return Err(de::Error::custom(format!("At least {MIN} item(s) required")));
    }
    if arr.len() > MAX {
        return Err(de::Error::custom(format!("Maximum {MAX} items allowed")));
    }
    arr.into_iter()
        .map(|v| serde_json::from_value(v).map_err(de::Error::custom))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommaIds(Vec<String>);
    struct TestJsonIds(Vec<String>);

    impl<'de> Deserialize<'de> for TestCommaIds {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            deserialize_csv::<String, D, 0, 5>(d).map(Self)
        }
    }

    impl<'de> Deserialize<'de> for TestJsonIds {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            deserialize_json_array::<String, D, 0, 5>(d).map(Self)
        }
    }

    #[test]
    fn comma_deserialize_valid() {
        let ids: TestCommaIds = serde_json::from_str(r#""a, b, c""#).unwrap();
        assert_eq!(ids.0, vec!["a", "b", "c"]);
    }

    #[test]
    fn comma_allows_empty_when_min_is_0() {
        let ids: TestCommaIds = serde_json::from_str(r#""""#).unwrap();
        assert!(ids.0.is_empty());
    }

    #[test]
    fn comma_rejects_over_max() {
        let result: Result<TestCommaIds, _> = serde_json::from_str(r#""a,b,c,d,e,f""#);
        assert!(result.is_err());
    }

    #[test]
    fn json_deserialize_valid() {
        let ids: TestJsonIds = serde_json::from_str(r#"["a", "b", "c"]"#).unwrap();
        assert_eq!(ids.0, vec!["a", "b", "c"]);
    }

    #[test]
    fn json_allows_empty_when_min_is_0() {
        let ids: TestJsonIds = serde_json::from_str(r#"[]"#).unwrap();
        assert!(ids.0.is_empty());
    }

    #[test]
    fn json_rejects_over_max() {
        let result: Result<TestJsonIds, _> = serde_json::from_str(r#"["a","b","c","d","e","f"]"#);
        assert!(result.is_err());
    }
}
