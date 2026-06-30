//! Temporary filesystem helpers.
//!
//! Thin wrappers around [`tempfile`] so tests get a disposable workspace
//! without reaching for the raw API every time.

use std::path::{Path, PathBuf};

pub use tempfile;

/// A disposable temp directory. Returned so the caller controls its lifetime.
#[must_use]
pub fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("create temp directory")
}

/// A temporary workspace directory with convenience writers.
///
/// The directory and its contents are removed when the `TempWorkspace` is
/// dropped.
pub struct TempWorkspace {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl TempWorkspace {
    /// Creates a new, empty temporary workspace.
    #[must_use]
    pub fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp workspace");
        let root = dir.path().to_path_buf();
        Self { _dir: dir, root }
    }

    /// The workspace root path.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes `content` to `rel` (relative to the workspace root) and returns
    /// the full path.
    pub fn write(&self, rel: impl AsRef<Path>, content: &str) -> PathBuf {
        let path = self.root.join(rel.as_ref());
        std::fs::write(&path, content).expect("write file in temp workspace");
        path
    }

    /// Writes a `rivora.toml` into the workspace and returns its path.
    pub fn write_config(&self, content: &str) -> PathBuf {
        self.write("rivora.toml", content)
    }
}

impl Default for TempWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_creates_and_writes() {
        let ws = TempWorkspace::new();
        let path = ws.write_config("[organization]\nid = \"x\"\n");
        assert!(path.is_file());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("organization"));
    }

    #[test]
    fn workspace_root_exists() {
        let ws = TempWorkspace::new();
        assert!(ws.root().exists());
    }
}
