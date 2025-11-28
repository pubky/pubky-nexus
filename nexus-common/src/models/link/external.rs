use crate::db::RedisOps;
use crate::types::DynError;
use blake3::Hash;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

const EXTERNAL_LINK_KEY_PREFIX: &str = "ExternalLink";
const EXTERNAL_LINK_HASH_LEN: usize = 32; // 128 bits represented as hex

#[derive(Debug, Error)]
pub enum ExternalLinkError {
    #[error("failed to parse url: {0}")]
    Parse(#[from] url::ParseError),
    #[error("failed to normalize url scheme")]
    Scheme,
    #[error("failed to normalize url host: {0}")]
    Host(url::ParseError),
    #[error("failed to normalize url port")]
    Port,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExternalLinkDetails {
    pub id: String,
    pub original_url: String,
    pub normalized_url: String,
    pub scheme: String,
    pub host: Option<String>,
    pub created_at: i64,
}

impl ExternalLinkDetails {
    pub fn from_url(raw_url: &str, created_at: i64) -> Result<Self, ExternalLinkError> {
        let normalized = NormalizedUrl::new(raw_url)?;
        let id = hash_to_hex(&normalized.normalized_url);

        Ok(Self {
            id,
            original_url: raw_url.to_string(),
            normalized_url: normalized.normalized_url,
            scheme: normalized.scheme,
            host: normalized.host,
            created_at,
        })
    }

    pub async fn upsert(&self) -> Result<(), DynError> {
        self.put_index_json(&[self.id.as_str()], None, None).await
    }

    pub async fn get(id: &str) -> Result<Option<Self>, DynError> {
        Self::try_from_index_json(&[id], None).await
    }
}

#[async_trait::async_trait]
impl RedisOps for ExternalLinkDetails {
    async fn prefix() -> String {
        EXTERNAL_LINK_KEY_PREFIX.to_string()
    }
}

struct NormalizedUrl {
    normalized_url: String,
    scheme: String,
    host: Option<String>,
}

impl NormalizedUrl {
    fn new(raw_url: &str) -> Result<Self, ExternalLinkError> {
        let mut url = Url::parse(raw_url)?;

        url.set_fragment(None);

        let lower_scheme = url.scheme().to_ascii_lowercase();
        if lower_scheme != url.scheme() {
            url.set_scheme(&lower_scheme)
                .map_err(|_| ExternalLinkError::Scheme)?;
        }

        if let Some(port) = url.port() {
            if Some(port) == url.port_or_known_default() {
                url.set_port(None).map_err(|_| ExternalLinkError::Port)?;
            }
        }

        if let Some(host) = url.host_str() {
            let lower = host.to_ascii_lowercase();
            if lower != host {
                url.set_host(Some(&lower))
                    .map_err(ExternalLinkError::Host)?;
            }
        }

        let host = url.host_str().map(|value| value.to_string());
        let scheme = url.scheme().to_string();
        let normalized_url = url.to_string();

        Ok(Self {
            normalized_url,
            scheme,
            host,
        })
    }
}

fn hash_to_hex(value: &str) -> String {
    let hash: Hash = blake3::hash(value.as_bytes());
    let mut hex = hash.to_hex().to_string();
    if hex.len() > EXTERNAL_LINK_HASH_LEN {
        hex.truncate(EXTERNAL_LINK_HASH_LEN);
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::ExternalLinkDetails;

    #[test]
    fn normalizes_scheme_host_and_fragment() {
        let details =
            ExternalLinkDetails::from_url("HTTP://Example.com:80/path#section", 1_000).unwrap();

        assert_eq!(details.scheme, "http");
        assert_eq!(details.host.as_deref(), Some("example.com"));
        assert_eq!(details.normalized_url, "http://example.com/path");
        assert_eq!(details.original_url, "HTTP://Example.com:80/path#section");
        assert_eq!(details.created_at, 1_000);
    }

    #[test]
    fn keeps_non_default_ports() {
        let details = ExternalLinkDetails::from_url("https://example.com:8443/", 0).unwrap();

        assert_eq!(details.normalized_url, "https://example.com:8443/");
    }

    #[test]
    fn normalizes_ipv6_hosts() {
        let details = ExternalLinkDetails::from_url("http://[2001:DB8::1]:80/", 0).unwrap();

        assert_eq!(details.host.as_deref(), Some("2001:db8::1"));
        assert_eq!(details.normalized_url, "http://[2001:db8::1]/");
    }
}
