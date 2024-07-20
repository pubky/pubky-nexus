// lib crate only needed for benchmarking with Criterion
pub mod config;
pub mod error;
pub mod models;
pub mod routes;
pub mod setup;

pub use error::{Error, Result};