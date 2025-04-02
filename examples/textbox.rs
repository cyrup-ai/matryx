use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
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

// Modal input system
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Visual,
}

struct TextBoxState {
    content: String,
    cursor_position: usize,
    mode: Mode,
    visual_start: Option<usize>,
    history: Vec<String>,
    history_index: Option<usize>,
    last_tick: Instant,
    show_cursor: bool,
}

impl TextBoxState {
    fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            mode: Mode::Normal,
            visual_start: None,
            history: Vec::new(),
            history_index: None,
            last_tick: Instant::now(),
            show_cursor: true,
        }
    }

    fn toggle_cursor(&mut self) {
        if self.last_tick.elapsed() >= Duration::from_millis(500) {
            self.show_cursor = !self.show_cursor;
            self.last_tick = Instant::now();
        }
    }

    fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
        self.visual_start = None;
        if self.cursor_position > 0 && !self.content.is_empty() {
            self.cursor_position = self.cursor_position.saturating_sub(1);
        }
    }

    fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    fn enter_visual_mode(&mut self) {
        self.mode = Mode::Visual;
        self.visual_start = Some(self.cursor_position);
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match self.mode {
            Mode::Normal => self.handle_normal_mode_key(key),
            Mode::Insert => self.handle_insert_mode_key(key),
            Mode::Visual => self.handle_visual_mode_key(key),
        }
    }

    fn handle_normal_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('i') => self.enter_insert_mode(),
            KeyCode::Char('a') => {
                if !self.content.is_empty() {
                    self.cursor_position = (self.cursor_position + 1).min(self.content.len());
                }
                self.enter_insert_mode();
            },
            KeyCode::Char('v') => self.enter_visual_mode(),
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('l') => self.move_cursor_right(),
            KeyCode::Char('0') => self.cursor_position = 0,
            KeyCode::Char('$') => {
                if !self.content.is_empty() {
                    self.cursor_position = self.content.len() - 1;
                }
            },
            KeyCode::Char('x') => self.delete_char(),
            KeyCode::Char('d') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.delete_line();
                }
            },
            KeyCode::Char('y') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.yank_line();
                }
            },
            KeyCode::Char('p') => self.paste(),
            KeyCode::Char('k') => self.previous_history(),
            KeyCode::Char('j') => self.next_history(),
            _ => {},
        }
    }

    fn handle_insert_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Backspace => self.delete_previous_char(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Left => self.move_cursor_left(),
            KeyCode::Right => self.move_cursor_right(),
            KeyCode::Up => self.previous_history(),
            KeyCode::Down => self.next_history(),
            _ => {},
        }
    }

    fn handle_visual_mode_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.enter_normal_mode(),
            KeyCode::Char('h') => self.move_cursor_left(),
            KeyCode::Char('l') => self.move_cursor_right(),
            KeyCode::Char('d') => self.delete_selection(),
            KeyCode::Char('y') => self.yank_selection(),
            _ => {},
        }
    }

    // Editing operations
    fn insert_char(&mut self, c: char) {
        if self.cursor_position >= self.content.len() {
            self.content.push(c);
        } else {
            self.content.insert(self.cursor_position, c);
        }
        self.cursor_position += 1;
    }

    fn delete_previous_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            if self.cursor_position < self.content.len() {
                self.content.remove(self.cursor_position);
            }
        }
    }

    fn delete_char(&mut self) {
        if self.cursor_position < self.content.len() {
            self.content.remove(self.cursor_position);
        }
    }

    fn move_cursor_left(&mut self) {
        self.cursor_position = self.cursor_position.saturating_sub(1);
    }

    fn move_cursor_right(&mut self) {
        if !self.content.is_empty() {
            self.cursor_position = (self.cursor_position + 1).min(self.content.len());
        }
    }

    fn delete_line(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
    }

    fn yank_line(&mut self) {
        // In a real implementation, this would save to clipboard
        // Here we just pretend
    }

    fn paste(&mut self) {
        // In a real implementation, this would paste from clipboard
        // Here we just add some example text
        let paste_text = "pasted text";
        if self.cursor_position >= self.content.len() {
            self.content.push_str(paste_text);
        } else {
            self.content.insert_str(self.cursor_position, paste_text);
        }
        self.cursor_position += paste_text.len();
    }

    fn delete_selection(&mut self) {
        if let Some(start) = self.visual_start {
            let (start_idx, end_idx) = if start <= self.cursor_position {
                (start, self.cursor_position)
            } else {
                (self.cursor_position, start)
            };

            if start_idx < self.content.len() {
                let end_idx = end_idx.min(self.content.len());
                self.content.replace_range(start_idx..=end_idx, "");
                self.cursor_position = start_idx;
            }
            self.visual_start = None;
        }
    }

    fn yank_selection(&mut self) {
        // In a real implementation, this would copy to clipboard
        self.enter_normal_mode();
    }

    fn add_to_history(&mut self) {
        if !self.content.is_empty() &&
            (self.history.is_empty() ||
                Some(self.content.as_str()) != self.history.last().map(|s| s.as_str()))
        {
            self.history.push(self.content.clone());
        }
        self.history_index = None;
    }

    fn previous_history(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => self.history.len() - 1,
            Some(0) => 0,
            Some(idx) => idx - 1,
        };

        self.history_index = Some(new_index);
        self.content = self.history[new_index].clone();
        self.cursor_position = self.content.len();
    }

    fn next_history(&mut self) {
        if self.history.is_empty() || self.history_index.is_none() {
            return;
        }

        let current_idx = self.history_index.unwrap();
        if current_idx >= self.history.len() - 1 {
            self.history_index = None;
            self.content.clear();
            self.cursor_position = 0;
        } else {
            let new_idx = current_idx + 1;
            self.history_index = Some(new_idx);
            self.content = self.history[new_idx].clone();
            self.cursor_position = self.content.len();
        }
    }

    // Visual selection helpers
    fn get_visual_selection(&self) -> Option<(usize, usize)> {
        self.visual_start.map(|start| {
            if start <= self.cursor_position {
                (start, self.cursor_position)
            } else {
                (self.cursor_position, start)
            }
        })
    }

    fn submit(&mut self) {
        self.add_to_history();
        self.content.clear();
        self.cursor_position = 0;
        self.mode = Mode::Normal;
        self.visual_start = None;
        self.history_index = None;
    }
}

struct TextBox {
    block: Block<'static>,
}

impl TextBox {
    fn new() -> Self {
        Self {
            block: Block::default()
                .borders(Borders::ALL)
                .title("Modal TextBox (Press i for Insert Mode, Esc for Normal)"),
        }
    }
}

impl StatefulWidget for TextBox {
    type State = TextBoxState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        self.block.render(area, buf);
        let inner_area = self.block.inner(area);

        // Mode indicator
        let mode_text = match state.mode {
            Mode::Normal => {
                Span::styled(" NORMAL ", Style::default().bg(Color::Blue).fg(Color::White))
            },
            Mode::Insert => {
                Span::styled(" INSERT ", Style::default().bg(Color::Green).fg(Color::White))
            },
            Mode::Visual => {
                Span::styled(" VISUAL ", Style::default().bg(Color::Magenta).fg(Color::White))
            },
        };

        // Add mode indicator at the end of the first line
        let mode_width = 8; // Width of the mode indicator
        let mode_area =
            Rect::new(inner_area.x + inner_area.width - mode_width, inner_area.y, mode_width, 1);

        Paragraph::new(Line::from(mode_text)).render(mode_area, buf);

        // Render text and cursor
        let text_area =
            Rect::new(inner_area.x, inner_area.y + 1, inner_area.width, inner_area.height - 1);

        // Handle visual selection if active
        let mut styled_text = String::new();
        if let Some((start, end)) = state.get_visual_selection() {
            if start < state.content.len() && end < state.content.len() {
                styled_text.push_str(&state.content[..start]);
                styled_text.push_str("\u{1b}[7m"); // Reverse video
                styled_text.push_str(&state.content[start..=end]);
                styled_text.push_str("\u{1b}[27m"); // Reset reverse video
                styled_text.push_str(&state.content[end + 1..]);
            } else {
                styled_text = state.content.clone();
            }
        } else {
            styled_text = state.content.clone();
        }

        let paragraph = Paragraph::new(styled_text);
        paragraph.render(text_area, buf);

        // Render cursor
        state.toggle_cursor();
        if state.show_cursor {
            let cursor_y = inner_area.y + 1;
            let cursor_x =
                inner_area.x + state.cursor_position.min(inner_area.width.saturating_sub(1));

            // Get the cell and apply cursor style
            let cell = buf.get_mut(cursor_x, cursor_y);
            let current_style = cell.style();
            cell.set_style(current_style.patch(Style::default().bg(Color::White).fg(Color::Black)));
        }
    }
}

// App struct to manage the application state
struct App {
    textbox_state: TextBoxState,
    last_submitted: String,
}

impl App {
    fn new() -> Self {
        Self {
            textbox_state: TextBoxState::new(),
            last_submitted: String::new(),
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => return false,
            KeyCode::Enter if self.textbox_state.mode == Mode::Insert => {
                self.last_submitted = self.textbox_state.content.clone();
                self.textbox_state.submit();
            },
            _ => self.textbox_state.handle_key_event(key),
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

            // Main layout with text box at the top and submitted text display at the bottom
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // TextBox
                    Constraint::Min(3),    // Last submitted text
                    Constraint::Length(1), // Help
                ])
                .split(size);

            // Render the modal text box
            let textbox = TextBox::new();
            f.render_stateful_widget(textbox, chunks[0], &mut app.textbox_state);

            // Render the last submitted text
            let submitted = Paragraph::new(app.last_submitted.as_str())
                .block(Block::default().borders(Borders::ALL).title("Last Submitted"))
                .style(Style::default());
            f.render_widget(submitted, chunks[1]);

            // Render help text
            let help =
                Paragraph::new("Ctrl+Q to quit | Enter to submit | Vim-like keys in Normal mode")
                    .style(Style::default().fg(Color::DarkGray));
            f.render_widget(help, chunks[2]);
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
