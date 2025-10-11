//! Git operations abstraction for Plux

use std::path::{Path, PathBuf};
use std::process::Command;

/// Errors that can occur during git operations
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("Git command '{command}' failed:\n{stderr}")]
    CommandFailed { command: String, stderr: String },

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Git operations handler
pub struct Git {
    repo_path: Option<PathBuf>,
}

impl Git {
    /// Create a new Git instance for running commands
    pub fn new() -> Self {
        Self { repo_path: None }
    }

    /// Create a Git instance for an existing repository
    pub fn in_repo(path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: Some(path.into()),
        }
    }

    /// Creates a git command with the appropriate working directory
    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        if let Some(path) = &self.repo_path {
            cmd.current_dir(path);
        }
        cmd
    }

    /// Performs a shallow clone of a repository
    pub fn clone_shallow(&self, url: &str, dest: &Path) -> Result<(), GitError> {
        let output = self
            .command()
            .args(["clone", "--depth", "1", url])
            .arg(dest)
            .output()
            .map_err(GitError::IoError)?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                command: format!("clone {}", url),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    /// Fetches all tags from the remote repository
    pub fn fetch_tags(&self) -> Result<(), GitError> {
        let output = self
            .command()
            .args(["fetch", "--all", "--tags"])
            .output()
            .map_err(GitError::IoError)?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                command: "fetch --all --tags".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    /// Checks out a specific version (tag, branch, or commit)
    pub fn checkout(&self, version: &str) -> Result<(), GitError> {
        let output = self
            .command()
            .args(["checkout", version.trim()])
            .output()
            .map_err(GitError::IoError)?;

        if output.status.success() {
            Ok(())
        } else {
            Err(GitError::CommandFailed {
                command: format!("checkout {}", version),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }

    /// Gets the default branch of the repository
    pub fn get_default_branch(&self) -> Result<String, GitError> {
        let output = self
            .command()
            .args(["rev-parse", "--abbrev-ref", "origin/HEAD"])
            .output()
            .map_err(GitError::IoError)?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout)
                .trim()
                .strip_prefix("origin/")
                .unwrap_or("")
                .to_string();
            Ok(branch)
        } else {
            Err(GitError::CommandFailed {
                command: "rev-parse --abbrev-ref origin/HEAD".to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}

impl Default for Git {
    fn default() -> Self {
        Self::new()
    }
}
