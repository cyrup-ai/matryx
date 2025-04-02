# Implementation Plan: Replacing Modalkit with Native Ratatui

This document outlines the steps to replace modalkit with native ratatui components in Cyrum, focusing on key integration points and required functionality. We'll use ratatui 0.30.0-alpha.2 as the foundation for our implementation.

## 1. Modal State System

### Requirements
- Track current input mode (Normal, Insert, Visual)
- Support mode transitions
- Store mode-specific state

### Implementation Approach

```rust
// src/modal/mode.rs
pub enum Mode {
    Normal,
    Insert,
    Visual,
    // Add other modes as needed
}

pub struct ModalState {
    pub mode: Mode,
    pub visual_selection: Option<(usize, usize)>,
    // Other mode-specific state
}

impl ModalState {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            visual_selection: None,
        }
    }
    
    pub fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
        self.visual_selection = None;
    }
    
    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }
    
    pub fn enter_visual_mode(&mut self) {
        self.mode = Mode::Visual;
    }
}
```

## 2. Keybinding System

### Requirements
- Map keys to actions based on current mode
- Support key sequences
- Handle special keys and modifiers

### Implementation Approach

```rust
// src/modal/keybindings.rs
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};

pub struct KeyBinding {
    pub key: KeyEvent,
    pub action: Action,
}

pub struct KeyBindingMap {
    normal_bindings: HashMap<KeyEvent, Action>,
    insert_bindings: HashMap<KeyEvent, Action>,
    visual_bindings: HashMap<KeyEvent, Action>,
}

impl KeyBindingMap {
    pub fn new() -> Self {
        let mut map = Self {
            normal_bindings: HashMap::new(),
            insert_bindings: HashMap::new(),
            visual_bindings: HashMap::new(),
        };
        
        map.setup_default_bindings();
        map
    }
    
    pub fn setup_default_bindings(&mut self) {
        // Normal mode bindings
        self.add_normal_binding(KeyCode::Char('j'), Action::MoveCursorDown);
        self.add_normal_binding(KeyCode::Char('k'), Action::MoveCursorUp);
        // Add more bindings...
    }
    
    pub fn add_normal_binding(&mut self, key: KeyCode, action: Action) {
        self.normal_bindings.insert(
            KeyEvent::new(key, KeyModifiers::NONE),
            action
        );
    }
    
    pub fn get_action(&self, mode: &Mode, key: KeyEvent) -> Option<Action> {
        match mode {
            Mode::Normal => self.normal_bindings.get(&key).cloned(),
            Mode::Insert => self.insert_bindings.get(&key).cloned(),
            Mode::Visual => self.visual_bindings.get(&key).cloned(),
        }
    }
}
```

## 3. Text Editing Component

### Requirements
- Editable text area with cursor
- Support for history/undo
- Vim-like editing commands

### Implementation Approach

```rust
// src/modal/textbox.rs
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, StatefulWidget, Widget};

pub struct TextBox {
    block: Option<Block<'static>>,
}

pub struct TextBoxState {
    content: String,
    cursor_position: usize,
    history: Vec<String>,
    history_index: usize,
}

impl TextBoxState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            history: Vec::new(),
            history_index: 0,
        }
    }
    
    pub fn handle_keypress(&mut self, key: KeyEvent, mode: &Mode) -> bool {
        match mode {
            Mode::Normal => self.handle_normal_mode_key(key),
            Mode::Insert => self.handle_insert_mode_key(key),
            Mode::Visual => self.handle_visual_mode_key(key),
        }
    }
    
    fn handle_insert_mode_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.insert_char(c);
                true
            }
            KeyCode::Backspace => {
                self.delete_previous_char();
                true
            }
            // Handle more keys...
            _ => false,
        }
    }
    
    fn insert_char(&mut self, c: char) {
        if self.cursor_position >= self.content.len() {
            self.content.push(c);
        } else {
            self.content.insert(self.cursor_position, c);
        }
        self.cursor_position += 1;
    }
    
    // Add more editing functions...
}

impl StatefulWidget for TextBox {
    type State = TextBoxState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Render the text content with cursor
        let block = self.block.unwrap_or_else(|| Block::default());
        let inner_area = block.inner(area);
        block.render(area, buf);
        
        // Render text with cursor...
        // Implementation details here
    }
}
```

## 4. Popup Dialog System

### Requirements
- Modal overlay with backdrop
- Confirmation dialogs (Yes/No)
- Multi-choice selection
- Text input dialogs

### Implementation Approach

```rust
// src/modal/dialog.rs
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, StatefulWidget, Widget};

pub enum DialogType {
    YesNo,
    MultiChoice,
    TextInput,
}

pub struct Dialog {
    title: String,
    message: String,
    dialog_type: DialogType,
}

pub struct DialogState {
    is_visible: bool,
    selected_index: usize,
    choices: Vec<String>,
    text_input: String,
}

impl Dialog {
    pub fn yes_no(title: String, message: String) -> Self {
        Self {
            title,
            message,
            dialog_type: DialogType::YesNo,
        }
    }
    
    pub fn multi_choice(title: String, message: String) -> Self {
        Self {
            title,
            message,
            dialog_type: DialogType::MultiChoice,
        }
    }
}

impl StatefulWidget for Dialog {
    type State = DialogState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_visible {
            return;
        }
        
        // Render semi-transparent backdrop
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = buf.get_mut(x, y);
                cell.set_bg(Color::Black);
                cell.set_fg(Color::DarkGray);
            }
        }
        
        // Calculate dialog size and position
        let dialog_width = area.width.min(50);
        let dialog_height = area.height.min(10);
        let dialog_x = (area.width - dialog_width) / 2;
        let dialog_y = (area.height - dialog_height) / 2;
        let dialog_area = Rect::new(dialog_x, dialog_y, dialog_width, dialog_height);
        
        // Render dialog box
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::Gray).fg(Color::Black));
        block.render(dialog_area, buf);
        
        // Render content based on dialog type
        let inner_area = block.inner(dialog_area);
        match self.dialog_type {
            DialogType::YesNo => {
                // Render Yes/No options
            }
            DialogType::MultiChoice => {
                // Render multiple choices
            }
            DialogType::TextInput => {
                // Render text input
            }
        }
    }
}
```

## 5. Window Management

### Requirements
- Support multiple windows
- Track active window
- Handle window focus

### Implementation Approach

```rust
// src/modal/window.rs
use std::collections::HashMap;
use ratatui::layout::Rect;

pub trait WindowTrait {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn handle_input(&mut self, key: KeyEvent) -> Option<Action>;
    fn title(&self) -> &str;
}

pub struct WindowManager {
    windows: Vec<Box<dyn WindowTrait>>,
    active_window_index: usize,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            active_window_index: 0,
        }
    }
    
    pub fn add_window(&mut self, window: Box<dyn WindowTrait>) {
        self.windows.push(window);
        self.active_window_index = self.windows.len() - 1;
    }
    
    pub fn active_window(&self) -> Option<&dyn WindowTrait> {
        self.windows.get(self.active_window_index).map(|w| w.as_ref())
    }
    
    pub fn active_window_mut(&mut self) -> Option<&mut dyn WindowTrait> {
        self.windows.get_mut(self.active_window_index).map(|w| w.as_mut())
    }
    
    pub fn handle_input(&mut self, key: KeyEvent) -> Option<Action> {
        if let Some(window) = self.active_window_mut() {
            window.handle_input(key)
        } else {
            None
        }
    }
    
    pub fn focus_next_window(&mut self) {
        if !self.windows.is_empty() {
            self.active_window_index = (self.active_window_index + 1) % self.windows.len();
        }
    }
    
    pub fn focus_prev_window(&mut self) {
        if !self.windows.is_empty() {
            self.active_window_index = 
                (self.active_window_index + self.windows.len() - 1) % self.windows.len();
        }
    }
}
```

## Implementation Steps

1. **Phase 1: Core Framework**
   - Create modal state system
   - Implement basic keybinding manager
   - Set up action dispatch system

2. **Phase 2: UI Components**
   - Develop text editing widget
   - Create dialog system
   - Implement window management

3. **Phase 3: Integration**
   - Replace modalkit imports with new components
   - Update existing code to use the new system
   - Ensure feature parity with existing functionality

4. **Phase 4: Testing & Refinement**
   - Test all modal functionality
   - Verify vim-like editing experience
   - Optimize performance

## Migration Roadmap

The migration process should be incremental, focusing on one component at a time:

1. Start with the dialog system as it's more isolated
2. Then implement the text editing component
3. Next, build the window management system
4. Finally, integrate the mode and keybinding systems

This approach allows for testing each component individually before full integration.

## Dependencies

The implementation will rely on:
- ratatui 0.30.0-alpha.2
- crossterm for event handling
- native Rust collections for data structures

No external modal or dialog libraries will be used to avoid creating new dependencies.