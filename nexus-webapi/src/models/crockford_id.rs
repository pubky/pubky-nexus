use std::fmt;
use std::ops::Deref;

use crate::Error;
use pubky_app_specs::validate_crockford_id;
use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

/// 13-character Crockford Base32 ID newtype, used for post and file IDs.
macro_rules! crockford_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, ToSchema)]
        #[schema(value_type = String, example = "00000039YD9DP")]
        pub struct $name(pub String);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl Deref for $name {
            type Target = str;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                validate_crockford_id(&s).map_err(Error::invalid_input)?;
                Ok($name(s))
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::try_from(s).map_err(de::Error::custom)
            }
        }
    };
}

crockford_id!(PostId, "13-character Crockford Base32-encoded post ID.");
crockford_id!(FileId, "13-character Crockford Base32-encoded file ID.");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_id() {
        let id = "00000039YD9C0".to_string();
        assert_eq!(id.len(), 13);
        assert!(PostId::try_from(id).is_ok());
    }

    #[test]
    fn rejects_too_short() {
        let result = PostId::try_from("ABCDEFG".to_string());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid ID length"));
    }

    #[test]
    fn rejects_too_long() {
        let result = PostId::try_from("00000039YD9C0X".to_string());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid ID length"));
    }

    #[test]
    fn rejects_invalid_encoding() {
        let result = PostId::try_from("!!!!!!!!!!!!!".to_string());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid Crockford Base32 encoding"));
    }

    #[test]
    fn rejects_empty_and_spaces() {
        assert!(PostId::try_from(String::new()).is_err());
        assert!(PostId::try_from("ABCDEFG HIJKL".to_string()).is_err());
    }

    #[test]
    fn file_id_shares_the_same_validation() {
        assert!(FileId::try_from("00000039YD9C0".to_string()).is_ok());
        assert!(FileId::try_from("ABCDEFG".to_string()).is_err());
    }
}
