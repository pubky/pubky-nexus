use std::error::Error;

pub mod collection;
pub mod taggers;

pub use collection::TagCollection;
pub use taggers::TaggersCollection;

pub type DynError = Box<dyn Error + Send + Sync>;
