use std::fmt;
use std::ops::Deref;

use crate::Error;
use pubky_app_specs::validate_crockford_id;
use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

use super::bounded_vec;

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

/// JSON array of file URI strings (min=1, max=100).
#[derive(Debug, ToSchema)]
pub struct FileUris(pub Vec<String>);

impl Deref for FileUris {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FileUris {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_json_array::<String, D, 1, 100>(d).map(Self)
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

    // --- FileUris tests ---

    fn make_uris(count: usize) -> String {
        let items: Vec<String> = (0..count).map(|i| format!("uri_{}", i)).collect();
        serde_json::to_string(&items).unwrap()
    }

    #[test]
    fn test_file_uris_rejects_empty_array() {
        let result: Result<FileUris, _> = serde_json::from_str("[]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least 1 item(s) required"));
    }

    #[test]
    fn test_file_uris_accepts_one_uri() {
        let result: Result<FileUris, _> = serde_json::from_str(r#"["single_uri"]"#);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.len(), 1);
    }

    #[test]
    fn test_file_uris_accepts_100_uris() {
        let json = make_uris(100);
        let result: Result<FileUris, _> = serde_json::from_str(&json);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0.len(), 100);
    }

    #[test]
    fn test_file_uris_rejects_101_uris() {
        let json = make_uris(101);
        let result: Result<FileUris, _> = serde_json::from_str(&json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Maximum 100 items allowed"));
    }
}
