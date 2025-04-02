use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;

/// Represents a position in a text buffer (0-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CursorPosition {
    /// Line position (row)
    pub line: usize,
    /// Column position
    pub column: usize,
}

impl CursorPosition {
    /// Create a new cursor position
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Get line position (row)
    pub fn line(&self) -> usize {
        self.line
    }

    /// Get column position
    pub fn column(&self) -> usize {
        self.column
    }

    /// Get position as tuple (line, column)
    pub fn as_tuple(&self) -> (usize, usize) {
        (self.line, self.column)
    }

    /// Set line position
    pub fn set_line(&mut self, line: usize) {
        self.line = line;
    }

    /// Set column position
    pub fn set_column(&mut self, column: usize) {
        self.column = column;
    }

    /// Move cursor to beginning of line
    pub fn move_to_line_start(&mut self) {
        self.column = 0;
    }

    /// Move cursor to specified position
    pub fn move_to(&mut self, line: usize, column: usize) {
        self.line = line;
        self.column = column;
    }
}

/// Selection range in the text
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    /// Start position of selection
    pub start: CursorPosition,
    /// End position of selection
    pub end: CursorPosition,
}

impl Selection {
    /// Create a new selection
    pub fn new(start: CursorPosition, end: CursorPosition) -> Self {
        Self { start, end }
    }

    /// Create a selection from a single position
    pub fn from_position(pos: CursorPosition) -> Self {
        Self::new(pos, pos)
    }

    /// Check if the selection is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Return the start and end positions in order (start <= end)
    pub fn ordered(&self) -> (CursorPosition, CursorPosition) {
        if self.start.line < self.end.line ||
            (self.start.line == self.end.line && self.start.column <= self.end.column)
        {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

/// Cursor style/shape
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    /// Block cursor (full character cell)
    Block,
    /// Underline cursor
    Underline,
    /// Vertical bar cursor
    Bar,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self::Block
    }
}

/// Cursor state for the editor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorState {
    /// Current cursor position
    position: CursorPosition,
    /// Cursor style
    style: CursorStyle,
    /// Whether the cursor is visible
    visible: bool,
    /// Whether the cursor should blink
    blinking: bool,
    /// Whether to show the cursor at the end of a line
    past_end_of_line: bool,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            position: CursorPosition::default(),
            style: CursorStyle::default(),
            visible: true,
            blinking: true,
            past_end_of_line: true,
        }
    }
}

impl CursorState {
    /// Create a new cursor state
    pub fn new() -> Self {
        Self::default()
    }

    /// Get cursor position
    pub fn position(&self) -> CursorPosition {
        self.position
    }

    /// Set cursor position
    pub fn set_position(&mut self, position: CursorPosition) {
        self.position = position;
    }

    /// Move cursor to position
    pub fn move_to(&mut self, line: usize, column: usize) {
        self.position.move_to(line, column);
    }

    /// Get cursor style
    pub fn style(&self) -> CursorStyle {
        self.style
    }

    /// Set cursor style
    pub fn set_style(&mut self, style: CursorStyle) {
        self.style = style;
    }

    /// Check if cursor is visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Set cursor visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Check if cursor is blinking
    pub fn is_blinking(&self) -> bool {
        self.blinking
    }

    /// Set cursor blinking
    pub fn set_blinking(&mut self, blinking: bool) {
        self.blinking = blinking;
    }

    /// Check if cursor can be past end of line
    pub fn allows_past_end_of_line(&self) -> bool {
        self.past_end_of_line
    }

    /// Set whether cursor can be past end of line
    pub fn set_allows_past_end_of_line(&mut self, allow: bool) {
        self.past_end_of_line = allow;
    }
}

/// Represents different editing modes for the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    /// Normal mode for navigating and executing commands
    Normal,
    /// Insert mode for text input
    Insert,
    /// Visual mode for selecting text
    Visual,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Insert => write!(f, "INSERT"),
            Mode::Visual => write!(f, "VISUAL"),
        }
    }
}

impl Mode {
    /// Returns true if the mode allows text input
    pub fn allows_input(&self) -> bool {
        matches!(self, Mode::Insert)
    }

    /// Returns true if the mode allows text selection
    pub fn allows_selection(&self) -> bool {
        matches!(self, Mode::Visual)
    }

    /// Returns true if the mode is for command execution
    pub fn is_command_mode(&self) -> bool {
        matches!(self, Mode::Normal)
    }

    /// Get the preferred cursor style for this mode
    pub fn cursor_style(&self) -> CursorStyle {
        match self {
            Mode::Normal => CursorStyle::Block,
            Mode::Insert => CursorStyle::Bar,
            Mode::Visual => CursorStyle::Underline,
        }
    }
}

/// Manages the modal state of the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalState {
    /// Current editing mode
    mode: Mode,
    /// Previous mode to return to when exiting current mode
    previous_mode: Option<Mode>,
    /// Cursor state
    cursor: CursorState,
    /// Current selection (if any)
    selection: Option<Selection>,
}

impl Default for ModalState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            previous_mode: None,
            cursor: CursorState::default(),
            selection: None,
        }
    }
}

impl ModalState {
    /// Creates a new modal state with Normal mode
    pub fn new() -> Self {
        Self::default()
    }

    /// Gets the current mode
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Sets the mode, storing the previous mode
    pub fn set_mode(&mut self, mode: Mode) {
        if self.mode != mode {
            // Store the previous mode
            self.previous_mode = Some(self.mode);

            // Update mode
            self.mode = mode;

            // Update cursor style based on mode
            self.cursor.set_style(mode.cursor_style());

            // Handle selection when entering/exiting visual mode
            if mode == Mode::Visual {
                // Start selection at cursor position if none exists
                if self.selection.is_none() {
                    self.selection = Some(Selection::from_position(self.cursor.position()));
                }
            } else if self.previous_mode == Some(Mode::Visual) {
                // Clear selection when exiting visual mode
                if !mode.allows_selection() {
                    self.selection = None;
                }
            }
        }
    }

    /// Returns to the previous mode if available
    pub fn return_to_previous_mode(&mut self) -> bool {
        if let Some(previous) = self.previous_mode {
            self.set_mode(previous);
            self.previous_mode = None;
            true
        } else {
            false
        }
    }

    /// Enter normal mode
    pub fn enter_normal_mode(&mut self) {
        self.set_mode(Mode::Normal);
    }

    /// Enter insert mode
    pub fn enter_insert_mode(&mut self) {
        self.set_mode(Mode::Insert);
    }

    /// Enter visual mode
    pub fn enter_visual_mode(&mut self) {
        self.set_mode(Mode::Visual);
    }

    /// Check if currently in normal mode
    pub fn is_normal_mode(&self) -> bool {
        self.mode == Mode::Normal
    }

    /// Check if currently in insert mode
    pub fn is_insert_mode(&self) -> bool {
        self.mode == Mode::Insert
    }

    /// Check if currently in visual mode
    pub fn is_visual_mode(&self) -> bool {
        self.mode == Mode::Visual
    }

    /// Get the cursor state
    pub fn cursor(&self) -> &CursorState {
        &self.cursor
    }

    /// Get mutable access to the cursor state
    pub fn cursor_mut(&mut self) -> &mut CursorState {
        &mut self.cursor
    }

    /// Get the cursor position
    pub fn cursor_position(&self) -> CursorPosition {
        self.cursor.position()
    }

    /// Set the cursor position
    pub fn set_cursor_position(&mut self, position: CursorPosition) {
        self.cursor.set_position(position);

        // If in visual mode, update the selection end
        if self.is_visual_mode() {
            if let Some(selection) = &mut self.selection {
                selection.end = position;
            }
        }
    }

    /// Move cursor to position
    pub fn move_cursor_to(&mut self, line: usize, column: usize) {
        self.set_cursor_position(CursorPosition::new(line, column));
    }

    /// Get the current selection (if any)
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Start a new selection at the current cursor position
    pub fn start_selection(&mut self) {
        self.selection = Some(Selection::from_position(self.cursor.position()));
    }

    /// Update the selection end to the current cursor position
    pub fn update_selection(&mut self) {
        if let Some(selection) = &mut self.selection {
            selection.end = self.cursor.position();
        } else {
            self.start_selection();
        }
    }

    /// Clear the current selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Check if there is an active selection
    pub fn has_selection(&self) -> bool {
        self.selection.map_or(false, |sel| !sel.is_empty())
    }

    /// Get cursor visibility appropriate for current mode
    pub fn cursor_visible(&self) -> bool {
        self.cursor.is_visible()
    }

    /// Update cursor style based on current mode
    pub fn update_cursor_style(&mut self) {
        self.cursor.set_style(self.mode.cursor_style());
    }

    /// Get the appropriate cursor style for the current mode
    pub fn get_cursor_style_for_mode(&self) -> CursorStyle {
        self.mode.cursor_style()
    }
}

/// Mode-specific data for extensions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModeData {
    /// Mode-specific data as key-value pairs
    data: std::collections::HashMap<String, String>,
}

impl ModeData {
    /// Create a new empty mode data
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a value by key
    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(|s| s.as_str())
    }

    /// Set a value
    pub fn set(&mut self, key: &str, value: &str) {
        self.data.insert(key.to_string(), value.to_string());
    }

    /// Remove a value
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.data.remove(key)
    }

    /// Check if contains a key
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.data.clear();
    }
}

impl ModalState {
    /// Get action for a key input based on the current mode
    pub fn get_action_for_key(&self, key: &crate::modal::Key) -> Option<crate::modal::EditorAction> {
        use crate::modal::{EditorAction, MovementAction, WindowAction};
        
        // This is a simplified implementation - in a full implementation, we would
        // look up the key in a keybinding map based on the current mode
        match key.code {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') if self.is_normal_mode() => {
                Some(EditorAction::Movement(MovementAction::Up))
            },
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') if self.is_normal_mode() => {
                Some(EditorAction::Movement(MovementAction::Down))
            },
            crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Char('h') if self.is_normal_mode() => {
                Some(EditorAction::Movement(MovementAction::Left))
            },
            crossterm::event::KeyCode::Right | crossterm::event::KeyCode::Char('l') if self.is_normal_mode() => {
                Some(EditorAction::Movement(MovementAction::Right))
            },
            crossterm::event::KeyCode::Home => {
                Some(EditorAction::Movement(MovementAction::First))
            },
            crossterm::event::KeyCode::End => {
                Some(EditorAction::Movement(MovementAction::Last))
            },
            crossterm::event::KeyCode::PageUp => {
                Some(EditorAction::Movement(MovementAction::PageUp))
            },
            crossterm::event::KeyCode::PageDown => {
                Some(EditorAction::Movement(MovementAction::PageDown))
            },
            crossterm::event::KeyCode::Char('q') if self.is_normal_mode() => {
                Some(EditorAction::Window(WindowAction::Close))
            },
            crossterm::event::KeyCode::Char('n') if key.has_ctrl() => {
                Some(EditorAction::Window(WindowAction::Next))
            },
            crossterm::event::KeyCode::Char('p') if key.has_ctrl() => {
                Some(EditorAction::Window(WindowAction::Previous))
            },
            _ => None,
        }
    }

    /// Get mode-specific data
    pub fn get_mode_data(&self, key: &str) -> Option<&str> {
        self.get_mode_data_for_mode(self.mode, key)
    }

    /// Get mode-specific data for a specific mode
    pub fn get_mode_data_for_mode(&self, mode: Mode, key: &str) -> Option<&str> {
        // This is a placeholder implementation
        // In a real implementation, we would store mode-specific data
        None
    }

    /// Set mode-specific data
    pub fn set_mode_data(&mut self, key: &str, value: &str) {
        self.set_mode_data_for_mode(self.mode, key, value);
    }

    /// Set mode-specific data for a specific mode
    pub fn set_mode_data_for_mode(&mut self, mode: Mode, key: &str, value: &str) {
        // This is a placeholder implementation
        // In a real implementation, we would store mode-specific data
    }

    /// Save state to a file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let serialized =
            serde_json::to_string(self).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let mut file = fs::File::create(path)?;
        file.write_all(serialized.as_bytes())?;
        Ok(())
    }

    /// Load state from a file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        serde_json::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    /// Save state to a string
    pub fn save_to_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Load state from a string
    pub fn load_from_string(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}
