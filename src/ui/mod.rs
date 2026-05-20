use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Help,
}

#[derive(Debug, Clone)]
pub struct SessionListItem {
    pub name: String,
    pub window_count: usize,
    pub attached: bool,
}

#[derive(Debug, Clone)]
pub enum ModalView {
    Input {
        title: String,
        prompt: String,
        value: String,
        help: String,
    },
    Confirm {
        title: String,
        message: String,
        help: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalButton {
    Confirm,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct ViewModel {
    pub screen: Screen,
    pub title: String,
    pub subtitle: String,
    pub sessions: Vec<SessionListItem>,
    pub selected_session: Option<usize>,
    pub detail_lines: Vec<String>,
    pub empty_message: String,
    pub footer_hint: String,
    pub status_message: String,
    pub modal: Option<ModalView>,
}

pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    mouse_enabled: bool,
}

impl TerminalGuard {
    pub fn new(config: &AppConfig) -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        if config.mouse_enabled {
            execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        } else {
            execute!(stdout, EnterAlternateScreen)?;
        }

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        Ok(Self {
            terminal,
            mouse_enabled: config.mouse_enabled,
        })
    }

    pub fn draw(&mut self, view_model: &ViewModel) -> Result<()> {
        self.terminal.draw(|frame| render(frame, view_model))?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();

        if self.mouse_enabled {
            let _ = execute!(
                self.terminal.backend_mut(),
                LeaveAlternateScreen,
                DisableMouseCapture
            );
        } else {
            let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        }

        let _ = self.terminal.show_cursor();
    }
}

pub fn render(frame: &mut Frame, view_model: &ViewModel) {
    let layout = root_layout(frame.area());

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            view_model.title.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(view_model.subtitle.as_str()),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("Header"),
    );

    frame.render_widget(header, layout.header);

    match view_model.screen {
        Screen::Home => render_home(frame, view_model, layout.content),
        Screen::Help => render_help(frame, view_model, layout.content),
    }

    let footer = Paragraph::new(view_model.footer_hint.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title("Help"),
    );

    let status = Paragraph::new(Line::from(Span::styled(
        view_model.status_message.as_str(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(footer, layout.footer);
    frame.render_widget(status, layout.status);

    if let Some(modal) = &view_model.modal {
        render_modal(frame, modal);
    }
}

pub fn session_index_at(
    width: u16,
    height: u16,
    session_count: usize,
    column: u16,
    row: u16,
) -> Option<usize> {
    if session_count == 0 {
        return None;
    }

    let root = root_layout(Rect::new(0, 0, width, height));
    let list_area = home_regions(root.content).list;
    if !contains(list_area, column, row) {
        return None;
    }

    let inner = inset_borders(list_area)?;
    if !contains(inner, column, row) {
        return None;
    }

    let relative_row = row.saturating_sub(inner.y) as usize;
    if relative_row < session_count {
        Some(relative_row)
    } else {
        None
    }
}

pub fn modal_button_at(
    width: u16,
    height: u16,
    modal: &ModalView,
    column: u16,
    row: u16,
) -> Option<ModalButton> {
    let area = modal_area(Rect::new(0, 0, width, height), modal);
    let buttons = modal_button_areas(area);

    if contains(buttons.confirm, column, row) {
        return Some(ModalButton::Confirm);
    }
    if contains(buttons.cancel, column, row) {
        return Some(ModalButton::Cancel);
    }
    None
}

fn render_home(frame: &mut Frame, view_model: &ViewModel, area: Rect) {
    let regions = home_regions(area);

    let list_items = if view_model.sessions.is_empty() {
        vec![ListItem::new(Line::from(view_model.empty_message.as_str()))]
    } else {
        view_model
            .sessions
            .iter()
            .map(|session| {
                let attached = if session.attached {
                    "attached"
                } else {
                    "detached"
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{} ", session.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("windows={} | {attached}", session.window_count)),
                ]))
            })
            .collect()
    };

    let list = List::new(list_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Blue))
                .title("Sessions | Home"),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Cyan)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut list_state = ListState::default();
    if !view_model.sessions.is_empty() {
        list_state.select(view_model.selected_session);
    }
    frame.render_stateful_widget(list, regions.list, &mut list_state);

    let details_text = if view_model.detail_lines.is_empty() {
        vec![Line::from("No detail available.")]
    } else {
        view_model
            .detail_lines
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect::<Vec<_>>()
    };

    let details = Paragraph::new(details_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta))
                .title("Details"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(details, regions.details);
}

fn render_help(frame: &mut Frame, view_model: &ViewModel, area: Rect) {
    let help_text = view_model
        .detail_lines
        .iter()
        .map(|line| Line::from(line.as_str()))
        .collect::<Vec<_>>();

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title("Help Guide"),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

fn render_modal(frame: &mut Frame, modal: &ModalView) {
    let area = modal_area(frame.area(), modal);
    let buttons = modal_button_areas(area);
    frame.render_widget(Clear, area);

    match modal {
        ModalView::Input {
            title,
            prompt,
            value,
            help,
        } => {
            let text = vec![
                Line::from(prompt.as_str()),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "> ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(value.as_str()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    help.as_str(),
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from("Use mouse to click Confirm or Cancel."),
            ];
            let widget = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan))
                        .title(title.as_str()),
                )
                .wrap(Wrap { trim: false });
            frame.render_widget(widget, area);
        }
        ModalView::Confirm {
            title,
            message,
            help,
        } => {
            let text = vec![
                Line::from(message.as_str()),
                Line::from(""),
                Line::from(Span::styled(
                    help.as_str(),
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from("Mouse clicks on buttons are supported here."),
            ];
            let widget = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red))
                        .title(title.as_str()),
                )
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });
            frame.render_widget(widget, area);
        }
    }

    render_modal_button(frame, buttons.confirm, "Confirm", true);
    render_modal_button(frame, buttons.cancel, "Cancel", false);
}

fn render_modal_button(frame: &mut Frame, area: Rect, label: &str, primary: bool) {
    let style = if primary {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    };

    let button = Paragraph::new(label)
        .style(style)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(button, area);
}

fn root_layout(area: Rect) -> RootLayout {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    RootLayout {
        header: chunks[0],
        content: chunks[1],
        footer: chunks[2],
        status: chunks[3],
    }
}

fn home_regions(area: Rect) -> HomeRegions {
    let compact = area.width < 96 || area.height < 18;
    let chunks = if compact {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
            .split(area)
    };

    HomeRegions {
        list: chunks[0],
        details: chunks[1],
    }
}

fn modal_area(area: Rect, _modal: &ModalView) -> Rect {
    let percent_x = if area.width < 90 { 78 } else { 60 };
    let percent_y = if area.height < 24 { 55 } else { 35 };
    centered_rect(percent_x, percent_y, area)
}

fn modal_button_areas(area: Rect) -> ModalButtons {
    let button_y = area.y + area.height.saturating_sub(3);
    let button_width = 12.min(area.width.saturating_sub(4));
    let spacing = 2;
    let total = button_width.saturating_mul(2).saturating_add(spacing);
    let start_x = area.x + area.width.saturating_sub(total) / 2;

    ModalButtons {
        confirm: Rect::new(start_x, button_y, button_width, 3),
        cancel: Rect::new(start_x + button_width + spacing, button_y, button_width, 3),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn inset_borders(area: Rect) -> Option<Rect> {
    if area.width <= 2 || area.height <= 2 {
        return None;
    }

    Some(Rect::new(
        area.x + 1,
        area.y + 1,
        area.width - 2,
        area.height - 2,
    ))
}

fn contains(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

struct RootLayout {
    header: Rect,
    content: Rect,
    footer: Rect,
    status: Rect,
}

struct HomeRegions {
    list: Rect,
    details: Rect,
}

struct ModalButtons {
    confirm: Rect,
    cancel: Rect,
}

#[cfg(test)]
mod tests {
    use super::{ModalButton, ModalView, Screen, ViewModel, modal_button_at, session_index_at};

    #[test]
    fn click_maps_to_session_index() {
        let index = session_index_at(120, 32, 3, 4, 5);
        assert_eq!(index, Some(1));
    }

    #[test]
    fn click_outside_session_list_is_ignored() {
        let index = session_index_at(120, 32, 3, 90, 5);
        assert_eq!(index, None);
    }

    #[test]
    fn modal_buttons_are_clickable() {
        let modal = ModalView::Confirm {
            title: "Kill".to_string(),
            message: "Kill session?".to_string(),
            help: "help".to_string(),
        };
        let hit = modal_button_at(120, 32, &modal, 50, 18);
        assert_eq!(hit, Some(ModalButton::Confirm));
    }

    #[test]
    fn help_screen_variant_exists_in_view_model_usage() {
        let view_model = ViewModel {
            screen: Screen::Help,
            title: "tmuxr".to_string(),
            subtitle: "help".to_string(),
            sessions: Vec::new(),
            selected_session: None,
            detail_lines: vec!["line".to_string()],
            empty_message: String::new(),
            footer_hint: String::new(),
            status_message: String::new(),
            modal: None,
        };
        assert!(matches!(view_model.screen, Screen::Help));
    }
}
