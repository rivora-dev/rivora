//! Typed Workspace intents — the boundary between UI language and Capabilities.

pub mod execute;
mod interpreter;
mod model;

pub use execute::{execute_intent, IntentExecutionResult};
pub use interpreter::interpret_prompt;
pub use model::*;
