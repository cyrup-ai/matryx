use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{
    disable_raw_mode,
    enable_raw_mode,
    EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, StatefulWidget, Widget};
use ratatui::Terminal;

// Dialog State
struct DialogState {
    visible: bool,
    selected_option: usize,
    options: Vec<String>,
}

impl DialogState {
    fn new(options: Vec<String>) -> Self {
        Self { visible: false, selected_option: 0, options }
    }

    fn show(&mut self) {
        self.visible = true;
        self.selected_option = 0;
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn next(&mut self) {
        self.selected_option = (self.selected_option + 1) % self.options.len();
    }

    fn previous(&mut self) {
        self.selected_option = (self.selected_option + self.options.len() - 1) % self.options.len();
    }

    fn selected(&self) -> Option<&String> {
        self.options.get(self.selected_option)
    }
}

// Dialog Widget
struct Dialog<'a> {
    title: &'a str,
    message: &'a str,
    block: Block<'a>,
}

impl<'a> Dialog<'a> {
    fn new(title: &'a str, message: &'a str) -> Self {
        Self {
            title,
            message,
            block: Block::default()
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::Gray).fg(Color::Black))
                .title(title),
        }
    }
}

impl<'a> StatefulWidget for Dialog<'a> {
    type State = DialogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.visible {
            return;
        }

        // Draw shadow/overlay on the entire screen
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = buf.get_mut(x, y);
                let style = Style::default().bg(Color::Black).fg(Color::DarkGray);
                cell.set_style(style);
            }
        }

        // Calculate dialog size and position (centered)
        let dialog_width = area.width.min(50);
        let dialog_height = area.height.min(8);
        let dialog_x = (area.width - dialog_width) / 2;
        let dialog_y = (area.height - dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);

        // Render dialog box
        self.block.render(dialog_area, buf);
        let inner_area = self.block.inner(dialog_area);

        // Render message
        let message = Paragraph::new(self.message)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Black));

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Message
                Constraint::Length(1), // Padding
                Constraint::Length(1), // Options
                Constraint::Min(0),    // Remaining space
            ])
            .split(inner_area);

        message.render(chunks[0], buf);

        // Render options in a horizontal layout
        let options_count = state.options.len();
        let option_constraints =
            vec![Constraint::Percentage(100 / options_count as u16); options_count];

        let option_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(option_constraints)
            .split(chunks[2]);

        for (i, option) in state.options.iter().enumerate() {
            let style = if i == state.selected_option {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Black)
            };

            let option_text =
                Paragraph::new(option.as_str()).alignment(Alignment::Center).style(style);

            option_text.render(option_chunks[i], buf);
        }
    }
}

// Modal-aware App
struct App {
    input_mode: InputMode,
    message: String,
    dialog_state: DialogState,
}

enum InputMode {
    Normal,
    Insert,
}

impl App {
    fn new() -> Self {
        Self {
            input_mode: InputMode::Normal,
            message: String::new(),
            dialog_state: DialogState::new(vec![
                "Yes".to_string(),
                "No".to_string(),
                "Cancel".to_string(),
            ]),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // If dialog is visible, handle dialog input
        if self.dialog_state.visible {
            match key.code {
                KeyCode::Left => {
                    self.dialog_state.previous();
                },
                KeyCode::Right => {
                    self.dialog_state.next();
                },
                KeyCode::Enter => {
                    if let Some(selected) = self.dialog_state.selected() {
                        self.message = format!("Selected: {}", selected);
                    }
                    self.dialog_state.hide();
                },
                KeyCode::Esc => {
                    self.dialog_state.hide();
                },
                _ => {},
            }
            return true;
        }

        // Otherwise handle normal app input
        match self.input_mode {
            InputMode::Normal => {
                match key.code {
                    KeyCode::Char('i') => {
                        self.input_mode = InputMode::Insert;
                    },
                    KeyCode::Char('q') => {
                        return false;
                    },
                    KeyCode::Char('d') => {
                        self.dialog_state.show();
                    },
                    _ => {},
                }
            },
            InputMode::Insert => {
                match key.code {
                    KeyCode::Esc => {
                        self.input_mode = InputMode::Normal;
                    },
                    KeyCode::Char(c) => {
                        self.message.push(c);
                    },
                    KeyCode::Backspace => {
                        self.message.pop();
                    },
                    _ => {},
                }
            },
        }
        true
    }
}

fn main() -> Result<(), io::Error> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App state
    let mut app = App::new();

    loop {
        terminal.draw(|f| {
            let size = f.size();

            // Main layout
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(size);

            // Mode indicator
            let mode = match app.input_mode {
                InputMode::Normal => "NORMAL",
                InputMode::Insert => "INSERT",
            };

            let mode_indicator = Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", mode),
                    Style::default()
                        .bg(match app.input_mode {
                            InputMode::Normal => Color::Blue,
                            InputMode::Insert => Color::Green,
                        })
                        .fg(Color::White),
                ),
                Span::raw(" Press 'd' to show dialog | 'q' to quit"),
            ]))
            .style(Style::default().fg(Color::White));

            f.render_widget(mode_indicator, chunks[1]);

            // Message area
            let message_area = Paragraph::new(app.message.as_str())
                .block(Block::default().borders(Borders::ALL).title("Message"));

            f.render_widget(message_area, chunks[0]);

            // Render dialog if visible
            let dialog = Dialog::new("Confirmation", "Do you want to proceed?");
            f.render_stateful_widget(dialog, size, &mut app.dialog_state);
        })?;

        // Input handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if !app.handle_key_event(key) {
                    break;
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}
