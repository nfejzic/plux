//! `murus` is implementation of tmux API in Rust.

use std::{io, process::Command};

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
}

pub struct Tmux {}

impl Tmux {
    pub fn try_new() -> Result<Self, Error> {
        match std::env::var("TMUX") {
            Ok(_) => Ok(Self {}),
            Err(_) => Err(Error::NotInTmux),
        }
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
}
