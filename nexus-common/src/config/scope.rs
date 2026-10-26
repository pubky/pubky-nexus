use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Most follow hops a seed scope may reach. Each hop multiplies the members, so beyond
/// three a seed scope is close to indexing everything.
pub const MAX_SEED_HOPS: u8 = 3;
const DEFAULT_SEED_HOPS: u8 = 2;

/// Which users this Nexus indexes, set in `[stack.scope]` so the watcher and the
/// web API read the same value.
///
/// Users outside the scope are kept as outsiders: their node exists so in-scope
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
    /// The seed user and everyone it reaches by following, up to `hops` follows away.
    /// Each member is read from its own stream; no feed is read whole.
    Seed {
        #[schema(value_type = String)]
        seed: PubkyId,
        hops: u8,
    },
}

/// What the config file may hold, before the rules for each kind are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScopeConfig {
    kind: String,
    homeservers: Option<Vec<PubkyId>>,
    seed: Option<PubkyId>,
    hops: Option<u8>,
}

fn only_with(field: &str, kind: &str) -> String {
    format!("scope.{field} is only valid with kind = \"{kind}\"")
}

impl TryFrom<RawScopeConfig> for ScopeConfig {
    type Error = String;

    fn try_from(raw: RawScopeConfig) -> Result<Self, Self::Error> {
        if raw.kind != "homeservers" && raw.homeservers.is_some() {
            return Err(only_with("homeservers", "homeservers"));
        }
        if raw.kind != "seed" {
            if raw.seed.is_some() {
                return Err(only_with("seed", "seed"));
            }
            if raw.hops.is_some() {
                return Err(only_with("hops", "seed"));
            }
        }
        match raw.kind.as_str() {
            "everything" => Ok(Self::Everything),
            "homeservers" => match raw.homeservers {
                Some(homeservers) if !homeservers.is_empty() => {
                    Ok(Self::Homeservers { homeservers })
                }
                _ => Err("scope.homeservers must list at least one homeserver key".into()),
            },
            "seed" => {
                let seed = raw.seed.ok_or("scope.seed is required with kind = \"seed\"")?;
                let hops = raw.hops.unwrap_or(DEFAULT_SEED_HOPS);
                if !(1..=MAX_SEED_HOPS).contains(&hops) {
                    return Err(format!(
                        "scope.hops must be between 1 and {MAX_SEED_HOPS}, got {hops}"
                    ));
                }
                Ok(Self::Seed { seed, hops })
            }
            other => Err(format!(
                "unknown scope.kind \"{other}\": expected \"everything\", \"homeservers\" or \"seed\""
            )),
        }
    }
}

/// How a new seed scope finds its members, set in the watcher's `[scope_bootstrap]`
/// (`[watcher.scope_bootstrap]` in a combined config).
///
/// Either way, every member's content is read from its own homeserver: a big Nexus only
/// suggests who the members are.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "source",
    rename_all = "snake_case",
    try_from = "RawScopeBootstrapConfig"
)]
pub enum ScopeBootstrapConfig {
    /// Find members hop by hop, as their follows are read.
    #[default]
    Crawl,
    /// Ask a big Nexus who the seed follows, and who they follow, up to the hops, so they
    /// are all read at once. Falls back to crawling if that Nexus can't be read.
    Nexus { nexus_url: String },
}

/// What the config file may hold, before the rules for each source are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScopeBootstrapConfig {
    source: String,
    nexus_url: Option<String>,
}

impl TryFrom<RawScopeBootstrapConfig> for ScopeBootstrapConfig {
    type Error = String;

    fn try_from(raw: RawScopeBootstrapConfig) -> Result<Self, Self::Error> {
        match (raw.source.as_str(), raw.nexus_url) {
            ("crawl", None) => Ok(Self::Crawl),
            ("crawl", Some(_)) => {
                Err("scope_bootstrap.nexus_url is only valid with source = \"nexus\"".into())
            }
            ("nexus", Some(nexus_url)) => Ok(Self::Nexus {
                nexus_url: http_url(&nexus_url, "scope_bootstrap.nexus_url")?,
            }),
            ("nexus", None) => {
                Err("scope_bootstrap.nexus_url is required with source = \"nexus\"".into())
            }
            (other, _) => Err(format!(
                "unknown scope_bootstrap.source \"{other}\": expected \"crawl\" or \"nexus\""
            )),
        }
    }
}

/// The inbound bridge of a seed scope, set in the watcher's `[inbound_bridge]`
/// (`[watcher.inbound_bridge]` in a combined config).
///
/// A seed scope never reads users outside it, so their replies, mentions, reposts, tags
/// and follows of the seed would never be seen. The bridge asks big Nexus instances for
/// the seed's notifications and fetches each record they point at from its author's
/// homeserver. Off without `nexus_urls`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawInboundBridgeConfig")]
pub struct InboundBridgeConfig {
    pub nexus_urls: Vec<String>,
    pub interval_ms: u64,
    /// How far back the first poll of each Nexus reads, in days; 0 reads everything.
    pub lookback_days: u32,
}

const DEFAULT_INBOUND_BRIDGE_INTERVAL_MS: u64 = 300_000;

impl Default for InboundBridgeConfig {
    fn default() -> Self {
        Self {
            nexus_urls: Vec::new(),
            interval_ms: DEFAULT_INBOUND_BRIDGE_INTERVAL_MS,
            lookback_days: 0,
        }
    }
}

impl InboundBridgeConfig {
    pub fn is_on(&self) -> bool {
        !self.nexus_urls.is_empty()
    }
}

/// What the config file may hold, before the rules are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInboundBridgeConfig {
    #[serde(default)]
    nexus_urls: Vec<String>,
    #[serde(default = "default_inbound_bridge_interval_ms")]
    interval_ms: u64,
    #[serde(default)]
    lookback_days: u32,
}

fn default_inbound_bridge_interval_ms() -> u64 {
    DEFAULT_INBOUND_BRIDGE_INTERVAL_MS
}

impl TryFrom<RawInboundBridgeConfig> for InboundBridgeConfig {
    type Error = String;

    fn try_from(raw: RawInboundBridgeConfig) -> Result<Self, Self::Error> {
        if raw.interval_ms == 0 {
            return Err("inbound_bridge.interval_ms must be greater than 0".into());
        }
        let mut nexus_urls: Vec<String> = Vec::new();
        for url in &raw.nexus_urls {
            let url = http_url(url, "inbound_bridge.nexus_urls")?;
            // Each Nexus keeps its own position, keyed by its URL.
            if nexus_urls.contains(&url) {
                return Err(format!("inbound_bridge.nexus_urls lists {url} twice"));
            }
            nexus_urls.push(url);
        }
        Ok(Self {
            nexus_urls,
            interval_ms: raw.interval_ms,
            lookback_days: raw.lookback_days,
        })
    }
}

/// Checks `value` is an http(s) URL an API path can be appended to (no query or
/// fragment, which the path would land in), and drops its trailing slash.
fn http_url(value: &str, field: &str) -> Result<String, String> {
    match url::Url::parse(value) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => {
            if url.query().is_some() || url.fragment().is_some() {
                return Err(format!(
                    "{field} must not have a query or fragment, as API paths are appended to it: {value}"
                ));
            }
            Ok(value.trim_end_matches('/').to_string())
        }
        _ => Err(format!("{field} is not an http(s) URL: {value}")),
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
    fn reads_a_seed_scope() {
        let seed = PubkyId::try_from(HS).unwrap();
        assert_eq!(
            parse(&format!("kind = \"seed\"\nseed = \"{HS}\"")),
            Ok(ScopeConfig::Seed {
                seed: seed.clone(),
                hops: 2
            }),
            "two hops by default"
        );
        assert_eq!(
            parse(&format!("kind = \"seed\"\nseed = \"{HS}\"\nhops = 3")),
            Ok(ScopeConfig::Seed { seed, hops: 3 })
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
                "kind = \"seed\"".to_string(),
                "scope.seed is required with kind = \"seed\"",
            ),
            (
                format!("kind = \"seed\"\nseed = \"{HS}\"\nhops = 0"),
                "scope.hops must be between 1 and 3, got 0",
            ),
            (
                format!("kind = \"seed\"\nseed = \"{HS}\"\nhops = 4"),
                "scope.hops must be between 1 and 3, got 4",
            ),
            (
                format!("kind = \"seed\"\nseed = \"{HS}\"\nhomeservers = [\"{HS}\"]"),
                "scope.homeservers is only valid with kind = \"homeservers\"",
            ),
            (
                format!("kind = \"everything\"\nseed = \"{HS}\""),
                "scope.seed is only valid with kind = \"seed\"",
            ),
            (
                format!("kind = \"homeservers\"\nhomeservers = [\"{HS}\"]\nhops = 2"),
                "scope.hops is only valid with kind = \"seed\"",
            ),
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
    fn reads_the_scope_bootstrap() {
        let parse = |text: &str| {
            toml::from_str::<ScopeBootstrapConfig>(text).map_err(|e| e.message().to_string())
        };
        assert_eq!(ScopeBootstrapConfig::default(), ScopeBootstrapConfig::Crawl);
        assert_eq!(parse("source = \"crawl\""), Ok(ScopeBootstrapConfig::Crawl));
        assert_eq!(
            parse("source = \"nexus\"\nnexus_url = \"https://nexus.example/\""),
            Ok(ScopeBootstrapConfig::Nexus {
                nexus_url: "https://nexus.example".into()
            }),
            "the trailing slash is dropped"
        );
        let cases = [
            ("source = \"nexus\"", "nexus_url is required"),
            (
                "source = \"crawl\"\nnexus_url = \"https://nexus.example\"",
                "only valid with source = \"nexus\"",
            ),
            (
                "source = \"nexus\"\nnexus_url = \"ftp://nexus.example\"",
                "not an http(s) URL",
            ),
            (
                "source = \"nexus\"\nnexus_url = \"nexus\"",
                "not an http(s) URL",
            ),
            (
                "source = \"nexus\"\nnexus_url = \"https://nexus.example/?token=x\"",
                "must not have a query or fragment",
            ),
            (
                "source = \"nexus\"\nnexus_url = \"https://nexus.example/#here\"",
                "must not have a query or fragment",
            ),
            ("source = \"ask\"", "unknown scope_bootstrap.source"),
        ];
        for (text, expected) in cases {
            let error = parse(text).expect_err(text);
            assert!(error.contains(expected), "{text:?} gave {error:?}");
        }
    }

    #[test]
    fn reads_the_inbound_bridge() {
        let parse = |text: &str| {
            toml::from_str::<InboundBridgeConfig>(text).map_err(|e| e.message().to_string())
        };
        assert!(!InboundBridgeConfig::default().is_on(), "off by default");
        let bridge = parse(
            "nexus_urls = [\"https://a.example/\", \"https://b.example\"]\nlookback_days = 30",
        )
        .unwrap();
        assert!(bridge.is_on());
        assert_eq!(
            bridge.nexus_urls,
            ["https://a.example", "https://b.example"]
        );
        assert_eq!(bridge.interval_ms, DEFAULT_INBOUND_BRIDGE_INTERVAL_MS);
        assert_eq!(bridge.lookback_days, 30);

        let cases = [
            ("interval_ms = 0", "must be greater than 0"),
            ("nexus_urls = [\"nexus\"]", "not an http(s) URL"),
            (
                "nexus_urls = [\"https://a.example/?x=1\"]",
                "must not have a query or fragment",
            ),
            (
                "nexus_urls = [\"https://a.example\", \"https://a.example/\"]",
                "twice",
            ),
            ("nexus_url = \"https://a.example\"", "unknown field"),
        ];
        for (text, expected) in cases {
            let error = parse(text).expect_err(text);
            assert!(error.contains(expected), "{text:?} gave {error:?}");
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
        let seed = ScopeConfig::Seed {
            seed: PubkyId::try_from(HS).unwrap(),
            hops: 2,
        };
        assert_eq!(
            serde_json::to_value(seed).unwrap(),
            serde_json::json!({ "kind": "seed", "seed": HS, "hops": 2 })
        );
    }
}
