use std::fmt::Display;
use std::time::Duration;
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

struct Printer<'tmux> {
    tmux: &'tmux Tmux,
    output: OutputChoice,
}

impl Printer<'_> {
    fn display_in_status_line(&self, msg: &str, duration: impl Into<Option<Duration>>) {
        if let Some(duration) = duration
            .into()
            .and_then(|d| u32::try_from(d.as_millis()).ok())
        {
            self.tmux
                .display_message_with_duration(msg, duration)
                .expect("tmux should be callable within tmux session");
        } else {
            self.tmux
                .display_message(msg)
                .expect("tmux should be callable within tmux session");
        }
    }

    pub fn display_msg(&self, msg: &str, duration: impl Into<Option<Duration>>) {
        self.display_choice((msg, msg), duration);
    }

    pub fn display_choice(
        &self,
        (status, stdout): (&str, &str),
        duration: impl Into<Option<Duration>>,
    ) {
        match self.output {
            OutputChoice::Stdout => println!("{stdout}"),
            OutputChoice::Status => self.display_in_status_line(status, duration),
        }
    }
}

/// Option that decides where the output should be printed to.
#[derive(Default, Debug, Clone, clap::ValueEnum)]
enum OutputChoice {
    /// Print messages to stdout
    #[default]
    Stdout,

    /// Print messages to tmux status line
    Status,
}

impl OutputChoice {
    fn as_str(&self) -> &'static str {
        match self {
            OutputChoice::Stdout => "stdout",
            OutputChoice::Status => "status",
        }
    }
}

impl Display for OutputChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(clap::Parser)]
#[command(version, author, about, long_about = None)]
#[command(help_template = HELP_TEMPLATE)]
#[command(after_help = AFTER_HELP)]
struct CliArgs {
    #[clap(short, long, default_value_t = OutputChoice::Stdout)]
    output: OutputChoice,
}

fn main() {
    // Parse CLI args first - this will handle --help and --version and exit early
    let args = CliArgs::parse();

    let load_result = Tmux::try_new()
        .map_err(PluxError::from)
        .and_then(|tmux| Config::load(&tmux).map(|config| (tmux, config)));

    let (tmux, config) = match load_result {
        Ok((tmux, config)) => (tmux, config),
        Err(error) => {
            eprintln!("[ERROR] Plux failed:\n{error}");

            match error {
                PluxError::NotInTmux => {
                    eprintln!("\nPlux must be run inside a tmux session.");
                    eprintln!("Start tmux first with: tmux");
                }

                PluxError::PathExpansion(_) => eprintln!(
                    "Could not find home directory. Make sure $HOME variable is set properly."
                ),

                PluxError::DirectoryCreation { path, .. }
                | PluxError::ConfigRead { path, .. }
                | PluxError::ConfigWrite { path, .. }
                | PluxError::ConfigParse { path, .. } => {
                    eprintln!();
                    eprintln!("\nTroubleshooting:");
                    eprintln!("  1. Check TOML syntax in {}", path.display());
                    eprintln!("  2. Ensure [plugins] section exists");
                    eprintln!(
                        "  3. Or delete the file and run plux again to regenerate the default config"
                    );
                }

                // NOTE(nfejzic): no meaningful message for other errors
                PluxError::PluginInstall(_) | PluxError::Tmux(_) => {}
            }

            std::process::exit(1);
        }
    };

    let printer = Printer {
        tmux: &tmux,
        output: args.output,
    };

    let banner = format!(" plux v{} - tmux plugin manager", env!("CARGO_PKG_VERSION"));
    printer.display_msg(&banner, Duration::from_millis(500));

    printer.display_msg(
        &format!("{LOGO}\n{banner}\n——————————————————————————————————————"),
        None,
    );

    run(&tmux, printer, config);
}

fn run(tmux: &Tmux, printer: Printer, config: Config) {
    // Show progress via display-message for real-time feedback in tmux
    printer.display_msg(
        " PLUX | Checking for orphaned plugins...",
        Duration::from_millis(1000),
    );
    remove_orphaned_plugins(&printer, &config.plugins_path, &config.spec);

    printer.display_msg(" PLUX | Installing plugins...", Duration::from_secs(20));
    install_plugins(&printer, &config.plugins_path, config.spec.clone());

    printer.display_msg(" PLUX | Sourcing plugins...", Duration::from_secs(1));
    source_plugins(tmux, &config.plugins_path, &config.spec);

    // Success message - show immediately via display-message
    let plugin_count = config.spec.plugins.len();
    let success_msg = if plugin_count > 0 {
        format!("Plux completed! {} plugin(s) loaded", plugin_count)
    } else {
        "Plux completed! No plugins configured yet".to_string()
    };

    let detailed_msg = {
        let mut msg = String::new();
        msg += "\n";
        msg += "Plux completed successfully!";

        if plugin_count > 0 {
            msg += &format!("\n  {} plugin(s) loaded and sourced", plugin_count);
        } else {
            msg += &format!(
                "\n  No plugins configured. Add plugins to {} to get started.",
                config.spec_path.display()
            );
        }

        msg
    };

    printer.display_choice((&success_msg, &detailed_msg), Duration::from_secs(1));
}

fn remove_orphaned_plugins(printer: &Printer, plugins_path: &Path, plugin_spec: &PluginSpecFile) {
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
                    printer.display_msg(
                        &format!("  Removed orphaned plugin: {dir_name}"),
                        Duration::from_secs(1),
                    );
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

fn source_plugins(tmux: &Tmux, plugins_path: &Path, plugin_spec: &PluginSpecFile) {
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

fn install_plugins(printer: &Printer, plugins_path: &Path, plugin_spec: PluginSpecFile) {
    enum Msg {
        PluginReady(String, PluginSpec),
        AlreadyInstalled(String),
        Err { error: String, plugin: String },
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
                        tx.send(Msg::AlreadyInstalled(plugin_name))
                            .expect("receiver is dropped after this thread scope");
                    }
                    Err(error) => {
                        tx.send(Msg::Err {
                            error: format!("Could not install plugin:\n{error}"),
                            plugin: plugin_name,
                        })
                        .expect("receiver is dropped after this thread scope");
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
                            let msg = format!("  [OK] {plugin_name} ({installed_version})");

                            printer.display_msg(&msg, Duration::from_secs(1));
                        }
                        Err(error) => {
                            eprintln!("  [ERROR] {plugin_name} - Failed to install: {error}");
                        }
                    }
                }
                Msg::AlreadyInstalled(plugin_name) => printer.display_msg(
                    &format!("  [OK] {plugin_name} (already installed)"),
                    Duration::from_secs(1),
                ),
                Msg::Err { error, plugin } => eprintln!("  [ERROR] {plugin} ({error})"),
            }
        }
    });
}
