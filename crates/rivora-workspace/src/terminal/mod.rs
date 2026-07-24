//! Terminal lifecycle: raw mode, alternate screen, panic-safe cleanup.

mod guard;

pub use guard::{is_interactive_terminal, TerminalGuard};
