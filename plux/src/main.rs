use std::{fs, path::Path};

use clap::Parser;
use murus::Tmux;
use plux::config::Config;
use plux::error::PluxError;
use plux::plugin::{InstallError, PluginSpec, PluginSpecFile};

const HELP_TEMPLATE: &str = r#"
{before-help}{name} {version}

{about} by: {author-with-newline}
{usage-heading} {usage}

{all-args}{after-help}
"#;

const AFTER_HELP: &str = r#"
CONFIGURATION:
  Plugin spec file:   ~/.config/tmux/plux.toml  (customize with @plux_toml_path)
  Plugins directory:  ~/.config/tmux/plux/      (customize with @plux_plugins_path)

PLUGIN SPECIFICATION:
  The plux.toml file contains a [plugins] table mapping plugin names to URLs:

    [plugins]
    some_plugin = "https://github.com/user/repo"

PLUGIN EXECUTION:
  Plux maintains backward compatibility with TPM plugins using two execution modes:
    1. If "plux.tmux" exists in the plugin root → sourced via tmux source-file
    2. Otherwise → all *.tmux files executed via tmux run-shell

"#;

const LOGO: &str = r#"
__________.____     ____ _______  ___
\______   \    |   |    |   \   \/  /
 |     ___/    |   |    |   /\     / 
 |    |   |    |___|    |  / /     \ 
 |____|   |_______ \______/ /___/\  \
                  \/              \_/
"#;

#[derive(clap::Parser)]
#[command(version, author, about, long_about = None)]
#[command(help_template = HELP_TEMPLATE)]
#[command(after_help = AFTER_HELP)]
struct CliArgs;

fn main() {
    // Parse CLI args first - this will handle --help and --version and exit early
    let _ = CliArgs::parse();

    // Only show banner when actually running the plugin manager
    if let Ok(tmux) = Tmux::try_new() {
        let banner = format!(" plux v{} - tmux plugin manager", env!("CARGO_PKG_VERSION"));
        println!("{}\n{}", LOGO, banner);
        println!("——————————————————————————————————————");

        let _ = tmux.display_message_with_duration(&banner, 500);
    }

    if let Err(error) = run() {
        eprintln!("Error: {error}");

        // Provide helpful context based on error type
        match &error {
            PluxError::NotInTmux => {
                eprintln!("\nPlux must be run inside a tmux session.");
                eprintln!("Start tmux first with: tmux");
            }
            PluxError::ConfigParse { path, .. } => {
                eprintln!("\nTroubleshooting:");
                eprintln!("  1. Check TOML syntax is valid");
                eprintln!("  2. Ensure [plugins] section exists");
                eprintln!("  3. See example format in {}", path.display());
            }
            _ => {}
        }

        std::process::exit(1);
    }
}

fn run() -> Result<(), PluxError> {
    let tmux = Tmux::try_new().map_err(|_| PluxError::NotInTmux)?;
    let config = Config::load(&tmux)?;

    // Show progress via display-message for real-time feedback in tmux
    let _ = tmux.display_message_with_duration(" PLUX | Checking for orphaned plugins...", 1000);
    remove_orphaned_plugins(&config.plugins_path, &config.spec);

    let _ = tmux.display_message_with_duration(" PLUX | Installing plugins...", 20_000);
    install_plugins(&config.plugins_path, config.spec.clone());

    let _ = tmux.display_message_with_duration(" PLUX | Sourcing plugins...", 1000);
    source_plugins(&config.plugins_path, &config.spec, &tmux);

    // Success message - show immediately via display-message
    let plugin_count = config.spec.plugins.len();
    let success_msg = if plugin_count > 0 {
        format!("Plux completed! {} plugin(s) loaded", plugin_count)
    } else {
        "Plux completed! No plugins configured yet".to_string()
    };
    let _ = tmux.display_message_with_duration(&success_msg, 1000);

    // Also log detailed info to stdout
    println!();
    println!("Plux completed successfully!");
    if plugin_count > 0 {
        println!("  {} plugin(s) loaded and sourced", plugin_count);
    } else {
        println!(
            "  No plugins configured. Add plugins to {} to get started.",
            config.spec_path.display()
        );
    }

    Ok(())
}

fn remove_orphaned_plugins(plugins_path: &Path, plugin_spec: &PluginSpecFile) {
    // If plugins directory doesn't exist, nothing to clean up
    if !plugins_path.exists() {
        return;
    }

    let Ok(entries) = fs::read_dir(plugins_path) else {
        eprintln!(
            "Could not read plugins directory at {}",
            plugins_path.display()
        );
        return;
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        // Only consider directories
        if !file_type.is_dir() {
            continue;
        }

        let dir_name_os = entry.file_name();
        let Some(dir_name) = dir_name_os.to_str() else {
            continue;
        };

        // Check if this directory name is in the plugin spec
        if !plugin_spec.plugins.contains_key(dir_name) {
            // This is an orphaned plugin - remove it
            let plugin_path = entry.path();
            match fs::remove_dir_all(&plugin_path) {
                Ok(_) => {
                    println!("  Removed orphaned plugin: {}", dir_name);
                }
                Err(error) => {
                    eprintln!(
                        "  Failed to remove orphaned plugin '{}': {}",
                        dir_name, error
                    );
                }
            }
        }
    }
}

fn source_plugins(plugins_path: &Path, plugin_spec: &PluginSpecFile, tmux: &Tmux) {
    let (stderr_tx, stderr_rx) = std::sync::mpsc::channel();

    std::thread::scope(move |scope| {
        let (tx, rx) = std::sync::mpsc::channel();

        for plugin in plugin_spec.plugins.keys() {
            let stderr = stderr_tx.clone();
            let tx = tx.clone();
            scope.spawn(move || {
                let plugin_dir = plugins_path.join(plugin);

                let read_dir = fs::read_dir(&plugin_dir).unwrap();
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
                let stderr = stderr_tx.clone();
                scope.spawn(move || {
                    if let Err(error) = tmux.run_shell(&entry.path()) {
                        stderr.send(format!("{error}")).unwrap();
                    }
                });
            }
        }

        drop(stderr_tx);

        while let Ok(error_msg) = stderr_rx.recv() {
            eprintln!("{error_msg}");
        }
    });
}

fn install_plugins(plugins_path: &Path, plugin_spec: PluginSpecFile) {
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
                            "  [OK] {plugin_name} (already installed)"
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
                            println!("  [OK] {plugin_name} ({installed_version})");
                        }
                        Err(error) => {
                            eprintln!("  [ERROR] {plugin_name} - Failed to install: {error}");
                        }
                    }
                }
                Msg::Stdout(msg) => println!("{msg}"),
            }
        }
    });
}
