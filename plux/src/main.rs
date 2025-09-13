// TODO:
//       -[x] where the install path for plugins is
//       -[x] list of plugins (plux.toml?)
//       -[?] source plugins from tmux, is there some standard on this?
//       -[ ] pull plugins (github + commit/tag)
//       -[ ] mark current version of plugin
//       -[ ] Update plugins

use std::{
    env::VarError,
    path::{Path, PathBuf},
};

use clap::Parser;
use murus::{OptionScope, Tmux};
use plux::plugin::PluginSpecFile;

const HELP_TEMPLATE: &str = r#"
{before-help}{name} {version}

{about} by: {author-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"#;

const AFTER_HELP: &str = r#"
Plux reads a plugin spec ("plux.toml") file by default at "~/.config/tmux/plux.toml" for plugins
specification. You can customize the location of plugin spec file by setting global variable
"@plux_toml_path" in your tmux configuration.

Plugin spec file is very simple and contains just a single toml table called "plugins" with each
entry being the plugin name set to a plugin URL value. For example:

```
[plugins]
some_plugin = "https://github.com/nfejzic/plux"
```

Plugins are installed by default at "~/.config/tmux/plux/" directory. This can be customized as 
well by setting global variable "@plux_plugins_path" in your tmux configuration.

To remain backwards compatible with plugins written for "tpm" plugin manager, plux runs plugins in 
one of two ways:

    1. If "plux_start.tmux" file is present in the plugin's top-level directory, this file will be
       sourced.
    2. Otherwise, all files with ".tmux" extension will be sourced.
"#;

#[derive(clap::Parser)]
#[command(version, author, about, long_about = None)]
#[command(help_template = HELP_TEMPLATE)]
#[command(after_help = AFTER_HELP)]
struct Config;

const PLUGINS_PATH_OPTION_NAME: &str = "@plux_plugins_path";
const SPEC_PATH_OPTION_NAME: &str = "@plux_toml_path";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = Config::parse();
    let Ok(tmux) = murus::Tmux::try_new() else {
        eprintln!("Plux must be called within a tmux session.");
        std::process::exit(1);
    };

    let plugin_spec_path = tmux
        .get_option(SPEC_PATH_OPTION_NAME, OptionScope::Global)
        .unwrap_or(plux::plugin::DEFAULT_SPEC_PATH.into());

    let plugin_spec_path = expand_path(plugin_spec_path)?;

    let plugins_path = tmux
        .get_option(PLUGINS_PATH_OPTION_NAME, OptionScope::Global)
        .unwrap_or(plux::plugin::DEFAULT_PLUGINS_PATH.into());

    let plugins_path = expand_path(plugins_path)?;

    let plugins_spec = match std::fs::read_to_string(&plugin_spec_path) {
        Ok(p) => p,
        Err(error) => {
            eprintln!(
                "Could not read plugins spec at {}",
                plugin_spec_path.display()
            );
            eprintln!("Error:\n{error}");
            let error_code = error.raw_os_error().unwrap_or(1);
            std::process::exit(error_code);
        }
    };
    let plugin_spec: PluginSpecFile = match toml::from_str(&plugins_spec) {
        Ok(ps) => ps,
        Err(error) => {
            eprintln!("Syntax error in plugins spec:\n{error}");
            std::process::exit(1);
        }
    };

    install_plugins(&plugins_path, &plugin_spec);
    source_plugins(&plugins_path, &plugin_spec, &tmux);

    Ok(())
}

fn source_plugins(plugins_path: &Path, plugin_spec: &PluginSpecFile, tmux: &Tmux) {
    for plugin in plugin_spec.plugins.keys() {
        let plugin_dir = plugins_path.join(plugin);

        let read_dir = std::fs::read_dir(&plugin_dir).unwrap();
        let entries: Vec<_> = read_dir.into_iter().map(Result::unwrap).collect();

        let plux_tmux_entry = entries.iter().find(|entry| {
            entry
                .path()
                .file_name()
                .is_some_and(|filename| filename == "plux.tmux")
        });

        if let Some(plux_tmux) = plux_tmux_entry
            && let Err(error) = tmux.source_tmux(&plux_tmux.path())
        {
            eprintln!("{error}");
        }

        for entry in entries
            .iter()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmux"))
        {
            if let Err(error) = tmux.run_shell(&entry.path()) {
                eprintln!("{error}");
            }
        }
    }
}

fn install_plugins(plugins_path: &Path, plugin_spec: &PluginSpecFile) {
    println!("installing plugins:");
    for (plugin_name, plugin_spec) in &plugin_spec.plugins {
        let plugin_dir = plugins_path.join(plugin_name);

        if plugin_dir.is_dir() {
            println!("\t{plugin_name} already installed, skipping git clone...");
        } else {
            println!(
                "\t{plugin_name} from '{plugin_spec:?}' -> '{}'",
                plugin_dir.display()
            );

            if let Err(error) = plugin_spec.try_install(&plugin_dir) {
                eprintln!("Could not install plugin:\n{error}");
                continue;
            }
        }

        // plugin successfully cloned, now let's try setting the version
        match plugin_spec.choose_version(&plugin_dir) {
            Ok(installed_version) => {
                println!("\t{plugin_name} intalled with {installed_version}")
            }
            Err(error) => {
                eprintln!("Failed to install '{plugin_name}', error:{error}");
            }
        }
    }
}

fn expand_path(mut path: String) -> Result<PathBuf, VarError> {
    let home = std::env::var("HOME")?;
    path = path.replace("$HOME", &home);
    path = path.replace("~", &home);

    Ok(PathBuf::from(path))
}
