use std::collections::HashMap;

pub const DEFAULT_PLUGINS_PATH: &str = "$HOME/.config/tmux/plux/";
pub const DEFAULT_SPEC_PATH: &str = "$HOME/.config/tmux/plux.toml";

#[derive(serde::Deserialize)]
pub struct PluginSpec {
    pub plugins: HashMap<String, String>,
}
