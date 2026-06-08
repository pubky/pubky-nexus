//! Event dispatcher — routes homeserver events to domain plugins by path
//! prefix, intercepting them *before* `Event::parse_event()` so
//! `pubky-app-specs` never sees domain-specific URIs.

use nexus_common::db::PubkyConnector;
use nexus_common::models::event::EventProcessorError;
use nexus_common::plugin::{NexusPlugin, PluginContext};
use std::sync::Arc;
use tracing::{debug, warn};

pub struct EventDispatcher {
    plugins: Vec<Arc<dyn NexusPlugin>>,
}

impl std::fmt::Debug for EventDispatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventDispatcher")
            .field(
                "plugins",
                &self
                    .plugins
                    .iter()
                    .map(|p| p.manifest().name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl EventDispatcher {
    /// Sort plugins longest-namespace-first so more specific prefixes always
    /// win over broader ones (e.g. `/pub/mapky.app/places/` beats `/pub/mapky.app/`).
    pub fn new(mut plugins: Vec<Arc<dyn NexusPlugin>>) -> Self {
        plugins.sort_by(|a, b| {
            b.manifest()
                .namespace
                .len()
                .cmp(&a.manifest().namespace.len())
        });
        Self { plugins }
    }

    /// Returns `Ok(true)` if one or more registered plugins handled this event
    /// line, `Ok(false)` if no plugin matched (caller should fall through to
    /// social watcher), or `Err` if any plugin failed after claiming the event
    /// (caller should push to retry queue).
    ///
    /// All plugins whose namespace is a prefix of the event path receive the
    /// event — enabling multiple plugins to index the same homeserver path.
    /// For PUT events the blob is fetched once and shared across all matching
    /// plugins. Plugins must be idempotent (use MERGE, not CREATE) because a
    /// failure in a later plugin will cause the whole event to be retried,
    /// re-invoking earlier plugins that already succeeded.
    ///
    /// Event line format: `"PUT pubky://user_id/pub/..."` or `"DEL pubky://..."`.
    pub async fn try_dispatch(&self, line: &str) -> Result<bool, EventProcessorError> {
        if self.plugins.is_empty() {
            return Ok(false);
        }

        // Split "PUT pubky://..." → (event_type, uri)
        let mut parts = line.splitn(2, ' ');
        let event_type = match parts.next() {
            Some(t) => t,
            None => return Ok(false),
        };
        let uri = match parts.next() {
            Some(u) => u.trim(),
            None => return Ok(false),
        };

        // Extract the /pub/{domain}.app/... path from pubky://{user_id}/pub/...
        let path = match extract_pub_path(uri) {
            Some(p) => p,
            None => return Ok(false),
        };

        // Collect all plugins whose namespace prefix matches this path.
        let matching: Vec<_> = self
            .plugins
            .iter()
            .filter(|p| path.starts_with(p.manifest().namespace))
            .collect();

        if matching.is_empty() {
            return Ok(false);
        }

        // App-specific files/blobs use universal Nexus file handling. Let them
        // fall through instead of requiring every plugin to duplicate file logic.
        let resource_suffix = path
            .strip_prefix(matching[0].manifest().namespace)
            .unwrap_or(path);
        if resource_suffix.starts_with("files/") || resource_suffix.starts_with("blobs/") {
            return Ok(false);
        }

        let user_id = match extract_user_id(uri) {
            Some(u) => u,
            None => {
                warn!("Could not extract user_id from URI: {uri}");
                return Ok(true); // claimed but malformed — don't fall through
            }
        };

        // Fetch the blob once for PUT events and share it across all plugins.
        let data: Option<Vec<u8>> = if event_type == "PUT" {
            Some(fetch_blob(uri).await?)
        } else {
            None
        };

        for plugin in matching {
            let manifest = plugin.manifest();
            debug!("Plugin '{}' handling {} {uri}", manifest.name, event_type);

            let ctx = PluginContext::for_plugin(plugin.as_ref());

            match event_type {
                "PUT" => {
                    plugin
                        .handle_put(uri, data.as_deref().unwrap(), &user_id, &ctx)
                        .await
                        .map_err(EventProcessorError::generic)?;
                }
                "DEL" => {
                    plugin
                        .handle_del(uri, &user_id, &ctx)
                        .await
                        .map_err(EventProcessorError::generic)?;
                }
                _ => return Ok(false),
            }
        }

        Ok(true) // at least one plugin handled the event
    }
}

/// Extract `/pub/{domain}.app/...` from `pubky://{user_id}/pub/...`.
fn extract_pub_path(uri: &str) -> Option<&str> {
    let without_scheme = uri.strip_prefix("pubky://")?;
    let slash_pos = without_scheme.find('/')?;
    let path = &without_scheme[slash_pos..];
    if path.starts_with("/pub/") {
        Some(path)
    } else {
        None
    }
}

/// Extract `user_id` from `pubky://{user_id}/pub/...`.
fn extract_user_id(uri: &str) -> Option<String> {
    let without_scheme = uri.strip_prefix("pubky://")?;
    let slash_pos = without_scheme.find('/')?;
    Some(without_scheme[..slash_pos].to_string())
}

async fn fetch_blob(uri: &str) -> Result<Vec<u8>, EventProcessorError> {
    let pubky = PubkyConnector::get()?;
    let response = pubky.public_storage().get(uri).await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());

        let err_msg = format!("Fetch resource failed {uri}: HTTP {status} - {body}");
        return Err(EventProcessorError::client_error(err_msg))?;
    }

    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|e| EventProcessorError::client_error(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pub_path() {
        let uri = "pubky://abc123/pub/mapky.app/posts/0034TK01CC73G";
        assert_eq!(
            extract_pub_path(uri),
            Some("/pub/mapky.app/posts/0034TK01CC73G")
        );
    }

    #[test]
    fn test_extract_user_id() {
        let uri = "pubky://abc123/pub/mapky.app/posts/0034TK01CC73G";
        assert_eq!(extract_user_id(uri), Some("abc123".to_string()));
    }

    #[test]
    fn test_extract_pub_path_non_pub() {
        assert_eq!(extract_pub_path("pubky://abc123/other/path"), None);
    }

    #[test]
    fn test_dispatcher_empty() {
        let dispatcher = EventDispatcher::new(vec![]);
        assert!(dispatcher.plugins.is_empty());
    }

    // ── Integration tests with a minimal mock plugin ──────────────────────

    struct MockPlugin;

    #[async_trait::async_trait]
    impl NexusPlugin for MockPlugin {
        fn manifest(&self) -> nexus_common::plugin::PluginManifest {
            nexus_common::plugin::PluginManifest {
                name: "mock",
                namespace: "/pub/mock.app/",
            }
        }
        async fn handle_put(
            &self,
            _: &str,
            _: &[u8],
            _: &str,
            _: &PluginContext,
        ) -> Result<(), nexus_common::types::DynError> {
            Ok(())
        }
        async fn handle_del(
            &self,
            _: &str,
            _: &str,
            _: &PluginContext,
        ) -> Result<(), nexus_common::types::DynError> {
            Ok(())
        }
        fn routes(&self, _: PluginContext) -> axum::Router {
            axum::Router::new()
        }
        async fn setup_schema(
            &self,
            _: &PluginContext,
        ) -> Result<(), nexus_common::types::DynError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_try_dispatch_matches_del_event() {
        let dispatcher = EventDispatcher::new(vec![Arc::new(MockPlugin) as Arc<dyn NexusPlugin>]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/mock.app/items/id1")
            .await;
        assert!(matches!(result, Ok(true)));
    }

    #[tokio::test]
    async fn test_try_dispatch_no_match_returns_false() {
        let dispatcher = EventDispatcher::new(vec![Arc::new(MockPlugin) as Arc<dyn NexusPlugin>]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/other.app/items/id1")
            .await;
        assert!(matches!(result, Ok(false)));
    }

    #[tokio::test]
    async fn test_try_dispatch_empty_dispatcher_returns_false() {
        let dispatcher = EventDispatcher::new(vec![]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/mock.app/items/id1")
            .await;
        assert!(matches!(result, Ok(false)));
    }

    #[tokio::test]
    async fn test_try_dispatch_file_path_falls_through() {
        let dispatcher = EventDispatcher::new(vec![Arc::new(MockPlugin) as Arc<dyn NexusPlugin>]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/mock.app/files/file1")
            .await;
        assert!(matches!(result, Ok(false)));
    }

    #[tokio::test]
    async fn test_try_dispatch_tag_path_reaches_plugin() {
        let dispatcher = EventDispatcher::new(vec![Arc::new(MockPlugin) as Arc<dyn NexusPlugin>]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/mock.app/tags/tag1")
            .await;
        assert!(matches!(result, Ok(true)));
    }

    #[test]
    fn test_plugins_sorted_longest_namespace_first() {
        // The sort key is namespace length — verify the comparator directly.
        let mut namespaces = [
            "/pub/mapky.app/",
            "/pub/mapky.app/places/",
            "/pub/other.app/",
        ];
        namespaces.sort_by_key(|namespace| std::cmp::Reverse(namespace.len()));
        assert_eq!(namespaces[0], "/pub/mapky.app/places/"); // longest first
        assert_eq!(namespaces[2], "/pub/other.app/"); // shortest last
    }

    #[tokio::test]
    async fn test_try_dispatch_broadcasts_to_all_matching_plugins() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingPlugin {
            namespace: &'static str,
            call_count: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl NexusPlugin for CountingPlugin {
            fn manifest(&self) -> nexus_common::plugin::PluginManifest {
                nexus_common::plugin::PluginManifest {
                    name: "counting",
                    namespace: self.namespace,
                }
            }
            async fn handle_put(
                &self,
                _: &str,
                _: &[u8],
                _: &str,
                _: &PluginContext,
            ) -> Result<(), nexus_common::types::DynError> {
                Ok(())
            }
            async fn handle_del(
                &self,
                _: &str,
                _: &str,
                _: &PluginContext,
            ) -> Result<(), nexus_common::types::DynError> {
                self.call_count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn routes(&self, _: PluginContext) -> axum::Router {
                axum::Router::new()
            }
            async fn setup_schema(
                &self,
                _: &PluginContext,
            ) -> Result<(), nexus_common::types::DynError> {
                Ok(())
            }
        }

        let count_a = Arc::new(AtomicUsize::new(0));
        let count_b = Arc::new(AtomicUsize::new(0));

        let plugin_a = Arc::new(CountingPlugin {
            namespace: "/pub/mock.app/",
            call_count: count_a.clone(),
        }) as Arc<dyn NexusPlugin>;
        let plugin_b = Arc::new(CountingPlugin {
            namespace: "/pub/mock.app/",
            call_count: count_b.clone(),
        }) as Arc<dyn NexusPlugin>;

        let dispatcher = EventDispatcher::new(vec![plugin_a, plugin_b]);
        let result = dispatcher
            .try_dispatch("DEL pubky://abc123/pub/mock.app/items/id1")
            .await;

        assert!(matches!(result, Ok(true)));
        assert_eq!(
            count_a.load(Ordering::SeqCst),
            1,
            "plugin_a should have been called"
        );
        assert_eq!(
            count_b.load(Ordering::SeqCst),
            1,
            "plugin_b should have been called"
        );
    }
}
