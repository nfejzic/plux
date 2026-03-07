//! Configuration management for Plux

use std::fs;
use std::path::{Path, PathBuf};

use murus::{OptionScope, Tmux};

use crate::error::PluxError;
use crate::plugin::{DEFAULT_PLUGINS_PATH, DEFAULT_SPEC_PATH, PluginSpecFile};

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# Plux Plugin Configuration
#
# Add your tmux plugins here. Example:
#
# [plugins]
# tmux-grimoire = "https://github.com/navahas/tmux-grimoire"
# tmux-yank = "https://github.com/tmux-plugins/tmux-yank"
# tmux-sensible = "https://github.com/tmux-plugins/tmux-sensible"
#
# You can also specify versions:
# my-plugin = { url = "https://github.com/user/plugin", tag = "v1.0.0" }
# my-plugin = { url = "https://github.com/user/plugin", branch = "main" }
# my-plugin = { url = "https://github.com/user/plugin", commit = "<hash>" }

[plugins]
"#;

/// Configuration for Plux, including paths and plugin specifications
pub struct Config {
    pub spec_path: PathBuf,
    pub plugins_path: PathBuf,
    pub spec: PluginSpecFile,
}

impl Config {
    /// Loads configuration from tmux options and file system
    pub fn load(tmux: &Tmux) -> Result<Self, PluxError> {
        let spec_path = Self::resolve_spec_path(tmux)?;
        let plugins_path = Self::resolve_plugins_path(tmux)?;

        // Ensure the plugins directory exists
        fs::create_dir_all(&plugins_path).map_err(|e| PluxError::DirectoryCreation {
            path: plugins_path.clone(),
            source: e,
        })?;

        let spec = Self::load_spec_file(&spec_path)?;

        Ok(Config {
            spec_path,
            plugins_path,
            spec,
        })
    }

    /// Resolves the plugin spec file path from tmux options or default
    fn resolve_spec_path(tmux: &Tmux) -> Result<PathBuf, PluxError> {
        let path = tmux
            .get_option("@plux_toml_path", OptionScope::Global)
            .unwrap_or_else(|_| DEFAULT_SPEC_PATH.into());
        expand_path(path)
    }

    /// Resolves the plugins directory path from tmux options or default
    fn resolve_plugins_path(tmux: &Tmux) -> Result<PathBuf, PluxError> {
        let path = tmux
            .get_option("@plux_plugins_path", OptionScope::Global)
            .unwrap_or_else(|_| DEFAULT_PLUGINS_PATH.into());
        expand_path(path)
    }

    /// Loads the plugin spec file, creating a default one if it doesn't exist
    fn load_spec_file(path: &Path) -> Result<PluginSpecFile, PluxError> {
        match fs::read_to_string(path) {
            Ok(contents) => toml::from_str(&contents).map_err(|e| PluxError::ConfigParse {
                path: path.to_owned(),
                source: e,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Self::create_default_config(path)?;
                // Return empty config after creation
                Ok(PluginSpecFile {
                    plugins: std::collections::HashMap::new(),
                })
            }
            Err(e) => Err(PluxError::ConfigRead {
                path: path.to_owned(),
                source: e,
            }),
        }
    }

    /// Creates a default config file at the specified path
    fn create_default_config(path: &Path) -> Result<(), PluxError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| PluxError::DirectoryCreation {
                path: parent.to_owned(),
                source: e,
            })?;
        }

        fs::write(path, DEFAULT_CONFIG_TEMPLATE).map_err(|e| PluxError::ConfigWrite {
            path: path.to_owned(),
            source: e,
        })?;

        println!("Created default config file at {}", path.display());
        println!("Add your plugins to this file and reload tmux configuration.");

        Ok(())
    }
}

/// Expands ~ and $HOME in paths
fn expand_path(mut path: String) -> Result<PathBuf, PluxError> {
    let home = std::env::var("HOME")?;
    path = path.replace("$HOME", &home);
    path = path.replace('~', &home);
    Ok(PathBuf::from(path))
}
