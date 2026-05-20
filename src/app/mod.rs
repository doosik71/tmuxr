use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::SessionSummary;
use crate::tmux::{TmuxClient, TmuxContext};
use crate::ui::{Screen, SessionListItem, TerminalGuard, ViewModel};

pub struct App {
    config: AppConfig,
    tmux_client: TmuxClient,
    state: AppState,
}

#[derive(Debug)]
struct AppState {
    screen: Screen,
    should_quit: bool,
    status_message: String,
    footer_hint: String,
    tmux_context: TmuxContext,
    sessions: Vec<SessionSummary>,
    selected_session: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            config: AppConfig::default(),
            tmux_client: TmuxClient::new(),
            state: AppState::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.refresh_sessions();

        let mut terminal = TerminalGuard::new(&self.config)?;
        while !self.state.should_quit {
            let view_model = self.state.view_model();
            terminal.draw(&view_model)?;

            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.handle_key_event(key)
                    }
                    Event::Resize(width, height) => self.state.handle_resize(width, height),
                    _ => {}
                }
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.state.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            KeyCode::Char('r') => self.refresh_sessions(),
            KeyCode::Up | KeyCode::Char('k') => self.state.select_previous_session(),
            KeyCode::Down | KeyCode::Char('j') => self.state.select_next_session(),
            KeyCode::Enter => self.state.activate_selected_session(),
            _ => {
                self.state.status_message = format!(
                    "Unhandled key: {}. Use Up/Down, Enter, r, q or Esc.",
                    describe_key_code(key.code)
                );
            }
        }
    }

    fn refresh_sessions(&mut self) {
        let context = self.tmux_client.detect();
        self.state.tmux_context = context.clone();

        if !context.binary_available {
            self.state.sessions.clear();
            self.state.selected_session = None;
            self.state.status_message =
                "tmux is not installed or not available in PATH.".to_string();
            return;
        }

        match self.tmux_client.list_sessions() {
            Ok(sessions) => {
                let session_count = sessions.len();
                self.state.sessions = sessions;
                self.state.ensure_valid_selection();
                self.state.status_message = if session_count == 0 {
                    "Connected to tmux. No sessions found.".to_string()
                } else {
                    format!("Loaded {session_count} tmux session(s).")
                };
            }
            Err(error) => {
                self.state.sessions.clear();
                self.state.selected_session = None;
                self.state.status_message = format!("Failed to load tmux sessions: {error}");
            }
        }
    }
}

impl AppState {
    fn new() -> Self {
        Self {
            screen: Screen::Home,
            should_quit: false,
            status_message: "Starting tmuxr...".to_string(),
            footer_hint: "Up/Down move | Enter inspect | r refresh | q/Esc quit".to_string(),
            tmux_context: TmuxContext::default(),
            sessions: Vec::new(),
            selected_session: None,
        }
    }

    fn handle_resize(&mut self, width: u16, height: u16) {
        self.status_message = format!("Terminal resized to {width}x{height}.");
    }

    fn ensure_valid_selection(&mut self) {
        self.selected_session = match self.sessions.len() {
            0 => None,
            len => match self.selected_session {
                Some(index) if index < len => Some(index),
                _ => Some(0),
            },
        };
    }

    fn select_previous_session(&mut self) {
        if self.sessions.is_empty() {
            self.selected_session = None;
            self.status_message = "No sessions available to select.".to_string();
            return;
        }

        let current = self.selected_session.unwrap_or(0);
        let next = if current == 0 {
            self.sessions.len() - 1
        } else {
            current - 1
        };
        self.selected_session = Some(next);
        self.status_message = format!("Selected session '{}'.", self.sessions[next].name);
    }

    fn select_next_session(&mut self) {
        if self.sessions.is_empty() {
            self.selected_session = None;
            self.status_message = "No sessions available to select.".to_string();
            return;
        }

        let current = self.selected_session.unwrap_or(0);
        let next = if current + 1 >= self.sessions.len() {
            0
        } else {
            current + 1
        };
        self.selected_session = Some(next);
        self.status_message = format!("Selected session '{}'.", self.sessions[next].name);
    }

    fn activate_selected_session(&mut self) {
        let Some(index) = self.selected_session else {
            self.status_message = "No session selected.".to_string();
            return;
        };

        let session = &self.sessions[index];
        self.status_message = format!(
            "Selected '{}' for a future action. Session actions land in the next phase.",
            session.name
        );
    }

    fn view_model(&self) -> ViewModel {
        let sessions = self
            .sessions
            .iter()
            .map(|session| SessionListItem {
                name: session.name.clone(),
                window_count: session.window_count,
                attached: session.attached,
            })
            .collect();

        ViewModel {
            screen: self.screen,
            title: "tmuxr".to_string(),
            subtitle: "Session list MVP".to_string(),
            sessions,
            selected_session: self.selected_session,
            detail_lines: self.detail_lines(),
            empty_message: self.empty_message(),
            footer_hint: self.footer_hint.clone(),
            status_message: self.status_message.clone(),
        }
    }

    fn detail_lines(&self) -> Vec<String> {
        let mut lines = vec![
            format!("Inside tmux client: {}", self.tmux_context.inside_client),
            format!("Known sessions: {}", self.sessions.len()),
            String::new(),
        ];

        if !self.tmux_context.binary_available {
            lines.push("tmux is not available in PATH.".to_string());
            lines.push("Install tmux, then press r to retry detection.".to_string());
            return lines;
        }

        let Some(index) = self.selected_session else {
            lines.push("No session selected.".to_string());
            lines.push("Use Up/Down to choose a session when one exists.".to_string());
            return lines;
        };

        let session = &self.sessions[index];
        lines.push(format!("Selected session: {}", session.name));
        lines.push(format!("Window count: {}", session.window_count));
        lines.push(format!("Attached: {}", session.attached));
        lines.push(String::new());
        lines.push("Enter currently marks this session as the action target.".to_string());
        lines.push("Create/attach/detach/kill actions arrive in the next phase.".to_string());
        lines
    }

    fn empty_message(&self) -> String {
        if !self.tmux_context.binary_available {
            "tmux not available".to_string()
        } else {
            "No tmux sessions running".to_string()
        }
    }
}

fn describe_key_code(code: KeyCode) -> String {
    match code {
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::BackTab => "BackTab".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::F(number) => format!("F{number}"),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::Null => "Null".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{AppState, describe_key_code};
    use crate::domain::SessionSummary;
    use crossterm::event::KeyCode;

    #[test]
    fn resize_updates_status_message() {
        let mut state = AppState::new();
        state.handle_resize(120, 40);
        assert_eq!(state.status_message, "Terminal resized to 120x40.");
    }

    #[test]
    fn key_code_description_is_human_readable() {
        assert_eq!(describe_key_code(KeyCode::Up), "Up");
        assert_eq!(describe_key_code(KeyCode::Char('r')), "r");
    }

    #[test]
    fn selection_defaults_to_first_session() {
        let mut state = AppState::new();
        state.sessions = vec![SessionSummary::new("dev", 2, true)];
        state.ensure_valid_selection();
        assert_eq!(state.selected_session, Some(0));
    }

    #[test]
    fn selection_wraps_forward() {
        let mut state = AppState::new();
        state.sessions = vec![
            SessionSummary::new("dev", 2, true),
            SessionSummary::new("ops", 1, false),
        ];
        state.ensure_valid_selection();
        state.select_next_session();
        state.select_next_session();
        assert_eq!(state.selected_session, Some(0));
    }

    #[test]
    fn selection_wraps_backward() {
        let mut state = AppState::new();
        state.sessions = vec![
            SessionSummary::new("dev", 2, true),
            SessionSummary::new("ops", 1, false),
        ];
        state.ensure_valid_selection();
        state.select_previous_session();
        assert_eq!(state.selected_session, Some(1));
    }
}
