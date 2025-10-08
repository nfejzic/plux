/// Represents the current state of a given session in tmux.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Attached,

    #[default]
    Detached,
}

/// Represents information about a single session in tmux.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Session {
    pub name: String,
    pub windows: usize,
    pub state: State,
}

impl From<&str> for Session {
    fn from(session_str: &str) -> Self {
        let mut split = session_str.split('(');

        let first_part = split.next().expect("creation timestamp in parenthesis");

        let (session_name, window_count) = first_part
            .split_once(":")
            .expect("session name and window count are guaranteed");

        let window_count = window_count
            .chars()
            .skip(1)
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap();

        let state = match split.nth(1) {
            Some(attach_info) if attach_info.contains("attached") => State::Attached,
            _ => State::Detached,
        };

        Self {
            name: session_name.to_string(),
            windows: window_count,
            state,
        }
    }
}
