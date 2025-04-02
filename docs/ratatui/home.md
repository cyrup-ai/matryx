# ratatui

Welcome to the user guide for the ratatui crate! This guide will help you build terminal user interfaces in Rust using ratatui.

## What is ratatui?

ratatui is a Rust library that provides widgets and utilities to build rich terminal user interfaces (TUIs) and dashboards.

## Features

- Extensive widgets: Paragraphs, Lists, Tables, Gauges, Sparklines, Bars, Calendars, Pop-ups, Layouts, and more!
- Interactive elements: Buttons, Checkboxes, Sliders, Radio Buttons, Form Fields
- Styling: Colors, Modifiers, Unicode Block, Gauge, Dot, Fade, Intensity, and Sparkline symbols
- Rendering: Supports real time, buffered, direct and non-direct rendering
- Extensible: Easy to create custom widgets and styles
- Clean architecture: Widgets are completely decoupled from the rendering target
- Zero allocation rendering: Widgets can be rendered without memory allocations
- Modular: Crossterm, Termion, termwiz, wezterm (more to come)
- Event handling: Mouse and keyboard events (via crossterm or termion)
- Terminal manipulation: Alternative screen, raw mode, clear
- Layout system: Split a terminal into multiple areas with different layouts

## Getting Started

To start with ratatui, add it as a dependency to your Cargo.toml file:

```toml
[dependencies]
ratatui = "0.25.0"
crossterm = "0.27.0"
```

Here's a simple "Hello, World!" example:

```rust
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

fn main() -> Result<(), io::Error> {
    // Setup terminal
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // Draw UI
    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default()
            .title("Hello, world!")
            .borders(Borders::ALL);
        let paragraph = Paragraph::new("Press 'q' to quit")
            .block(block);
        f.render_widget(paragraph, size);
    })?;

    // Wait for 'q' key press
    loop {
        if let Ok(true) = crossterm::event::poll(std::time::Duration::from_millis(100)) {
            if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                if key.code == crossterm::event::KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    
    Ok(())
}
```

## Architecture

ratatui is designed to be a flexible and extensible library. The main components are:

- **Backend**: Abstracts the terminal implementation. Supports multiple terminal libraries (crossterm, termion, etc.).
- **Terminal**: Main entry point for rendering. Takes a backend and provides methods to draw to the terminal.
- **Frame**: Represents a canvas that can be drawn upon during each render pass.
- **Widgets**: UI elements that can be rendered on a frame (Paragraph, List, Table, etc.).
- **Layout**: System to arrange widgets on the screen in complex layouts.
- **Style**: Appearance of widgets (colors, modifiers, etc.).

## Project History

ratatui is a maintained fork of [tui-rs](https://github.com/fdehau/tui-rs). The name is a play on "Rust TUI" (ra-ta-tui).

## Next Steps

Explore the examples and documentation to learn more about ratatui's capabilities. The user guide will walk you through building increasingly complex TUIs, from basic concepts to advanced techniques.