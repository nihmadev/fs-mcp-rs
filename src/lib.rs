//! A bounded, root-isolated filesystem server for the Model Context Protocol (MCP).
//!
//! The crate separates path-policy enforcement, filesystem operations, search,
//! protocol types, configuration, and child-process management into independent
//! modules. Applications can embed these modules directly or run the bundled
//! `fs-mcp-rs` binary using either HTTP POST or STDIO JSON-RPC transports.
//!
//! # Transports & Execution Modes
//!
//! - **STDIO Mode**: Standard input/output line-delimited JSON-RPC 2.0 streaming,
//!   ideal for local MCP client subprocess integration (e.g. via `npx fs-mcp-rs`).
//! - **HTTP Mode**: Streamable HTTP server exposing `/mcp` and `/health` endpoints
//!   with optional OAuth 2.0 / OIDC authentication metadata.
//! - **CLI Quickstart**: Roots can be passed directly as positional arguments or configured
//!   via TOML files and environment variables.
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
//! - [`tree`] generates bounded and paginated directory trees.
//! - [`patch`] validates and applies unified diff text patches.
//! - [`cli_format`] generates client configuration snippets and help views.
//! - [`wizard`] interactive setup wizard for initial configuration.
#![warn(missing_docs)]

pub mod filesystem;
pub mod protocol;
pub mod search;
pub mod security;
pub mod settings;
pub mod terminal;

pub mod patch;
pub mod tree;

pub mod cli_format;
pub mod wizard;

/// Server infrastructure (always available).
pub mod app;
pub mod handler;
pub mod server;
pub mod stdio;
pub mod oauth;
pub mod launcher;
pub mod tools;
