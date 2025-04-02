# Advanced Topics in Ratatui

This document covers advanced topics and techniques for building sophisticated terminal user interfaces with Ratatui.

## Custom Rendering

While Ratatui provides many built-in widgets, sometimes you need to render custom elements. You can do this by directly manipulating the buffer:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

struct CustomWidget;

impl Widget for CustomWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Skip rendering if the area is too small
        if area.width < 3 || area.height < 3 {
            return;
        }
        
        // Draw a custom shape (e.g., a circle)
        let center_x = area.x + area.width / 2;
        let center_y = area.y + area.height / 2;
        let radius = (area.width.min(area.height) / 2).saturating_sub(1);
        
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                let dx = (x as i32 - center_x as i32).abs() as u16;
                let dy = (y as i32 - center_y as i32).abs() as u16;
                
                // Simple distance check for a circle
                if dx * dx + dy * dy <= radius * radius {
                    buf.get_mut(x, y).set_style(Style::default().bg(Color::Red));
                }
            }
        }
    }
}
```

## Custom Borders and Symbols

You can create custom border styles for your widgets:

```rust
use ratatui::widgets::{Block, Borders, BorderType};

// Using built-in border types
let plain_border = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Plain);

let rounded_border = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Rounded);

let double_border = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Double);

let thick_border = Block::default()
    .borders(Borders::ALL)
    .border_type(BorderType::Thick);

// Custom border symbols
use ratatui::symbols::{border, line};

let custom_border = Block::default()
    .borders(Borders::ALL)
    .border_set(border::ROUNDED);

// You can also define completely custom border sets
use ratatui::symbols::border::Set as BorderSet;

let my_border_set = BorderSet {
    top_left: '╔',
    top_right: '╗',
    bottom_left: '╚',
    bottom_right: '╝',
    horizontal: '═',
    vertical: '║',
    top_t: '╦',
    bottom_t: '╩',
    left_t: '╠',
    right_t: '╣',
    cross: '╬',
};

let custom_border = Block::default()
    .borders(Borders::ALL)
    .border_set(my_border_set);
```

## Triple-Buffering Technique

For smoother rendering, especially for animations or frequent updates, you can use a triple-buffering technique:

```rust
use ratatui::{
    backend::Backend,
    buffer::Buffer,
    layout::Rect,
    Terminal,
};

struct TripleBufferedTerminal<B: Backend> {
    terminal: Terminal<B>,
    front_buffer: Buffer,
    back_buffer: Buffer,
}

impl<B: Backend> TripleBufferedTerminal<B> {
    fn new(terminal: Terminal<B>) -> Self {
        let size = terminal.size().unwrap_or(Rect::new(0, 0, 0, 0));
        Self {
            terminal,
            front_buffer: Buffer::empty(size),
            back_buffer: Buffer::empty(size),
        }
    }
    
    fn draw<F>(&mut self, render_fn: F) -> std::io::Result<()>
    where
        F: FnOnce(&mut Buffer),
    {
        let size = self.terminal.size()?;
        
        // Resize buffers if terminal size has changed
        if self.back_buffer.area.width != size.width || self.back_buffer.area.height != size.height {
            self.back_buffer = Buffer::empty(size);
            self.front_buffer = Buffer::empty(size);
        }
        
        // Render into back buffer
        render_fn(&mut self.back_buffer);
        
        // Swap buffers and render only the differences
        self.terminal.draw(|f| {
            let area = f.size();
            for y in 0..area.height {
                for x in 0..area.width {
                    let back_cell = self.back_buffer.get(x, y);
                    let front_cell = self.front_buffer.get(x, y);
                    
                    if back_cell != front_cell {
                        f.buffer_mut().get_mut(x, y).clone_from(back_cell);
                    }
                }
            }
            
            // Update front buffer
            std::mem::swap(&mut self.front_buffer, &mut self.back_buffer);
        })
    }
}
```

## Asynchronous Rendering

For applications that need to handle I/O or other tasks without blocking the UI:

```rust
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use std::io;
use crossterm::event::{self, Event};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

enum UiEvent {
    Tick,
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
    AsyncTask(String),
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Terminal setup code...
    
    let (tx, mut rx) = mpsc::channel(32);
    let tick_tx = tx.clone();
    let event_tx = tx.clone();
    let async_tx = tx.clone();
    
    // Spawn tick task
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            if tick_tx.send(UiEvent::Tick).await.is_err() {
                break;
            }
        }
    });
    
    // Spawn input handling task
    tokio::spawn(async move {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                match event::read().unwrap() {
                    Event::Key(key) => {
                        if event_tx.send(UiEvent::Key(key)).await.is_err() {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        if event_tx.send(UiEvent::Mouse(mouse)).await.is_err() {
                            break;
                        }
                    }
                    Event::Resize(width, height) => {
                        if event_tx.send(UiEvent::Resize(width, height)).await.is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    
    // Spawn a simulated async task
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(2));
        let mut counter = 0;
        loop {
            interval.tick().await;
            counter += 1;
            if async_tx.send(UiEvent::AsyncTask(format!("Task result {}", counter))).await.is_err() {
                break;
            }
        }
    });
    
    // Application state
    let mut app_state = AppState::default();
    
    // Main event loop
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    loop {
        // Render UI
        terminal.draw(|f| {
            // Render app state to UI
            render_ui(f, &app_state);
        })?;
        
        // Handle events
        if let Some(event) = rx.recv().await {
            match event {
                UiEvent::Tick => {
                    app_state.update_on_tick();
                }
                UiEvent::Key(key) => {
                    if app_state.handle_key(key) {
                        break; // Exit requested
                    }
                }
                UiEvent::Mouse(mouse) => {
                    app_state.handle_mouse(mouse);
                }
                UiEvent::Resize(width, height) => {
                    app_state.handle_resize(width, height);
                }
                UiEvent::AsyncTask(result) => {
                    app_state.handle_async_result(result);
                }
            }
        }
    }
    
    // Terminal cleanup code...
    
    Ok(())
}
```

## Custom Scrollable Widgets

Creating scrollable widgets that handle larger content than their visible area:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, StatefulWidget, Widget},
};

struct ScrollableText {
    text: String,
    block: Option<Block>,
    style: Style,
}

struct ScrollableTextState {
    offset: usize,
    max_scroll: usize,
}

impl ScrollableText {
    fn new<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            block: None,
            style: Style::default(),
        }
    }
    
    fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }
    
    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl StatefulWidget for ScrollableText {
    type State = ScrollableTextState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Get area to render the text
        let text_area = match self.block {
            Some(block) => {
                let inner_area = block.inner(area);
                block.render(area, buf);
                inner_area
            }
            None => area,
        };
        
        if text_area.width == 0 || text_area.height == 0 {
            return;
        }
        
        // Split text into lines
        let lines: Vec<&str> = self.text.split('\n').collect();
        
        // Calculate maximum scroll offset
        state.max_scroll = lines.len().saturating_sub(text_area.height as usize);
        
        // Ensure scroll offset is valid
        if state.offset > state.max_scroll {
            state.offset = state.max_scroll;
        }
        
        // Render visible lines
        for (i, line) in lines.iter().skip(state.offset).take(text_area.height as usize).enumerate() {
            let y = text_area.y + i as u16;
            
            // Handle lines longer than the width
            if line.len() > text_area.width as usize {
                buf.set_string(
                    text_area.x,
                    y,
                    &line[..text_area.width as usize],
                    self.style,
                );
            } else {
                buf.set_string(text_area.x, y, line, self.style);
            }
        }
    }
}

// Usage:
// let scrollable = ScrollableText::new(long_text)
//     .block(Block::default().title("Scrollable").borders(Borders::ALL));
// let mut state = ScrollableTextState { offset: 0, max_scroll: 0 };
// f.render_stateful_widget(scrollable, area, &mut state);
//
// // To scroll:
// if key.code == KeyCode::Down && state.offset < state.max_scroll {
//     state.offset += 1;
// } else if key.code == KeyCode::Up && state.offset > 0 {
//     state.offset -= 1;
// }
```

## Custom List with Search Functionality

Implementing a searchable list widget:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, StatefulWidget, Widget},
};

struct SearchableList<T> {
    items: Vec<T>,
    block: Option<Block>,
    style: Style,
    highlight_style: Style,
}

struct SearchableListState {
    selected: Option<usize>,
    offset: usize,
    search_query: String,
    search_active: bool,
}

impl<T: AsRef<str>> SearchableList<T> {
    fn new(items: Vec<T>) -> Self {
        Self {
            items,
            block: None,
            style: Style::default(),
            highlight_style: Style::default().fg(Color::Yellow),
        }
    }
    
    fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }
    
    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    
    fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }
}

impl<T: AsRef<str>> StatefulWidget for SearchableList<T> {
    type State = SearchableListState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Get area to render the list
        let list_area = match self.block {
            Some(block) => {
                let inner_area = block.inner(area);
                block.render(area, buf);
                inner_area
            }
            None => area,
        };
        
        if list_area.width == 0 || list_area.height == 0 {
            return;
        }
        
        // Filter items based on search query
        let filtered_items: Vec<&T> = if state.search_active && !state.search_query.is_empty() {
            self.items.iter()
                .filter(|item| {
                    item.as_ref().to_lowercase().contains(&state.search_query.to_lowercase())
                })
                .collect()
        } else {
            self.items.iter().collect()
        };
        
        // Adjust selection if filtered list is smaller
        if let Some(selected) = state.selected {
            if selected >= filtered_items.len() {
                state.selected = if filtered_items.is_empty() {
                    None
                } else {
                    Some(filtered_items.len() - 1)
                };
            }
        }
        
        // Calculate visible items
        let max_display = list_area.height as usize;
        
        // Adjust offset if necessary
        if let Some(selected) = state.selected {
            if selected >= state.offset + max_display {
                state.offset = selected - max_display + 1;
            } else if selected < state.offset {
                state.offset = selected;
            }
        }
        
        // Render search bar if active
        let mut y_offset = 0;
        if state.search_active {
            let search_prompt = format!("Search: {}", state.search_query);
            buf.set_string(
                list_area.x,
                list_area.y,
                &search_prompt,
                Style::default().fg(Color::Yellow),
            );
            y_offset = 1;
        }
        
        // Render visible items
        for (i, item) in filtered_items.iter()
            .skip(state.offset)
            .take(max_display.saturating_sub(y_offset))
            .enumerate()
        {
            let y = list_area.y + i as u16 + y_offset;
            let style = if Some(state.offset + i) == state.selected {
                self.highlight_style
            } else {
                self.style
            };
            
            let item_str = item.as_ref();
            if item_str.len() > list_area.width as usize {
                buf.set_string(
                    list_area.x,
                    y,
                    &item_str[..list_area.width as usize],
                    style,
                );
            } else {
                buf.set_string(list_area.x, y, item_str, style);
            }
        }
    }
}

// Usage:
// let items = vec!["Item 1", "Item 2", "Item 3", ...];
// let list = SearchableList::new(items)
//     .block(Block::default().title("List").borders(Borders::ALL));
// let mut state = SearchableListState {
//     selected: Some(0),
//     offset: 0,
//     search_query: String::new(),
//     search_active: false,
// };
//
// // Handle input for search:
// match key.code {
//     KeyCode::Char('/') => {
//         state.search_active = true;
//     }
//     KeyCode::Esc => {
//         state.search_active = false;
//         state.search_query.clear();
//     }
//     KeyCode::Char(c) if state.search_active => {
//         state.search_query.push(c);
//     }
//     KeyCode::Backspace if state.search_active => {
//         state.search_query.pop();
//     }
//     // Navigation keys:
//     KeyCode::Down => { /* move selection down */ }
//     KeyCode::Up => { /* move selection up */ }
//     // ...
// }
```

## Animations and Transitions

Creating smooth animations in Ratatui:

```rust
use std::time::{Duration, Instant};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

struct AnimatedWidget {
    start_time: Instant,
    duration: Duration,
    style: Style,
}

impl AnimatedWidget {
    fn new(duration_ms: u64) -> Self {
        Self {
            start_time: Instant::now(),
            duration: Duration::from_millis(duration_ms),
            style: Style::default(),
        }
    }
    
    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for AnimatedWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let elapsed = self.start_time.elapsed();
        
        // Calculate progress (0.0 to 1.0)
        let progress = if elapsed >= self.duration {
            1.0
        } else {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        };
        
        // Example: Animate width of a rectangle
        let width = (progress * area.width as f32) as u16;
        
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + width {
                buf.get_mut(x, y).set_style(self.style);
            }
        }
    }
}

// Usage in a loop:
// let mut last_tick = Instant::now();
// let tick_rate = Duration::from_millis(16); // ~60 FPS
//
// loop {
//     let now = Instant::now();
//     if now.duration_since(last_tick) >= tick_rate {
//         terminal.draw(|f| {
//             let animated = AnimatedWidget::new(1000) // 1 second animation
//                 .style(Style::default().bg(Color::Blue));
//             f.render_widget(animated, area);
//         })?;
//         
//         last_tick = now;
//     }
//     
//     // Handle input events...
// }
```

## Custom Progress Bar with Animation

An animated progress bar with custom styling:

```rust
use std::time::Instant;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

struct AnimatedProgressBar {
    progress: f32, // 0.0 to 1.0
    start_time: Instant,
    style: Style,
    fill_char: char,
    pulse: bool,
}

impl AnimatedProgressBar {
    fn new(progress: f32) -> Self {
        Self {
            progress: progress.max(0.0).min(1.0),
            start_time: Instant::now(),
            style: Style::default(),
            fill_char: '█',
            pulse: false,
        }
    }
    
    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    
    fn fill_char(mut self, c: char) -> Self {
        self.fill_char = c;
        self
    }
    
    fn pulse(mut self, pulse: bool) -> Self {
        self.pulse = pulse;
        self
    }
}

impl Widget for AnimatedProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Skip rendering if the area is too small
        if area.width == 0 || area.height == 0 {
            return;
        }
        
        // Calculate the filled width based on progress
        let mut filled_width = (self.progress * area.width as f32) as u16;
        
        // If pulsing, animate a moving segment
        if self.pulse {
            let elapsed_ms = self.start_time.elapsed().as_millis() as u32;
            let cycle_ms = 1000; // 1 second for full cycle
            let position = (elapsed_ms % cycle_ms) as f32 / cycle_ms as f32;
            
            // Create a moving highlight
            let pulse_width = (area.width / 5).max(1);
            let pulse_position = (position * (area.width + pulse_width) as f32) as u16;
            
            // Render background
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    let in_pulse_area = x >= area.x + pulse_position.saturating_sub(pulse_width) 
                        && x < area.x + pulse_position.min(area.width);
                    
                    let style = if in_pulse_area {
                        self.style
                    } else {
                        self.style.fg(Color::DarkGray)
                    };
                    
                    buf.get_mut(x, y).set_char(self.fill_char).set_style(style);
                }
            }
        } else {
            // Standard progress bar rendering
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    if x < area.x + filled_width {
                        buf.get_mut(x, y).set_char(self.fill_char).set_style(self.style);
                    } else {
                        buf.get_mut(x, y)
                            .set_char(self.fill_char)
                            .set_style(Style::default().fg(Color::DarkGray));
                    }
                }
            }
        }
    }
}
```

## Handling Custom Colors

Working with custom colors in terminals with different capabilities:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

enum ColorSupport {
    Basic,      // 8 colors
    Extended,   // 16 colors
    Indexed,    // 256 colors
    RGB,        // 24-bit/true color
}

fn detect_color_support() -> ColorSupport {
    // This is a simplification - actual detection would be more complex
    // and might involve checking terminal capabilities or environment variables
    let term = std::env::var("TERM").unwrap_or_default();
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    
    if colorterm == "truecolor" || colorterm == "24bit" {
        ColorSupport::RGB
    } else if term.contains("256color") {
        ColorSupport::Indexed
    } else if term.ends_with("-color") {
        ColorSupport::Extended
    } else {
        ColorSupport::Basic
    }
}

fn get_adapted_color(color: Color, support: &ColorSupport) -> Color {
    match color {
        Color::Rgb(r, g, b) => match support {
            ColorSupport::RGB => color,
            ColorSupport::Indexed => {
                // Convert RGB to closest 256-color index
                // This is a simplified conversion
                if r == g && g == b {
                    // Grayscale
                    if r < 8 {
                        Color::Black
                    } else if r < 128 {
                        Color::DarkGray
                    } else if r < 240 {
                        Color::Gray
                    } else {
                        Color::White
                    }
                } else {
                    // Find closest basic color
                    if r > g && r > b {
                        if r > 128 { Color::LightRed } else { Color::Red }
                    } else if g > r && g > b {
                        if g > 128 { Color::LightGreen } else { Color::Green }
                    } else {
                        if b > 128 { Color::LightBlue } else { Color::Blue }
                    }
                }
            }
            ColorSupport::Extended | ColorSupport::Basic => {
                // Convert to one of the basic 8/16 colors
                if r > 192 && g > 192 && b > 192 {
                    Color::White
                } else if r < 64 && g < 64 && b < 64 {
                    Color::Black
                } else if r > g && r > b {
                    if support == &ColorSupport::Extended && r > 128 {
                        Color::LightRed
                    } else {
                        Color::Red
                    }
                } else if g > r && g > b {
                    if support == &ColorSupport::Extended && g > 128 {
                        Color::LightGreen
                    } else {
                        Color::Green
                    }
                } else {
                    if support == &ColorSupport::Extended && b > 128 {
                        Color::LightBlue
                    } else {
                        Color::Blue
                    }
                }
            }
        },
        _ => color,
    }
}

struct ColorAdaptingWidget {
    inner: Box<dyn Widget>,
    color_support: ColorSupport,
}

impl ColorAdaptingWidget {
    fn new<W: Widget + 'static>(widget: W) -> Self {
        Self {
            inner: Box::new(widget),
            color_support: detect_color_support(),
        }
    }
}

// Note: This is conceptual - actual implementation would be more complex
// as we don't have direct access to modify the colors during render
impl Widget for ColorAdaptingWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Render the inner widget
        self.inner.render(area, buf);
        
        // This would need to adapt colors in the buffer
        // but actual implementation is more complex
    }
}
```

## Creating Complex UI Patterns

### Tabs with Content

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Spans,
    widgets::{Block, Borders, Tabs, Widget},
    Frame,
};

struct TabsWithContent<'a> {
    titles: Vec<&'a str>,
    selected: usize,
}

impl<'a> TabsWithContent<'a> {
    fn new(titles: Vec<&'a str>, selected: usize) -> Self {
        Self {
            titles,
            selected: selected.min(titles.len().saturating_sub(1)),
        }
    }
    
    fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(area);
        
        let titles: Vec<Spans> = self.titles
            .iter()
            .map(|t| Spans::from(*t))
            .collect();
        
        let tabs = Tabs::new(titles)
            .block(Block::default().title("Tabs").borders(Borders::ALL))
            .select(self.selected)
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        
        f.render_widget(tabs, chunks[0]);
        
        let content_block = Block::default()
            .title(format!("Content: {}", self.titles[self.selected]))
            .borders(Borders::ALL);
        
        f.render_widget(content_block, chunks[1]);
        
        // Render specific content based on selected tab
        let content_area = content_block.inner(chunks[1]);
        match self.selected {
            0 => self.render_tab1(f, content_area),
            1 => self.render_tab2(f, content_area),
            2 => self.render_tab3(f, content_area),
            _ => {}
        }
    }
    
    fn render_tab1(&self, f: &mut Frame, area: Rect) {
        // Tab 1 content
    }
    
    fn render_tab2(&self, f: &mut Frame, area: Rect) {
        // Tab 2 content
    }
    
    fn render_tab3(&self, f: &mut Frame, area: Rect) {
        // Tab 3 content
    }
}
```

### Modal Dialogs

```rust
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Clear, Paragraph, Widget},
    Frame,
};

enum DialogType {
    Info,
    Warning,
    Error,
    Confirm,
}

struct Modal<'a> {
    title: &'a str,
    text: &'a str,
    dialog_type: DialogType,
}

impl<'a> Modal<'a> {
    fn new(title: &'a str, text: &'a str, dialog_type: DialogType) -> Self {
        Self {
            title,
            text,
            dialog_type,
        }
    }
    
    fn render(&self, f: &mut Frame, size: Rect) {
        let dialog_width = 60.min(size.width - 4);
        let dialog_height = 10.min(size.height - 4);
        
        let dialog_area = centered_rect(dialog_width, dialog_height, size);
        
        // Clear the area under the modal
        f.render_widget(Clear, dialog_area);
        
        // Create the modal block
        let (title_style, border_style) = match self.dialog_type {
            DialogType::Info => (
                Style::default().fg(Color::Cyan),
                Style::default().fg(Color::Cyan),
            ),
            DialogType::Warning => (
                Style::default().fg(Color::Yellow),
                Style::default().fg(Color::Yellow),
            ),
            DialogType::Error => (
                Style::default().fg(Color::Red),
                Style::default().fg(Color::Red),
            ),
            DialogType::Confirm => (
                Style::default().fg(Color::Green),
                Style::default().fg(Color::Green),
            ),
        };
        
        let modal_block = Block::default()
            .title(self.title)
            .title_style(title_style)
            .borders(Borders::ALL)
            .border_style(border_style);
        
        f.render_widget(modal_block, dialog_area);
        
        // Split the modal area for content and buttons
        let inner_area = modal_block.inner(dialog_area);
        
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner_area);
        
        // Render content
        let text = Paragraph::new(self.text)
            .alignment(Alignment::Center)
            .wrap(ratatui::widgets::Wrap { trim: true });
        
        f.render_widget(text, chunks[0]);
        
        // Render buttons
        let button_text = match self.dialog_type {
            DialogType::Info => "[ OK ]",
            DialogType::Warning => "[ OK ]",
            DialogType::Error => "[ OK ]",
            DialogType::Confirm => "[ Yes ]   [ No ]",
        };
        
        let buttons = Paragraph::new(button_text)
            .alignment(Alignment::Center);
        
        f.render_widget(buttons, chunks[1]);
    }
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Length((r.height.saturating_sub(height)) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((r.width.saturating_sub(width)) / 2),
            Constraint::Length(width),
            Constraint::Length((r.width.saturating_sub(width)) / 2),
        ])
        .split(popup_layout[1])[1]
}
```

## Image Rendering

Using unicode blocks for simple image rendering:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

struct SimpleImage {
    pixels: Vec<Vec<Color>>,
    scale: f32,
}

impl SimpleImage {
    fn new(pixels: Vec<Vec<Color>>, scale: f32) -> Self {
        Self { pixels, scale }
    }
}

impl Widget for SimpleImage {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 || self.pixels.is_empty() {
            return;
        }
        
        let original_height = self.pixels.len();
        let original_width = self.pixels[0].len();
        
        let scaled_width = (original_width as f32 * self.scale) as usize;
        let scaled_height = (original_height as f32 * self.scale) as usize;
        
        // Each terminal cell can show two pixels vertically
        // using the Unicode block characters
        for y in 0..area.height.min(scaled_height as u16 / 2) {
            for x in 0..area.width.min(scaled_width as u16) {
                let top_y = (y as f32 * 2.0 / self.scale) as usize;
                let bottom_y = ((y as f32 * 2.0 + 1.0) / self.scale) as usize;
                let pixel_x = (x as f32 / self.scale) as usize;
                
                if top_y < original_height && bottom_y < original_height && pixel_x < original_width {
                    let top_color = self.pixels[top_y][pixel_x];
                    let bottom_color = self.pixels[bottom_y][pixel_x];
                    
                    // Use the Unicode "upper half block" character to display two pixels vertically
                    buf.get_mut(area.x + x, area.y + y)
                        .set_char('▀') // Upper half block
                        .set_style(Style::default().fg(bottom_color).bg(top_color));
                }
            }
        }
    }
}
```

## Performance Optimization

Tips for optimizing Ratatui applications:

1. **Minimize Rendering**: Only redraw when necessary
2. **Use Appropriate Update Rates**: Adjust tick rates based on content
3. **Avoid Excessive Layout Calculations**: Cache layouts when possible
4. **Handle Large Data Efficiently**: Virtualize lists and only render visible items
5. **Reduce Allocations**: Reuse buffers and data structures
6. **Profile Your Application**: Identify bottlenecks

Example of a lazy rendering approach:

```rust
struct AppState {
    data: Vec<String>,
    last_modified: Instant,
    needs_redraw: bool,
}

impl AppState {
    fn update(&mut self) {
        // Update state
        let now = Instant::now();
        if (now - self.last_modified) > Duration::from_secs(1) {
            self.data.push(format!("New data at {}", now.elapsed().as_secs()));
            self.last_modified = now;
            self.needs_redraw = true;
        }
    }
}

// In the main loop:
loop {
    // Update app state
    app_state.update();
    
    // Only redraw if needed
    if app_state.needs_redraw {
        terminal.draw(|f| {
            // Render UI
        })?;
        app_state.needs_redraw = false;
    }
    
    // Handle events
    if event::poll(Duration::from_millis(100))? {
        match event::read()? {
            Event::Key(key) => {
                // Handle key events
                app_state.needs_redraw = true;
            }
            Event::Mouse(_) => {
                // Handle mouse events
                app_state.needs_redraw = true;
            }
            Event::Resize(_, _) => {
                // Always redraw on resize
                app_state.needs_redraw = true;
            }
            _ => {}
        }
    }
}
```

By implementing these advanced techniques, you can create sophisticated and responsive terminal user interfaces that rival graphical applications in functionality and user experience.