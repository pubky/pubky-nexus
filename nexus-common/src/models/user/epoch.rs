use serde::de::{value, IntoDeserializer};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use utoipa::ToSchema;

/// A user's indexing epoch: Nexus indexing state, not app data. Users are born `V0`.
///
/// Stored as `epoch` on the `:User` node and carried by [`super::UserDetails`]:
/// read it with `UserDetails::get_by_id`. It has no index of its own.
#[derive(Serialize, Deserialize, ToSchema, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Epoch {
    #[default]
    V0,
    V1,
}

impl Epoch {
    /// The plain string form (`"v0"`/`"v1"`), the same one serde writes to the
    /// graph property and the JSON field.
    pub fn as_str(self) -> &'static str {
        match self {
            Epoch::V0 => "v0",
            Epoch::V1 => "v1",
        }
    }
}

impl FromStr for Epoch {
    type Err = value::Error;

    /// Parses the plain string form through serde, so `"V1"` is rejected exactly
    /// as it is in the graph and the JSON.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::deserialize(s.into_deserializer())
    }
}

#[cfg(test)]
mod tests {
    use super::Epoch;

    #[test]
    fn plain_string_form_matches_serde() {
        for (epoch, s) in [(Epoch::V0, "v0"), (Epoch::V1, "v1")] {
            assert_eq!(epoch.as_str(), s);
            assert_eq!(serde_json::to_value(epoch).unwrap(), s);
            assert_eq!(serde_json::from_value::<Epoch>(s.into()).unwrap(), epoch);
            assert_eq!(s.parse::<Epoch>().unwrap(), epoch);
        }
        assert_eq!(Epoch::default(), Epoch::V0);
    }

    #[test]
    fn rejects_unknown_forms() {
        for raw in ["V1", "v2", ""] {
            assert!(raw.parse::<Epoch>().is_err(), "{raw:?} should not parse");
        }
    }
}
