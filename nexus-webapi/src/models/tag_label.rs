use std::fmt;
use std::ops::Deref;

use crate::Error;
use pubky_app_specs::traits::Validatable;
use pubky_app_specs::PubkyAppTag;
use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, ToSchema)]
pub struct TagLabel(pub String);

crate::path_extractor_impl!(TagLabel);

impl fmt::Display for TagLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for TagLabel {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TagLabel {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate_and_sanitize(label: String) -> Result<String, Error> {
        let temp_tag = PubkyAppTag::new(
            "pubky://user_pubky_id/pub/pubky.app/profile.json".into(),
            label,
        );
        temp_tag
            .validate(None)
            .map_err(|e| Error::invalid_input(&e.to_string()))?;
        Ok(temp_tag.label)
    }
}

impl<'de> Deserialize<'de> for TagLabel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl TryFrom<String> for TagLabel {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(TagLabel(Self::validate_and_sanitize(s)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_valid_label() {
        let result = TagLabel::try_from("rust".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "rust");
    }

    #[test]
    fn try_from_sanitizes_whitespace_and_case() {
        let result = TagLabel::try_from("  Rust  ".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "rust");
    }

    #[test]
    fn try_from_rejects_empty() {
        assert!(TagLabel::try_from("".to_string()).is_err());
    }

    #[test]
    fn try_from_rejects_too_long() {
        assert!(TagLabel::try_from("a".repeat(21)).is_err());
    }

    #[test]
    fn try_from_accepts_max_length() {
        assert!(TagLabel::try_from("a".repeat(20)).is_ok());
    }

    #[test]
    fn try_from_rejects_comma() {
        assert!(TagLabel::try_from("tag,other".to_string()).is_err());
    }

    #[test]
    fn try_from_rejects_colon() {
        assert!(TagLabel::try_from("tag:other".to_string()).is_err());
    }

    #[test]
    fn try_from_rejects_internal_space() {
        assert!(TagLabel::try_from("co ol".to_string()).is_err());
    }

    #[test]
    fn try_from_rejects_tab() {
        assert!(TagLabel::try_from("tag\tother".to_string()).is_err());
    }

    #[test]
    fn deserialize_valid_label() {
        let label: TagLabel = serde_json::from_str("\"rust\"").unwrap();
        assert_eq!(label.0, "rust");
    }

    #[test]
    fn deserialize_sanitizes() {
        let label: TagLabel = serde_json::from_str("\"  Rust  \"").unwrap();
        assert_eq!(label.0, "rust");
    }

    #[test]
    fn deserialize_rejects_invalid() {
        let result: Result<TagLabel, _> = serde_json::from_str("\"a,b\"");
        assert!(result.is_err());
    }

    #[test]
    fn display_trait() {
        let label = TagLabel("rust".to_string());
        assert_eq!(format!("{}", label), "rust");
    }
}
