//! Window management for the Matrix application
//!
//! This module provides the main window management functionality for Matrix,
//! including implementing a tabbed interface and managing window state.

use std::collections::HashMap;

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Tabs, Widget},
};

use crate::{
    base::{maxtryxId, maxtryxResult, ProgramStore},
    modal::{ActionResult, InputEvent, ModalState},
    windows::MatrixWindow,
};

/// The main window manager that handles tab creation and switching
pub struct WindowManager {
    /// The currently open windows in tab order
    windows: Vec<MatrixWindow>,
    /// The active window index
    active_index: usize,
    /// Whether to show tabs (when only one window, tabs might be hidden)
    show_tabs: bool,
    /// The modal state for managing key modes and commands
    modal_state: ModalState,
}

impl WindowManager {
    /// Create a new window manager
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            active_index: 0,
            show_tabs: true,
            modal_state: ModalState::default(),
        }
    }

    /// Initialize with a welcome window
    pub fn init(&mut self, store: &mut ProgramStore) -> maxtryxResult<()> {
        let welcome = MatrixWindow::new(maxtryxId::Welcome, store)?;
        self.add_window(welcome);
        Ok(())
    }

    /// Add a window to the manager
    pub fn add_window(&mut self, window: MatrixWindow) {
        self.windows.push(window);
        self.active_index = self.windows.len() - 1;
    }

    /// Get the active window
    pub fn active_window(&self) -> Option<&MatrixWindow> {
        self.windows.get(self.active_index)
    }

    /// Get the active window mutably
    pub fn active_window_mut(&mut self) -> Option<&mut MatrixWindow> {
        self.windows.get_mut(self.active_index)
    }

    /// Switch to the next tab
    pub fn next_tab(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        
        if self.active_index >= self.windows.len() - 1 {
            self.active_index = 0;
        } else {
            self.active_index += 1;
        }
    }

    /// Switch to the previous tab
    pub fn prev_tab(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        
        if self.active_index == 0 {
            self.active_index = self.windows.len() - 1;
        } else {
            self.active_index -= 1;
        }
    }

    /// Switch to a specific tab by index
    pub fn switch_to_tab(&mut self, index: usize) {
        if index < self.windows.len() {
            self.active_index = index;
        }
    }

    /// Switch to a specific window by ID
    pub fn switch_to_window(&mut self, id: &maxtryxId) -> bool {
        for (idx, window) in self.windows.iter().enumerate() {
            if &window.id() == id {
                self.active_index = idx;
                return true;
            }
        }
        false
    }

    /// Open a new window
    pub fn open_window(&mut self, id: maxtryxId, store: &mut ProgramStore) -> maxtryxResult<()> {
        // Check if window already exists
        for (idx, window) in self.windows.iter().enumerate() {
            if window.id() == id {
                self.active_index = idx;
                return Ok(());
            }
        }
        
        // Create and add new window
        let window = MatrixWindow::new(id, store)?;
        self.add_window(window);
        Ok(())
    }

    /// Close the active window
    pub fn close_active_window(&mut self, store: &mut ProgramStore) -> bool {
        if self.windows.is_empty() {
            return false;
        }
        
        if let Some(window) = self.active_window_mut() {
            if window.close(store) {
                self.windows.remove(self.active_index);
                if self.active_index >= self.windows.len() && !self.windows.is_empty() {
                    self.active_index = self.windows.len() - 1;
                }
                return true;
            }
        }
        
        false
    }

    /// Handle input events for the window system
    pub fn handle_event(&mut self, event: &InputEvent, store: &mut ProgramStore) -> bool {
        // First check for window management commands
        if self.handle_window_commands(event) {
            return true;
        }
        
        // Then delegate to active window
        if let Some(window) = self.active_window_mut() {
            match event {
                InputEvent::Key(key) => {
                    // Convert the key to an action using modal_state's current mode
                    let action = self.modal_state.get_action_for_key(key);
                    
                    if let Some(action) = action {
                        match action {
                            // Handle window-specific actions
                            EditorAction::Window(window_action) => {
                                self.handle_window_action(&window_action, store)
                            }
                            // Handle various editor actions
                            EditorAction::Movement(_) | 
                            EditorAction::Edit(_) |
                            EditorAction::Search(_) => {
                                // Let the window handle the action
                                let ctx = ProgramContext::default();
                                window.execute_action(&action, &ctx, store).is_ok()
                            }
                            // Handle custom window management
                            EditorAction::Matrix(MatrixAction::ToggleScrollbackFocus) => {
                                window.focus_toggle();
                                true
                            }
                            // Handle other actions
                            _ => false,
                        }
                    } else {
                        false
                    }
                },
                InputEvent::Mouse(_) => false, // Not handling mouse events yet
                InputEvent::Resize(_) => false, // No need to handle resize events here
            }
        } else {
            false
        }
    }

    /// Handle window management commands (e.g., tab switching)
    fn handle_window_commands(&mut self, event: &InputEvent) -> bool {
        if let InputEvent::Key(key) = event {
            // Handle window/tab management key shortcuts in normal mode
            if self.modal_state.is_normal_mode() {
                match key.code {
                    crossterm::event::KeyCode::Tab => {
                        if key.has_shift() {
                            self.prev_tab();
                        } else {
                            self.next_tab();
                        }
                        return true;
                    },
                    crossterm::event::KeyCode::Char('h') if key.has_ctrl() => {
                        self.prev_tab();
                        return true;
                    },
                    crossterm::event::KeyCode::Char('l') if key.has_ctrl() => {
                        self.next_tab();
                        return true;
                    },
                    crossterm::event::KeyCode::F(n) if n >= 1 && n <= 12 => {
                        let idx = (n - 1) as usize;
                        if idx < self.windows.len() {
                            self.switch_to_tab(idx);
                            return true;
                        }
                    },
                    crossterm::event::KeyCode::Char('w') if key.has_ctrl() => {
                        self.close_active_window(store);
                        return true;
                    },
                    crossterm::event::KeyCode::Char('t') if key.has_ctrl() => {
                        // Open a new welcome tab
                        if let Ok(()) = self.open_window(maxtryxId::Welcome, store) {
                            return true;
                        }
                    },
                    _ => {},
                }
            }
        }
        false
    }
    
    /// Handle window actions
    fn handle_window_action(&mut self, action: &WindowAction, store: &mut ProgramStore) -> bool {
        match action {
            WindowAction::Close => {
                self.close_active_window(store);
                true
            },
            WindowAction::Next => {
                self.next_tab();
                true
            },
            WindowAction::Previous => {
                self.prev_tab();
                true
            },
            WindowAction::SwitchById(id) => {
                if let Ok(()) = self.open_window(id.clone(), store) {
                    true
                } else {
                    false
                }
            },
            WindowAction::SwitchByName(name) => {
                // Try to find window by name
                if let Ok(window) = MatrixWindow::find(name.clone(), store) {
                    self.add_window(window);
                    true
                } else {
                    false
                }
            },
            _ => false,
        }
    }

    /// Toggle visibility of the tab bar
    pub fn toggle_tabs(&mut self) {
        self.show_tabs = !self.show_tabs;
    }

    /// Render the window manager with all windows and tabs
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, store: &mut ProgramStore) {
        if self.windows.is_empty() {
            return;
        }

        // Calculate layouts with/without tab bar
        let chunks = if self.show_tabs && self.windows.len() > 1 {
            Layout::default()
                .constraints([Constraint::Length(3), Constraint::Min(1)].as_ref())
                .split(area)
        } else {
            vec![area]
        };

        // Draw tab bar if enabled and more than one window
        if self.show_tabs && self.windows.len() > 1 {
            self.render_tabs(chunks[0], buf, store);
        }
        
        // Draw active window
        if let Some(window) = self.active_window_mut() {
            // Content area is either below tabs or full area if no tabs
            let content_area = if self.show_tabs && self.windows.len() > 1 {
                chunks[1]
            } else {
                chunks[0]
            };
            
            window.draw(content_area, buf, true, store);
        }
    }

    /// Render the tab bar
    fn render_tabs(&mut self, area: Rect, buf: &mut Buffer, store: &mut ProgramStore) {
        let tab_titles: Vec<Line> = self.windows
            .iter()
            .enumerate()
            .map(|(idx, window)| {
                let title = window.tab_title(store);
                if idx == self.active_index {
                    // Highlight active tab
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled(
                            format!(" {} ", idx + 1),
                            Style::default().fg(Color::Black).bg(Color::LightCyan),
                        ),
                        Span::styled(" ", Style::default()),
                        Span::styled(
                            title.spans.iter().map(|span| span.content.clone()).collect::<Vec<_>>().join(""),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" ", Style::default()),
                    ])
                } else {
                    // Regular tab
                    Line::from(vec![
                        Span::styled(" ", Style::default()),
                        Span::styled(
                            format!(" {} ", idx + 1),
                            Style::default().bg(Color::DarkGray),
                        ),
                        Span::styled(" ", Style::default()),
                        Span::raw(title.spans.iter().map(|span| span.content.clone()).collect::<Vec<_>>().join("")),
                        Span::styled(" ", Style::default()),
                    ])
                }
            })
            .collect();

        Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .select(self.active_index)
            .render(area, buf);
    }
}