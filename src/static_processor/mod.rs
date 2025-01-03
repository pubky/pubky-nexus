mod middleware;
mod processor;
mod processors;
mod store;

pub use middleware::{legacy_static_files_middleware, static_files_middleware};
pub use processor::StaticProcessor;
pub use store::StaticStorage;
