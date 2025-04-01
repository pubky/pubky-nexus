//! # Nexus Common
//!
//! The `nexus-common` crate provides the core shared utilities and data models for the Nexus stack.
//!
//! It is responsible for:
//! - **Configuration Management:** Loading configuration from TOML files via a trait-based loader.
//! - **Database Connectivity:** Providing connectors and helper functions for interacting with Neo4j and Redis.
//! - **Media Processing:** Implementing image and video processors to generate different file variants.
//! - **Data Models:** Defining common models for files, users, posts, tags, notifications, and follow relationships.
//! - **Indexing and Caching:** Offering utilities for indexing data in Redis and retrieving it using cache-first strategies.
//! - **Shared Types and Traits:** Exposing common types (e.g. pagination, timeframe, stream sorting) and traits
//!   used throughout the Nexus stack.
//!
//! This crate forms the foundation for other Nexus services, ensuring consistency and reuse across the backend.

mod config;
pub mod db;
pub mod media;
pub mod models;
mod stack;
pub mod types;

pub use config::*;
pub use stack::*;
