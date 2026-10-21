use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Which users this Nexus indexes, set in `[stack.scope]` so the watcher and the
/// web API read the same value.
///
/// Users outside the scope are kept as stubs: their node exists so in-scope
/// records can point at them, but their own content is never read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", try_from = "RawScopeConfig")]
pub enum ScopeConfig {
    /// Every user Nexus can discover, as before scopes existed.
    #[default]
    Everything,
    /// Users hosted on these homeservers. Each one's whole event feed is read.
    Homeservers {
        #[schema(value_type = Vec<String>)]
        homeservers: Vec<PubkyId>,
    },
}

/// What the config file may hold, before the rules for each kind are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScopeConfig {
    kind: String,
    homeservers: Option<Vec<PubkyId>>,
}

impl TryFrom<RawScopeConfig> for ScopeConfig {
    type Error = String;

    fn try_from(raw: RawScopeConfig) -> Result<Self, Self::Error> {
        match raw.kind.as_str() {
            "everything" if raw.homeservers.is_some() => {
                Err("scope.homeservers is only valid with kind = \"homeservers\"".into())
            }
            "everything" => Ok(Self::Everything),
            "homeservers" => match raw.homeservers {
                Some(homeservers) if !homeservers.is_empty() => {
                    Ok(Self::Homeservers { homeservers })
                }
                _ => Err("scope.homeservers must list at least one homeserver key".into()),
            },
            other => Err(format!(
                "unknown scope.kind \"{other}\": expected \"everything\" or \"homeservers\""
            )),
        }
    }
}

/// How often the watcher re-checks who is in scope, set in the watcher's
/// `[scope_maintenance]` (`[watcher.scope_maintenance]` in a combined config).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeMaintenanceConfig {
    #[serde(
        default = "default_scope_maintenance_interval_ms",
        deserialize_with = "positive_interval_ms"
    )]
    pub interval_ms: u64,
}

const DEFAULT_SCOPE_MAINTENANCE_INTERVAL_MS: u64 = 300_000;

impl Default for ScopeMaintenanceConfig {
    fn default() -> Self {
        Self {
            interval_ms: DEFAULT_SCOPE_MAINTENANCE_INTERVAL_MS,
        }
    }
}

fn default_scope_maintenance_interval_ms() -> u64 {
    DEFAULT_SCOPE_MAINTENANCE_INTERVAL_MS
}

/// Refuses `0`, which the task's timer can't run on: it would panic and stop the watcher.
fn positive_interval_ms<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u64, D::Error> {
    match u64::deserialize(deserializer)? {
        0 => Err(serde::de::Error::custom(
            "scope_maintenance.interval_ms must be greater than 0",
        )),
        interval_ms => Ok(interval_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HS: &str = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";

    fn parse(toml_text: &str) -> Result<ScopeConfig, String> {
        toml::from_str::<ScopeConfig>(toml_text).map_err(|e| e.message().to_string())
    }

    #[test]
    fn defaults_to_everything() {
        assert_eq!(ScopeConfig::default(), ScopeConfig::Everything);
        assert_eq!(parse(r#"kind = "everything""#), Ok(ScopeConfig::Everything));
    }

    #[test]
    fn reads_a_homeserver_list() {
        let homeservers = parse(&format!("kind = \"homeservers\"\nhomeservers = [\"{HS}\"]"));
        assert_eq!(
            homeservers,
            Ok(ScopeConfig::Homeservers {
                homeservers: vec![PubkyId::try_from(HS).unwrap()]
            })
        );
    }

    #[test]
    fn rejects_invalid_settings() {
        let cases = [
            (
                "kind = \"homeservers\"\nhomeservers = []".to_string(),
                "must list at least one",
            ),
            (
                "kind = \"homeservers\"".to_string(),
                "must list at least one",
            ),
            (
                "kind = \"homeservers\"\nhomeservers = [\"not-a-key\"]".to_string(),
                "",
            ),
            (
                format!("kind = \"everything\"\nhomeservers = [\"{HS}\"]"),
                "scope.homeservers is only valid with kind = \"homeservers\"",
            ),
            ("kind = \"some\"".to_string(), "unknown scope.kind"),
            (
                "kind = \"everything\"\nhopz = 2".to_string(),
                "unknown field",
            ),
        ];

        for (toml_text, expected) in cases {
            let error = parse(&toml_text).expect_err(&toml_text);
            assert!(error.contains(expected), "{toml_text:?} gave {error:?}");
        }
    }

    #[test]
    fn maintenance_interval_must_be_positive() {
        let parse = |text: &str| toml::from_str::<ScopeMaintenanceConfig>(text);
        assert_eq!(parse("interval_ms = 1000").unwrap().interval_ms, 1000);
        assert_eq!(
            parse("").unwrap().interval_ms,
            DEFAULT_SCOPE_MAINTENANCE_INTERVAL_MS
        );
        let error = parse("interval_ms = 0").unwrap_err();
        assert!(error.message().contains("greater than 0"), "{error}");
    }

    #[test]
    fn serializes_the_way_v0_info_publishes_it() {
        assert_eq!(
            serde_json::to_value(ScopeConfig::Everything).unwrap(),
            serde_json::json!({ "kind": "everything" })
        );
        let homeservers = ScopeConfig::Homeservers {
            homeservers: vec![PubkyId::try_from(HS).unwrap()],
        };
        assert_eq!(
            serde_json::to_value(homeservers).unwrap(),
            serde_json::json!({ "kind": "homeservers", "homeservers": [HS] })
        );
    }
}
