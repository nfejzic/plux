use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
};

pub const DEFAULT_PLUGINS_PATH: &str = "$HOME/.config/tmux/plux/";
pub const DEFAULT_SPEC_PATH: &str = "$HOME/.config/tmux/plux.toml";

#[derive(serde::Deserialize)]
pub struct PluginSpecFile {
    pub plugins: HashMap<String, PluginSpec>,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct FullPluginSpec {
    /// Url to the git repository where plugin is hosted.
    pub url: String,

    /// Optional version specification for the given plugin.
    #[serde(flatten)]
    pub tag_or_commit: Option<Version>,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Plugin is already installed.")]
    AlreadyInstalled,

    #[error("could not clone the plugin repository: {}", .0)]
    GitClone(#[from] std::io::Error),

    #[error("could not checkout the specified plugin version '{version}', error: {error}")]
    GitCheckout { version: String, error: String },

    #[error("could not determine plugin's version, error: {}", .0)]
    Version(String),

    #[error("could not fetch available tags: {}", .0)]
    TagFetch(String),
}

impl InstallError {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(untagged)]
pub enum PluginSpec {
    Url(String),
    Full(FullPluginSpec),
}

impl PluginSpec {
    pub fn url(&self) -> &str {
        match self {
            PluginSpec::Url(url) => url,
            PluginSpec::Full(full_plugin_spec) => &full_plugin_spec.url,
        }
    }

    pub fn try_install(&self, destination_dir: &Path) -> Result<(), InstallError> {
        let mut cmd = Command::new("git");

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

    pub fn choose_version(&self, destination_dir: &Path) -> Result<Version, InstallError> {
        let res = Command::new("git")
            .args(["fetch", "--all", "--tags"])
            .current_dir(destination_dir)
            .output();

        InstallError::wrap_cmd_res(res, InstallError::Version)?;

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
                InstallError::Version,
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
