//! Shared cross-cutting types used by multiple ought crates.
//!
//! Types that are genuinely cross-cutting (read by more than one crate) live
//! here so no crate has to pull another unrelated crate in just for a type.
//! Crate-specific config types stay in their owning crate.

pub mod context;

pub use context::ContextConfig;
