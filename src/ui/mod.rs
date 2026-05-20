use std::io::{self, Stdout};

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::config::AppConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
}

#[derive(Debug, Clone)]
pub struct ViewModel {
    pub screen: Screen,
    pub title: String,
    pub subtitle: String,
    pub content_lines: Vec<String>,
    pub footer_hint: String,
    pub status_message: String,
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
            Constraint::Min(8),
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

    let screen_name = match view_model.screen {
        Screen::Home => "Home",
    };

    let content = Paragraph::new(
        view_model
            .content_lines
            .iter()
            .map(|line| Line::from(line.as_str()))
            .collect::<Vec<_>>(),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Content | {screen_name}")),
    )
    .wrap(Wrap { trim: false });

    let footer = Paragraph::new(view_model.footer_hint.as_str())
        .block(Block::default().borders(Borders::ALL).title("Help"));

    let status = Paragraph::new(Line::from(Span::styled(
        view_model.status_message.as_str(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(header, layout[0]);
    frame.render_widget(content, layout[1]);
    frame.render_widget(footer, layout[2]);
    frame.render_widget(status, layout[3]);
}
