//! A bounded, root-isolated filesystem server for the Model Context Protocol (MCP).
//!
//! The crate separates path-policy enforcement, filesystem operations, search,
//! protocol types, configuration, and child-process management into independent
//! modules. Applications can embed these modules directly or use the bundled
//! `fs-mcp-rs` HTTP server.
//!
//! # Security model
//!
//! All filesystem entry points accept a [`security::Policy`]. The policy
//! canonicalizes paths, confines them to configured roots, optionally rejects
//! symbolic links, and can disable every write operation.
//!
//! # Resource bounds
//!
//! Reads, writes, search results, terminal output, terminal reads, execution
//! time, and concurrency are bounded by configuration. File replacement is
//! performed through a temporary file in the destination directory.
//!
//! # Modules
//!
//! - [`filesystem`] provides bounded and atomic filesystem operations.
//! - [`search`] implements parallel filename and text search.
//! - [`security`] validates paths against the configured access policy.
//! - [`protocol`] contains the JSON-RPC and MCP wire types.
//! - [`settings`] loads and validates TOML configuration.
//! - [`terminal`] manages bounded persistent command sessions.
#![warn(missing_docs)]

pub mod filesystem;
pub mod protocol;
pub mod search;
pub mod security;
pub mod settings;
pub mod terminal;

pub mod patch;
pub mod tree;
