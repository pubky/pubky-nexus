use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

use serde::{de, Deserialize};
use utoipa::ToSchema;

#[derive(ToSchema, Debug)]
pub struct PubkyId(pub pubky_app_specs::PubkyId);

impl FromStr for PubkyId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PubkyId(pubky_app_specs::PubkyId::try_from(s)?))
    }
}

impl<'de> Deserialize<'de> for PubkyId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl TryFrom<String> for PubkyId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        pubky_app_specs::PubkyId::try_from(s.as_str()).map(PubkyId)
    }
}

impl Deref for PubkyId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl fmt::Display for PubkyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.deref(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_string_valid() {
        let valid_key = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo".to_string();
        let result = PubkyId::try_from(valid_key.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.as_ref(), valid_key);
    }

    #[test]
    fn test_try_from_string_invalid_length() {
        let invalid_key = "short".to_string();
        let result = PubkyId::try_from(invalid_key);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Validation Error: the string is not 52 utf chars"
        );
    }
}
