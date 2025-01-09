mod middleware;
mod processor;
mod processors;
mod store;

pub use middleware::static_files_middleware;
pub use processor::StaticProcessor;
pub use store::StaticStorage;
