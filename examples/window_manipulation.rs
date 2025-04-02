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

use cyrum::widgets::{
    layout::{LayoutConstraint, LayoutManager, LayoutNode, LayoutType},
    window::{WindowComponent, WindowState},
    dialog::{Dialog, DialogButton, DialogState, DialogType},
    dialogmanager::{DialogManager, DialogResult},
};
use cyrum::modal::{InputEvent, Key, ModalState};

/// Simple text view component
struct TextView {
    title: String,
    content: String,
    focused: bool,
}

impl TextView {
    fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            focused: false,
        }
    }
}

impl WindowComponent for TextView {
    fn handle_event(&mut self, _event: &InputEvent, _modal_state: &mut ModalState) -> bool {
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

/// Simple list view component
struct ListView {
    title: String,
    items: Vec<String>,
    state: ListState,
    focused: bool,
}

impl ListView {
    fn new(title: impl Into<String>, items: Vec<String>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            title: title.into(),
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
    fn handle_event(&mut self, event: &InputEvent, modal_state: &mut ModalState) -> bool {
        if !self.focused {
            return false;
        }

        match event {
            InputEvent::Key(key) => {
                if modal_state.is_normal_mode() {
                    match key.code {
                        KeyCode::Down | KeyCode::Char('j') => {
                            self.next();
                            true
                        },
                        KeyCode::Up | KeyCode::Char('k') => {
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

/// Window size state for display
#[derive(Debug, Clone, PartialEq, Eq)]
enum WindowSize {
    Normal,
    Maximized,
    Minimized,
}

/// Application state
struct App {
    /// Layout manager
    layout_manager: LayoutManager,
    /// Window components
    windows: std::collections::HashMap<String, Box<dyn WindowComponent>>,
    /// Currently active window
    active_window: Option<String>,
    /// Map of window sizes for maximized/minimized windows
    window_sizes: std::collections::HashMap<String, WindowSize>,
    /// Dialog manager for window closing confirmations
    dialog_manager: DialogManager,
    /// Window waiting for close confirmation
    window_to_close: Option<String>,
    /// Modal state for keyboard input
    modal_state: ModalState,
    /// Should quit the application
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        // Create window components
        let mut windows = std::collections::HashMap::new();
        
        // Create list view
        let list_view = ListView::new("File Explorer", vec![
            "Documents".to_string(),
            "Pictures".to_string(),
            "Music".to_string(),
            "Videos".to_string(),
            "Downloads".to_string(),
        ]);
        windows.insert("file-explorer".to_string(), Box::new(list_view));
        
        // Create text editor view
        let instructions = concat!(
            "Window Manipulation Demo\n\n",
            "Controls:\n",
            "- TAB: Switch focus between windows\n",
            "- F2: Resize active window\n",
            "- F3: Move active window within parent\n",
            "- F4: Close active window (with confirmation)\n",
            "- F5: Maximize active window\n",
            "- F6: Restore active window\n",
            "- F7: Minimize active window\n",
            "- F8: Create new window\n",
            "- Ctrl+Q: Quit\n",
        );
        let text_view = TextView::new("Main Editor", instructions);
        windows.insert("main-editor".to_string(), Box::new(text_view));
        
        // Create a terminal view
        let terminal_view = TextView::new("Terminal", "$ ls -la\ntotal 48\ndrwxr-xr-x  12 user group   384 Jan 25 10:17 .\ndrwxr-xr-x   5 user group   160 Jan 24 09:30 ..\n-rw-r--r--   1 user group  1256 Jan 21 15:45 .gitignore\n-rw-r--r--   1 user group   102 Jan 20 11:32 README.md\n-rw-r--r--   1 user group  3106 Jan 25 10:15 Cargo.toml\ndrwxr-xr-x   3 user group    96 Jan 22 13:40 src");
        windows.insert("terminal".to_string(), Box::new(terminal_view));
        
        // Create initial layout
        let layout = LayoutNode::parent(
            LayoutType::Vertical,
            vec![
                LayoutConstraint::Percentage(70),
                LayoutConstraint::Percentage(30),
            ],
            vec![
                LayoutNode::parent(
                    LayoutType::Horizontal,
                    vec![
                        LayoutConstraint::Percentage(25),
                        LayoutConstraint::Percentage(75),
                    ],
                    vec![
                        LayoutNode::leaf("file-explorer"),
                        LayoutNode::leaf("main-editor"),
                    ],
                ),
                LayoutNode::leaf("terminal"),
            ],
        );
        
        // Create layout manager
        let mut layout_manager = LayoutManager::new();
        layout_manager.set_root(layout);
        
        Self {
            layout_manager,
            windows,
            active_window: Some("main-editor".to_string()),
            window_sizes: std::collections::HashMap::new(),
            dialog_manager: DialogManager::default(),
            window_to_close: None,
            modal_state: ModalState::default(),
            should_quit: false,
        }
    }
    
    /// Focus the next window
    fn focus_next_window(&mut self) {
        let layout = self.layout_manager.compute_layout(Rect::default());
        
        if layout.is_empty() {
            return;
        }
        
        // If no window is focused, focus the first one
        if self.active_window.is_none() {
            self.active_window = Some(layout[0].0.clone());
            if let Some(window) = self.windows.get_mut(&layout[0].0) {
                window.set_focus(true);
            }
            return;
        }
        
        // Find the current focused window's index
        let mut current_idx = 0;
        for (i, (id, _)) in layout.iter().enumerate() {
            if Some(id) == self.active_window.as_ref() {
                current_idx = i;
                break;
            }
        }
        
        // Unfocus the current window
        if let Some(focused) = &self.active_window {
            if let Some(window) = self.windows.get_mut(focused) {
                window.set_focus(false);
            }
        }
        
        // Focus the next window
        let next_idx = (current_idx + 1) % layout.len();
        self.active_window = Some(layout[next_idx].0.clone());
        
        if let Some(window) = self.windows.get_mut(&layout[next_idx].0) {
            window.set_focus(true);
        }
    }
    
    /// Toggle window maximize state
    fn toggle_maximize_window(&mut self) {
        if let Some(window_id) = &self.active_window {
            let is_maximized = self.window_sizes.get(window_id) == Some(&WindowSize::Maximized);
            
            if is_maximized {
                // Restore window
                self.window_sizes.remove(window_id);
            } else {
                // Maximize window
                self.window_sizes.insert(window_id.clone(), WindowSize::Maximized);
            }
        }
    }
    
    /// Toggle window minimize state
    fn toggle_minimize_window(&mut self) {
        if let Some(window_id) = &self.active_window {
            let is_minimized = self.window_sizes.get(window_id) == Some(&WindowSize::Minimized);
            
            if is_minimized {
                // Restore window
                self.window_sizes.remove(window_id);
            } else {
                // Minimize window
                self.window_sizes.insert(window_id.clone(), WindowSize::Minimized);
            }
        }
    }
    
    /// Create a new window
    fn create_new_window(&mut self) {
        let window_id = format!("window-{}", self.windows.len());
        let window = TextView::new(
            format!("Window {}", self.windows.len()),
            "This is a new window created at runtime.",
        );
        self.windows.insert(window_id.clone(), Box::new(window));
        
        // Split the active window to add the new one
        if let Some(active_id) = &self.active_window {
            self.layout_manager.split_window(active_id, LayoutType::Horizontal, &window_id);
            
            // Focus the new window
            if let Some(old_window) = self.windows.get_mut(active_id) {
                old_window.set_focus(false);
            }
            if let Some(new_window) = self.windows.get_mut(&window_id) {
                new_window.set_focus(true);
            }
            self.active_window = Some(window_id);
        }
    }
    
    /// Start window close with confirmation
    fn start_close_window(&mut self) {
        if let Some(window_id) = &self.active_window {
            let window_title = self.windows.get(window_id).map(|w| w.title()).unwrap_or("Unknown");
            
            // Create confirmation dialog
            let dialog = Dialog::default()
                .title("Close Window")
                .width_percent(40)
                .height_percent(20);
                
            let message = format!("Are you sure you want to close \"{}\"?", window_title);
            let state = DialogState::confirmation(
                "Close Window",
                &message,
                &[
                    DialogButton::new("Yes", true, true),
                    DialogButton::new("No", false, false),
                ],
            );
            
            // Create a callback for the dialog result
            let window_to_close = window_id.clone();
            let callback = move |result: DialogResult| {
                if let Some(button_index) = result.button_index {
                    if button_index == 0 {
                        // Yes was selected
                        return Some(window_to_close.clone());
                    }
                }
                None
            };
            
            // Add the dialog
            self.dialog_manager.add_dialog(dialog, state, true, None, Some(Box::new(callback)));
            self.window_to_close = Some(window_id.clone());
        }
    }
    
    /// Close a window
    fn close_window(&mut self, window_id: &str) {
        // Remove the window from the layout
        self.layout_manager.close_window(window_id);
        
        // Remove the window component
        self.windows.remove(window_id);
        
        // Update active window
        if self.active_window.as_deref() == Some(window_id) {
            // Choose a new active window
            let layout = self.layout_manager.compute_layout(Rect::default());
            if !layout.is_empty() {
                self.active_window = Some(layout[0].0.clone());
                if let Some(window) = self.windows.get_mut(&layout[0].0) {
                    window.set_focus(true);
                }
            } else {
                self.active_window = None;
            }
        }
    }
    
    /// Handle keyboard input
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<(), Box<dyn Error>> {
        // Check for global shortcuts first
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return Ok(());
            },
            KeyCode::Tab => {
                self.focus_next_window();
                return Ok(());
            },
            KeyCode::F(2) => {
                // Resize window
                if let Some(active_id) = &self.active_window {
                    // This is a simple example - in a real app, we'd have more sophisticated resizing
                    let delta = 5; // Increase size by 5%
                    
                    // Find the layout entry containing this window
                    let layout = self.layout_manager.compute_layout(Rect::default());
                    for (id, _) in &layout {
                        if id == active_id {
                            // Found it - adjust the constraint
                            // In a real implementation, we'd modify the layout manager's constraints
                            // For this example, we'll just show a message
                            
                            let dialog = Dialog::default()
                                .title("Resize Window")
                                .width_percent(40)
                                .height_percent(20);
                                
                            let message = format!("Window {} resized by {}%", active_id, delta);
                            let state = DialogState::message(
                                "Resize Window",
                                &message,
                            );
                            
                            self.dialog_manager.add_dialog(dialog, state, true, None, None);
                            break;
                        }
                    }
                }
                return Ok(());
            },
            KeyCode::F(3) => {
                // Move window
                if let Some(active_id) = &self.active_window {
                    let dialog = Dialog::default()
                        .title("Move Window")
                        .width_percent(40)
                        .height_percent(20);
                        
                    let message = format!("Window {} moved to new position", active_id);
                    let state = DialogState::message(
                        "Move Window",
                        &message,
                    );
                    
                    self.dialog_manager.add_dialog(dialog, state, true, None, None);
                }
                return Ok(());
            },
            KeyCode::F(4) => {
                self.start_close_window();
                return Ok(());
            },
            KeyCode::F(5) => {
                // Maximize window
                if let Some(active_id) = &self.active_window {
                    self.window_sizes.insert(active_id.clone(), WindowSize::Maximized);
                }
                return Ok(());
            },
            KeyCode::F(6) => {
                // Restore window
                if let Some(active_id) = &self.active_window {
                    self.window_sizes.remove(active_id);
                }
                return Ok(());
            },
            KeyCode::F(7) => {
                // Minimize window
                if let Some(active_id) = &self.active_window {
                    self.window_sizes.insert(active_id.clone(), WindowSize::Minimized);
                }
                return Ok(());
            },
            KeyCode::F(8) => {
                self.create_new_window();
                return Ok(());
            },
            _ => {},
        }
        
        // Convert to cyrum event for window components
        let cyrum_key = Key {
            code: key.code,
            modifiers: key.modifiers,
        };
        let cyrum_event = InputEvent::Key(cyrum_key);
        
        // First check if the dialog manager handles the event
        if self.dialog_manager.handle_event(&cyrum_event, &mut self.modal_state) {
            return Ok(());
        }
        
        // Then check if the active window handles the event
        if let Some(active_id) = &self.active_window {
            if let Some(window) = self.windows.get_mut(active_id) {
                if window.handle_event(&cyrum_event, &mut self.modal_state) {
                    return Ok(());
                }
            }
        }
        
        Ok(())
    }
    
    /// Render the application
    fn render(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
        // Process any dialog results
        self.dialog_manager.process_results();
        
        // Check for window close confirmations
        if let Some(result) = self.dialog_manager.take_result_callback_output::<String>() {
            if let Some(window_id) = result {
                self.close_window(&window_id);
                self.window_to_close = None;
            }
        }
        
        terminal.draw(|f| {
            // Compute layout
            let layout = self.layout_manager.compute_layout(f.size());
            
            // Render windows based on their size state
            for (id, area) in layout {
                // Skip minimized windows
                if self.window_sizes.get(&id) == Some(&WindowSize::Minimized) {
                    continue;
                }
                
                // For maximized windows, use the entire screen area
                let render_area = if self.window_sizes.get(&id) == Some(&WindowSize::Maximized) {
                    // Reserve a 1-line status bar at the bottom
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Min(1),
                            Constraint::Length(1),
                        ])
                        .split(f.size());
                    chunks[0]
                } else {
                    area
                };
                
                // Render the window component
                if let Some(window) = self.windows.get_mut(&id) {
                    window.render(render_area, &mut f.buffer_mut());
                }
            }
            
            // Render dialogs on top
            self.dialog_manager.render(f.size(), &mut f.buffer_mut());
            
            // Render status bar
            let status_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(f.size())[1];
                
            let mode_text = Span::styled(" NORMAL ", Style::default().bg(Color::Blue).fg(Color::White));
            
            let active_text = if let Some(active_id) = &self.active_window {
                let window_title = self.windows.get(active_id)
                    .map(|w| w.title())
                    .unwrap_or("None");
                format!(" {} ", window_title)
            } else {
                " None ".to_string()
            };
            
            let status = Line::from(vec![
                mode_text,
                Span::raw(" | "),
                Span::raw("Active:"),
                Span::styled(active_text, Style::default().fg(Color::Cyan)),
                Span::raw(" | "),
                Span::raw("F2: Resize | F3: Move | F4: Close | F5: Max | F6: Restore | F7: Min | F8: New | Ctrl+Q: Quit"),
            ]);
            
            let status_bar = Paragraph::new(status)
                .style(Style::default().bg(Color::DarkGray).fg(Color::White));
                
            f.render_widget(status_bar, status_area);
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
    
    // Create app
    let mut app = App::new();
    
    // Event loop
    loop {
        // Render
        app.render(&mut terminal)?;
        
        // Check if should quit
        if app.should_quit {
            break;
        }
        
        // Handle input
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key_event(key)?;
            }
        }
    }
    
    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}