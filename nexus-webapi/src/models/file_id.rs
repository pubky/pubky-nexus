use std::fmt;
use std::ops::Deref;

use crate::Error;
use pubky_app_specs::validate_crockford_id;
use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

/// 13-character Crockford Base32-encoded file ID.
#[derive(Debug, ToSchema)]
#[schema(value_type = String, example = "00000039YD9DP")]
pub struct FileId(pub String);

impl fmt::Display for FileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for FileId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl FileId {
    /// Validates that the provided ID is a valid Crockford Base32-encoded timestamp,
    /// 13 characters long, and decodes to 8 bytes.
    fn validate_id(id: &str) -> Result<(), Error> {
        validate_crockford_id(id).map_err(|e| Error::invalid_input(&e))?;
        Ok(())
    }
}

impl TryFrom<String> for FileId {
    type Error = Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate_id(&s)?;
        Ok(FileId(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_from_valid_id() {
        let id = "00000039YD9C0".to_string();
        assert_eq!(id.len(), 13);
        assert!(FileId::try_from(id).is_ok());
    }

    #[test]
    fn test_try_from_invalid_length_too_short() {
        let result = FileId::try_from("ABCDEFG".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid ID length"));
    }

    #[test]
    fn test_try_from_invalid_length_too_long() {
        let id = "00000039YD9C0".to_string() + "X";
        assert_eq!(id.len(), 14);
        let result = FileId::try_from(id);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid ID length"));
    }

    #[test]
    fn test_try_from_invalid_encoding() {
        let result = FileId::try_from("!!!!!!!!!!!!!".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid Crockford Base32 encoding"));
    }

    #[test]
    fn test_validate_id_empty_string() {
        assert!(FileId::try_from("".to_string()).is_err());
    }

    #[test]
    fn test_validate_id_with_space() {
        let result = FileId::try_from("ABCDEFG HIJKL".to_string());
        assert!(result.is_err());
    }
}
