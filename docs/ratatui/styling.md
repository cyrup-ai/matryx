# Styling in Ratatui

Ratatui provides a flexible styling system that allows you to customize the appearance of your widgets. This document covers how to use styles, colors, and modifiers to enhance your terminal UI.

## Basic Styling

The core of Ratatui's styling system is the `Style` struct, which contains foreground and background colors and modifiers:

```rust
use ratatui::style::{Style, Color, Modifier};

// Simple style with only foreground color
let style = Style::default().fg(Color::Red);

// Style with background color
let style = Style::default().bg(Color::Blue);

// Style with foreground, background, and modifiers
let style = Style::default()
    .fg(Color::White)
    .bg(Color::Black)
    .add_modifier(Modifier::BOLD);
```

## Colors

Ratatui supports a variety of colors:

### Named Colors

```rust
use ratatui::style::Color;

// Basic colors
Color::Black
Color::Red
Color::Green
Color::Yellow
Color::Blue
Color::Magenta
Color::Cyan
Color::White
Color::Gray
Color::DarkGray
Color::LightRed
Color::LightGreen
Color::LightYellow
Color::LightBlue
Color::LightMagenta
Color::LightCyan

// Special colors
Color::Reset    // Terminal default color
```

### RGB Colors

For terminals that support it, you can specify exact RGB colors:

```rust
// RGB color (0-255 for each component)
Color::Rgb(31, 86, 115)    // Deep blue
Color::Rgb(255, 165, 0)    // Orange
Color::Rgb(128, 0, 128)    // Purple
```

### Indexed Colors

Some terminals support indexed colors (0-255):

```rust
// Indexed color (0-255)
Color::Indexed(16)    // Usually bright black
Color::Indexed(196)   // Usually bright red
```

### Dynamic Color Selection

You can choose colors dynamically based on your application's state:

```rust
fn get_color_for_value(value: i32) -> Color {
    if value < 0 {
        Color::Red
    } else if value == 0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

let style = Style::default().fg(get_color_for_value(item.value));
```

## Modifiers

Modifiers change the text appearance:

```rust
use ratatui::style::Modifier;

// Single modifier
let style = Style::default().add_modifier(Modifier::BOLD);

// Multiple modifiers
let style = Style::default()
    .add_modifier(Modifier::BOLD | Modifier::ITALIC);

// Remove modifiers
let style = Style::default()
    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
    .remove_modifier(Modifier::BOLD);
```

Available modifiers:
- `Modifier::BOLD`
- `Modifier::DIM`
- `Modifier::ITALIC`
- `Modifier::UNDERLINED`
- `Modifier::SLOW_BLINK`
- `Modifier::RAPID_BLINK` 
- `Modifier::REVERSED`
- `Modifier::HIDDEN`
- `Modifier::CROSSED_OUT`

Note that not all terminals support all modifiers.

## Applying Styles to Widgets

Most widgets accept styles directly:

```rust
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::style::{Style, Color, Modifier};

let block = Block::default()
    .title("Title")
    .borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Cyan))
    .style(Style::default().bg(Color::Black));

let paragraph = Paragraph::new("Hello, world!")
    .block(block)
    .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
```

## Styling Text

For rich text styling, use the `Text` and `Span` types:

```rust
use ratatui::text::{Text, Span, Spans};
use ratatui::style::{Style, Color, Modifier};

// Single styled span
let span = Span::styled(
    "Bold red text",
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
);

// Multiple spans in a line
let spans = Spans::from(vec![
    Span::styled("Normal ", Style::default()),
    Span::styled("Bold", Style::default().add_modifier(Modifier::BOLD)),
    Span::styled(" and ", Style::default()),
    Span::styled("Italic", Style::default().add_modifier(Modifier::ITALIC)),
]);

// Multiple lines with different styles
let text = Text::from(vec![
    Spans::from("Regular line"),
    Spans::from(vec![
        Span::styled("Colored ", Style::default().fg(Color::Yellow)),
        Span::styled("line", Style::default().fg(Color::Blue)),
    ]),
    Spans::from(Span::styled("Bold line", Style::default().add_modifier(Modifier::BOLD))),
]);

// Use with a paragraph
let paragraph = Paragraph::new(text);
```

## Style Inheritance and Combination

Styles can be combined:

```rust
let base_style = Style::default().fg(Color::White).bg(Color::Black);
let highlight_style = base_style.add_modifier(Modifier::BOLD);

// Add specific color to the base style
let error_style = base_style.fg(Color::Red);
let success_style = base_style.fg(Color::Green);

// Patch a style with another style
let combined_style = base_style.patch(highlight_style);
```

## Conditional Styling

You can apply styles conditionally based on state:

```rust
for (i, item) in items.iter().enumerate() {
    let style = if i == selected_index {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    
    let list_item = ListItem::new(item.name.clone()).style(style);
    items_list.push(list_item);
}
```

## Styling Components Consistently

For a consistent look, create style constants or functions:

```rust
const NORMAL_STYLE: Style = Style::default().fg(Color::White).bg(Color::Black);
const HIGHLIGHT_STYLE: Style = Style::default().fg(Color::Yellow).bg(Color::Blue).add_modifier(Modifier::BOLD);
const ERROR_STYLE: Style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
const SUCCESS_STYLE: Style = Style::default().fg(Color::Green);

fn get_status_style(status: &Status) -> Style {
    match status {
        Status::Ok => SUCCESS_STYLE,
        Status::Warning => Style::default().fg(Color::Yellow),
        Status::Error => ERROR_STYLE,
        Status::Unknown => NORMAL_STYLE,
    }
}
```

## Styled Widgets in Container Widgets

When using widgets inside container widgets, styles can be combined:

```rust
let outer_block = Block::default()
    .title("Container")
    .borders(Borders::ALL)
    .style(Style::default().fg(Color::White).bg(Color::Black));

let inner_paragraph = Paragraph::new("Content")
    .style(Style::default().fg(Color::Yellow));

// The inner paragraph will have yellow text on a black background
f.render_widget(outer_block.inner(inner_paragraph), chunk);
```

## Custom Style Builder

You can create your own style builder functions for consistency:

```rust
fn primary_button_style(is_active: bool) -> Style {
    let mut style = Style::default()
        .fg(Color::White)
        .bg(Color::Blue);
    
    if is_active {
        style = style.add_modifier(Modifier::BOLD);
    }
    
    style
}

fn secondary_button_style(is_active: bool) -> Style {
    let mut style = Style::default()
        .fg(Color::Black)
        .bg(Color::Gray);
    
    if is_active {
        style = style.add_modifier(Modifier::BOLD);
    }
    
    style
}

// Usage
let button = Paragraph::new("Save")
    .style(primary_button_style(is_focused));
```

## Terminal Color Support

Different terminals support different color features. For best results:

1. Use named colors for maximum compatibility
2. Provide fallbacks for terminals with limited color support
3. Test your application in different terminal emulators

For terminals with limited color support, consider:

```rust
// Detect color support level
let color_support = /* determine color support */;

let highlight_color = if color_support.has_rgb_colors() {
    Color::Rgb(255, 165, 0)  // Orange
} else if color_support.has_indexed_colors() {
    Color::Indexed(208)      // Close to orange in 256-color terminals
} else {
    Color::Yellow            // Fallback for basic terminals
};
```

## Style Best Practices

1. **Consistency**: Use consistent styles for similar elements
2. **Contrast**: Ensure text is readable against its background
3. **Simplicity**: Don't overuse colors and styles; focus on clarity
4. **Meaning**: Use colors to convey meaning (red for errors, green for success, etc.)
5. **Accessibility**: Consider users with color vision deficiencies
6. **Terminal Support**: Test in different terminals to ensure compatibility

## Example Theme System

For larger applications, consider implementing a theme system:

```rust
struct Theme {
    primary: Color,
    secondary: Color,
    background: Color,
    text: Color,
    error: Color,
    warning: Color,
    success: Color,
}

impl Theme {
    fn default() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            background: Color::Black,
            text: Color::White,
            error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
        }
    }
    
    fn dark() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            background: Color::Black,
            text: Color::White,
            error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
        }
    }
    
    fn light() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            background: Color::White,
            text: Color::Black,
            error: Color::Red,
            warning: Color::Yellow,
            success: Color::Green,
        }
    }
    
    fn button_style(&self, is_active: bool) -> Style {
        let mut style = Style::default()
            .fg(self.text)
            .bg(self.primary);
        
        if is_active {
            style = style.add_modifier(Modifier::BOLD);
        }
        
        style
    }
    
    fn title_style(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }
    
    fn error_style(&self) -> Style {
        Style::default()
            .fg(self.error)
            .add_modifier(Modifier::BOLD)
    }
    
    // More style methods...
}

// Application using the theme
struct App {
    theme: Theme,
    // Other fields...
}

impl App {
    fn new() -> Self {
        Self {
            theme: Theme::default(),
            // Initialize other fields...
        }
    }
    
    fn render(&self, f: &mut Frame) {
        let block = Block::default()
            .title("My App")
            .title_style(self.theme.title_style())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.secondary));
        
        f.render_widget(block, f.size());
        
        // Render other widgets using theme styles...
    }
    
    fn toggle_theme(&mut self) {
        if std::mem::discriminant(&self.theme) == std::mem::discriminant(&Theme::dark()) {
            self.theme = Theme::light();
        } else {
            self.theme = Theme::dark();
        }
    }
}
```

With a well-designed styling system, you can create visually appealing and user-friendly terminal applications.