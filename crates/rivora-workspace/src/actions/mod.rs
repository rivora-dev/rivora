//! Single typed action registry for `/`, Ctrl+P, help, and shortcuts.

mod registry;

pub use registry::{
    action_registry, filter_actions, ActionAvailability, ActionContext, WorkspaceActionCategory,
    WorkspaceActionDescriptor,
};
