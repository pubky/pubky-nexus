//! Benchmark for `EventDispatcher::try_dispatch()` routing overhead.
//!
//! Measures pure DEL dispatch latency with mock plugins, so no homeserver or DB
//! calls are involved.
//! Run with: `cargo bench -p nexus-watcher --bench dispatcher`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use criterion::{criterion_group, criterion_main, Criterion};
use nexus_common::plugin::{NexusPlugin, PluginContext, PluginManifest};
use nexus_common::types::DynError;
use nexus_watcher::dispatcher::EventDispatcher;

struct BenchPlugin {
    namespace: &'static str,
}

#[async_trait]
impl NexusPlugin for BenchPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: "bench",
            namespace: self.namespace,
        }
    }

    async fn handle_put(
        &self,
        _: &str,
        _: &[u8],
        _: &str,
        _: &PluginContext,
    ) -> Result<(), DynError> {
        Ok(())
    }

    async fn handle_del(&self, _: &str, _: &str, _: &PluginContext) -> Result<(), DynError> {
        Ok(())
    }

    fn routes(&self, _: PluginContext) -> Router {
        Router::new()
    }

    async fn setup_schema(&self, _: &PluginContext) -> Result<(), DynError> {
        Ok(())
    }
}

fn make_plugin(namespace: &'static str) -> Arc<dyn NexusPlugin> {
    Arc::new(BenchPlugin { namespace })
}

fn bench_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let event = "DEL pubky://abc123def456/pub/bench.app/items/id1";

    let empty = EventDispatcher::new(vec![]);
    c.bench_function("dispatch_0_plugins", |b| {
        b.iter(|| rt.block_on(empty.try_dispatch(event)))
    });

    let one = EventDispatcher::new(vec![make_plugin("/pub/bench.app/")]);
    c.bench_function("dispatch_1_plugin_match", |b| {
        b.iter(|| rt.block_on(one.try_dispatch(event)))
    });

    let one_miss = EventDispatcher::new(vec![make_plugin("/pub/other.app/")]);
    c.bench_function("dispatch_1_plugin_miss", |b| {
        b.iter(|| rt.block_on(one_miss.try_dispatch(event)))
    });

    let five = EventDispatcher::new(vec![
        make_plugin("/pub/alpha.app/"),
        make_plugin("/pub/beta.app/"),
        make_plugin("/pub/bench.app/"),
        make_plugin("/pub/gamma.app/"),
        make_plugin("/pub/delta.app/"),
    ]);
    c.bench_function("dispatch_5_plugins_one_match", |b| {
        b.iter(|| rt.block_on(five.try_dispatch(event)))
    });
}

criterion_group!(benches, bench_dispatch);
criterion_main!(benches);
