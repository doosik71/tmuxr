use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::SessionSummary;
use crate::tmux::{TmuxClient, TmuxContext};
use crate::ui::{Screen, TerminalGuard, ViewModel};

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
            _ => {
                self.state.status_message = format!(
                    "Unhandled key: {}. Press q or Esc to quit, r to refresh.",
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
            self.state.status_message =
                "tmux is not installed or not available in PATH.".to_string();
            return;
        }

        match self.tmux_client.list_sessions() {
            Ok(sessions) => {
                let session_count = sessions.len();
                self.state.sessions = sessions;
                self.state.status_message = if session_count == 0 {
                    "Connected to tmux. No sessions found.".to_string()
                } else {
                    format!("Loaded {session_count} tmux session(s).")
                };
            }
            Err(error) => {
                self.state.sessions.clear();
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
            footer_hint: "q/Esc quit | r refresh | resize supported".to_string(),
            tmux_context: TmuxContext::default(),
            sessions: Vec::new(),
        }
    }

    fn handle_resize(&mut self, width: u16, height: u16) {
        self.status_message = format!("Terminal resized to {width}x{height}.");
    }

    fn view_model(&self) -> ViewModel {
        let mut content_lines = vec![
            format!("Inside tmux client: {}", self.tmux_context.inside_client),
            format!("Known sessions: {}", self.sessions.len()),
            String::new(),
        ];

        if !self.tmux_context.binary_available {
            content_lines.push("tmux is not available in PATH.".to_string());
            content_lines.push("Install tmux, then press r to retry detection.".to_string());
        } else if self.sessions.is_empty() {
            content_lines.push("No tmux sessions are currently running.".to_string());
            content_lines
                .push("The read-only session list view will land in the next phase.".to_string());
        } else {
            content_lines.push("Sessions discovered during startup or refresh:".to_string());
            content_lines.push(String::new());
            for session in &self.sessions {
                content_lines.push(format!(
                    "- {} | windows={} | attached={}",
                    session.name, session.window_count, session.attached
                ));
            }
        }

        ViewModel {
            screen: self.screen,
            title: "tmuxr".to_string(),
            subtitle: "Terminal UI framework bootstrap".to_string(),
            content_lines,
            footer_hint: self.footer_hint.clone(),
            status_message: self.status_message.clone(),
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
}
