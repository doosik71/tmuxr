use std::env;
use std::ffi::OsStr;
use std::io;
use std::process::Command;

use thiserror::Error;

use crate::domain::SessionSummary;

const TMUX_BIN: &str = "tmux";
const SESSION_FORMAT: &str = "#S|#{session_windows}|#{?session_attached,1,0}";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TmuxContext {
    pub binary_available: bool,
    pub inside_client: bool,
}

#[derive(Debug, Error)]
pub enum TmuxError {
    #[error("tmux binary not found in PATH")]
    BinaryNotFound,
    #[error("tmux command failed: {command}")]
    CommandFailed { command: String, stderr: String },
    #[error("failed to parse tmux session line: {line}")]
    InvalidSessionLine { line: String },
    #[error("invalid window count in tmux session line: {value}")]
    InvalidWindowCount { value: String },
    #[error("session name cannot be empty")]
    EmptySessionName,
    #[error("detach is only available inside a tmux client")]
    DetachRequiresClient,
}

#[derive(Debug, Clone)]
pub struct TmuxClient {
    program: String,
}

impl Default for TmuxClient {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxClient {
    pub fn new() -> Self {
        Self {
            program: TMUX_BIN.to_string(),
        }
    }

    pub fn detect(&self) -> TmuxContext {
        TmuxContext {
            binary_available: self.is_available(),
            inside_client: is_inside_tmux(),
        }
    }

    pub fn is_available(&self) -> bool {
        match self.command(["-V"]).status() {
            Ok(status) => status.success(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => false,
            Err(_) => false,
        }
    }

    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>, TmuxError> {
        let output = self.run(["list-sessions", "-F", SESSION_FORMAT])?;

        if is_no_server_error(&output.stderr) {
            return Ok(Vec::new());
        }

        parse_sessions_output(&output.stdout)
    }

    pub fn create_session(&self, name: &str, detached: bool) -> Result<(), TmuxError> {
        let name = validate_session_name(name)?;
        if detached {
            self.run(["new-session", "-d", "-s", name])?;
        } else {
            self.run(["new-session", "-s", name])?;
        }
        Ok(())
    }

    pub fn attach_or_switch_session(
        &self,
        name: &str,
        inside_client: bool,
    ) -> Result<(), TmuxError> {
        let name = validate_session_name(name)?;
        if inside_client {
            self.run(["switch-client", "-t", name])?;
        } else {
            self.run(["attach-session", "-t", name])?;
        }
        Ok(())
    }

    pub fn kill_session(&self, name: &str) -> Result<(), TmuxError> {
        let name = validate_session_name(name)?;
        self.run(["kill-session", "-t", name])?;
        Ok(())
    }

    pub fn detach_client(&self, inside_client: bool) -> Result<(), TmuxError> {
        if !inside_client {
            return Err(TmuxError::DetachRequiresClient);
        }
        self.run(["detach-client"])?;
        Ok(())
    }

    fn run<I, S>(&self, args: I) -> Result<TmuxOutput, TmuxError>
    where
        I: IntoIterator<Item = S> + Clone,
        S: AsRef<OsStr>,
    {
        let output = self.command(args.clone()).output().map_err(map_io_error)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if output.status.success() || is_no_server_error(&stderr) {
            return Ok(TmuxOutput { stdout, stderr });
        }

        let command = format_command(&self.program, args);
        Err(TmuxError::CommandFailed { command, stderr })
    }

    fn command<I, S>(&self, args: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = Command::new(&self.program);
        command.args(args);
        command
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TmuxOutput {
    stdout: String,
    stderr: String,
}

pub fn is_inside_tmux() -> bool {
    env::var_os("TMUX").is_some_and(|value| !value.is_empty())
}

fn validate_session_name(name: &str) -> Result<&str, TmuxError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(TmuxError::EmptySessionName);
    }
    Ok(trimmed)
}

fn parse_sessions_output(output: &str) -> Result<Vec<SessionSummary>, TmuxError> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(parse_session_line)
        .collect()
}

fn parse_session_line(line: &str) -> Result<SessionSummary, TmuxError> {
    let mut parts = line.split('|');
    let name = parts.next().unwrap_or_default();
    let window_count = parts.next().unwrap_or_default();
    let attached = parts.next().unwrap_or_default();

    if name.is_empty() || window_count.is_empty() || attached.is_empty() || parts.next().is_some() {
        return Err(TmuxError::InvalidSessionLine {
            line: line.to_string(),
        });
    }

    let window_count =
        window_count
            .parse::<usize>()
            .map_err(|_| TmuxError::InvalidWindowCount {
                value: window_count.to_string(),
            })?;

    let attached = match attached {
        "1" => true,
        "0" => false,
        _ => {
            return Err(TmuxError::InvalidSessionLine {
                line: line.to_string(),
            });
        }
    };

    Ok(SessionSummary::new(name, window_count, attached))
}

fn is_no_server_error(stderr: &str) -> bool {
    let stderr = stderr.trim();
    stderr.contains("no server running")
        || stderr.contains("failed to connect to server")
        || stderr.contains("no sessions")
}

fn map_io_error(error: io::Error) -> TmuxError {
    if error.kind() == io::ErrorKind::NotFound {
        TmuxError::BinaryNotFound
    } else {
        TmuxError::CommandFailed {
            command: TMUX_BIN.to_string(),
            stderr: error.to_string(),
        }
    }
}

fn format_command<I, S>(program: &str, args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut parts = vec![program.to_string()];
    for arg in args {
        parts.push(arg.as_ref().to_string_lossy().into_owned());
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        TmuxError, is_no_server_error, parse_session_line, parse_sessions_output,
        validate_session_name,
    };
    use crate::domain::SessionSummary;

    #[test]
    fn parses_single_session_line() {
        let session = parse_session_line("dev|3|1").expect("session should parse");
        assert_eq!(session, SessionSummary::new("dev", 3, true));
    }

    #[test]
    fn parses_multiple_sessions() {
        let sessions = parse_sessions_output("dev|3|1\nops|1|0\n").expect("sessions should parse");

        assert_eq!(
            sessions,
            vec![
                SessionSummary::new("dev", 3, true),
                SessionSummary::new("ops", 1, false)
            ]
        );
    }

    #[test]
    fn rejects_invalid_window_count() {
        let error = parse_session_line("dev|abc|1").expect_err("line should fail");
        assert!(matches!(error, TmuxError::InvalidWindowCount { .. }));
    }

    #[test]
    fn rejects_invalid_attached_flag() {
        let error = parse_session_line("dev|2|attached").expect_err("line should fail");
        assert!(matches!(error, TmuxError::InvalidSessionLine { .. }));
    }

    #[test]
    fn detects_no_server_errors() {
        assert!(is_no_server_error(
            "no server running on /tmp/tmux-1000/default"
        ));
        assert!(is_no_server_error("failed to connect to server"));
        assert!(is_no_server_error("no sessions"));
        assert!(!is_no_server_error("permission denied"));
    }

    #[test]
    fn validates_session_name() {
        assert_eq!(validate_session_name(" dev ").expect("valid"), "dev");
        assert!(matches!(
            validate_session_name("   "),
            Err(TmuxError::EmptySessionName)
        ));
    }
}
