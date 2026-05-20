use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::config::AppConfig;
use crate::domain::SessionSummary;
use crate::tmux::{TmuxClient, TmuxContext};
use crate::ui::{ModalView, Screen, SessionListItem, TerminalGuard, ViewModel};

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
    tmux_context: TmuxContext,
    sessions: Vec<SessionSummary>,
    selected_session: Option<usize>,
    interaction: InteractionState,
}

#[derive(Debug, Clone)]
enum InteractionState {
    Browsing,
    Creating { detached: bool, name: String },
    ConfirmKill { session_name: String },
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
        match self.state.interaction.clone() {
            InteractionState::Browsing => self.handle_browsing_key_event(key),
            InteractionState::Creating { detached, .. } => {
                self.handle_create_key_event(key, detached)
            }
            InteractionState::ConfirmKill { session_name } => {
                self.handle_confirm_kill_key_event(key, &session_name)
            }
        }
    }

    fn handle_browsing_key_event(&mut self, key: KeyEvent) {
        if self.state.screen == Screen::Help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Char('h') | KeyCode::Esc => self.state.show_home(),
                KeyCode::Char('q') => self.state.should_quit = true,
                _ => {
                    self.state.status_message =
                        "Help is open. Press h, ?, or Esc to go back.".to_string();
                }
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.state.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.should_quit = true;
            }
            KeyCode::Char('?') | KeyCode::Char('h') => self.state.show_help(),
            KeyCode::Char('r') => self.refresh_sessions(),
            KeyCode::Up | KeyCode::Char('k') => self.state.select_previous_session(),
            KeyCode::Down | KeyCode::Char('j') => self.state.select_next_session(),
            KeyCode::Enter | KeyCode::Char('a') => self.attach_or_switch_selected_session(),
            KeyCode::Char('n') => self.state.start_create(false),
            KeyCode::Char('N') => self.state.start_create(true),
            KeyCode::Char('d') => self.detach_current_client(),
            KeyCode::Char('x') => self.state.start_kill_confirmation(),
            _ => {
                self.state.status_message = format!(
                    "Unhandled key: {}. Press ? for help.",
                    describe_key_code(key.code)
                );
            }
        }
    }

    fn handle_create_key_event(&mut self, key: KeyEvent, detached: bool) {
        match key.code {
            KeyCode::Esc => self.state.cancel_interaction("Canceled session creation."),
            KeyCode::Enter => self.create_session(detached),
            KeyCode::Backspace => self.state.pop_create_input(),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.push_create_input(ch)
            }
            _ => {
                self.state.status_message =
                    "Type a session name, Enter to create, Esc to cancel.".to_string();
            }
        }
    }

    fn handle_confirm_kill_key_event(&mut self, key: KeyEvent, session_name: &str) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') => self
                .state
                .cancel_interaction("Canceled kill session action."),
            KeyCode::Enter | KeyCode::Char('y') => self.kill_session(session_name),
            _ => {
                self.state.status_message =
                    "Press y or Enter to kill, Esc or n to cancel.".to_string();
            }
        }
    }

    fn create_session(&mut self, detached: bool) {
        let Some(name) = self.state.create_name() else {
            self.state.status_message = "No session name entered.".to_string();
            return;
        };

        match self.tmux_client.create_session(&name, detached) {
            Ok(()) => {
                self.state.finish_interaction();
                self.refresh_sessions();
                self.state.status_message = if detached {
                    format!("Created detached session '{name}'.")
                } else {
                    format!("Created session '{name}'.")
                };
            }
            Err(error) => {
                self.state.status_message = format!("Failed to create session: {error}");
            }
        }
    }

    fn attach_or_switch_selected_session(&mut self) {
        let Some(session) = self.state.selected_session_name() else {
            self.state.status_message = "No session selected.".to_string();
            return;
        };

        match self
            .tmux_client
            .attach_or_switch_session(&session, self.state.tmux_context.inside_client)
        {
            Ok(()) => {
                self.refresh_sessions();
                self.state.status_message = if self.state.tmux_context.inside_client {
                    format!("Switched current client to '{session}'.")
                } else {
                    format!("Attached to session '{session}'.")
                };
            }
            Err(error) => {
                self.state.status_message = format!("Failed to attach or switch session: {error}");
            }
        }
    }

    fn detach_current_client(&mut self) {
        match self
            .tmux_client
            .detach_client(self.state.tmux_context.inside_client)
        {
            Ok(()) => {
                self.refresh_sessions();
                self.state.status_message = "Detached current tmux client.".to_string();
            }
            Err(error) => {
                self.state.status_message = format!("Failed to detach client: {error}");
            }
        }
    }

    fn kill_session(&mut self, session_name: &str) {
        match self.tmux_client.kill_session(session_name) {
            Ok(()) => {
                self.state.finish_interaction();
                self.refresh_sessions();
                self.state.status_message = format!("Killed session '{session_name}'.");
            }
            Err(error) => {
                self.state.status_message = format!("Failed to kill session: {error}");
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
                if self.state.is_browsing() {
                    self.state.status_message = if session_count == 0 {
                        "Connected to tmux. No sessions found.".to_string()
                    } else {
                        format!("Loaded {session_count} tmux session(s).")
                    };
                }
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
            tmux_context: TmuxContext::default(),
            sessions: Vec::new(),
            selected_session: None,
            interaction: InteractionState::Browsing,
        }
    }

    fn is_browsing(&self) -> bool {
        matches!(self.interaction, InteractionState::Browsing)
    }

    fn current_footer_hint(&self) -> String {
        match (&self.screen, &self.interaction) {
            (_, InteractionState::Creating { .. }) => {
                "Type name | Enter confirm | Esc cancel".to_string()
            }
            (_, InteractionState::ConfirmKill { .. }) => {
                "y/Enter confirm kill | n/Esc cancel".to_string()
            }
            (Screen::Help, InteractionState::Browsing) => {
                "h/?/Esc back | q quit".to_string()
            }
            (Screen::Home, InteractionState::Browsing) => {
                "?/h help | Up/Down move | Enter/a attach | n new | N new -d | d detach | x kill | r refresh | q quit".to_string()
            }
        }
    }

    fn show_help(&mut self) {
        self.screen = Screen::Help;
        self.status_message = "Opened help guide.".to_string();
    }

    fn show_home(&mut self) {
        self.screen = Screen::Home;
        self.status_message = "Returned to the main screen.".to_string();
    }

    fn handle_resize(&mut self, width: u16, height: u16) {
        self.status_message = format!("Terminal resized to {width}x{height}.");
    }

    fn finish_interaction(&mut self) {
        self.interaction = InteractionState::Browsing;
    }

    fn cancel_interaction(&mut self, message: &str) {
        self.interaction = InteractionState::Browsing;
        self.status_message = message.to_string();
    }

    fn start_create(&mut self, detached: bool) {
        self.interaction = InteractionState::Creating {
            detached,
            name: String::new(),
        };
        self.status_message = if detached {
            "Enter a name for the new detached session.".to_string()
        } else {
            "Enter a name for the new session.".to_string()
        };
    }

    fn create_name(&self) -> Option<String> {
        match &self.interaction {
            InteractionState::Creating { name, .. } => Some(name.clone()),
            _ => None,
        }
    }

    fn push_create_input(&mut self, ch: char) {
        if let InteractionState::Creating { name, .. } = &mut self.interaction {
            name.push(ch);
        }
    }

    fn pop_create_input(&mut self) {
        if let InteractionState::Creating { name, .. } = &mut self.interaction {
            name.pop();
        }
    }

    fn start_kill_confirmation(&mut self) {
        let Some(session_name) = self.selected_session_name() else {
            self.status_message = "No session selected.".to_string();
            return;
        };
        self.interaction = InteractionState::ConfirmKill {
            session_name: session_name.clone(),
        };
        self.status_message = format!("Confirm killing session '{session_name}'.");
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

    fn selected_session_name(&self) -> Option<String> {
        self.selected_session
            .and_then(|index| self.sessions.get(index))
            .map(|session| session.name.clone())
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
            subtitle: match self.screen {
                Screen::Home => "Session actions".to_string(),
                Screen::Help => "Help and guidance".to_string(),
            },
            sessions,
            selected_session: self.selected_session,
            detail_lines: self.detail_lines(),
            empty_message: self.empty_message(),
            footer_hint: self.current_footer_hint(),
            status_message: self.status_message.clone(),
            modal: self.modal_view(),
        }
    }

    fn detail_lines(&self) -> Vec<String> {
        if self.screen == Screen::Help {
            return self.help_lines();
        }

        let mut lines = vec![
            format!("Inside tmux client: {}", self.tmux_context.inside_client),
            format!("Known sessions: {}", self.sessions.len()),
            String::new(),
        ];

        if !self.tmux_context.binary_available {
            lines.push("tmux is not available in PATH.".to_string());
            lines.push("Install tmux, then press r to retry detection.".to_string());
            lines.push("Press ? to see the built-in guide.".to_string());
            return lines;
        }

        let Some(index) = self.selected_session else {
            lines.push("No session selected.".to_string());
            lines.push("Use Up/Down to choose a session when one exists.".to_string());
            lines.push("Use n or N to create a session.".to_string());
            lines.push("Press ? to view the hotkey guide.".to_string());
            return lines;
        };

        let session = &self.sessions[index];
        lines.push(format!("Selected session: {}", session.name));
        lines.push(format!("Window count: {}", session.window_count));
        lines.push(format!("Attached: {}", session.attached));
        lines.push(String::new());
        lines.push(
            "Actions: Enter/a attach or switch, d detach current client, x kill.".to_string(),
        );
        lines.push("Creation: n for attached session, N for detached session.".to_string());
        lines.push("Guidance: press ? or h to open the full help screen.".to_string());
        lines
    }

    fn help_lines(&self) -> Vec<String> {
        vec![
            "tmuxr help".to_string(),
            String::new(),
            "Navigation".to_string(),
            "- Up/Down or j/k: move through the session list".to_string(),
            "- Enter or a: attach to the selected session, or switch client inside tmux"
                .to_string(),
            "- r: refresh the tmux session list".to_string(),
            "- h or ?: open/close this help screen".to_string(),
            "- Esc: go back from help, cancel dialogs, or quit from the main screen".to_string(),
            "- q: quit tmuxr".to_string(),
            String::new(),
            "Session actions".to_string(),
            "- n: create a new session".to_string(),
            "- N: create a new detached session".to_string(),
            "- d: detach the current client when running inside tmux".to_string(),
            "- x: open kill confirmation for the selected session".to_string(),
            String::new(),
            "Dialogs".to_string(),
            "- Create dialog: type a name, Enter confirms, Esc cancels".to_string(),
            "- Kill dialog: y or Enter confirms, n or Esc cancels".to_string(),
            String::new(),
            "Notes".to_string(),
            "- tmuxr is a helper UI on top of tmux commands.".to_string(),
            "- Some actions depend on whether you launched tmuxr inside tmux.".to_string(),
            "- Mouse support will be expanded in a later phase.".to_string(),
        ]
    }

    fn empty_message(&self) -> String {
        if !self.tmux_context.binary_available {
            "tmux not available".to_string()
        } else {
            "No tmux sessions running".to_string()
        }
    }

    fn modal_view(&self) -> Option<ModalView> {
        match &self.interaction {
            InteractionState::Browsing => None,
            InteractionState::Creating { detached, name } => Some(ModalView::Input {
                title: if *detached {
                    "Create Detached Session".to_string()
                } else {
                    "Create Session".to_string()
                },
                prompt: "Session name".to_string(),
                value: name.clone(),
                help: "Type a name, press Enter to confirm, Esc to cancel.".to_string(),
            }),
            InteractionState::ConfirmKill { session_name } => Some(ModalView::Confirm {
                title: "Kill Session".to_string(),
                message: format!("Kill session '{session_name}'?"),
                help: "Press y or Enter to confirm, Esc or n to cancel.".to_string(),
            }),
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
    use super::{AppState, InteractionState, describe_key_code};
    use crate::domain::SessionSummary;
    use crate::tmux::TmuxError;
    use crate::ui::Screen;
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

    #[test]
    fn create_flow_tracks_input() {
        let mut state = AppState::new();
        state.start_create(true);
        state.push_create_input('d');
        state.push_create_input('e');
        state.pop_create_input();
        assert_eq!(state.create_name().as_deref(), Some("d"));
        assert!(matches!(
            state.interaction,
            InteractionState::Creating { detached: true, .. }
        ));
    }

    #[test]
    fn kill_confirmation_uses_selected_session() {
        let mut state = AppState::new();
        state.sessions = vec![SessionSummary::new("dev", 2, true)];
        state.ensure_valid_selection();
        state.start_kill_confirmation();
        assert!(matches!(
            state.interaction,
            InteractionState::ConfirmKill { .. }
        ));
    }

    #[test]
    fn detach_requires_client_error_exists() {
        assert_eq!(
            TmuxError::DetachRequiresClient.to_string(),
            "detach is only available inside a tmux client"
        );
    }

    #[test]
    fn help_screen_can_be_opened_and_closed() {
        let mut state = AppState::new();
        state.show_help();
        assert_eq!(state.screen, Screen::Help);
        state.show_home();
        assert_eq!(state.screen, Screen::Home);
    }

    #[test]
    fn footer_hint_changes_with_help_screen() {
        let mut state = AppState::new();
        assert!(state.current_footer_hint().contains("help"));
        state.show_help();
        assert!(state.current_footer_hint().contains("back"));
    }
}
