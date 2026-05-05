use std::fmt;
use std::ops::Deref;

use base32::{decode, Alphabet};
use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::models::bounded_vec;

#[derive(Debug, ToSchema)]
pub struct PostId(pub String);

/// Comma-separated list of `PostId` values (min=0, max=100).
#[derive(Debug, ToSchema)]
pub struct PostIds(pub Vec<PostId>);

impl Deref for PostIds {
    type Target = Vec<PostId>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PostIds {
    fn deserialize<D: serde::de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        bounded_vec::deserialize_csv::<PostId, D, 0, 100>(d).map(Self)
    }
}

impl fmt::Display for PostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for PostId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for PostId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl PostId {
    /// Validates that the provided ID is a valid Crockford Base32-encoded timestamp,
    /// 13 characters long, and decodes to 8 bytes.
    fn validate_id(id: &str) -> Result<(), String> {
        if id.len() != 13 {
            return Err("Validation Error: Invalid ID length: must be 13 characters".into());
        }

        let decoded_bytes = decode(Alphabet::Crockford, id)
            .ok_or("Validation Error: Invalid Crockford Base32 encoding")?;

        if decoded_bytes.len() != 8 {
            return Err("Validation Error: Invalid ID length after decoding".into());
        }

        Ok(())
    }
}

impl TryFrom<String> for PostId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate_id(&s)?;
        Ok(PostId(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base32::{encode, Alphabet};

    fn valid_post_id() -> String {
        encode(Alphabet::Crockford, &1727740800000000i64.to_be_bytes())
    }

    #[test]
    fn test_try_from_valid_id() {
        let id = valid_post_id();
        assert_eq!(id.len(), 13);
        assert!(PostId::try_from(id).is_ok());
    }

    #[test]
    fn test_try_from_invalid_length_too_short() {
        let result = PostId::try_from("ABCDEFG".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid ID length"));
    }

    #[test]
    fn test_try_from_invalid_length_too_long() {
        let id = valid_post_id() + "X";
        assert_eq!(id.len(), 14);
        let result = PostId::try_from(id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid ID length"));
    }

    #[test]
    fn test_try_from_invalid_encoding() {
        let result = PostId::try_from("!!!!!!!!!!!!!".to_string());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("Invalid Crockford Base32 encoding"));
    }

    #[test]
    fn test_validate_id_empty_string() {
        assert!(PostId::try_from("".to_string()).is_err());
    }

    #[test]
    fn test_validate_id_with_space() {
        let result = PostId::try_from("ABCDEFG HIJKL".to_string());
        assert!(result.is_err());
    }
}
