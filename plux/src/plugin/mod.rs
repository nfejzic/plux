use std::{collections::HashMap, path::PathBuf};

pub const DEFAULT_PLUGINS_PATH: &str = "$HOME/.config/tmux/plux/";
pub const DEFAULT_SPEC_PATH: &str = "$HOME/.config/tmux/plux.toml";

#[derive(serde::Deserialize)]
pub struct PluginSpecFile {
    pub plugins: HashMap<String, PluginSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagOrCommit {
    /// Git tag to be used as plugin's version.
    Tag(String),
    /// Git commit hash to be used as plugin's version.
    Commit(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
pub struct FullPluginSpec {
    /// Url to the git repository where plugin is hosted.
    pub url: String,

    /// Optional version specification for the given plugin.
    #[serde(flatten)]
    pub tag_or_commit: Option<TagOrCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(untagged)]
pub enum PluginSpec {
    Url(String),
    Full(FullPluginSpec),
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
}

impl PluginSpec {
    pub fn try_install(&self, destination_dir: PathBuf) -> Result<TagOrCommit, InstallError> {
        let mut cmd = std::process::Command::new("git");

        let url = match self {
            PluginSpec::Url(url) => url,
            PluginSpec::Full(full_plugin_spec) => &full_plugin_spec.url,
        };

        cmd.args(["clone", "--depth", "1", url])
            .arg(&destination_dir);

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(std::io::Error::other(format!(
                "Failed cloning plugin. Error:\n\tstderr = '{stderr}'"
            ))
            .into());
        }

        // plugin successfully cloned, now let's try setting the version
        self.choose_version(destination_dir)
    }

    fn choose_version(&self, destination_dir: PathBuf) -> Result<TagOrCommit, InstallError> {
        let tag_or_commit = if let PluginSpec::Full(full_plugin_spec) = self
            && let Some(tag_or_commit) = &full_plugin_spec.tag_or_commit
        {
            tag_or_commit
        } else {
            let result = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&destination_dir)
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    let commit_hash =
                        String::from_utf8(output.stdout).expect("commit hash is ascii");
                    &TagOrCommit::Commit(commit_hash)
                }
                Ok(output) => {
                    let stderr = String::from_utf8(output.stderr).expect("git produces valid utf8");
                    return Err(InstallError::Version(stderr));
                }
                Err(error) => return Err(InstallError::Version(error.to_string())),
            }
        };

        let version = match &tag_or_commit {
            TagOrCommit::Tag(tag) => tag,
            TagOrCommit::Commit(version) => version,
        };

        match std::process::Command::new("git")
            .args(["checkout", version])
            .current_dir(destination_dir)
            .output()
        {
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
