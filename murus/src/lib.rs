//! `murus` is implementation of tmux API in Rust.

use std::{
    io,
    path::Path,
    process::{Command, Output},
};

use session::Session;

pub mod session;

#[cfg(debug_assertions)]
fn format_cmd(cmd: &Command) -> String {
    let mut output: String = cmd.get_program().to_string_lossy().to_string();

    for arg in cmd.get_args() {
        output += " ";
        output += &arg.to_string_lossy();
    }

    output
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("We are currently not in a tmux instance")]
    NotInTmux,

    #[error("Command failed {}", .0)]
    CommandFailed(#[from] io::Error),

    #[error("Option not found {}", .0)]
    OptionNotFound(String),

    #[error("Failed sourcing file. stdout:\n{stdout}\n\nstderr:\n{stderr}")]
    SourceFile { stdout: String, stderr: String },
}

pub struct Tmux {}

impl Tmux {
    pub fn try_new() -> Result<Self, Error> {
        match std::env::var("TMUX") {
            Ok(_) => Ok(Self {}),
            Err(_) => Err(Error::NotInTmux),
        }
    }

    pub fn get_option(&self, option: &str, scope: OptionScope) -> Result<String, Error> {
        let mut cmd = std::process::Command::new("tmux");

        // NOTE: -v makes sure only value is returned without option name
        cmd.arg("show").arg("-v");

        if let Some(scope) = scope.to_arg() {
            cmd.arg(scope);
        }

        let output = cmd.arg(option).output()?;

        if !output.stderr.is_empty() {
            return Err(Error::OptionNotFound(
                String::from_utf8(output.stderr).expect("tmux uses utf8"),
            ));
        }

        Ok(read_stdout(output))
    }

    pub fn set_option(&self, option: &str, value: &str, scope: OptionScope) -> Result<(), Error> {
        let mut cmd = std::process::Command::new("tmux");

        cmd.arg("set");

        if let Some(scope) = scope.to_arg() {
            cmd.arg(scope);
        }

        cmd.arg(option).arg(value).spawn()?.wait()?;

        Ok(())
    }

    fn run_cmd(mut cmd: Command) -> Result<(), Error> {
        let output = cmd.output()?;

        if !output.status.success() {
            let stdout = String::from_utf8(output.stdout).expect("tmux uses utf8");
            let stderr = String::from_utf8(output.stderr).expect("tmux uses utf8");
            return Err(Error::SourceFile { stdout, stderr });
        }

        Ok(())
    }

    pub fn source_tmux(&self, path: &Path) -> Result<(), Error> {
        let mut cmd = std::process::Command::new("tmux");
        cmd.arg("source-file").arg(path);

        Self::run_cmd(cmd)
    }

    pub fn run_shell(&self, path: &Path) -> Result<(), Error> {
        let mut cmd = std::process::Command::new("tmux");
        cmd.arg("run-shell").arg("-b").arg(path);

        Self::run_cmd(cmd)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>, Error> {
        let mut cmd = std::process::Command::new("tmux");
        cmd.arg("list-sessions");

        #[cfg(debug_assertions)]
        println!("cmd = {}", format_cmd(&cmd));

        let output = cmd.output()?;

        let sessions = String::from_utf8(output.stdout)
            .expect("tmux uses utf8 for names")
            .lines()
            .map(Session::from)
            .collect();

        Ok(sessions)
    }

    pub fn switch_session(&self, session: &Session) -> Result<(), Error> {
        std::process::Command::new("tmux")
            .arg("switch-client")
            .arg("-t")
            .arg(&session.name)
            .spawn()?
            .wait()?;

        Ok(())
    }

    /// Displays a message in the tmux status line.
    /// This is useful for showing real-time feedback from scripts run via run-shell.
    /// The message will be displayed for the default duration (750ms by default in tmux).
    pub fn display_message(&self, message: &str) -> Result<(), Error> {
        std::process::Command::new("tmux")
            .arg("display-message")
            .arg(message)
            .spawn()?
            .wait()?;

        Ok(())
    }

    /// Displays a message in the tmux status line with a custom duration.
    /// Duration is specified in milliseconds.
    pub fn display_message_with_duration(&self, message: &str, duration_ms: u32) -> Result<(), Error> {
        std::process::Command::new("tmux")
            .arg("display-message")
            .arg("-d")
            .arg(duration_ms.to_string())
            .arg(message)
            .spawn()?
            .wait()?;

        Ok(())
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionScope {
    #[default]
    Session,
    Pane,
    Window,
    Server,
    Global,
}

impl OptionScope {
    fn to_arg(self) -> Option<&'static str> {
        match self {
            OptionScope::Session => None,
            OptionScope::Pane => Some("-p"),
            OptionScope::Window => Some("-w"),
            OptionScope::Server => Some("-s"),
            OptionScope::Global => Some("-g"),
        }
    }
}

fn read_stdout(output: Output) -> String {
    let stdout = output.stdout;
    String::from_utf8(stdout).expect("tmux uses utf8")
}
