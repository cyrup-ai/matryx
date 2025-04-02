# Backend Integration in Ratatui

Ratatui is designed to be backend-agnostic, allowing you to use different terminal backends. This document covers how to integrate and work with the various supported backends.

## Supported Backends

Ratatui currently supports the following backends:

1. **Crossterm**: A cross-platform terminal library (Windows, macOS, Linux)
2. **Termion**: A pure Rust terminal library (Unix-only)
3. **Termwiz**: A terminal UI library primarily for Wezterm
4. **Wezterm**: A GPU-accelerated terminal emulator 

## Crossterm (Most Common)

Crossterm is the most widely used backend due to its cross-platform support.

### Setup with Crossterm

```rust
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crossterm::{
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{self, EnableMouseCapture, DisableMouseCapture, Event, KeyCode},
    execute,
};

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run application
    let res = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            // UI rendering code
        })?;

        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
```

### Crossterm Event Handling

```rust
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

fn handle_events() -> io::Result<bool> {
    if event::poll(std::time::Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(KeyEvent { code, modifiers, .. }) => {
                match (code, modifiers) {
                    (KeyCode::Char('q'), KeyModifiers::NONE) => {
                        return Ok(true); // Signal to quit
                    }
                    // Other key handling
                    _ => {}
                }
            }
            Event::Mouse(MouseEvent { kind, column, row, .. }) => {
                match kind {
                    MouseEventKind::Down(_) => {
                        // Handle mouse down event
                    }
                    MouseEventKind::Up(_) => {
                        // Handle mouse up event
                    }
                    MouseEventKind::Drag(_) => {
                        // Handle mouse drag event
                    }
                    // Other mouse event kinds
                    _ => {}
                }
            }
            Event::Resize(width, height) => {
                // Handle terminal resize
            }
            _ => {}
        }
    }
    
    Ok(false) // Continue running
}
```

### Additional Crossterm Features

```rust
use crossterm::{
    cursor,
    style::{Color as CrosstermColor, SetForegroundColor, SetBackgroundColor, Print, ResetColor},
    terminal::{Clear, ClearType},
};

// Direct terminal manipulation (outside of Ratatui draw)
fn direct_terminal_output() -> io::Result<()> {
    let mut stdout = io::stdout();
    
    // Position cursor and output colored text
    execute!(
        stdout,
        cursor::MoveTo(10, 5),
        SetForegroundColor(CrosstermColor::Red),
        Print("Direct terminal output"),
        ResetColor
    )?;
    
    // Clear part of the screen
    execute!(
        stdout,
        cursor::MoveTo(0, 10),
        Clear(ClearType::FromCursorDown)
    )?;
    
    Ok(())
}
```

## Termion

Termion is a pure Rust terminal manipulation library for Unix-like systems.

### Setup with Termion

```rust
use std::io;
use ratatui::{
    backend::TermionBackend,
    Terminal,
};
use termion::{
    raw::IntoRawMode,
    input::MouseTerminal,
    screen::AlternateScreen,
    event::Key,
};
use std::io::{Write, stdout};

fn main() -> Result<(), io::Error> {
    // Setup terminal
    let stdout = stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run application
    let res = run_app(&mut terminal);

    // Terminal is automatically restored when dropped

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            // UI rendering code
        })?;

        // Event handling for Termion is different from Crossterm
        // You would typically use termion::async_stdin() or similar
    }
}
```

### Termion Event Handling

```rust
use termion::event::Key;
use termion::input::TermRead;
use std::io::stdin;

fn handle_events() -> io::Result<bool> {
    let stdin = stdin();
    for evt in stdin.keys() {
        match evt? {
            Key::Char('q') => {
                return Ok(true); // Signal to quit
            }
            Key::Up => {
                // Handle up arrow
            }
            Key::Down => {
                // Handle down arrow
            }
            // Other key handling
            _ => {}
        }
    }
    
    Ok(false) // Continue running
}
```

## Termwiz

Termwiz is a terminal UI library primarily used with the Wezterm terminal.

### Setup with Termwiz

```rust
use std::io;
use ratatui::{
    backend::TermwizBackend,
    Terminal,
};
use termwiz::caps::Capabilities;
use termwiz::surface::Surface;

fn main() -> Result<(), io::Error> {
    // Setup terminal with Termwiz
    let caps = Capabilities::new_from_env()?;
    let surface = Surface::new(caps.clone());
    let backend = TermwizBackend::new(surface);
    let mut terminal = Terminal::new(backend)?;

    // Run application
    let res = run_app(&mut terminal);

    // Terminal cleanup should be handled as needed

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            // UI rendering code
        })?;

        // Event handling for Termwiz would be implemented here
    }
}
```

## Wezterm

Wezterm provides a GPU-accelerated terminal backend.

### Setup with Wezterm

```rust
use std::io;
use ratatui::{
    backend::WezTermBackend,
    Terminal,
};
use wezterm_term::{Terminal as WezTerminal, TerminalSize};

fn main() -> Result<(), io::Error> {
    // Create a Wezterm terminal
    let size = TerminalSize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };
    let mut wez_terminal = WezTerminal::new(size, None);
    
    // Setup Ratatui with Wezterm backend
    let backend = WezTermBackend::new(wez_terminal);
    let mut terminal = Terminal::new(backend)?;

    // Run application
    let res = run_app(&mut terminal);

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            // UI rendering code
        })?;

        // Event handling would need to be implemented separately
    }
}
```

## Writing a Custom Backend

If you need to integrate with a different terminal library, you can implement your own backend by implementing the `Backend` trait:

```rust
use ratatui::backend::Backend;
use ratatui::buffer::Cell;
use ratatui::layout::Rect;
use std::io;

struct MyCustomBackend {
    // Your backend-specific fields
}

impl Backend for MyCustomBackend {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        for (x, y, cell) in content {
            // Translate Ratatui cells to your backend's representation
            // and draw them to your terminal
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        // Implement cursor hiding
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        // Implement cursor showing
        Ok(())
    }

    fn get_cursor(&mut self) -> io::Result<(u16, u16)> {
        // Return the current cursor position
        Ok((0, 0))
    }

    fn set_cursor(&mut self, x: u16, y: u16) -> io::Result<()> {
        // Set the cursor position
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        // Clear the terminal
        Ok(())
    }

    fn size(&self) -> io::Result<Rect> {
        // Return the terminal size
        Ok(Rect::new(0, 0, 80, 24))
    }

    fn flush(&mut self) -> io::Result<()> {
        // Flush any buffered output
        Ok(())
    }
}
```

## Terminal Setup Best Practices

### Error Handling

Always ensure proper terminal cleanup, even when errors occur:

```rust
fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run application with error handling
    let app_result = run_app(&mut terminal);

    // Restore terminal regardless of application result
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Now handle any application errors
    if let Err(err) = app_result {
        println!("Application error: {:?}", err);
    }

    Ok(())
}
```

### Using the Drop Trait for Cleanup

You can use Rust's `Drop` trait to ensure terminal cleanup:

```rust
struct AppCleanup {
    terminal_restored: bool,
}

impl AppCleanup {
    fn new() -> Self {
        Self {
            terminal_restored: false,
        }
    }

    fn restore_terminal(&mut self) -> io::Result<()> {
        if !self.terminal_restored {
            disable_raw_mode()?;
            execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture
            )?;
            self.terminal_restored = true;
        }
        Ok(())
    }
}

impl Drop for AppCleanup {
    fn drop(&mut self) {
        let _ = self.restore_terminal();
    }
}

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create cleanup handler
    let mut cleanup = AppCleanup::new();

    // Run application
    let app_result = run_app(&mut terminal);

    // Explicitly restore terminal
    cleanup.restore_terminal()?;

    // Handle application errors
    if let Err(err) = app_result {
        println!("Application error: {:?}", err);
    }

    Ok(())
}
```

## Backend-Specific Features

### Crossterm-Specific

#### Async Event Handling with Crossterm

```rust
use tokio::select;
use tokio::time::{sleep, Duration};
use crossterm::event::{self, Event};

async fn handle_events_async() -> io::Result<bool> {
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    
    loop {
        select! {
            _ = interval.tick() => {
                // Check for events with non-blocking poll
                if event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) => {
                            if key.code == KeyCode::Char('q') {
                                return Ok(true); // Quit
                            }
                            // Handle other keys
                        }
                        // Handle other events
                        _ => {}
                    }
                }
            }
            
            // Handle other async operations
        }
        
        // Update application state
        
        // Return false to continue running
        return Ok(false);
    }
}
```

#### Crossterm Signals

```rust
use crossterm::event::{self, Event, EventStream};
use futures::{FutureExt, StreamExt};
use tokio::select;

async fn handle_events_with_signals() -> io::Result<bool> {
    let mut reader = EventStream::new();
    
    #[cfg(unix)]
    let mut sigwinch = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::window_change())?;
    
    loop {
        select! {
            maybe_event = reader.next().fuse() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => {
                        if key.code == KeyCode::Char('q') {
                            return Ok(true); // Quit
                        }
                        // Handle other keys
                    }
                    Some(Ok(Event::Resize(width, height))) => {
                        // Handle resize
                    }
                    // Handle other events
                    _ => {}
                }
            },
            
            #[cfg(unix)]
            _ = sigwinch.recv().fuse() => {
                // Terminal was resized
                if let Ok((w, h)) = crossterm::terminal::size() {
                    // Handle resize
                }
            }
        }
        
        // Return false to continue running
        return Ok(false);
    }
}
```

### Termion-Specific

#### Async Event Handling with Termion

```rust
use termion::async_stdin;
use tokio::time::{sleep, Duration};
use termion::event::Key;
use termion::input::TermRead;

async fn handle_events_async_termion() -> io::Result<bool> {
    let mut stdin = async_stdin().keys();
    
    loop {
        // Non-blocking check for keys
        if let Some(key) = stdin.next() {
            match key? {
                Key::Char('q') => {
                    return Ok(true); // Quit
                }
                // Handle other keys
                _ => {}
            }
        }
        
        // Sleep a bit to avoid CPU hogging
        sleep(Duration::from_millis(10)).await;
        
        // Return false to continue running
        return Ok(false);
    }
}
```

## Mouse Support

### Crossterm Mouse Support

```rust
use crossterm::event::{EnableMouseCapture, DisableMouseCapture, MouseEventKind, MouseButton};

// In terminal setup
execute!(stdout, EnableMouseCapture)?;

// In event handling
if let Event::Mouse(mouse) = event::read()? {
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Handle left click at (mouse.column, mouse.row)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Handle drag
        }
        MouseEventKind::Up(MouseButton::Left) => {
            // Handle release
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

// In terminal cleanup
execute!(stdout, DisableMouseCapture)?;
```

### Termion Mouse Support

```rust
use termion::input::MouseTerminal;
use termion::event::{MouseEvent, MouseButton};

// In terminal setup
let stdout = MouseTerminal::from(stdout().into_raw_mode()?);

// In event handling
if let Event::Mouse(me) = termion_event {
    match me {
        MouseEvent::Press(MouseButton::Left, x, y) => {
            // Handle left click at (x, y)
        }
        MouseEvent::Hold(x, y) => {
            // Handle drag at (x, y)
        }
        MouseEvent::Release(x, y) => {
            // Handle release at (x, y)
        }
        _ => {}
    }
}
```

## Testing with Mock Backends

For testing without a real terminal, you can create a mock backend:

```rust
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use ratatui::buffer::Buffer;

#[test]
fn test_ui_rendering() {
    // Create a test backend with a specific size
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    
    // Draw UI
    terminal.draw(|f| {
        // Render UI components
    }).unwrap();
    
    // Get the buffer that would be rendered
    let buffer = terminal.backend().buffer().clone();
    
    // Check specific cells in the buffer
    assert_eq!(buffer.get(0, 0).symbol, "H");
    assert_eq!(buffer.get(1, 0).symbol, "e");
    assert_eq!(buffer.get(2, 0).symbol, "l");
    
    // Or check regions of the buffer
    for y in 5..10 {
        for x in 5..10 {
            assert_eq!(buffer.get(x, y).bg, Color::Blue);
        }
    }
}
```

## Backend Feature Compatibility

When designing your application to work with multiple backends, be aware of the differences:

1. **Mouse Support**: Available in all supported backends, but with different APIs
2. **Color Support**: Varies by backend and terminal
3. **Event Handling**: Significantly different between backends
4. **Unicode Support**: Varies by terminal, not just backend

### Feature Detection

You can detect terminal capabilities at runtime:

```rust
fn detect_terminal_features() -> io::Result<()> {
    // Check color support (crossterm example)
    let color_support = crossterm::style::available_color_support()?;
    let supports_rgb = color_support.has_rgb();
    let supports_indexed = color_support.has_indexed();
    
    // Check terminal size
    let (width, height) = crossterm::terminal::size()?;
    
    // Check for specific environment variables
    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    
    // Adjust application behavior based on detected features
    
    Ok(())
}
```

## Conclusion

When integrating backends with Ratatui, choose the backend that best fits your requirements:

- **Crossterm** for cross-platform applications
- **Termion** for Unix-only applications with minimal dependencies
- **Termwiz** or **Wezterm** for advanced rendering features

Ensure proper terminal setup and cleanup, and be aware of the differences in feature support between backends. By understanding these integration patterns, you can create robust terminal applications that work across various environments.