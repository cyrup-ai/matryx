use std::collections::HashMap;
use std::fmt;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, StatefulWidget, Widget},
};

use crate::modal::{InputEvent, Key, ModalState, Mode};

/// Unique identifier for a window
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WindowId(String);

impl WindowId {
    /// Create a new window ID
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Get the string value of the window ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for WindowId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for WindowId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Trait for window components
pub trait WindowComponent {
    /// Handle an input event
    fn handle_event(&mut self, event: &InputEvent, modal_state: &mut ModalState) -> bool;

    /// Render the component
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    /// Check if the component can receive focus
    fn can_focus(&self) -> bool {
        true
    }

    /// Set focus state
    fn set_focus(&mut self, focused: bool);

    /// Check if the component is focused
    fn is_focused(&self) -> bool;

    /// Get the component title
    fn title(&self) -> &str;
}

/// Window state
#[derive(Debug, Clone)]
pub struct WindowState {
    /// Window title
    pub title: String,
    /// Window is active
    pub active: bool,
    /// Window is visible
    pub visible: bool,
    /// Window position and size
    pub area: Option<Rect>,
    /// Window modal state
    pub modal_state: ModalState,
}

impl WindowState {
    /// Create a new window state
    pub fn new<S: Into<String>>(title: S) -> Self {
        Self {
            title: title.into(),
            active: false,
            visible: true,
            area: None,
            modal_state: ModalState::default(),
        }
    }
}

/// Window widget
pub struct Window<'a> {
    /// Block for styling the window
    pub block: Option<Block<'a>>,
    /// Style for the window
    pub style: Style,
    /// Style for an active window
    pub active_style: Style,
    /// Components in this window
    components: Vec<Box<dyn WindowComponent>>,
    /// Currently focused component index
    focused_component: Option<usize>,
}

impl<'a> Default for Window<'a> {
    fn default() -> Self {
        Self {
            block: None,
            style: Style::default(),
            active_style: Style::default().fg(Color::Cyan),
            components: Vec::new(),
            focused_component: None,
        }
    }
}

impl<'a> Window<'a> {
    /// Create a new window
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the block for the window
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for the window
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the style for an active window
    pub fn active_style(mut self, style: Style) -> Self {
        self.active_style = style;
        self
    }

    /// Add a component to the window
    pub fn add_component<T: WindowComponent + 'static>(&mut self, component: T) -> &mut Self {
        self.components.push(Box::new(component));
        if self.focused_component.is_none() && component.can_focus() {
            self.focused_component = Some(self.components.len() - 1);
            self.components.last_mut().unwrap().set_focus(true);
        }
        self
    }

    /// Focus the next component
    pub fn focus_next(&mut self) -> bool {
        if self.components.is_empty() {
            return false;
        }

        if let Some(current) = self.focused_component {
            self.components[current].set_focus(false);
        }

        if let Some(current) = self.focused_component {
            let mut next = (current + 1) % self.components.len();

            // Find the next focusable component
            while next != current && !self.components[next].can_focus() {
                next = (next + 1) % self.components.len();
            }

            if self.components[next].can_focus() {
                self.focused_component = Some(next);
                self.components[next].set_focus(true);
                return true;
            }
        }

        false
    }

    /// Focus the previous component
    pub fn focus_prev(&mut self) -> bool {
        if self.components.is_empty() {
            return false;
        }

        if let Some(current) = self.focused_component {
            self.components[current].set_focus(false);
        }

        if let Some(current) = self.focused_component {
            let mut prev = if current == 0 {
                self.components.len() - 1
            } else {
                current - 1
            };

            // Find the previous focusable component
            let start = prev;
            while prev != current && !self.components[prev].can_focus() {
                prev = if prev == 0 {
                    self.components.len() - 1
                } else {
                    prev - 1
                };

                // Avoid infinite loop if no focusable components
                if prev == start {
                    break;
                }
            }

            if self.components[prev].can_focus() {
                self.focused_component = Some(prev);
                self.components[prev].set_focus(true);
                return true;
            }
        }

        false
    }

    /// Handle an input event
    pub fn handle_event(&mut self, event: &InputEvent, state: &mut WindowState) -> bool {
        // First, check if the focused component handles the event
        if let Some(focused) = self.focused_component {
            if self.components[focused].handle_event(event, &mut state.modal_state) {
                return true;
            }
        }

        // Then check window-level events
        match event {
            InputEvent::Key(key) => {
                if state.modal_state.is_normal_mode() {
                    match key.code {
                        // Tab to cycle focus
                        crossterm::event::KeyCode::Tab => {
                            if key.has_shift() {
                                return self.focus_prev();
                            } else {
                                return self.focus_next();
                            }
                        },
                        _ => {},
                    }
                }
            },
            _ => {},
        }

        false
    }
}

impl<'a> StatefulWidget for Window<'a> {
    type State = WindowState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Update the window area
        state.area = Some(area);

        // Create the window block
        let block = self.block.unwrap_or_else(|| {
            Block::default()
                .title(state.title.clone())
                .borders(Borders::ALL)
                .border_style(if state.active {
                    self.active_style
                } else {
                    self.style
                })
        });

        // Render the window block
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Render each component
        for component in self.components {
            component.render(inner_area, buf);
        }
    }
}

/// Window manager that handles multiple windows
pub struct WindowManager {
    /// Collection of windows
    windows: HashMap<WindowId, Box<dyn WindowComponent>>,
    /// Z-order of windows (front to back)
    z_order: Vec<WindowId>,
    /// Currently active window
    active_window: Option<WindowId>,
}

impl Default for WindowManager {
    fn default() -> Self {
        Self {
            windows: HashMap::new(),
            z_order: Vec::new(),
            active_window: None,
        }
    }
}

impl WindowManager {
    /// Create a new window manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a window to the manager
    pub fn add_window<T: WindowComponent + 'static, I: Into<WindowId>>(
        &mut self,
        id: I,
        window: T,
    ) -> &mut Self {
        let window_id = id.into();
        self.windows.insert(window_id.clone(), Box::new(window));
        self.z_order.push(window_id.clone());

        if self.active_window.is_none() {
            self.active_window = Some(window_id);
        }

        self
    }

    /// Remove a window from the manager
    pub fn remove_window<I: Into<WindowId>>(&mut self, id: I) -> Option<Box<dyn WindowComponent>> {
        let window_id = id.into();
        let window = self.windows.remove(&window_id);

        self.z_order.retain(|id| id != &window_id);

        if self.active_window.as_ref() == Some(&window_id) {
            self.active_window = self.z_order.first().cloned();
        }

        window
    }

    /// Get a reference to a window by ID
    pub fn get_window<I: Into<WindowId>>(&self, id: I) -> Option<&Box<dyn WindowComponent>> {
        self.windows.get(&id.into())
    }

    /// Get a mutable reference to a window by ID
    pub fn get_window_mut<I: Into<WindowId>>(
        &mut self,
        id: I,
    ) -> Option<&mut Box<dyn WindowComponent>> {
        self.windows.get_mut(&id.into())
    }

    /// Activate a window
    pub fn activate_window<I: Into<WindowId>>(&mut self, id: I) -> bool {
        let window_id = id.into();

        if !self.windows.contains_key(&window_id) {
            return false;
        }

        // Move the window to the front of the z-order
        self.z_order.retain(|id| id != &window_id);
        self.z_order.insert(0, window_id.clone());

        // Set as active window
        self.active_window = Some(window_id);

        true
    }

    /// Get the active window
    pub fn active_window(&self) -> Option<&Box<dyn WindowComponent>> {
        self.active_window.as_ref().and_then(|id| self.windows.get(id))
    }

    /// Get the active window (mutable)
    pub fn active_window_mut(&mut self) -> Option<&mut Box<dyn WindowComponent>> {
        if let Some(id) = &self.active_window {
            self.windows.get_mut(id)
        } else {
            None
        }
    }

    /// Handle an input event
    pub fn handle_event(&mut self, event: &InputEvent, modal_state: &mut ModalState) -> bool {
        // First try the active window
        if let Some(active_id) = &self.active_window {
            if let Some(window) = self.windows.get_mut(active_id) {
                if window.handle_event(event, modal_state) {
                    return true;
                }
            }
        }

        // Check for window switching keys
        if modal_state.is_normal_mode() {
            if let InputEvent::Key(key) = event {
                // Handle window navigation keys
                match key.code {
                    crossterm::event::KeyCode::F(n) if n <= 12 => {
                        let idx = (n - 1) as usize;
                        if idx < self.z_order.len() {
                            let window_id = self.z_order[idx].clone();
                            return self.activate_window(window_id);
                        }
                    },
                    _ => {},
                }
            }
        }

        false
    }

    /// Render all windows
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Render windows from back to front
        for window_id in self.z_order.iter().rev() {
            if let Some(window) = self.windows.get_mut(window_id) {
                window.render(area, buf);
            }
        }
    }
}
