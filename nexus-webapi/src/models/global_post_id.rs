use std::fmt;
use std::ops::Deref;

use serde::de;
use serde::Deserialize;
use utoipa::ToSchema;

use super::post_id::PostId;
use super::pubky_id::PubkyId;
use crate::define_bounded_vec;

#[derive(Debug, ToSchema)]
pub struct GlobalPostId(pub String);

define_bounded_vec!(
    name: GlobalPostIds,
    element_type: GlobalPostId,
    min: 1,
    max: 100,
    serialize_as: comma_separated_string,
);

impl fmt::Display for GlobalPostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Deref for GlobalPostId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl GlobalPostIds {
    /// Consumes self and returns a `Vec<String>` of the underlying ID strings.
    pub fn into_string_vec(self) -> Vec<String> {
        self.0.into_iter().map(|id| id.0).collect()
    }
}

impl GlobalPostId {
    fn validate(s: &str) -> Result<(), String> {
        let (pubky_part, post_part) = s.split_once(':').ok_or_else(|| {
            "Validation Error: GlobalPostId must be in the format '{PubkyId}:{PostId}'".to_string()
        })?;

        if post_part.contains(':') {
            return Err(
                "Validation Error: GlobalPostId must contain exactly one ':' separator".to_string(),
            );
        }

        // Validate PubkyId part
        PubkyId::try_from(pubky_part.to_string())
            .map_err(|e| format!("Validation Error: Invalid PubkyId: {}", e))?;

        // Validate PostId part
        PostId::try_from(post_part.to_string())
            .map_err(|e| format!("Validation Error: Invalid PostId: {}", e))?;

        Ok(())
    }
}

impl TryFrom<String> for GlobalPostId {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::validate(&s)?;
        Ok(GlobalPostId(s))
    }
}

impl<'de> Deserialize<'de> for GlobalPostId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Self::try_from(s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_pubky_id() -> &'static str {
        "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo"
    }

    fn valid_post_id() -> &'static str {
        "00000039YD9DP"
    }

    fn valid_global_post_id() -> String {
        format!("{}:{}", valid_pubky_id(), valid_post_id())
    }

    #[test]
    fn test_try_from_valid() {
        let id = valid_global_post_id();
        let result = GlobalPostId::try_from(id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_try_from_missing_separator() {
        let result = GlobalPostId::try_from(
            "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo00000039YD9DP".to_string(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("format"));
    }

    #[test]
    fn test_try_from_extra_separator() {
        let result = GlobalPostId::try_from("a:b:c".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("separator"));
    }

    #[test]
    fn test_try_from_invalid_pubky_id() {
        let id = "short:00000039YD9DP".to_string();
        let result = GlobalPostId::try_from(id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid PubkyId"));
    }

    #[test]
    fn test_try_from_invalid_post_id() {
        let id = format!("{}:short", valid_pubky_id());
        let result = GlobalPostId::try_from(id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid PostId"));
    }

    #[test]
    fn test_deserialize_valid() {
        let json = format!(r#""{}""#, valid_global_post_id());
        let result: Result<GlobalPostId, _> = serde_json::from_str(&json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deserialize_invalid() {
        let json = r#""invalid""#;
        let result: Result<GlobalPostId, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
