use std::fmt;
use std::str::FromStr;

use crate::Error;
use serde::{de, Deserialize};
use utoipa::ToSchema;

pub const USER_ID_SEARCH_MIN_PREFIX_LEN: usize = 3;

/// User-ID search prefix (minimum 3 characters).
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "ope")]
pub struct UserIdPrefix(pub String);

impl<'de> Deserialize<'de> for UserIdPrefix {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl FromStr for UserIdPrefix {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_owned();
        Self::validate(&s)?;
        Ok(UserIdPrefix(s))
    }
}

impl fmt::Display for UserIdPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl UserIdPrefix {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(prefix: &str) -> Result<(), Error> {
        if prefix.chars().count() < USER_ID_SEARCH_MIN_PREFIX_LEN {
            return Err(Error::invalid_input(&format!(
                "ID prefix must be at least {USER_ID_SEARCH_MIN_PREFIX_LEN} chars"
            )));
        }
        Ok(())
    }
}

impl TryFrom<String> for UserIdPrefix {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate(&s)?;
        Ok(UserIdPrefix(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate() tests ----

    #[test]
    fn test_validate_empty_string() {
        assert!(UserIdPrefix::validate("").is_err());
    }

    #[test]
    fn test_validate_single_char() {
        assert!(UserIdPrefix::validate("a").is_err());
    }

    #[test]
    fn test_validate_two_chars() {
        assert!(UserIdPrefix::validate("ab").is_err());
    }

    #[test]
    fn test_validate_exact_min_length() {
        assert!(UserIdPrefix::validate("abc").is_ok());
    }

    #[test]
    fn test_validate_above_min_length() {
        assert!(UserIdPrefix::validate("abcd").is_ok());
    }

    #[test]
    fn test_validate_whitespace_only() {
        // "   " is 3 chars (spaces count as characters), so it's valid
        assert!(UserIdPrefix::validate("   ").is_ok());
    }

    #[test]
    fn test_validate_whitespace_only_too_short() {
        // "  " is only 2 chars -> invalid
        assert!(UserIdPrefix::validate("  ").is_err());
    }

    #[test]
    fn test_validate_whitespace_padding() {
        // " abc " is 5 chars (including spaces) -> valid
        assert!(UserIdPrefix::validate(" abc ").is_ok());
    }

    #[test]
    fn test_validate_whitespace_padding_too_short() {
        // " ab " is 4 chars (including spaces) -> valid
        assert!(UserIdPrefix::validate(" ab ").is_ok());
    }

    #[test]
    fn test_validate_unicode_chars() {
        // Unicode chars are counted by .chars(), not bytes
        assert!(UserIdPrefix::validate("🔥🔥🔥").is_ok());
    }

    #[test]
    fn test_validate_unicode_chars_too_short() {
        assert!(UserIdPrefix::validate("🔥🔥").is_err());
    }

    #[test]
    fn test_validate_error_message_contains_min_length() {
        let result = UserIdPrefix::validate("ab");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains(&USER_ID_SEARCH_MIN_PREFIX_LEN.to_string()));
    }

    // ---- TryFrom<String> tests ----

    #[test]
    fn test_try_from_string_success() {
        let result = UserIdPrefix::try_from("abc123".to_string());
        assert!(result.is_ok());
        let prefix = result.unwrap();
        assert_eq!(prefix.0, "abc123");
    }

    #[test]
    fn test_try_from_string_failure_too_short() {
        let result = UserIdPrefix::try_from("ab".to_string());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput { .. }));
    }
}
