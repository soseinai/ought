//! Library surface for the `ought` CLI.
//!
//! The CLI is primarily a binary (see `main.rs`), but a small lib target lets
//! integration tests exercise the aggregate `Config` and its loading logic.

pub mod config;
