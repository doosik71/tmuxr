use std::time::Duration;

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use crossterm::terminal;

use crate::config::AppConfig;
use crate::domain::SessionSummary;
use crate::tmux::{TmuxClient, TmuxContext};
use crate::ui::{
    FocusArea, ModalButton, ModalView, NavButton, Screen, SessionListItem, TerminalGuard,
    ViewModel, detail_button_index_at, menu_button_index_at, modal_button_at, session_index_at,
};

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
    terminal_size: (u16, u16),
    pending_attach: Option<String>,
    focus: FocusArea,
    menu_index: usize,
    detail_action_index: usize,
    needs_clear: bool,
}

#[derive(Debug, Clone)]
enum InteractionState {
    Browsing,
    Creating { detached: bool, name: String },
    ConfirmKill { session_name: String },
}

impl App {
    pub fn new() -> Self {
        let terminal_size = terminal::size().unwrap_or((100, 32));
        Self {
            config: AppConfig::default(),
            tmux_client: TmuxClient::new(),
            state: AppState::new(terminal_size),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.refresh_sessions();

        while !self.state.should_quit {
            let mut terminal = TerminalGuard::new(&self.config)?;
            terminal.clear()?; // Ensure fresh start for every new guard

            while !self.state.should_quit && self.state.pending_attach.is_none() {
                if self.state.needs_clear {
                    let _ = terminal.clear();
                    self.state.needs_clear = false;
                }
                let view_model = self.state.view_model();
                terminal.draw(&view_model)?;

                if event::poll(Duration::from_millis(250))? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            self.handle_key_event(key)
                        }
                        Event::Mouse(mouse) => self.handle_mouse_event(mouse),
                        Event::Resize(width, height) => {
                            self.state.handle_resize(width, height);
                            let _ = terminal.clear(); // Clear on resize
                        }
                        _ => {}
                    }
                }
            }

            if let Some(session_name) = self.state.pending_attach.take() {
                drop(terminal);
                if let Err(error) = self.tmux_client.spawn_attach(&session_name) {
                    self.state.status_message = format!("Failed to attach: {error}");
                }
                self.refresh_sessions();
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

    fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => self.handle_left_click(mouse),
            MouseEventKind::ScrollUp => self.state.select_previous_session(),
            MouseEventKind::ScrollDown => self.state.select_next_session(),
            _ => {}
        }
    }

    fn handle_left_click(&mut self, mouse: MouseEvent) {
        if let Some(modal) = self.state.modal_view() {
            if let Some(button) = modal_button_at(
                self.state.terminal_size.0,
                self.state.terminal_size.1,
                &modal,
                mouse.column,
                mouse.row,
            ) {
                match button {
                    ModalButton::Confirm => match self.state.interaction.clone() {
                        InteractionState::Creating { detached, .. } => self.create_session(detached),
                        InteractionState::ConfirmKill { session_name } => {
                            self.kill_session(&session_name)
                        }
                        InteractionState::Browsing => {}
                    },
                    ModalButton::Cancel => match self.state.interaction {
                        InteractionState::Creating { .. } => {
                            self.state.cancel_interaction("Canceled session creation.")
                        }
                        InteractionState::ConfirmKill { .. } => self
                            .state
                            .cancel_interaction("Canceled kill session action."),
                        InteractionState::Browsing => {}
                    },
                }
            }
            return;
        }

        // Check MenuBar
        if let Some(index) = menu_button_index_at(
            self.state.terminal_size.0,
            self.state.terminal_size.1,
            mouse.column,
            mouse.row,
        ) {
            self.state.focus = FocusArea::MenuBar;
            self.state.menu_index = index;
            self.trigger_menu_action();
            return;
        }

        // Check SessionList
        if let Some(index) = session_index_at(
            self.state.terminal_size.0,
            self.state.terminal_size.1,
            self.state.sessions.len(),
            mouse.column,
            mouse.row,
        ) {
            self.state.focus = FocusArea::SessionList;
            self.state.select_session(index);
            return;
        }

        // Check Details Actions
        let button_count = self.state.detail_buttons_count();
        if let Some(index) = detail_button_index_at(
            self.state.terminal_size.0,
            self.state.terminal_size.1,
            button_count,
            mouse.column,
            mouse.row,
        ) {
            self.state.focus = FocusArea::Details;
            self.state.detail_action_index = index;
            self.trigger_detail_action();
            return;
        }
    }

    fn handle_browsing_key_event(&mut self, key: KeyEvent) {
        if self.state.screen == Screen::Help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc => self.state.show_home(),
                KeyCode::Char('q') => self.state.should_quit = true,
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Tab => {
                self.state.focus = match self.state.focus {
                    FocusArea::MenuBar => FocusArea::SessionList,
                    FocusArea::SessionList => FocusArea::Details,
                    FocusArea::Details => FocusArea::MenuBar,
                };
            }
            KeyCode::BackTab => {
                self.state.focus = match self.state.focus {
                    FocusArea::MenuBar => FocusArea::Details,
                    FocusArea::SessionList => FocusArea::MenuBar,
                    FocusArea::Details => FocusArea::SessionList,
                };
            }
            KeyCode::Up | KeyCode::Char('k') => match self.state.focus {
                FocusArea::SessionList => self.state.select_previous_session(),
                FocusArea::Details => self.state.move_detail_focus_up(),
                _ => {}
            },
            KeyCode::Down | KeyCode::Char('j') => match self.state.focus {
                FocusArea::MenuBar => self.state.focus = FocusArea::SessionList,
                FocusArea::SessionList => self.state.select_next_session(),
                FocusArea::Details => self.state.move_detail_focus_down(),
            },
            KeyCode::Left | KeyCode::Char('h') => match self.state.focus {
                FocusArea::MenuBar => self.state.move_menu_focus_left(),
                FocusArea::SessionList => self.state.focus = FocusArea::MenuBar,
                FocusArea::Details => self.state.focus = FocusArea::SessionList,
            },
            KeyCode::Right | KeyCode::Char('l') => match self.state.focus {
                FocusArea::MenuBar => self.state.move_menu_focus_right(),
                FocusArea::SessionList => self.state.focus = FocusArea::Details,
                _ => {}
            },
            KeyCode::Enter | KeyCode::Char(' ') => match self.state.focus {
                FocusArea::MenuBar => self.trigger_menu_action(),
                FocusArea::SessionList => self.attach_or_switch_selected_session(),
                FocusArea::Details => self.trigger_detail_action(),
            },
            // Legacy hotkeys for convenience
            KeyCode::Char('n') => self.state.start_create(false),
            KeyCode::Char('N') => self.state.start_create(true),
            KeyCode::Char('r') => self.refresh_sessions(),
            KeyCode::Char('d') => self.detach_current_client(),
            KeyCode::Char('x') => self.state.start_kill_confirmation(),
            KeyCode::Char('?') => self.state.show_help(),
            KeyCode::Char('q') | KeyCode::Esc => self.state.should_quit = true,
            _ => {}
        }
    }

    fn trigger_menu_action(&mut self) {
        match self.state.menu_index {
            0 => self.state.start_create(false),
            1 => self.refresh_sessions(),
            2 => self.state.show_help(),
            3 => self.state.should_quit = true,
            _ => {}
        }
    }

    fn trigger_detail_action(&mut self) {
        let session_attached = self
            .state
            .selected_session
            .and_then(|i| self.state.sessions.get(i))
            .map(|s| s.attached)
            .unwrap_or(false);

        let action = match (session_attached, self.state.tmux_context.inside_client) {
            (true, true) => match self.state.detail_action_index {
                0 => "attach",
                1 => "detach",
                2 => "kill",
                _ => return,
            },
            _ => match self.state.detail_action_index {
                0 => "attach",
                1 => "kill",
                _ => return,
            },
        };

        match action {
            "attach" => self.attach_or_switch_selected_session(),
            "detach" => self.detach_current_client(),
            "kill" => self.state.start_kill_confirmation(),
            _ => {}
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

        match self.tmux_client.create_session(&name) {
            Ok(()) => {
                self.state.finish_interaction();
                self.refresh_sessions();
                self.state.needs_clear = true;

                if !detached && self.state.tmux_context.inside_client {
                    if let Err(error) = self
                        .tmux_client
                        .attach_or_switch_session(&name, self.state.tmux_context.inside_client)
                    {
                        self.state.status_message =
                            format!("Created session but failed to switch: {error}");
                        return;
                    }
                }

                self.state.status_message = if detached {
                    format!("Created detached session '{name}'.")
                } else if self.state.tmux_context.inside_client {
                    format!("Created and switched to session '{name}'.")
                } else {
                    format!("Created session '{name}'. (Use tmux attach -t {name})")
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
            Ok(needs_interactive) => {
                self.state.needs_clear = true;
                if needs_interactive {
                    self.state.pending_attach = Some(session);
                } else {
                    self.refresh_sessions();
                    self.state.status_message =
                        format!("Switched current client to '{session}'.");
                }
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
                self.state.needs_clear = true;
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
                self.state.needs_clear = true;
                self.state.status_message = format!("Killed session '{session_name}'.");
            }
            Err(error) => {
                self.state.status_message = format!("Failed to kill session: {error}");
            }
        }
    }

    fn refresh_sessions(&mut self) {
        self.state.needs_clear = true;
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
    fn new(terminal_size: (u16, u16)) -> Self {
        Self {
            screen: Screen::Home,
            should_quit: false,
            status_message: "Starting tmuxr...".to_string(),
            tmux_context: TmuxContext::default(),
            sessions: Vec::new(),
            selected_session: None,
            interaction: InteractionState::Browsing,
            terminal_size,
            pending_attach: None,
            focus: FocusArea::SessionList,
            menu_index: 0,
            detail_action_index: 0,
            needs_clear: false,
        }
    }

    fn is_browsing(&self) -> bool {
        matches!(self.interaction, InteractionState::Browsing)
    }

    fn current_footer_hint(&self) -> String {
        match (&self.screen, &self.interaction) {
            (_, InteractionState::Creating { .. }) => {
                "Type name | Enter confirm | Click Confirm/Cancel | Esc cancel".to_string()
            }
            (_, InteractionState::ConfirmKill { .. }) => {
                "y/Enter confirm kill | Click buttons | n/Esc cancel".to_string()
            }
            (Screen::Help, InteractionState::Browsing) => {
                "?/Esc back | q quit".to_string()
            }
            (Screen::Home, InteractionState::Browsing) => {
                "? help | Up/Down or wheel move | Click select | Enter/a attach | n/N create | d detach | x kill | r refresh | q quit".to_string()
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
        self.terminal_size = (width, height);
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

    fn select_session(&mut self, index: usize) {
        if index < self.sessions.len() {
            self.selected_session = Some(index);
            self.status_message = format!("Selected session '{}'.", self.sessions[index].name);
        }
    }

    fn move_menu_focus_left(&mut self) {
        if self.menu_index > 0 {
            self.menu_index -= 1;
        } else {
            self.menu_index = 3; // Wrap to Quit
        }
    }

    fn move_menu_focus_right(&mut self) {
        if self.menu_index < 3 {
            self.menu_index += 1;
        } else {
            self.menu_index = 0; // Wrap to New
        }
    }

    fn move_detail_focus_up(&mut self) {
        if self.detail_action_index > 0 {
            self.detail_action_index -= 1;
        } else {
            let max = self.detail_buttons_count().saturating_sub(1);
            self.detail_action_index = max;
        }
    }

    fn move_detail_focus_down(&mut self) {
        let max = self.detail_buttons_count().saturating_sub(1);
        if self.detail_action_index < max {
            self.detail_action_index += 1;
        } else {
            self.detail_action_index = 0;
        }
    }

    fn detail_buttons_count(&self) -> usize {
        let session = match self.selected_session.and_then(|i| self.sessions.get(i)) {
            Some(s) => s,
            None => return 0,
        };
        if session.attached && self.tmux_context.inside_client {
            3 // Attach, Detach, Kill
        } else {
            2 // Attach, Kill
        }
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

        let menu_buttons = vec![
            NavButton {
                label: "New".to_string(),
                key_hint: "n".to_string(),
                selected: self.focus == FocusArea::MenuBar && self.menu_index == 0,
            },
            NavButton {
                label: "Refresh".to_string(),
                key_hint: "r".to_string(),
                selected: self.focus == FocusArea::MenuBar && self.menu_index == 1,
            },
            NavButton {
                label: "Help".to_string(),
                key_hint: "?".to_string(),
                selected: self.focus == FocusArea::MenuBar && self.menu_index == 2,
            },
            NavButton {
                label: "Quit".to_string(),
                key_hint: "q".to_string(),
                selected: self.focus == FocusArea::MenuBar && self.menu_index == 3,
            },
        ];

        let mut detail_buttons = Vec::new();
        if let Some(session) = self.selected_session.and_then(|i| self.sessions.get(i)) {
            detail_buttons.push(NavButton {
                label: "Attach".to_string(),
                key_hint: "a".to_string(),
                selected: self.focus == FocusArea::Details && self.detail_action_index == 0,
            });
            if session.attached && self.tmux_context.inside_client {
                let idx = detail_buttons.len();
                detail_buttons.push(NavButton {
                    label: "Detach".to_string(),
                    key_hint: "d".to_string(),
                    selected: self.focus == FocusArea::Details && self.detail_action_index == idx,
                });
            }
            let idx = detail_buttons.len();
            detail_buttons.push(NavButton {
                label: "Kill".to_string(),
                key_hint: "x".to_string(),
                selected: self.focus == FocusArea::Details && self.detail_action_index == idx,
            });
        }

        ViewModel {
            screen: self.screen,
            title: "tmuxr".to_string(),
            subtitle: match self.screen {
                Screen::Home => {
                    if self.terminal_size.0 < 96 || self.terminal_size.1 < 18 {
                        "Session actions | compact layout".to_string()
                    } else {
                        "Session actions".to_string()
                    }
                }
                Screen::Help => "Help and guidance".to_string(),
            },
            sessions,
            selected_session: self.selected_session,
            detail_lines: self.detail_lines(),
            empty_message: self.empty_message(),
            footer_hint: self.current_footer_hint(),
            status_message: self.status_message.clone(),
            modal: self.modal_view(),
            focus: self.focus,
            menu_buttons,
            detail_buttons,
        }
    }

    fn detail_lines(&self) -> Vec<String> {
        if self.screen == Screen::Help {
            return self.help_lines();
        }

        let mut lines = vec![
            format!("Inside tmux client: {}", self.tmux_context.inside_client),
            format!("Known sessions: {}", self.sessions.len()),
            format!(
                "Terminal size: {}x{}",
                self.terminal_size.0, self.terminal_size.1
            ),
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
            lines.push("Use Up/Down, mouse wheel, or click a session to choose one.".to_string());
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
        lines.push("Mouse: click a session to select it, scroll to move.".to_string());
        lines.push("Creation: n for attached session, N for detached session.".to_string());
        lines.push("Guidance: press ? to open the full help screen.".to_string());
        lines
    }

    fn help_lines(&self) -> Vec<String> {
        vec![
            "tmuxr help".to_string(),
            String::new(),
            "Navigation".to_string(),
            "- Up/Down or j/k: move through the session list".to_string(),
            "- Mouse wheel: move through the session list".to_string(),
            "- Left click on a session: select it".to_string(),
            "- Enter or a: attach to the selected session, or switch client inside tmux"
                .to_string(),
            "- r: refresh the tmux session list".to_string(),
            "- ?: open/close this help screen".to_string(),
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
            "- Mouse: click Confirm or Cancel in dialogs".to_string(),
            String::new(),
            "Layout".to_string(),
            "- The UI switches to a compact stacked layout in smaller terminals.".to_string(),
            "- Resize the terminal and tmuxr will adapt the session/detail arrangement."
                .to_string(),
            String::new(),
            "Notes".to_string(),
            "- tmuxr is a helper UI on top of tmux commands.".to_string(),
            "- Some actions depend on whether you launched tmuxr inside tmux.".to_string(),
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

// fn describe_key_code(code: KeyCode) -> String {
//     match code {
//         KeyCode::Backspace => "Backspace".to_string(),
//         KeyCode::Enter => "Enter".to_string(),
//         KeyCode::Left => "Left".to_string(),
//         KeyCode::Right => "Right".to_string(),
//         KeyCode::Up => "Up".to_string(),
//         KeyCode::Down => "Down".to_string(),
//         KeyCode::Home => "Home".to_string(),
//         KeyCode::End => "End".to_string(),
//         KeyCode::PageUp => "PageUp".to_string(),
//         KeyCode::PageDown => "PageDown".to_string(),
//         KeyCode::Tab => "Tab".to_string(),
//         KeyCode::BackTab => "BackTab".to_string(),
//         KeyCode::Delete => "Delete".to_string(),
//         KeyCode::Insert => "Insert".to_string(),
//         KeyCode::F(number) => format!("F{number}"),
//         KeyCode::Char(character) => character.to_string(),
//         KeyCode::Null => "Null".to_string(),
//         KeyCode::Esc => "Esc".to_string(),
//         other => format!("{other:?}"),
//     }
// }

#[cfg(test)]
mod tests {
    use super::{AppState, InteractionState, describe_key_code};
    use crate::domain::SessionSummary;
    use crate::tmux::TmuxError;
    use crate::ui::Screen;
    use crossterm::event::KeyCode;

    #[test]
    fn resize_updates_status_message() {
        let mut state = AppState::new((100, 30));
        state.handle_resize(120, 40);
        assert_eq!(state.status_message, "Terminal resized to 120x40.");
        assert_eq!(state.terminal_size, (120, 40));
    }

    #[test]
    fn key_code_description_is_human_readable() {
        assert_eq!(describe_key_code(KeyCode::Up), "Up");
        assert_eq!(describe_key_code(KeyCode::Char('r')), "r");
    }

    #[test]
    fn selection_defaults_to_first_session() {
        let mut state = AppState::new((100, 30));
        state.sessions = vec![SessionSummary::new("dev", 2, true)];
        state.ensure_valid_selection();
        assert_eq!(state.selected_session, Some(0));
    }

    #[test]
    fn selection_wraps_forward() {
        let mut state = AppState::new((100, 30));
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
        let mut state = AppState::new((100, 30));
        state.sessions = vec![
            SessionSummary::new("dev", 2, true),
            SessionSummary::new("ops", 1, false),
        ];
        state.ensure_valid_selection();
        state.select_previous_session();
        assert_eq!(state.selected_session, Some(1));
    }

    #[test]
    fn direct_selection_sets_status() {
        let mut state = AppState::new((100, 30));
        state.sessions = vec![
            SessionSummary::new("dev", 2, true),
            SessionSummary::new("ops", 1, false),
        ];
        state.select_session(1);
        assert_eq!(state.selected_session, Some(1));
        assert!(state.status_message.contains("ops"));
    }

    #[test]
    fn create_flow_tracks_input() {
        let mut state = AppState::new((100, 30));
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
        let mut state = AppState::new((100, 30));
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
        let mut state = AppState::new((100, 30));
        state.show_help();
        assert_eq!(state.screen, Screen::Help);
        state.show_home();
        assert_eq!(state.screen, Screen::Home);
    }

    #[test]
    fn footer_hint_changes_with_help_screen() {
        let mut state = AppState::new((100, 30));
        assert!(state.current_footer_hint().contains("Click select"));
        state.show_help();
        assert!(state.current_footer_hint().contains("back"));
    }
}
