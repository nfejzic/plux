use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
};

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

    /// An error occurred while trying to clone plugin's repository.
    #[error("could not clone the plugin repository: {}", .0)]
    GitClone(#[from] std::io::Error),

    /// Git checkout of the specified version failed.
    #[error("could not checkout the specified plugin version '{version}', error: {error}")]
    GitCheckout { version: String, error: String },

    /// No version was specified and Plux could not determine the plugin's default branch.
    #[error("could not determine plugin's default branch: {}", .0)]
    DefaultBranch(String),

    /// Plux could not fetch available tags for a given plugin.
    #[error("could not fetch available tags: {}", .0)]
    TagFetch(String),
}

impl InstallError {
    /// Helper function to pull out `stdout` from command's output if it succeeded. If command
    /// failed, either the `stderr` or the [`std::io::Error`] is wrapped with the provided wrapper
    /// and returned as [`Err`].
    fn wrap_cmd_res(
        output: std::io::Result<Output>,
        wrapper: impl FnOnce(String) -> Self,
    ) -> Result<String, Self> {
        match output {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8(output.stdout).expect("commands return utf8");
                Ok(stdout)
            }
            Ok(output) => {
                let stderr = String::from_utf8(output.stderr).expect("commands return utf8");
                Err(wrapper(stderr))
            }
            Err(error) => Err(wrapper(error.to_string())),
        }
    }
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
        let mut cmd = Command::new("git");

        if destination_dir.is_dir() {
            return Err(InstallError::AlreadyInstalled);
        }

        let url = match self {
            PluginSpec::Url(url) => url,
            PluginSpec::Full(full_plugin_spec) => &full_plugin_spec.url,
        };

        cmd.args(["clone", "--depth", "1", url])
            .arg(destination_dir);

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(std::io::Error::other(format!(
                "Failed cloning plugin. Error:\n\tstderr = '{stderr}'"
            ))
            .into());
        }

        Ok(())
    }

    /// Determines the version of plugin that should be used and tries to choose that version.
    pub fn choose_version(&self, destination_dir: &Path) -> Result<Version, InstallError> {
        InstallError::wrap_cmd_res(
            Command::new("git")
                .args(["fetch", "--all", "--tags"])
                .current_dir(destination_dir)
                .output(),
            InstallError::TagFetch,
        )?;

        let tag_or_commit = if let PluginSpec::Full(full_plugin_spec) = self
            && let Some(tag_or_commit) = &full_plugin_spec.tag_or_commit
        {
            tag_or_commit
        } else {
            let default_branch = InstallError::wrap_cmd_res(
                Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "origin/HEAD"])
                    .current_dir(destination_dir)
                    .output(),
                InstallError::DefaultBranch,
            )?;

            let branch = default_branch
                .strip_prefix("origin/")
                .map(String::from)
                .unwrap_or(default_branch);

            &Version::Branch(branch)
        };

        let version = match &tag_or_commit {
            Version::Tag(tag) => tag,
            Version::Commit(version) => version,
            Version::Branch(branch) => branch,
        };

        let mut cmd = Command::new("git");
        cmd.args(["checkout", version.trim()])
            .current_dir(destination_dir);

        match cmd.output() {
            Ok(output) if output.status.success() => Ok(tag_or_commit.clone()),
            Ok(output) => Err(InstallError::GitCheckout {
                version: version.clone(),
                error: String::from_utf8(output.stderr).expect("tmux uses utf8"),
            }),
            Err(error) => Err(InstallError::GitCheckout {
                version: version.clone(),
                error: error.to_string(),
            }),
        }
    }
}
