use std::fmt;
use std::ops::Deref;

use crate::Error;
use serde::de::{self, Deserializer};
use utoipa::ToSchema;

/// Username search prefix (must be non-empty).
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "alice")]
pub struct UsernamePrefix(pub String);

impl fmt::Display for UsernamePrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for UsernamePrefix {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl UsernamePrefix {
    fn validate(prefix: &str) -> Result<(), Error> {
        if prefix.is_empty() {
            return Err(Error::invalid_input("Username cannot be empty"));
        }
        Ok(())
    }
}

impl<'de> serde::Deserialize<'de> for UsernamePrefix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl TryFrom<String> for UsernamePrefix {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let s = s.trim().to_owned();
        Self::validate(&s)?;
        Ok(UsernamePrefix(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate tests ---

    #[test]
    fn validate_rejects_empty_string() {
        assert!(UsernamePrefix::validate("").is_err());
    }

    #[test]
    fn validate_accepts_whitespace_only() {
        // Whitespace-only strings are non-empty, so they are accepted
        assert!(UsernamePrefix::validate("   ").is_ok());
        assert!(UsernamePrefix::validate("\t\n").is_ok());
    }

    #[test]
    fn validate_accepts_non_empty_string() {
        assert!(UsernamePrefix::validate("alice").is_ok());
        // " alice " is now accepted as-is (8 chars, not empty)
        assert!(UsernamePrefix::validate(" alice ").is_ok());
    }

    // --- TryFrom<String> tests ---

    #[test]
    fn try_from_success() {
        let prefix = UsernamePrefix::try_from("alice".to_string()).unwrap();
        assert_eq!(prefix.0, "alice");
    }

    #[test]
    fn try_from_fails_on_empty() {
        let err = UsernamePrefix::try_from("".to_string()).unwrap_err();
        assert!(matches!(err, Error::InvalidInput { .. }));
    }

    // --- Deserialize tests ---

    #[test]
    fn deserialize_accepts_valid_string() {
        let prefix: UsernamePrefix = serde_json::from_str("\"alice\"").unwrap();
        assert_eq!(prefix.0, "alice");
    }

    #[test]
    fn deserialize_trims_whitespace() {
        let prefix: UsernamePrefix = serde_json::from_str("\"  alice  \"").unwrap();
        assert_eq!(prefix.0, "alice");
    }

    #[test]
    fn deserialize_rejects_empty_string() {
        let result: Result<UsernamePrefix, _> = serde_json::from_str("\"\"");
        assert!(result.is_err());
    }

    #[test]
    fn deserialize_rejects_whitespace_only() {
        // "   " trims to "" -> empty -> invalid
        let result: Result<UsernamePrefix, _> = serde_json::from_str("\"   \"");
        assert!(result.is_err());
    }
}
