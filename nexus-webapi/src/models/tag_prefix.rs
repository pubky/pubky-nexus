use std::fmt;

use crate::Error;
use pubky_app_specs::traits::Validatable;
use pubky_app_specs::PubkyAppTag;
use utoipa::ToSchema;

#[derive(Debug, ToSchema)]
pub struct TagPrefix(pub String);

crate::path_extractor_impl!(TagPrefix);

impl fmt::Display for TagPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TagPrefix {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate_and_sanitize(prefix: String) -> Result<String, Error> {
        let temp_tag = PubkyAppTag::new(
            "pubky://user_pubky_id/pub/pubky.app/profile.json".into(),
            prefix,
        );
        temp_tag
            .validate(None)
            .map_err(|e| Error::invalid_input(&e.to_string()))?;
        Ok(temp_tag.label)
    }
}

impl TryFrom<String> for TagPrefix {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(TagPrefix(Self::validate_and_sanitize(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_valid_prefix() {
        let result = TagPrefix::try_from("rust".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "rust");
    }

    #[test]
    fn test_try_from_valid_prefix_with_sanitization() {
        let result = TagPrefix::try_from("  Rust  ".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "rust");
    }

    #[test]
    fn test_try_from_empty_string() {
        let result = TagPrefix::try_from("".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_too_long_prefix() {
        let result = TagPrefix::try_from("a".repeat(21));
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_prefix_with_comma() {
        let result = TagPrefix::try_from("tag,other".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_prefix_with_colon() {
        let result = TagPrefix::try_from("tag:other".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_prefix_with_internal_space() {
        let result = TagPrefix::try_from("co ol".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_prefix_with_tab() {
        let result = TagPrefix::try_from("tag\tother".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_try_from_max_length_prefix() {
        // 20 chars is the max allowed
        let result = TagPrefix::try_from("a".repeat(20));
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_from_min_length_prefix() {
        // 1 char is the min allowed
        let result = TagPrefix::try_from("a".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_display_trait() {
        let prefix = TagPrefix("test".to_string());
        assert_eq!(format!("{}", prefix), "test");
    }
}
