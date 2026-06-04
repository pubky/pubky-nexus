//! Plugin system — domain apps extend Nexus without forking it.
//!
//! Each plugin handles a path namespace (e.g. `/pub/mapky.app/`) and exposes
//! its own API routes mounted under `/v0/{name}/` by nexusd.
//!
//! Plugins access Neo4j and Redis via the global singleton connectors
//! (`get_neo4j_graph()`, `get_redis_conn()`) rather than receiving them in
//! the context — consistent with how the existing social handlers work.

use axum::Router;

use crate::types::DynError;

/// Context provided to plugins by Nexus core.
#[derive(Clone, Debug)]
pub struct PluginContext {
    /// Scoped Redis key prefix to prevent key collisions between plugins.
    /// e.g. `"mapky"` → keys like `mapky:place:node/123`
    pub redis_prefix: String,
}

impl PluginContext {
    /// Build the standard context for a plugin from its manifest.
    /// All construction sites should use this so new fields stay in sync.
    pub fn for_plugin(plugin: &dyn NexusPlugin) -> Self {
        Self {
            redis_prefix: plugin.manifest().name.to_string(),
        }
    }
}

/// Static metadata returned by every plugin.
pub struct PluginManifest {
    /// Unique plugin name used in Redis key scoping and route mounting.
    /// e.g. `"mapky"`
    pub name: &'static str,
    /// The `/pub/{domain}.app/` path prefix this plugin claims.
    /// The dispatcher matches event URIs against this prefix before
    /// `Event::parse_event()` — so `pubky-app-specs` never sees domain URIs.
    /// e.g. `"/pub/mapky.app/"`
    pub namespace: &'static str,
}

/// The trait every domain plugin implements.
///
/// Plugins are registered at startup. The watcher dispatcher routes events
/// to the matching plugin by path prefix before the social watcher sees them.
#[async_trait::async_trait]
pub trait NexusPlugin: Send + Sync {
    /// Return static plugin metadata.
    fn manifest(&self) -> PluginManifest;

    /// Handle a PUT event for a URI in this plugin's namespace.
    ///
    /// `data` is the raw JSON blob already fetched from the homeserver.
    async fn handle_put(
        &self,
        uri: &str,
        data: &[u8],
        user_id: &str,
        ctx: &PluginContext,
    ) -> Result<(), DynError>;

    /// Handle a DEL event for a URI in this plugin's namespace.
    async fn handle_del(
        &self,
        uri: &str,
        user_id: &str,
        ctx: &PluginContext,
    ) -> Result<(), DynError>;

    /// Build the Axum router for this plugin's API endpoints.
    /// Mounted at `/v0/{name}/` by nexusd.
    fn routes(&self, ctx: PluginContext) -> Router;

    /// Create Neo4j constraints and indexes on startup (idempotent).
    async fn setup_schema(&self, ctx: &PluginContext) -> Result<(), DynError>;

    /// Return an OpenAPI document for this plugin's routes, or `None` if the
    /// plugin does not expose API documentation.  nexusd mounts each `Some`
    /// doc at `/api-docs/{name}/openapi.json`.
    fn openapi_docs(&self) -> Option<utoipa::openapi::OpenApi> {
        None
    }
}
