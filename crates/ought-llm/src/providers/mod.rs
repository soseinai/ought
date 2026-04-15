//! Provider adapters. Each adapter implements [`crate::Llm`] for one
//! upstream API.

pub mod anthropic;
pub mod openai;
