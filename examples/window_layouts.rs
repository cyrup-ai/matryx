use std::error::Error;
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
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Widget};
use ratatui::Terminal;

use maxtryx::widgets::{
    LayoutConstraint, LayoutManager, LayoutNode, LayoutType, 
    Tabs, TabsState, Window, WindowComponent, WindowState
};

/// Mode system
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Normal,
    Insert,
}

/// Action enum for window operations
enum Action {
    Quit,
    SwitchMode(Mode),
    NewWindow,
    SplitHorizontal,
    SplitVertical,
    CreateTab,
    NextTab,
    PrevTab,
    CloseWindow,
    MaximizeWindow,
}

/// A simple list view component
struct ListView {
    title: String,
    items: Vec<String>,
    state: ListState,
    focused: bool,
}

impl ListView {
    fn new(title: &str, items: Vec<String>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            title: title.to_string(),
            items,
            state,
            focused: false,
        }
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

impl WindowComponent for ListView {
    fn handle_event(&mut self, event: &maxtryx::modal::InputEvent, modal_state: &mut maxtryx::modal::ModalState) -> bool {
        if !self.focused {
            return false;
        }

        match event {
            maxtryx::modal::InputEvent::Key(key) => {
                if modal_state.is_normal_mode() {
                    match key.code {
                        crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                            self.next();
                            true
                        },
                        crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                            self.previous();
                            true
                        },
                        _ => false,
                    }
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.title.clone())
            .borders(Borders::ALL)
            .border_style(if self.focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let items: Vec<ListItem> = self.items.iter().map(|i| ListItem::new(i.as_str())).collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

        StatefulWidget::render(list, area, buf, &mut self.state);
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn can_focus(&self) -> bool {
        true
    }
}

/// A simple text view component
struct TextView {
    title: String,
    content: String,
    focused: bool,
}

impl TextView {
    fn new(title: &str, content: &str) -> Self {
        Self {
            title: title.to_string(),
            content: content.to_string(),
            focused: false,
        }
    }
}

impl WindowComponent for TextView {
    fn handle_event(&mut self, _event: &maxtryx::modal::InputEvent, _modal_state: &mut maxtryx::modal::ModalState) -> bool {
        false
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.title.clone())
            .borders(Borders::ALL)
            .border_style(if self.focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            });

        let paragraph = Paragraph::new(self.content.clone()).block(block);

        paragraph.render(area, buf);
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn is_focused(&self) -> bool {
        self.focused
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn can_focus(&self) -> bool {
        true
    }
}

/// Window layout application
struct App {
    /// Application mode
    mode: Mode,
    /// Layout manager
    layout_manager: LayoutManager,
    /// Map of window components
    windows: std::collections::HashMap<String, Box<dyn WindowComponent>>,
    /// Currently focused window
    focused_window: Option<String>,
    /// Next window counter
    next_window_id: usize,
    /// Tab state for tabbed layout
    tab_state: TabsState,
}

impl App {
    fn new() -> Self {
        // Create initial layout with a list view and a text view
        let mut windows = std::collections::HashMap::new();
        let mut tab_titles = Vec::new();

        // Create list view
        let list_view = ListView::new("List View", vec![
            "Item 1".to_string(),
            "Item 2".to_string(),
            "Item 3".to_string(),
            "Item 4".to_string(),
            "Item 5".to_string(),
        ]);
        windows.insert("list-view".to_string(), Box::new(list_view));

        // Create text view
        let text_content = "Window Layouts Example\n\nControls:\n- Tab: Switch focus\n- Ctrl+H: Split horizontally\n- Ctrl+V: Split vertically\n- Ctrl+T: Create new tab\n- Ctrl+N: Next tab\n- Ctrl+P: Previous tab\n- Ctrl+W: Close window\n- Ctrl+M: Maximize window\n- Ctrl+Q: Quit";
        let text_view = TextView::new("Text View", text_content);
        windows.insert("text-view".to_string(), Box::new(text_view));

        // Create initial layout
        let layout = LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Percentage(30),
                LayoutConstraint::Percentage(70),
            ],
            vec![
                LayoutNode::leaf("list-view"),
                LayoutNode::leaf("text-view"),
            ],
        );

        // Create layout manager
        let mut layout_manager = LayoutManager::new();
        layout_manager.set_root(layout);

        // Create tab state
        tab_titles.push("Main".to_string());

        Self {
            mode: Mode::Normal,
            layout_manager,
            windows,
            focused_window: Some("list-view".to_string()),
            next_window_id: 1,
            tab_state: TabsState::new(tab_titles),
        }
    }

    /// Create a new window
    fn new_window(&mut self, title: &str, content: &str) -> String {
        let window_id = format!("window-{}", self.next_window_id);
        self.next_window_id += 1;

        let text_view = TextView::new(title, content);
        self.windows.insert(window_id.clone(), Box::new(text_view));

        window_id
    }

    /// Split the currently focused window
    fn split_window(&mut self, layout_type: LayoutType) {
        if let Some(focused) = &self.focused_window {
            let new_window_id = self.new_window("New Window", "This is a new window created by splitting");
            
            // Split the window in the layout
            self.layout_manager.split_window(focused, layout_type, &new_window_id);
            
            // Focus the new window
            self.focused_window = Some(new_window_id);
        }
    }

    /// Create a new tab
    fn create_tab(&mut self) {
        let tab_title = format!("Tab {}", self.tab_state.len() + 1);
        self.tab_state.add_tab(tab_title);
        self.tab_state.select(self.tab_state.len() - 1);
    }

    /// Handle key events
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<(), Box<dyn Error>> {
        // Create a maxtryx key event from crossterm event
        let maxtryx_key = maxtryx::modal::Key {
            code: key.code,
            modifiers: key.modifiers,
        };
        let maxtryx_event = maxtryx::modal::InputEvent::Key(maxtryx_key);

        // Initialize modal state (we're not fully using it in this example)
        let mut modal_state = maxtryx::modal::ModalState::default();
        if self.mode == Mode::Insert {
            modal_state.enter_insert_mode();
        } else {
            modal_state.enter_normal_mode();
        }

        // First check if the focused window handles the event
        if let Some(focused) = &self.focused_window {
            if let Some(window) = self.windows.get_mut(focused) {
                if window.handle_event(&maxtryx_event, &mut modal_state) {
                    return Ok(());
                }
            }
        }

        // Then handle global key events
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Err("quit".into());
            },
            KeyCode::Char('i') if self.mode == Mode::Normal => {
                self.mode = Mode::Insert;
            },
            KeyCode::Esc if self.mode == Mode::Insert => {
                self.mode = Mode::Normal;
            },
            KeyCode::Tab => {
                self.focus_next_window();
            },
            KeyCode::BackTab => {
                self.focus_prev_window();
            },
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.split_window(LayoutType::Horizontal);
            },
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.split_window(LayoutType::Vertical);
            },
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.create_tab();
            },
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tab_state.next();
            },
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tab_state.prev();
            },
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_focused_window();
            },
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Maximize window implementation would go here
            },
            _ => {},
        }

        Ok(())
    }

    /// Focus the next window
    fn focus_next_window(&mut self) {
        let layout = self.layout_manager.compute_layout(Rect::default());
        
        if layout.is_empty() {
            return;
        }

        // If no window is focused, focus the first one
        if self.focused_window.is_none() {
            self.focused_window = Some(layout[0].0.clone());
            if let Some(window) = self.windows.get_mut(&layout[0].0) {
                window.set_focus(true);
            }
            return;
        }

        // Find the current focused window's index
        let mut current_idx = 0;
        for (i, (id, _)) in layout.iter().enumerate() {
            if Some(id) == self.focused_window.as_ref() {
                current_idx = i;
                break;
            }
        }

        // Unfocus the current window
        if let Some(focused) = &self.focused_window {
            if let Some(window) = self.windows.get_mut(focused) {
                window.set_focus(false);
            }
        }

        // Focus the next window
        let next_idx = (current_idx + 1) % layout.len();
        self.focused_window = Some(layout[next_idx].0.clone());
        
        if let Some(window) = self.windows.get_mut(&layout[next_idx].0) {
            window.set_focus(true);
        }
    }

    /// Focus the previous window
    fn focus_prev_window(&mut self) {
        let layout = self.layout_manager.compute_layout(Rect::default());
        
        if layout.is_empty() {
            return;
        }

        // If no window is focused, focus the last one
        if self.focused_window.is_none() {
            self.focused_window = Some(layout.last().unwrap().0.clone());
            if let Some(window) = self.windows.get_mut(&layout.last().unwrap().0) {
                window.set_focus(true);
            }
            return;
        }

        // Find the current focused window's index
        let mut current_idx = 0;
        for (i, (id, _)) in layout.iter().enumerate() {
            if Some(id) == self.focused_window.as_ref() {
                current_idx = i;
                break;
            }
        }

        // Unfocus the current window
        if let Some(focused) = &self.focused_window {
            if let Some(window) = self.windows.get_mut(focused) {
                window.set_focus(false);
            }
        }

        // Focus the previous window
        let prev_idx = if current_idx == 0 {
            layout.len() - 1
        } else {
            current_idx - 1
        };
        
        self.focused_window = Some(layout[prev_idx].0.clone());
        
        if let Some(window) = self.windows.get_mut(&layout[prev_idx].0) {
            window.set_focus(true);
        }
    }

    /// Close the focused window
    fn close_focused_window(&mut self) {
        if let Some(focused) = &self.focused_window {
            // Remove the window from the layout
            self.layout_manager.close_window(focused);
            
            // Remove the window from the component map
            self.windows.remove(focused);
            
            // Focus another window if available
            let layout = self.layout_manager.compute_layout(Rect::default());
            if !layout.is_empty() {
                self.focused_window = Some(layout[0].0.clone());
                if let Some(window) = self.windows.get_mut(&layout[0].0) {
                    window.set_focus(true);
                }
            } else {
                self.focused_window = None;
            }
        }
    }

    /// Render the application
    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        terminal.draw(|f| {
            // Split the screen into tabs area and content area
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1), // Tab bar
                    Constraint::Min(0),    // Content area
                    Constraint::Length(1), // Status bar
                ])
                .split(f.size());

            // Render tabs
            let tabs = Tabs::default()
                .style(Style::default().fg(Color::Gray))
                .selected_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .highlight_selected(true);
            
            f.render_stateful_widget(tabs, chunks[0], &mut self.tab_state);

            // Compute window layout
            let computed_layout = self.layout_manager.compute_layout(chunks[1]);
            
            // Render each window
            for (id, area) in computed_layout {
                if let Some(window) = self.windows.get_mut(&id) {
                    window.render(area, &mut f.buffer_mut());
                }
            }

            // Render status bar
            let mode_text = match self.mode {
                Mode::Normal => {
                    Span::styled(" NORMAL ", Style::default().bg(Color::Blue).fg(Color::White))
                },
                Mode::Insert => {
                    Span::styled(" INSERT ", Style::default().bg(Color::Green).fg(Color::White))
                },
            };

            let focused_text = if let Some(focused) = &self.focused_window {
                if let Some(window) = self.windows.get(focused) {
                    format!(" {}", window.title())
                } else {
                    " None".to_string()
                }
            } else {
                " None".to_string()
            };

            let status = Line::from(vec![
                mode_text,
                Span::raw(" | "),
                Span::raw("Focused:"),
                Span::styled(focused_text, Style::default().fg(Color::Cyan)),
                Span::raw(" | "),
                Span::raw("Ctrl+H: Split-H | Ctrl+V: Split-V | Ctrl+T: New Tab | Ctrl+W: Close | Ctrl+Q: Quit"),
            ]);

            let status_bar =
                Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White));

            f.render_widget(status_bar, chunks[2]);
        })?;

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Application
    let mut app = App::new();

    // Event loop
    loop {
        // Render
        app.render(&mut terminal)?;

        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match app.handle_key_event(key) {
                    Ok(_) => {},
                    Err(e) if e.to_string() == "quit" => break,
                    Err(e) => {
                        // In a real app, we'd handle errors better
                        eprintln!("Error: {}", e);
                        break;
                    },
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}