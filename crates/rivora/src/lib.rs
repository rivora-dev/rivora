//! Rivora core library: Engineering Object Model, Runtime, and Capabilities.
//!
//! Architectural boundaries:
//! - Domain types are pure data with validation.
//! - Runtime owns all engineering reasoning.
//! - Capabilities coordinate Runtime behavior for interfaces.
//! - Storage provides durable local persistence.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capabilities;
pub mod domain;
pub mod error;
pub mod runtime;
pub mod storage;

pub use capabilities::CapabilityService;
pub use domain::*;
pub use error::{RivoraError, RivoraResult};
pub use runtime::Runtime;
pub use storage::{LocalStore, Store};
