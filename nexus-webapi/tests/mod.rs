// Tower's service combinators generate deeply-nested types; the default limit of 128
// is insufficient when layering multiple middleware in the middleware tests.
#![recursion_limit = "256"]

pub mod endpoints;
pub mod events;
pub mod files;
pub mod post;
pub mod resource;
pub mod stream;
pub mod tags;
pub mod user;
pub mod utils;
