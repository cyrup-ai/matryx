# Event Handling in Ratatui

Ratatui doesn't include direct event handling as this can vary depending on the backend you're using. Most commonly, events are handled using the backend's event system (such as crossterm or termion).

## Event Handling with Crossterm

Here's how to set up basic event handling with crossterm:

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
        
        // Poll for events with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Up => {
                        // Handle up arrow
                    }
                    KeyCode::Down => {
                        // Handle down arrow
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

## Types of Events

### Keyboard Events

```rust
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
    match (code, modifiers) {
        // Ctrl+C
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            break;
        }
        // Arrow keys
        (KeyCode::Up, _) => {
            // Move cursor up
        }
        (KeyCode::Down, _) => {
            // Move cursor down
        }
        (KeyCode::Left, _) => {
            // Move cursor left
        }
        (KeyCode::Right, _) => {
            // Move cursor right
        }
        // Function keys
        (KeyCode::F(n), _) => {
            // Handle function key F1-F12
        }
        // Standard characters
        (KeyCode::Char(c), _) => {
            // Handle character input
        }
        // Special keys
        (KeyCode::Enter, _) => {
            // Handle Enter key
        }
        (KeyCode::Backspace, _) => {
            // Handle Backspace key
        }
        (KeyCode::Delete, _) => {
            // Handle Delete key
        }
        (KeyCode::Tab, _) => {
            // Handle Tab key
        }
        (KeyCode::Esc, _) => {
            // Handle Escape key
        }
        _ => {}
    }
}
```

### Mouse Events

```rust
use crossterm::event::{Event, MouseEvent, MouseEventKind};

if let Event::Mouse(MouseEvent { kind, column, row, modifiers }) = event::read()? {
    match kind {
        MouseEventKind::Down(_) => {
            // Handle mouse button down at (column, row)
        }
        MouseEventKind::Up(_) => {
            // Handle mouse button up at (column, row)
        }
        MouseEventKind::Drag(_) => {
            // Handle mouse drag at (column, row)
        }
        MouseEventKind::Moved => {
            // Handle mouse move at (column, row)
        }
        MouseEventKind::ScrollDown => {
            // Handle scroll down
        }
        MouseEventKind::ScrollUp => {
            // Handle scroll up
        }
        _ => {}
    }
}
```

### Window Resize Events

```rust
use crossterm::event::Event;

if let Event::Resize(width, height) = event::read()? {
    // Handle resize event
    // Redraw UI with new dimensions
}
```

## Event Loop Patterns

### Blocking Loop

```rust
loop {
    terminal.draw(|f| {
        // UI rendering
    })?;
    
    // Wait for event (blocking)
    match event::read()? {
        Event::Key(key) => {
            // Handle key event
        }
        Event::Mouse(mouse) => {
            // Handle mouse event
        }
        Event::Resize(width, height) => {
            // Handle resize event
        }
        _ => {}
    }
}
```

### Non-blocking Loop with Polling

```rust
loop {
    terminal.draw(|f| {
        // UI rendering
    })?;
    
    // Poll for events with a timeout
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) => {
                // Handle key event
            }
            Event::Mouse(mouse) => {
                // Handle mouse event
            }
            Event::Resize(width, height) => {
                // Handle resize event
            }
            _ => {}
        }
    }
    
    // Do other work when no events are pending
    // For example, update timers, animations, etc.
}
```

### Tick-based Loop

```rust
use std::time::{Duration, Instant};

// Set a tick rate (e.g., 250ms)
let tick_rate = Duration::from_millis(250);
let mut last_tick = Instant::now();

loop {
    // Wait for events, but timeout after tick rate
    let timeout = tick_rate
        .checked_sub(last_tick.elapsed())
        .unwrap_or_else(|| Duration::from_secs(0));
        
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => {
                // Handle key event
            }
            // Handle other events
            _ => {}
        }
    }
    
    // Check if it's time for a tick
    if last_tick.elapsed() >= tick_rate {
        // Perform tick operations (update state, etc.)
        last_tick = Instant::now();
    }
    
    terminal.draw(|f| {
        // UI rendering
    })?;
}
```

## Enabling Mouse Support

To capture mouse events, you need to enable it explicitly:

```rust
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use crossterm::event::{EnableMouseCapture};
use std::io;

fn setup_terminal() -> Result<(), io::Error> {
    enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableMouseCapture
    )?;
    Ok(())
}

// Don't forget to disable mouse capture on exit
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

## Clipboard Considerations

When mouse capture is enabled, it can interfere with the terminal's built-in clipboard functionality. Consider:

1. Providing a way to toggle mouse capture when text selection is needed
2. Implementing custom clipboard functionality using libraries like `clipboard` or `arboard`
3. Offering keyboard shortcuts for clipboard operations (copy, cut, paste)

## Advanced Event Handling

For more complex applications, consider creating a dedicated event handler:

```rust
struct AppState {
    running: bool,
    counter: u8,
    // Other application state
}

impl AppState {
    fn new() -> Self {
        Self {
            running: true,
            counter: 0,
        }
    }
    
    fn handle_event(&mut self, event: Event) -> Result<(), io::Error> {
        match event {
            Event::Key(key) => self.handle_key_event(key),
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            Event::Resize(width, height) => self.handle_resize(width, height),
            _ => Ok(()),
        }
    }
    
    fn handle_key_event(&mut self, key: KeyEvent) -> Result<(), io::Error> {
        match key.code {
            KeyCode::Char('q') => {
                self.running = false;
            }
            KeyCode::Char('+') => {
                self.counter = self.counter.saturating_add(1);
            }
            KeyCode::Char('-') => {
                self.counter = self.counter.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }
    
    fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Result<(), io::Error> {
        // Handle mouse events
        Ok(())
    }
    
    fn handle_resize(&mut self, width: u16, height: u16) -> Result<(), io::Error> {
        // Handle resize event
        Ok(())
    }
}
```

Then in your main loop:

```rust
let mut app = AppState::new();

while app.running {
    terminal.draw(|f| {
        // Draw UI based on app state
    })?;
    
    if event::poll(Duration::from_millis(100))? {
        let event = event::read()?;
        app.handle_event(event)?;
    }
}
```

This approach allows for clean separation between event handling logic and rendering logic.