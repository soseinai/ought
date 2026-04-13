//! Dogfooding tests for the ought project.
//!
//! The `.ought.md` specs under `ought/engine/`, `ought/cli/`, `ought/analysis/`,
//! etc. are the project's own requirements. This crate holds the generated
//! tests that verify ought's code satisfies those requirements. One file per
//! subsection, each containing every clause's test.
//!
//! The crate is test-only — nothing here is compiled into the `ought` binary.

#![cfg(test)]

// Shared helpers for CLI integration tests (scaffolding, binary locator, etc).
pub mod helpers;

// Subsystem modules. Added as each bundle is ported over from the
// corresponding `crates/<crate>/tests/generated_tests.rs`.
pub mod analysis;
pub mod cli;
pub mod generator;
pub mod mcp_server;
pub mod ought;
pub mod parser;
pub mod reporter;
pub mod runner;
