use std::{
    env::VarError,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
};

use clap::Parser;
use murus::{OptionScope, Tmux};
use plux::plugin::{InstallError, PluginSpec, PluginSpecFile};

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

#[derive(Default)]
struct Logger {
    stdout: String,
    stderr: String,
}

macro_rules! stdout (
    ($logger:ident, $($fmt:tt)*) => {
        ::std::writeln!(&mut $logger.stdout, $($fmt)*).expect("writing to string is infallible")
    }
);

macro_rules! stderr (
    ($logger:ident, $($fmt:tt)*) => {
        ::std::writeln!(&mut $logger.stderr, $($fmt)*).expect("writing to string is infallible")
    }
);

fn main() {
    let mut logger = Logger::default();

    if let Err(error) = run(&mut logger) {
        eprintln!("Plugin installation failed...");
        eprintln!("stdout:\n{}", logger.stdout);
        eprintln!("stderr:\n{}", logger.stderr);
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(logger: &mut Logger) -> Result<(), Box<dyn std::error::Error>> {
    let _ = Config::parse();
    let Ok(tmux) = murus::Tmux::try_new() else {
        stdout!(logger, "Plux must be called within a tmux session.");
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

    // Ensure the plugins directory exists
    if let Err(error) = fs::create_dir_all(&plugins_path) {
        stderr!(
            logger,
            "Could not create plugins directory at {}",
            plugins_path.display()
        );
        stderr!(logger, "Error: {error}");
        std::process::exit(1);
    }

    // Check if config file exists, create it if not
    let plugins_spec = match std::fs::read_to_string(&plugin_spec_path) {
        Ok(p) => p,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Try to create the parent directory
            if let Some(parent) = plugin_spec_path.parent() {
                if let Err(create_error) = std::fs::create_dir_all(parent) {
                    stderr!(
                        logger,
                        "Could not create config directory at {}",
                        parent.display()
                    );
                    stderr!(logger, "Error: {create_error}");
                    std::process::exit(1);
                }
            }

            // Create a default config file with example content
            let default_config = r#"# Plux Plugin Configuration
#
# Add your tmux plugins here. Example:
#
# [plugins]
# tmux-grimoire = "https://github.com/navahas/tmux-grimoire"
# tmux-sensible = "https://github.com/tmux-plugins/tmux-sensible"
# tmux-yank = "https://github.com/tmux-plugins/tmux-yank"
#
# You can also specify versions:
# my-plugin = { url = "https://github.com/user/plugin", tag = "v1.0.0" }
# my-plugin = { url = "https://github.com/user/plugin", branch = "main" }
# my-plugin = { url = "https://github.com/user/plugin", commit = "<hash>" }

[plugins]
"#;

            if let Err(write_error) = std::fs::write(&plugin_spec_path, default_config) {
                stderr!(
                    logger,
                    "Could not create default config file at {}",
                    plugin_spec_path.display()
                );
                stderr!(logger, "Error: {write_error}");
                std::process::exit(1);
            }

            stdout!(
                logger,
                "Created default config file at {}",
                plugin_spec_path.display()
            );
            stdout!(logger, "Add your plugins to this file and reload tmux configuration.");

            default_config.to_string()
        }
        Err(error) => {
            stderr!(
                logger,
                "Could not read plugins spec at {}",
                plugin_spec_path.display()
            );
            stderr!(logger, "Error: {error}");
            stderr!(
                logger,
                "\nTroubleshooting:\n  1. Check file permissions\n  2. Verify the path is correct\n  3. Try creating an empty config file manually"
            );
            let error_code = error.raw_os_error().unwrap_or(1);
            std::process::exit(error_code);
        }
    };

    let plugin_spec: PluginSpecFile = match toml::from_str(&plugins_spec) {
        Ok(ps) => ps,
        Err(error) => {
            stderr!(
                logger,
                "Syntax error in plugins spec at {}:",
                plugin_spec_path.display()
            );
            stderr!(logger, "{error}");
            stderr!(
                logger,
                "\nTroubleshooting:\n  1. Check TOML syntax is valid\n  2. Ensure [plugins] section exists\n  3. See example format in the config file"
            );
            std::process::exit(1);
        }
    };

    install_plugins(logger, &plugins_path, plugin_spec.clone());

    source_plugins(logger, &plugins_path, &plugin_spec, &tmux);

    Ok(())
}

fn source_plugins(
    logger: &mut Logger,
    plugins_path: &Path,
    plugin_spec: &PluginSpecFile,
    tmux: &Tmux,
) {
    let (stderr, stderr_rx) = std::sync::mpsc::channel();

    std::thread::scope(move |scope| {
        let (tx, rx) = std::sync::mpsc::channel();

        for plugin in plugin_spec.plugins.keys() {
            let stderr = stderr.clone();
            let tx = tx.clone();
            scope.spawn(move || {
                let plugin_dir = plugins_path.join(plugin);

                let read_dir = std::fs::read_dir(&plugin_dir).unwrap();
                let entries: Vec<_> = read_dir.into_iter().map(Result::unwrap).collect();

                let plux_tmux_entry = entries.iter().find(|entry| {
                    entry
                        .path()
                        .file_name()
                        .is_some_and(|filename| filename == "plux.tmux")
                });

                if let Some(plux_tmux) = plux_tmux_entry {
                    match tmux.source_tmux(&plux_tmux.path()) {
                        Err(error) => stderr.send(format!("{error}")).unwrap(),
                        Ok(_) => return,
                    }
                }

                tx.send(entries).unwrap();
            });
        }

        drop(tx);

        while let Ok(entries) = rx.recv() {
            for entry in entries
                .into_iter()
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "tmux"))
            {
                let stderr = stderr.clone();
                scope.spawn(move || {
                    if let Err(error) = tmux.run_shell(&entry.path()) {
                        stderr.send(format!("{error}")).unwrap();
                    }
                });
            }
        }

        drop(stderr);

        while let Ok(stderr) = stderr_rx.recv() {
            stderr!(logger, "{stderr}");
        }
    });
}

fn install_plugins(logger: &mut Logger, plugins_path: &Path, plugin_spec: PluginSpecFile) {
    stdout!(logger, "installing plugins:");

    enum Msg {
        PluginReady(String, PluginSpec),
        Stdout(String),
    }

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::scope(|s| {
        for (plugin_name, plugin_spec) in plugin_spec.plugins {
            let tx = tx.clone();

            s.spawn(move || {
                let plugin_dir = plugins_path.join(&plugin_name);
                match plugin_spec.try_install(&plugin_dir) {
                    Ok(_) => tx.send(Msg::PluginReady(plugin_name, plugin_spec)).unwrap(),
                    Err(InstallError::AlreadyInstalled) => {
                        tx.send(Msg::Stdout(format!(
                            "\t{plugin_name} already installed, skipping git clone..."
                        )))
                        .unwrap();
                    }
                    Err(error) => {
                        tx.send(Msg::Stdout(format!("Could not install plugin:\n{error}")))
                            .unwrap();
                    }
                }
            });
        }

        drop(tx);

        while let Ok(msg) = rx.recv() {
            match msg {
                Msg::PluginReady(plugin_name, plugin_spec) => {
                    // plugin successfully cloned, now let's try setting the version
                    let plugin_dir = plugins_path.join(&plugin_name);
                    match plugin_spec.choose_version(&plugin_dir) {
                        Ok(installed_version) => {
                            stdout!(logger, "\t{plugin_name} intalled with {installed_version}");
                        }
                        Err(error) => {
                            stderr!(logger, "Failed to install '{plugin_name}', error:{error}");
                        }
                    }
                }
                Msg::Stdout(msg) => stdout!(logger, "{msg}"),
            }
        }
    });
}

fn expand_path(mut path: String) -> Result<PathBuf, VarError> {
    let home = std::env::var("HOME")?;
    path = path.replace("$HOME", &home);
    path = path.replace("~", &home);

    Ok(PathBuf::from(path))
}
