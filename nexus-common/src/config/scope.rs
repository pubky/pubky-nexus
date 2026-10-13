use pubky_app_specs::PubkyId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Most follow hops a seed scope may reach. Each hop multiplies the members, so
/// beyond three a seed scope is close to indexing everything.
pub const MAX_SCOPE_HOPS: u8 = 3;
const DEFAULT_SCOPE_HOPS: u8 = 2;

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
    /// The seed user and everyone it reaches by following, up to `hops` follows away.
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

impl TryFrom<RawScopeConfig> for ScopeConfig {
    type Error = String;

    fn try_from(raw: RawScopeConfig) -> Result<Self, Self::Error> {
        let only_with =
            |field: &str, kind: &str| format!("scope.{field} is only valid with kind = \"{kind}\"");

        match raw.kind.as_str() {
            "everything" => {
                if raw.homeservers.is_some() {
                    return Err(only_with("homeservers", "homeservers"));
                }
                if raw.seed.is_some() || raw.hops.is_some() {
                    return Err(only_with(if raw.seed.is_some() { "seed" } else { "hops" }, "seed"));
                }
                Ok(Self::Everything)
            }
            "homeservers" => {
                if raw.seed.is_some() || raw.hops.is_some() {
                    return Err(only_with(if raw.seed.is_some() { "seed" } else { "hops" }, "seed"));
                }
                match raw.homeservers {
                    Some(homeservers) if !homeservers.is_empty() => Ok(Self::Homeservers { homeservers }),
                    _ => Err("scope.homeservers must list at least one homeserver key".into()),
                }
            }
            "seed" => {
                if raw.homeservers.is_some() {
                    return Err(only_with("homeservers", "homeservers"));
                }
                let seed = raw.seed.ok_or("scope.seed is required with kind = \"seed\"")?;
                let hops = raw.hops.unwrap_or(DEFAULT_SCOPE_HOPS);
                if !(1..=MAX_SCOPE_HOPS).contains(&hops) {
                    return Err(format!("scope.hops must be between 1 and {MAX_SCOPE_HOPS}"));
                }
                Ok(Self::Seed { seed, hops })
            }
            other => Err(format!(
                "unknown scope.kind \"{other}\": expected \"everything\", \"homeservers\" or \"seed\""
            )),
        }
    }
}

/// How a new seed-scoped Nexus finds its members, set in the watcher's
/// `[scope_bootstrap]` (`[watcher.scope_bootstrap]` in a combined config).
///
/// Either way, every member's content is read from its own homeserver: a big Nexus
/// only tells the new one who the members are, so it can't hide or delay content.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "source",
    rename_all = "snake_case",
    try_from = "RawScopeBootstrapConfig"
)]
pub enum ScopeBootstrapConfig {
    /// Discover members hop by hop, as their follow events are read.
    #[default]
    Crawl,
    /// Work out every member at start from a big Nexus's `/v0/events` log, then read
    /// them all in parallel. Falls back to crawling if that Nexus can't be read.
    Nexus { nexus_url: String },
}

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
            ("nexus", Some(nexus_url)) => match url::Url::parse(&nexus_url) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(Self::Nexus {
                    nexus_url: nexus_url.trim_end_matches('/').to_string(),
                }),
                _ => Err(format!(
                    "scope_bootstrap.nexus_url is not an http(s) URL: {nexus_url}"
                )),
            },
            ("nexus", None) => {
                Err("scope_bootstrap.nexus_url is required with source = \"nexus\"".into())
            }
            (other, _) => Err(format!(
                "unknown scope_bootstrap.source \"{other}\": expected \"crawl\" or \"nexus\""
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HS: &str = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
    const SEED: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";

    fn parse(toml_text: &str) -> Result<ScopeConfig, String> {
        toml::from_str::<ScopeConfig>(toml_text).map_err(|e| e.message().to_string())
    }

    #[test]
    fn defaults_to_everything() {
        assert_eq!(ScopeConfig::default(), ScopeConfig::Everything);
        assert_eq!(parse(r#"kind = "everything""#), Ok(ScopeConfig::Everything));
    }

    #[test]
    fn reads_each_kind() {
        let homeservers =
            parse(&format!("kind = \"homeservers\"\nhomeservers = [\"{HS}\"]")).unwrap();
        assert_eq!(
            homeservers,
            ScopeConfig::Homeservers {
                homeservers: vec![PubkyId::try_from(HS).unwrap()]
            }
        );

        let seed = parse(&format!("kind = \"seed\"\nseed = \"{SEED}\"\nhops = 3")).unwrap();
        assert_eq!(
            seed,
            ScopeConfig::Seed {
                seed: PubkyId::try_from(SEED).unwrap(),
                hops: 3
            }
        );
    }

    #[test]
    fn seed_hops_default_to_two() {
        let seed = parse(&format!("kind = \"seed\"\nseed = \"{SEED}\"")).unwrap();
        assert!(matches!(seed, ScopeConfig::Seed { hops: 2, .. }));
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
                format!("kind = \"seed\"\nseed = \"{SEED}\"\nhops = 0"),
                "between 1 and 3",
            ),
            (
                format!("kind = \"seed\"\nseed = \"{SEED}\"\nhops = 4"),
                "between 1 and 3",
            ),
            ("kind = \"seed\"".to_string(), "scope.seed is required"),
            ("kind = \"seed\"\nseed = \"not-a-key\"".to_string(), ""),
            (
                format!("kind = \"everything\"\nseed = \"{SEED}\""),
                "scope.seed is only valid with kind = \"seed\"",
            ),
            (
                format!("kind = \"seed\"\nseed = \"{SEED}\"\nhomeservers = [\"{HS}\"]"),
                "scope.homeservers is only valid",
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
    fn serializes_the_way_v0_info_publishes_it() {
        let seed = ScopeConfig::Seed {
            seed: PubkyId::try_from(SEED).unwrap(),
            hops: 2,
        };
        assert_eq!(
            serde_json::to_value(&seed).unwrap(),
            serde_json::json!({ "kind": "seed", "seed": SEED, "hops": 2 })
        );
        assert_eq!(
            serde_json::to_value(ScopeConfig::Everything).unwrap(),
            serde_json::json!({ "kind": "everything" })
        );
    }

    #[test]
    fn bootstrap_reads_each_source_and_rejects_invalid_ones() {
        let parse = |toml_text: &str| {
            toml::from_str::<ScopeBootstrapConfig>(toml_text).map_err(|e| e.message().to_string())
        };

        assert_eq!(
            parse(r#"source = "crawl""#),
            Ok(ScopeBootstrapConfig::Crawl)
        );
        assert_eq!(
            parse("source = \"nexus\"\nnexus_url = \"https://nexus.pubky.app/\""),
            Ok(ScopeBootstrapConfig::Nexus {
                nexus_url: "https://nexus.pubky.app".into()
            })
        );

        let cases = [
            ("source = \"nexus\"", "is required"),
            (
                "source = \"nexus\"\nnexus_url = \"ftp://x\"",
                "not an http(s) URL",
            ),
            (
                "source = \"crawl\"\nnexus_url = \"https://x\"",
                "only valid with",
            ),
            ("source = \"peers\"", "unknown scope_bootstrap.source"),
        ];
        for (toml_text, expected) in cases {
            let error = parse(toml_text).expect_err(toml_text);
            assert!(error.contains(expected), "{toml_text:?} gave {error:?}");
        }
    }
}
