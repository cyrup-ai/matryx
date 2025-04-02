use std::io;
use std::time::Duration;

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
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block,
    Borders,
    List,
    ListItem,
    ListState,
    Paragraph,
    StatefulWidget,
    Widget,
};
use ratatui::Terminal;

// Mode system
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Normal,
    Insert,
}

// Window trait for different window types
trait WindowTrait {
    fn render(&mut self, rect: Rect, buf: &mut Buffer, focused: bool);
    fn handle_key_event(&mut self, key: KeyEvent, mode: &Mode) -> Option<Action>;
    fn title(&self) -> &str;
}

// Action enum for window operations
enum Action {
    Quit,
    FocusNext,
    FocusPrevious,
    SwitchMode(Mode),
    CloseWindow,
    NewWindow,
    SplitHorizontal,
    SplitVertical,
}

// A basic list window
struct ListView {
    title: String,
    items: Vec<String>,
    state: ListState,
}

impl ListView {
    fn new(title: &str, items: Vec<String>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self { title: title.to_string(), items, state }
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            },
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            },
            None => 0,
        };
        self.state.select(Some(i));
    }
}

impl WindowTrait for ListView {
    fn render(&mut self, rect: Rect, buf: &mut Buffer, focused: bool) {
        let block =
            Block::default()
                .title(self.title.clone())
                .borders(Borders::ALL)
                .style(if focused {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default()
                });

        let items: Vec<ListItem> = self.items.iter().map(|i| ListItem::new(i.as_str())).collect();

        let list = List::new(items).block(block).highlight_style(
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        );

        let mut list_state = self.state.clone();

        // We need to manually render with state since we're using a Buffer
        list.render(rect, buf);

        // Apply highlighting for selected item
        if let Some(selected) = list_state.selected() {
            if selected < self.items.len() {
                let list_area = list.inner(rect);
                if selected < list_area.height as usize {
                    let highlight_area =
                        Rect::new(list_area.x, list_area.y + selected as u16, list_area.width, 1);

                    for x in highlight_area.left()..highlight_area.right() {
                        let cell = buf.get_mut(x, highlight_area.y);
                        let style = Style::default()
                            .fg(Color::White)
                            .bg(Color::Blue)
                            .add_modifier(Modifier::BOLD);
                        cell.set_style(style);
                    }
                }
            }
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent, mode: &Mode) -> Option<Action> {
        match mode {
            Mode::Normal => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.next();
                        None
                    },
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.previous();
                        None
                    },
                    KeyCode::Char('q') => Some(Action::Quit),
                    KeyCode::Char('i') => Some(Action::SwitchMode(Mode::Insert)),
                    KeyCode::Tab => Some(Action::FocusNext),
                    KeyCode::BackTab => Some(Action::FocusPrevious),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::CloseWindow)
                    },
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::NewWindow)
                    },
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::SplitHorizontal)
                    },
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::SplitVertical)
                    },
                    _ => None,
                }
            },
            Mode::Insert => {
                match key.code {
                    KeyCode::Esc => Some(Action::SwitchMode(Mode::Normal)),
                    _ => None,
                }
            },
        }
    }

    fn title(&self) -> &str {
        &self.title
    }
}

// A basic text window
struct TextView {
    title: String,
    content: String,
}

impl TextView {
    fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            content: content.to_string(),
        }
    }
}

impl WindowTrait for TextView {
    fn render(&mut self, rect: Rect, buf: &mut Buffer, focused: bool) {
        let block =
            Block::default()
                .title(self.title.clone())
                .borders(Borders::ALL)
                .style(if focused {
                    Style::default().fg(Color::White).bg(Color::DarkGray)
                } else {
                    Style::default()
                });

        let text = Paragraph::new(self.content.clone()).block(block);

        text.render(rect, buf);
    }

    fn handle_key_event(&mut self, key: KeyEvent, mode: &Mode) -> Option<Action> {
        match mode {
            Mode::Normal => {
                match key.code {
                    KeyCode::Char('q') => Some(Action::Quit),
                    KeyCode::Char('i') => Some(Action::SwitchMode(Mode::Insert)),
                    KeyCode::Tab => Some(Action::FocusNext),
                    KeyCode::BackTab => Some(Action::FocusPrevious),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::CloseWindow)
                    },
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::NewWindow)
                    },
                    KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::SplitHorizontal)
                    },
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        Some(Action::SplitVertical)
                    },
                    _ => None,
                }
            },
            Mode::Insert => {
                match key.code {
                    KeyCode::Esc => Some(Action::SwitchMode(Mode::Normal)),
                    _ => None,
                }
            },
        }
    }

    fn title(&self) -> &str {
        &self.title
    }
}

// Node structure for window tree
enum Node {
    Leaf(Box<dyn WindowTrait>),
    Split {
        direction: Direction,
        ratio: u16,
        left: Box<Node>,
        right: Box<Node>,
    },
}

impl Node {
    fn render(&mut self, rect: Rect, buf: &mut Buffer, focused_path: &[bool]) {
        match self {
            Node::Leaf(window) => {
                window.render(rect, buf, focused_path.is_empty());
            },
            Node::Split { direction, ratio, left, right } => {
                let constraints = match direction {
                    Direction::Horizontal => {
                        [
                            Constraint::Percentage(*ratio),
                            Constraint::Percentage(100 - *ratio),
                        ]
                    },
                    Direction::Vertical => {
                        [
                            Constraint::Percentage(*ratio),
                            Constraint::Percentage(100 - *ratio),
                        ]
                    },
                };

                let chunks = Layout::default()
                    .direction(*direction)
                    .constraints(constraints)
                    .split(rect);

                if !focused_path.is_empty() {
                    if focused_path[0] {
                        left.render(chunks[0], buf, &focused_path[1..]);
                        right.render(chunks[1], buf, &[]);
                    } else {
                        left.render(chunks[0], buf, &[]);
                        right.render(chunks[1], buf, &focused_path[1..]);
                    }
                } else {
                    // If no focus path, don't focus anything
                    left.render(chunks[0], buf, &[]);
                    right.render(chunks[1], buf, &[]);
                }
            },
        }
    }

    fn handle_key_event(
        &mut self,
        key: KeyEvent,
        mode: &Mode,
        focused_path: &[bool],
    ) -> Option<Action> {
        match self {
            Node::Leaf(window) => {
                if focused_path.is_empty() {
                    window.handle_key_event(key, mode)
                } else {
                    None
                }
            },
            Node::Split { left, right, .. } => {
                if !focused_path.is_empty() {
                    if focused_path[0] {
                        left.handle_key_event(key, mode, &focused_path[1..])
                    } else {
                        right.handle_key_event(key, mode, &focused_path[1..])
                    }
                } else {
                    None
                }
            },
        }
    }

    fn focus_next(&self, current_path: &[bool]) -> Vec<bool> {
        match self {
            Node::Leaf(_) => {
                // Can't go further in a leaf
                vec![]
            },
            Node::Split { left, right, .. } => {
                if current_path.is_empty() {
                    // Start with the left node
                    vec![true]
                } else if current_path[0] {
                    // Currently in the left subtree
                    let next_path = left.focus_next(&current_path[1..]);
                    if !next_path.is_empty() {
                        // Found a next focus in the left subtree
                        let mut path = vec![true];
                        path.extend(next_path);
                        path
                    } else {
                        // Move to the right subtree
                        vec![false]
                    }
                } else {
                    // Currently in the right subtree
                    let next_path = right.focus_next(&current_path[1..]);
                    if !next_path.is_empty() {
                        // Found a next focus in the right subtree
                        let mut path = vec![false];
                        path.extend(next_path);
                        path
                    } else {
                        // Wrap around (return empty to indicate that)
                        vec![]
                    }
                }
            },
        }
    }

    fn focus_previous(&self, current_path: &[bool]) -> Vec<bool> {
        match self {
            Node::Leaf(_) => {
                // Can't go further in a leaf
                vec![]
            },
            Node::Split { left, right, .. } => {
                if current_path.is_empty() {
                    // Start with the right node
                    vec![false]
                } else if !current_path[0] {
                    // Currently in the right subtree
                    let prev_path = right.focus_previous(&current_path[1..]);
                    if !prev_path.is_empty() {
                        // Found a previous focus in the right subtree
                        let mut path = vec![false];
                        path.extend(prev_path);
                        path
                    } else {
                        // Move to the left subtree
                        vec![true]
                    }
                } else {
                    // Currently in the left subtree
                    let prev_path = left.focus_previous(&current_path[1..]);
                    if !prev_path.is_empty() {
                        // Found a previous focus in the left subtree
                        let mut path = vec![true];
                        path.extend(prev_path);
                        path
                    } else {
                        // Wrap around (return empty to indicate that)
                        vec![]
                    }
                }
            },
        }
    }

    fn get_node_at_path<'a>(&'a mut self, path: &[bool]) -> Option<&'a mut Box<dyn WindowTrait>> {
        match self {
            Node::Leaf(window) => {
                if path.is_empty() {
                    Some(window)
                } else {
                    None
                }
            },
            Node::Split { left, right, .. } => {
                if path.is_empty() {
                    None
                } else if path[0] {
                    left.get_node_at_path(&path[1..])
                } else {
                    right.get_node_at_path(&path[1..])
                }
            },
        }
    }

    fn split(&mut self, path: &[bool], direction: Direction, window: Box<dyn WindowTrait>) -> bool {
        match self {
            Node::Leaf(_) => {
                if path.is_empty() {
                    // Replace the leaf with a split node
                    let leaf = std::mem::replace(self, Node::Split {
                        direction,
                        ratio: 50,
                        left: Box::new(Node::Leaf(window)),
                        right: Box::new(Node::Leaf(Box::new(TextView::new(
                            "New Window",
                            "This is a new window created by splitting",
                        )))),
                    });

                    // Put the original leaf in the left side
                    if let Node::Split { left, .. } = self {
                        *left = Box::new(leaf);
                    }

                    true
                } else {
                    false
                }
            },
            Node::Split { left, right, .. } => {
                if path.is_empty() {
                    false
                } else if path[0] {
                    left.split(&path[1..], direction, window)
                } else {
                    right.split(&path[1..], direction, window)
                }
            },
        }
    }
}

// Window manager to handle the window tree
struct WindowManager {
    root: Node,
    focus_path: Vec<bool>,
    mode: Mode,
}

impl WindowManager {
    fn new() -> Self {
        let list_view = ListView::new("List Window", vec![
            "Item 1".to_string(),
            "Item 2".to_string(),
            "Item 3".to_string(),
            "Item 4".to_string(),
            "Item 5".to_string(),
        ]);

        let text_view = TextView::new(
            "Text Window",
            "This is a text window.\nUse Tab to switch focus.\nCtrl+N to create a new window.\nCtrl+S to split horizontally.\nCtrl+V to split vertically.\nCtrl+C to close window.\nq to quit."
        );

        Self {
            root: Node::Split {
                direction: Direction::Horizontal,
                ratio: 30,
                left: Box::new(Node::Leaf(Box::new(list_view))),
                right: Box::new(Node::Leaf(Box::new(text_view))),
            },
            focus_path: vec![true], // Focus the left window initially
            mode: Mode::Normal,
        }
    }

    fn render(&mut self, rect: Rect, buf: &mut Buffer) {
        // Reserve space for the status bar
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(rect);

        // Render the window tree
        self.root.render(chunks[0], buf, &self.focus_path);

        // Render the status bar
        let mode_text = match self.mode {
            Mode::Normal => {
                Span::styled(" NORMAL ", Style::default().bg(Color::Blue).fg(Color::White))
            },
            Mode::Insert => {
                Span::styled(" INSERT ", Style::default().bg(Color::Green).fg(Color::White))
            },
        };

        let focused_window = self.get_focused_window_title().unwrap_or("None");
        let status = Line::from(vec![
            mode_text,
            Span::raw(" | "),
            Span::raw(format!("Focused: {}", focused_window)),
            Span::raw(" | "),
            Span::raw(
                "Tab: next, Shift+Tab: prev, Ctrl+S: split-h, Ctrl+V: split-v, Ctrl+C: close",
            ),
        ]);

        let status_bar =
            Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White));

        status_bar.render(chunks[1], buf);
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> bool {
        // First, let the focused window handle the event
        let action = self.root.handle_key_event(key, &self.mode, &self.focus_path);

        // Then handle any returned actions
        match action {
            Some(Action::Quit) => return false,
            Some(Action::FocusNext) => self.focus_next(),
            Some(Action::FocusPrevious) => self.focus_previous(),
            Some(Action::SwitchMode(mode)) => self.mode = mode,
            Some(Action::CloseWindow) => {
                // In a real implementation, this would actually close the window
                // and reorganize the tree. For this example, we'll just log it.
                println!("Close window not implemented in this example");
            },
            Some(Action::NewWindow) => {
                // Create a new window
                let new_window = Box::new(TextView::new(
                    "New Window",
                    "This is a new window created with Ctrl+N",
                ));

                // Split the currently focused window
                self.split_focused(Direction::Vertical, new_window);
            },
            Some(Action::SplitHorizontal) => {
                let new_window = Box::new(TextView::new(
                    "Horizontal Split",
                    "This is a new window created with Ctrl+S (horizontal split)",
                ));

                self.split_focused(Direction::Horizontal, new_window);
            },
            Some(Action::SplitVertical) => {
                let new_window = Box::new(TextView::new(
                    "Vertical Split",
                    "This is a new window created with Ctrl+V (vertical split)",
                ));

                self.split_focused(Direction::Vertical, new_window);
            },
            None => {},
        }

        true
    }

    fn focus_next(&mut self) {
        let next_path = self.root.focus_next(&self.focus_path);
        if !next_path.is_empty() {
            self.focus_path = next_path;
        } else {
            // Wrap around to the first window
            self.focus_path = vec![true]; // Assuming we always have at least one window
        }
    }

    fn focus_previous(&mut self) {
        let prev_path = self.root.focus_previous(&self.focus_path);
        if !prev_path.is_empty() {
            self.focus_path = prev_path;
        } else {
            // Wrap around to the last window
            self.focus_path = vec![false]; // Assuming we always have at least one window
        }
    }

    fn get_focused_window_title(&mut self) -> Option<&str> {
        self.root.get_node_at_path(&self.focus_path).map(|window| window.title())
    }

    fn split_focused(&mut self, direction: Direction, window: Box<dyn WindowTrait>) {
        self.root.split(&self.focus_path, direction, window);
    }
}

fn main() -> Result<(), io::Error> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Window manager
    let mut window_manager = WindowManager::new();

    loop {
        terminal.draw(|f| {
            let mut buf = Buffer::empty(f.size());
            window_manager.render(f.size(), &mut buf);
            f.render_buffer(buf);
        })?;

        // Input handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if !window_manager.handle_key_event(key) {
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
