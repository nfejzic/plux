//! Error types for Plux

use std::path::PathBuf;

/// Main error type for Plux operations
#[derive(Debug, thiserror::Error)]
pub enum PluxError {
    #[error("Plux must be called within a tmux session")]
    NotInTmux,

    #[error("Could not create directory at {path}: {source}")]
    DirectoryCreation {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Could not read config at {path}: {source}")]
    ConfigRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Could not write config at {path}: {source}")]
    ConfigWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid TOML syntax in {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Could not expand path: {0}")]
    PathExpansion(#[from] std::env::VarError),

    #[error("Plugin installation error: {0}")]
    PluginInstall(#[from] crate::plugin::InstallError),

    #[error("Tmux error: {0}")]
    Tmux(#[from] murus::Error),
}
