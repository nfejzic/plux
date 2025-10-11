use std::{collections::HashMap, path::Path};

pub const DEFAULT_PLUGINS_PATH: &str = "$HOME/.config/tmux/plux/";
pub const DEFAULT_SPEC_PATH: &str = "$HOME/.config/tmux/plux.toml";

/// Models the TOML file used to specify plugins to install. See [`PluginSpec`] for more
/// information.
#[derive(Clone, serde::Deserialize)]
pub struct PluginSpecFile {
    pub plugins: HashMap<String, PluginSpec>,
}

/// Models supported version specifiers for a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Version {
    /// Git tag to be used as plugin's version.
    Tag(String),
    /// Git commit hash to be used as plugin's version.
    Commit(String),
    /// Git branch to use as version. Latest commit of that branch will be used.
    Branch(String),
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (prefix, version) = match self {
            Version::Tag(tag) => ("tag", tag),
            Version::Commit(hash) => ("commit", hash),
            Version::Branch(branch) => ("branch", branch),
        };

        f.write_fmt(format_args!("{prefix} '{}'", version.trim()))
    }
}

/// Models the full plugin specification (as opposed to URL-only). Main use of this struct is to
/// support specifying the version of plugin to be installed. For example, this allows the
/// following:
///
/// ```toml
/// # tag as version
/// first = { url = "...", tag = "v1.0.0" }
/// # branch as version
/// second = { url = "...", branch = "main" }
/// # commit hash as version
/// third = { url = "...", commit = "<commit hash>" }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct FullPluginSpec {
    /// Url to the git repository where plugin is hosted.
    pub url: String,

    /// Optional version specification for the given plugin.
    #[serde(flatten)]
    pub tag_or_commit: Option<Version>,
}

/// Errors that can occur during installation of plugin.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    /// Directory for this plugin already exists and does not need to be created again.
    #[error("Plugin is already installed.")]
    AlreadyInstalled,

    /// An error occurred during git operations
    #[error("Git operation failed: {0}")]
    Git(#[from] crate::git::GitError),
}

/// Models specification of a single plugin. This can either be URL-only, or full plugin
/// specification. See [`FullPluginSpec`] for more details.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(untagged)]
pub enum PluginSpec {
    Url(String),
    Full(FullPluginSpec),
}

impl PluginSpec {
    /// Returns the URL specified for this plugin as.
    pub fn url(&self) -> &str {
        match self {
            PluginSpec::Url(url) => url,
            PluginSpec::Full(full_plugin_spec) => &full_plugin_spec.url,
        }
    }

    /// Tries to install plugin at the provided path. This involves cloning the git repository if
    /// it's not already installed.
    pub fn try_install(&self, destination_dir: &Path) -> Result<(), InstallError> {
        if destination_dir.is_dir() {
            return Err(InstallError::AlreadyInstalled);
        }

        let git = crate::git::Git::new();
        git.clone_shallow(self.url(), destination_dir)?;

        Ok(())
    }

    /// Determines the version of plugin that should be used and tries to choose that version.
    pub fn choose_version(&self, destination_dir: &Path) -> Result<Version, InstallError> {
        let git = crate::git::Git::in_repo(destination_dir);

        git.fetch_tags()?;

        let version = if let PluginSpec::Full(full_plugin_spec) = self
            && let Some(tag_or_commit) = &full_plugin_spec.tag_or_commit
        {
            tag_or_commit
        } else {
            let branch = git.get_default_branch()?;
            &Version::Branch(branch)
        };

        let version_str = match version {
            Version::Tag(tag) => tag,
            Version::Commit(commit) => commit,
            Version::Branch(branch) => branch,
        };

        git.checkout(version_str)?;

        Ok(version.clone())
    }
}
