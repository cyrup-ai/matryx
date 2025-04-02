# Input Handling in Ratatui Applications

Ratatui itself doesn't include direct input handling, as this is typically handled by the backend you're using (such as crossterm or termion). This document covers common patterns and best practices for handling user input in Ratatui applications.

## Basic Input Handling with Crossterm

Here's a basic setup for handling keyboard input with crossterm:

```rust
use std::{io, time::Duration};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{Terminal, backend::CrosstermBackend};

fn main() -> Result<(), io::Error> {
    // Terminal setup code...
    
    // Application loop
    loop {
        terminal.draw(|f| {
            // UI rendering code
        })?;
        
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    // Handle other keys
                    _ => {}
                }
            }
        }
    }
    
    // Terminal cleanup code...
    
    Ok(())
}
```

## Types of Input Events

### Keyboard Events

Handling different types of keyboard events:

```rust
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
    match (code, modifiers) {
        // Quit on Ctrl+C or q
        (KeyCode::Char('c'), KeyModifiers::CONTROL) | 
        (KeyCode::Char('q'), KeyModifiers::NONE) => {
            break;
        }
        
        // Function keys
        (KeyCode::F(1), _) => {
            // Show help
        }
        
        // Navigation keys
        (KeyCode::Up, _) => {
            app.move_cursor_up();
        }
        (KeyCode::Down, _) => {
            app.move_cursor_down();
        }
        (KeyCode::Left, _) => {
            app.move_cursor_left();
        }
        (KeyCode::Right, _) => {
            app.move_cursor_right();
        }
        
        // Page navigation
        (KeyCode::Home, _) => {
            app.go_to_start();
        }
        (KeyCode::End, _) => {
            app.go_to_end();
        }
        (KeyCode::PageUp, _) => {
            app.page_up();
        }
        (KeyCode::PageDown, _) => {
            app.page_down();
        }
        
        // Text editing
        (KeyCode::Char(c), _) => {
            app.insert_char(c);
        }
        (KeyCode::Enter, _) => {
            app.insert_newline();
        }
        (KeyCode::Backspace, _) => {
            app.delete_char_before_cursor();
        }
        (KeyCode::Delete, _) => {
            app.delete_char_at_cursor();
        }
        (KeyCode::Tab, _) => {
            app.insert_tab();
        }
        
        // Clipboard operations with modifiers
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.copy_selection();
        }
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            app.cut_selection();
        }
        (KeyCode::Char('v'), KeyModifiers::CONTROL) => {
            app.paste();
        }
        
        // Selection with Shift
        (KeyCode::Left, KeyModifiers::SHIFT) => {
            app.extend_selection_left();
        }
        (KeyCode::Right, KeyModifiers::SHIFT) => {
            app.extend_selection_right();
        }
        (KeyCode::Up, KeyModifiers::SHIFT) => {
            app.extend_selection_up();
        }
        (KeyCode::Down, KeyModifiers::SHIFT) => {
            app.extend_selection_down();
        }
        
        _ => {}
    }
}
```

### Mouse Events

Mouse event handling for interactive applications:

```rust
use crossterm::event::{Event, MouseEvent, MouseEventKind, MouseButton};

// First, enable mouse capture in terminal setup
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use crossterm::event::{EnableMouseCapture};

fn setup_terminal() -> Result<(), io::Error> {
    enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    Ok(())
}

// Then handle mouse events in the event loop
if let Event::Mouse(MouseEvent { kind, column, row, modifiers }) = event::read()? {
    match kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Start selection or position cursor
            app.set_cursor_position(row as usize, column as usize);
            app.start_selection(row as usize, column as usize);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // End selection
            app.end_selection(row as usize, column as usize);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Extend selection
            app.extend_selection(row as usize, column as usize);
        }
        MouseEventKind::ScrollDown => {
            // Scroll down
            app.scroll_down();
        }
        MouseEventKind::ScrollUp => {
            // Scroll up
            app.scroll_up();
        }
        _ => {}
    }
}

// Don't forget to disable mouse capture on cleanup
use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};
use crossterm::event::{DisableMouseCapture};

fn restore_terminal() -> Result<(), io::Error> {
    execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    disable_raw_mode()?;
    Ok(())
}
```

## Input Handling Architecture

For larger applications, it's best to separate input handling from the application logic:

```rust
enum InputMode {
    Normal,
    Insert,
    Visual,
}

struct App {
    input_mode: InputMode,
    // Other application state
}

impl App {
    fn handle_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode_input(event),
            InputMode::Insert => self.handle_insert_mode_input(event),
            InputMode::Visual => self.handle_visual_mode_input(event),
        }
    }
    
    fn handle_normal_mode_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Char('q') => {
                        return Ok(true); // Signal to quit
                    }
                    KeyCode::Char('i') => {
                        self.input_mode = InputMode::Insert;
                    }
                    KeyCode::Char('v') => {
                        self.input_mode = InputMode::Visual;
                    }
                    // Handle other normal mode keys
                    _ => {}
                }
            }
            // Handle other event types
            _ => {}
        }
        Ok(false)
    }
    
    fn handle_insert_mode_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Esc => {
                        self.input_mode = InputMode::Normal;
                    }
                    // Handle text input
                    KeyCode::Char(c) => {
                        self.insert_char(c);
                    }
                    // Handle other insert mode keys
                    _ => {}
                }
            }
            // Handle other event types
            _ => {}
        }
        Ok(false)
    }
    
    fn handle_visual_mode_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Esc => {
                        self.input_mode = InputMode::Normal;
                        self.clear_selection();
                    }
                    // Handle selection keys
                    KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => {
                        self.extend_selection(key.code);
                    }
                    // Handle operations on selection
                    KeyCode::Char('y') => {
                        self.copy_selection();
                        self.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char('d') => {
                        self.cut_selection();
                        self.input_mode = InputMode::Normal;
                    }
                    // Handle other visual mode keys
                    _ => {}
                }
            }
            // Handle other event types
            _ => {}
        }
        Ok(false)
    }
    
    // Application logic methods
    fn insert_char(&mut self, c: char) {
        // Implementation
    }
    
    fn extend_selection(&mut self, direction: KeyCode) {
        // Implementation
    }
    
    fn clear_selection(&mut self) {
        // Implementation
    }
    
    fn copy_selection(&mut self) {
        // Implementation
    }
    
    fn cut_selection(&mut self) {
        // Implementation
    }
}

// In the main event loop
let mut app = App::new();
loop {
    terminal.draw(|f| {
        // Render UI based on app state
    })?;
    
    if event::poll(Duration::from_millis(100))? {
        let event = event::read()?;
        if app.handle_input(event)? {
            break; // Quit was requested
        }
    }
}
```

## Input Focus

For interfaces with multiple input areas, you need to track which one has focus:

```rust
enum FocusArea {
    FileName,
    Editor,
    CommandLine,
}

struct App {
    focus: FocusArea,
    filename: String,
    editor_content: Vec<String>,
    command: String,
    // Other state
}

impl App {
    fn handle_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                // Global keys (regardless of focus)
                match key.code {
                    KeyCode::Tab => {
                        self.cycle_focus();
                        return Ok(false);
                    }
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(true); // Quit
                    }
                    _ => {}
                }
                
                // Focus-specific keys
                match self.focus {
                    FocusArea::FileName => self.handle_filename_input(key),
                    FocusArea::Editor => self.handle_editor_input(key),
                    FocusArea::CommandLine => self.handle_command_input(key),
                }
            }
            // Handle other event types
            _ => {}
        }
        Ok(false)
    }
    
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            FocusArea::FileName => FocusArea::Editor,
            FocusArea::Editor => FocusArea::CommandLine,
            FocusArea::CommandLine => FocusArea::FileName,
        };
    }
    
    fn handle_filename_input(&mut self, key: KeyEvent) -> Result<bool, io::Error> {
        // Handle input for filename field
        match key.code {
            KeyCode::Char(c) => {
                self.filename.push(c);
            }
            KeyCode::Backspace => {
                self.filename.pop();
            }
            // Other keys
            _ => {}
        }
        Ok(false)
    }
    
    // Similar handlers for other focus areas
}
```

## Command Handling

For applications with command interfaces (like vim-style commands):

```rust
struct App {
    command_mode: bool,
    command_buffer: String,
    // Other state
}

impl App {
    fn handle_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                if self.command_mode {
                    match key.code {
                        KeyCode::Esc => {
                            self.command_mode = false;
                            self.command_buffer.clear();
                        }
                        KeyCode::Enter => {
                            self.execute_command();
                            self.command_mode = false;
                            self.command_buffer.clear();
                        }
                        KeyCode::Char(c) => {
                            self.command_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            self.command_buffer.pop();
                        }
                        // Other command mode keys
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char(':') => {
                            self.command_mode = true;
                        }
                        // Other normal mode keys
                        _ => {}
                    }
                }
            }
            // Handle other event types
            _ => {}
        }
        Ok(false)
    }
    
    fn execute_command(&mut self) {
        let cmd = self.command_buffer.trim();
        
        match cmd {
            "q" | "quit" => {
                // Signal to quit
            }
            "w" | "write" => {
                // Save file
            }
            // Other commands
            _ => {
                // Unknown command
            }
        }
    }
}
```

## Modal Input

For vim-like modal interfaces:

```rust
enum Mode {
    Normal,
    Insert,
    Visual,
    Command,
}

struct App {
    mode: Mode,
    // Other state
}

impl App {
    fn handle_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match self.mode {
            Mode::Normal => self.handle_normal_mode_input(event),
            Mode::Insert => self.handle_insert_mode_input(event),
            Mode::Visual => self.handle_visual_mode_input(event),
            Mode::Command => self.handle_command_mode_input(event),
        }
    }
    
    // Separate handler methods for each mode
}
```

## Input for Text Editing

Detailed input handling for a text editor:

```rust
struct TextEditor {
    lines: Vec<String>,
    cursor: (usize, usize), // (row, column)
    scroll_offset: (usize, usize), // (row, column)
    selection: Option<((usize, usize), (usize, usize))>, // (start, end)
}

impl TextEditor {
    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            // Cursor movement
            KeyCode::Up => self.move_cursor_up(1),
            KeyCode::Down => self.move_cursor_down(1),
            KeyCode::Left => self.move_cursor_left(1),
            KeyCode::Right => self.move_cursor_right(1),
            
            KeyCode::Home => self.move_cursor_to_line_start(),
            KeyCode::End => self.move_cursor_to_line_end(),
            
            KeyCode::PageUp => self.move_cursor_up(10),
            KeyCode::PageDown => self.move_cursor_down(10),
            
            // Text editing
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Enter => self.insert_newline(),
            KeyCode::Tab => self.insert_tab(),
            KeyCode::Backspace => self.delete_char_before_cursor(),
            KeyCode::Delete => self.delete_char_at_cursor(),
            
            // Selection (with shift modifier)
            KeyCode::Left if key.modifiers.contains(KeyModifiers::SHIFT) => 
                self.extend_selection_left(),
            KeyCode::Right if key.modifiers.contains(KeyModifiers::SHIFT) => 
                self.extend_selection_right(),
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => 
                self.extend_selection_up(),
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => 
                self.extend_selection_down(),
            
            // Clipboard operations
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => 
                self.copy_selection(),
            KeyCode::Char('x') if key.modifiers.contains(KeyModifiers::CONTROL) => 
                self.cut_selection(),
            KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => 
                self.paste(),
            
            // Other keys
            _ => {}
        }
    }
    
    // Implementation of editor operations
    fn move_cursor_up(&mut self, count: usize) {
        // Implementation
    }
    
    fn move_cursor_down(&mut self, count: usize) {
        // Implementation
    }
    
    fn move_cursor_left(&mut self, count: usize) {
        // Implementation
    }
    
    fn move_cursor_right(&mut self, count: usize) {
        // Implementation
    }
    
    fn move_cursor_to_line_start(&mut self) {
        // Implementation
    }
    
    fn move_cursor_to_line_end(&mut self) {
        // Implementation
    }
    
    fn insert_char(&mut self, c: char) {
        // Implementation
    }
    
    fn insert_newline(&mut self) {
        // Implementation
    }
    
    fn insert_tab(&mut self) {
        // Implementation
    }
    
    fn delete_char_before_cursor(&mut self) {
        // Implementation
    }
    
    fn delete_char_at_cursor(&mut self) {
        // Implementation
    }
    
    fn extend_selection_left(&mut self) {
        // Implementation
    }
    
    fn extend_selection_right(&mut self) {
        // Implementation
    }
    
    fn extend_selection_up(&mut self) {
        // Implementation
    }
    
    fn extend_selection_down(&mut self) {
        // Implementation
    }
    
    fn copy_selection(&mut self) {
        // Implementation
    }
    
    fn cut_selection(&mut self) {
        // Implementation
    }
    
    fn paste(&mut self) {
        // Implementation
    }
}
```

## Handling KeySequences (Chords)

For complex key sequences (like Emacs or Vim):

```rust
struct KeySequence {
    keys: Vec<KeyEvent>,
    timeout: Duration,
    last_key_time: Instant,
}

impl KeySequence {
    fn new(timeout: Duration) -> Self {
        Self {
            keys: Vec::new(),
            timeout,
            last_key_time: Instant::now(),
        }
    }
    
    fn push(&mut self, key: KeyEvent) {
        let now = Instant::now();
        
        // Clear sequence if timeout has elapsed
        if now.duration_since(self.last_key_time) > self.timeout {
            self.keys.clear();
        }
        
        self.keys.push(key);
        self.last_key_time = now;
    }
    
    fn matches(&self, sequence: &[KeyEvent]) -> bool {
        if self.keys.len() != sequence.len() {
            return false;
        }
        
        self.keys.iter().zip(sequence.iter()).all(|(a, b)| {
            a.code == b.code && a.modifiers == b.modifiers
        })
    }
    
    fn clear(&mut self) {
        self.keys.clear();
    }
}

// Usage
let mut key_sequence = KeySequence::new(Duration::from_millis(500));

// In event loop
if let Event::Key(key) = event::read()? {
    key_sequence.push(key);
    
    // Check for known sequences
    if key_sequence.matches(&[
        KeyEvent { code: KeyCode::Char('g'), modifiers: KeyModifiers::NONE },
        KeyEvent { code: KeyCode::Char('g'), modifiers: KeyModifiers::NONE },
    ]) {
        // Handle 'gg' sequence (e.g., go to start of file)
        app.go_to_start();
        key_sequence.clear();
    } else if key_sequence.matches(&[
        KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::NONE },
        KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::NONE },
    ]) {
        // Handle 'dd' sequence (e.g., delete line)
        app.delete_current_line();
        key_sequence.clear();
    }
    
    // Handle regular keys
    // ...
}
```

## Clipboard Support

Clipboard operations in terminal applications:

```rust
use clipboard::{ClipboardContext, ClipboardProvider};

struct App {
    // Other state
    clipboard: Option<ClipboardContext>,
}

impl App {
    fn new() -> Self {
        Self {
            // Initialize other state
            clipboard: ClipboardProvider::new().ok(),
        }
    }
    
    fn copy_to_clipboard(&mut self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut ctx) = self.clipboard {
            ctx.set_contents(text.to_owned())?;
        }
        Ok(())
    }
    
    fn paste_from_clipboard(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(ref mut ctx) = self.clipboard {
            Ok(ctx.get_contents()?)
        } else {
            Ok(String::new())
        }
    }
    
    fn handle_input(&mut self, event: Event) -> Result<bool, io::Error> {
        match event {
            Event::Key(key) => {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        let selected_text = self.get_selected_text();
                        if !selected_text.is_empty() {
                            let _ = self.copy_to_clipboard(&selected_text);
                        }
                    }
                    KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        if let Ok(text) = self.paste_from_clipboard() {
                            self.insert_text(&text);
                        }
                    }
                    // Other keys
                    _ => {}
                }
            }
            // Other events
            _ => {}
        }
        Ok(false)
    }
    
    fn get_selected_text(&self) -> String {
        // Implementation
        String::new()
    }
    
    fn insert_text(&mut self, text: &str) {
        // Implementation
    }
}
```

## Input Validation and Error Handling

For form-like inputs with validation:

```rust
enum ValidationError {
    Empty,
    TooShort,
    TooLong,
    InvalidFormat,
    // Other validation errors
}

struct InputField {
    label: String,
    value: String,
    max_length: Option<usize>,
    min_length: Option<usize>,
    pattern: Option<regex::Regex>,
    error: Option<ValidationError>,
}

impl InputField {
    fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            value: String::new(),
            max_length: None,
            min_length: None,
            pattern: None,
            error: None,
        }
    }
    
    fn with_max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max);
        self
    }
    
    fn with_min_length(mut self, min: usize) -> Self {
        self.min_length = Some(min);
        self
    }
    
    fn with_pattern(mut self, pattern: &str) -> Self {
        self.pattern = regex::Regex::new(pattern).ok();
        self
    }
    
    fn handle_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                if let Some(max) = self.max_length {
                    if self.value.len() >= max {
                        self.error = Some(ValidationError::TooLong);
                        return;
                    }
                }
                self.value.push(c);
                self.validate();
            }
            KeyCode::Backspace => {
                self.value.pop();
                self.validate();
            }
            // Other keys
            _ => {}
        }
    }
    
    fn validate(&mut self) -> bool {
        if self.value.is_empty() {
            if self.min_length.is_some() {
                self.error = Some(ValidationError::Empty);
                return false;
            }
        }
        
        if let Some(min) = self.min_length {
            if self.value.len() < min {
                self.error = Some(ValidationError::TooShort);
                return false;
            }
        }
        
        if let Some(ref pattern) = self.pattern {
            if !pattern.is_match(&self.value) {
                self.error = Some(ValidationError::InvalidFormat);
                return false;
            }
        }
        
        self.error = None;
        true
    }
    
    fn is_valid(&self) -> bool {
        self.error.is_none()
    }
    
    fn get_error_message(&self) -> Option<String> {
        match self.error {
            Some(ValidationError::Empty) => Some("Field cannot be empty".to_string()),
            Some(ValidationError::TooShort) => {
                if let Some(min) = self.min_length {
                    Some(format!("Must be at least {} characters", min))
                } else {
                    Some("Input is too short".to_string())
                }
            }
            Some(ValidationError::TooLong) => {
                if let Some(max) = self.max_length {
                    Some(format!("Must be at most {} characters", max))
                } else {
                    Some("Input is too long".to_string())
                }
            }
            Some(ValidationError::InvalidFormat) => Some("Invalid format".to_string()),
            None => None,
        }
    }
}
```

## Best Practices for Input Handling

1. **Decouple input handling from rendering**: Keep input handling logic separate from rendering logic.
2. **Handle timeouts**: Use polling with timeouts to prevent blocking the UI.
3. **Provide feedback**: Show visual feedback for keypresses and errors.
4. **Support different input modes**: Implement modes for different input contexts (normal, insert, command, etc.).
5. **Handle focus**: Track which UI element has focus for input.
6. **Use command patterns**: For complex applications, use a command pattern to handle user actions.
7. **Implement clipboard support**: Provide clipboard operations for text editing.
8. **Add keyboard shortcuts**: Make common operations accessible via keyboard shortcuts.
9. **Validate input**: Validate user input and show appropriate error messages.
10. **Make input accessible**: Consider accessibility when designing input handling.

By following these patterns and best practices, you can create responsive and user-friendly input handling in your Ratatui applications.