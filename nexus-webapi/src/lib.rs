//! # Nexus API
//!
//! The `nexus-webapi` crate implements the RESTful API server for the Nexus platform.
//! It is responsible for:
//!
//! - **Defining and serving HTTP endpoints:** Implements endpoints for users, posts, files,
//!   tags, notifications, and more using the Axum framework.
//! - **Asynchronous request handling:** Leverages Tokio for asynchronous operations,
//!   ensuring high performance and scalability.
//! - **Database interactions:** Integrates with underlying data stores such as Neo4j for
//!   graph-based data and Redis for caching and key-value storage.
//! - **Middleware and observability:** Provides middleware for logging and tracing (via OpenTelemetry)
//!   to monitor API activity and performance.
//! - **OpenAPI Documentation:** Automatically generates API documentation using the `utoipa` crate,
//!   and serves a Swagger UI for interactive exploration.
//! - **Testing and benchmarking:** Supports integration tests and benchmarking suites to ensure
//!   the API meets performance and reliability requirements.
//!
//! ## Getting Started
//!
//! To start the API server, use the provided builder API:
//!
//! ```rust
//! use nexus_webapi::builder::NexusApi;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure and run the server
//!     NexusApi::builder()
//!         // Customize public address, log level, files path, etc.
//!         .run()
//!         .await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Further Information
//!
//! For more details on configuration and customizations, refer to the internal modules,
//! such as [config](crate::config) and [routes](crate::routes).
//!
//! ## License
//!
//! This project is licensed under the terms of the MIT License.

mod builder;
pub mod error;
pub mod mock;
pub mod models;
pub mod routes;

pub use builder::{NexusApi, NexusApiBuilder};
pub use error::{Error, Result};
