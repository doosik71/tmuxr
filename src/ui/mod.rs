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
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            view_model.title.as_str(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(view_model.subtitle.as_str()),
    ])
    .block(Block::default().borders(Borders::ALL).title("Header"));

    frame.render_widget(header, layout[0]);

    match view_model.screen {
        Screen::Home => render_home(frame, view_model, layout[1]),
        Screen::Help => render_help(frame, view_model, layout[1]),
    }

    let footer = Paragraph::new(view_model.footer_hint.as_str())
        .block(Block::default().borders(Borders::ALL).title("Help"));

    let status = Paragraph::new(Line::from(Span::styled(
        view_model.status_message.as_str(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(footer, layout[2]);
    frame.render_widget(status, layout[3]);

    if let Some(modal) = &view_model.modal {
        render_modal(frame, modal);
    }
}

fn render_home(frame: &mut Frame, view_model: &ViewModel, area: Rect) {
    let content_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(52), Constraint::Percentage(48)])
        .split(area);

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
    frame.render_stateful_widget(list, content_layout[0], &mut list_state);

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
        .block(Block::default().borders(Borders::ALL).title("Details"))
        .wrap(Wrap { trim: false });

    frame.render_widget(details, content_layout[1]);
}

fn render_help(frame: &mut Frame, view_model: &ViewModel, area: Rect) {
    let help_text = view_model
        .detail_lines
        .iter()
        .map(|line| Line::from(line.as_str()))
        .collect::<Vec<_>>();

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help Guide"))
        .wrap(Wrap { trim: false });

    frame.render_widget(help, area);
}

fn render_modal(frame: &mut Frame, modal: &ModalView) {
    let area = centered_rect(60, 35, frame.area());
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
            ];
            let widget = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(title.as_str()))
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
            ];
            let widget = Paragraph::new(text)
                .block(Block::default().borders(Borders::ALL).title(title.as_str()))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: false });
            frame.render_widget(widget, area);
        }
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
