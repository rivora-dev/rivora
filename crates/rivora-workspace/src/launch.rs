//! Shared Workspace launch configuration and Capability bootstrap.

use std::path::PathBuf;
use std::sync::Arc;

use rivora::storage::LocalStore;
use rivora::{CapabilityService, MockExecutionCapability, Runtime};
use rivora_connectors::register_first_party_github_execution_capabilities;

use crate::err;

/// Launch configuration for the shared Workspace entrypoint.
#[derive(Debug, Clone)]
pub struct WorkspaceLaunchConfig {
    /// Data directory for local Runtime storage.
    pub data_dir: PathBuf,
    /// Run a single non-interactive demo workflow (for tests/CI).
    pub smoke: bool,
}

impl WorkspaceLaunchConfig {
    /// Interactive Workspace with the given data directory.
    pub fn interactive(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            smoke: false,
        }
    }

    /// Non-interactive smoke workflow with the given data directory.
    pub fn smoke(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            smoke: true,
        }
    }
}

/// Open local storage, Runtime, register first-party execution capabilities.
pub fn open_capabilities(data_dir: &PathBuf) -> Result<CapabilityService, String> {
    let store = LocalStore::open(data_dir).map_err(err)?;
    let runtime = Arc::new(Runtime::new(Arc::new(store)));
    runtime
        .register_execution_capability(Arc::new(MockExecutionCapability::new()))
        .map_err(err)?;
    register_first_party_github_execution_capabilities(runtime.execution_registry())
        .map_err(err)?;
    Ok(CapabilityService::new(runtime))
}
