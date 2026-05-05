use std::fmt;
use std::str::FromStr;

use serde::{de, Deserialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, ToSchema)]
pub struct ResourceId(pub String);

impl FromStr for ResourceId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::validate_id(s)?;
        Ok(ResourceId(s.to_owned()))
    }
}

impl<'de> Deserialize<'de> for ResourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl ResourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate_id(id: &str) -> Result<(), String> {
        if id.len() != 32
            || !id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(format!(
                "resource_id must be 32-char lowercase hex, got: {id}"
            ));
        }
        Ok(())
    }
}

impl TryFrom<String> for ResourceId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate_id(&s)?;
        Ok(ResourceId(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_resource_id() -> String {
        "0123456789abcdef0123456789abcdef".to_string()
    }

    // -- validate_id unit tests --

    #[test]
    fn test_validate_id_valid_lowercase_hex() {
        let id = valid_resource_id();
        assert_eq!(id.len(), 32);
        assert!(ResourceId::validate_id(&id).is_ok());
    }

    #[test]
    fn test_validate_id_valid_all_zeros() {
        let id = "00000000000000000000000000000000".to_string();
        assert!(ResourceId::validate_id(&id).is_ok());
    }

    #[test]
    fn test_validate_id_valid_all_ffs() {
        let id = "ffffffffffffffffffffffffffffffff".to_string();
        assert!(ResourceId::validate_id(&id).is_ok());
    }

    #[test]
    fn test_validate_id_too_short() {
        let id = "0123456789abcdef0123456789abcde"; // 31 chars
        assert_eq!(id.len(), 31);
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be 32-char"));
    }

    #[test]
    fn test_validate_id_too_long() {
        let id = "0123456789abcdef0123456789abcdef0"; // 33 chars
        assert_eq!(id.len(), 33);
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be 32-char"));
    }

    #[test]
    fn test_validate_id_contains_uppercase() {
        let id = "0123456789ABCDEF0123456789abcdef".to_string();
        assert_eq!(id.len(), 32);
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("lowercase hex"));
    }

    #[test]
    fn test_validate_id_all_uppercase() {
        let id = "0123456789ABCDEF0123456789ABCDEF".to_string();
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_id_contains_invalid_chars() {
        let id = "0123456789abcdef0123456789abcdeg".to_string();
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_id_contains_space() {
        let id = "0123456789abcdef 23456789abcdef".to_string();
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_id_empty_string() {
        let result = ResourceId::validate_id("");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_id_with_special_chars() {
        let id = "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!".to_string();
        let result = ResourceId::validate_id(&id);
        assert!(result.is_err());
    }

    // -- TryFrom<String> tests --

    #[test]
    fn test_try_from_valid_id() {
        let id = valid_resource_id();
        let result = ResourceId::try_from(id);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "0123456789abcdef0123456789abcdef");
    }

    #[test]
    fn test_try_from_invalid_uppercase_id() {
        let id = "0123456789ABCDEF0123456789ABCDEF".to_string();
        let result = ResourceId::try_from(id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("lowercase hex"));
    }
}
