# Text Handling in Ratatui

Ratatui provides a flexible system for displaying and styling text in terminal user interfaces. This document covers the core text components and how to use them effectively.

## Text Components

Ratatui's text handling is built around three main components:

- `Span`: A piece of text with a uniform style
- `Line` (previously `Spans`): A collection of `Span`s that form a single line
- `Text`: A collection of `Line`s that form a multi-line text

## Spans

A `Span` is the most basic text unit, consisting of content and an optional style:

```rust
use ratatui::{
    style::{Color, Style},
    text::Span,
};

// Simple span with default style
let span1 = Span::raw("Plain text");

// Span with a custom style
let span2 = Span::styled(
    "Colored text",
    Style::default().fg(Color::Red)
);

// Creating a span from a string reference
let text = "Reference text";
let span3 = Span::from(text);
```

## Lines

A `Line` (previously called `Spans`) represents a single line composed of multiple `Span`s:

```rust
use ratatui::text::{Line, Span};
use ratatui::style::{Color, Style};

// Create a line with multiple differently styled spans
let line = Line::from(vec![
    Span::raw("Normal "),
    Span::styled("bold", Style::default().add_modifier(Modifier::BOLD)),
    Span::raw(" and "),
    Span::styled("italic", Style::default().add_modifier(Modifier::ITALIC)),
    Span::raw(" text"),
]);

// Create a line from a single span
let simple_line = Line::from("Simple line of text");
```

## Text

`Text` is a collection of `Line`s, representing multi-line text:

```rust
use ratatui::text::{Line, Span, Text};
use ratatui::style::{Color, Style};

// Create multi-line text
let text = Text::from(vec![
    Line::from("First line"),
    Line::from(vec![
        Span::raw("Second line with "),
        Span::styled("styled", Style::default().fg(Color::Yellow)),
        Span::raw(" text"),
    ]),
    Line::from("Third line"),
]);

// Create text from a string (split by newlines)
let simple_text = Text::from("Line 1\nLine 2\nLine 3");
```

## Text Styling

Both `Span` and `Line` can be styled:

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

// Styled span
let bold_red = Span::styled(
    "Bold red text",
    Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::BOLD)
);

// Styled line (applies to all contained spans that don't have their own style)
let yellow_line = Line::styled(
    vec![
        Span::raw("This will be yellow "),
        Span::styled("except this", Style::default().fg(Color::Blue)),
    ],
    Style::default().fg(Color::Yellow)
);
```

## Text Widget: Paragraph

The `Paragraph` widget is the primary way to render text in Ratatui:

```rust
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::layout::Alignment;

let text = Text::from("This is a paragraph with multiple\nlines of text.");

let paragraph = Paragraph::new(text)
    .block(Block::default().title("Paragraph").borders(Borders::ALL))
    .style(Style::default().fg(Color::White).bg(Color::Black))
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true });

f.render_widget(paragraph, area);
```

## Text Truncation and Wrapping

The `Paragraph` widget can handle text that's too long for its container:

```rust
use ratatui::widgets::{Paragraph, Wrap};

// Without wrapping (text will be truncated)
let paragraph = Paragraph::new("Very long text that won't fit on one line")
    .block(Block::default().title("No Wrap").borders(Borders::ALL));

// With wrapping (text will wrap to the next line)
let paragraph = Paragraph::new("Very long text that will be wrapped to fit")
    .block(Block::default().title("With Wrap").borders(Borders::ALL))
    .wrap(Wrap { trim: true });  // trim removes trailing whitespace
```

## Text Scrolling

`Paragraph` supports scrolling for text that doesn't fit in the container:

```rust
use ratatui::widgets::Paragraph;

let paragraph = Paragraph::new(long_text)
    .block(Block::default().title("Scrollable").borders(Borders::ALL))
    .scroll((vertical_scroll, horizontal_scroll));

// In your event handler:
match key.code {
    KeyCode::Up => {
        if vertical_scroll > 0 {
            vertical_scroll -= 1;
        }
    }
    KeyCode::Down => {
        vertical_scroll += 1;
    }
    // Handle horizontal scrolling similarly
}
```

## Text Alignment

`Paragraph` supports different text alignments:

```rust
use ratatui::widgets::Paragraph;
use ratatui::layout::Alignment;

let left_aligned = Paragraph::new("Left aligned text")
    .alignment(Alignment::Left);  // This is the default

let center_aligned = Paragraph::new("Center aligned text")
    .alignment(Alignment::Center);

let right_aligned = Paragraph::new("Right aligned text")
    .alignment(Alignment::Right);
```

## Unicode and Special Characters

Ratatui has good support for Unicode:

```rust
let unicode_text = Text::from(vec![
    Line::from("Regular ASCII text"),
    Line::from("Unicode symbols: ★ ☺ ♠ ♣ ♥ ♦"),
    Line::from("Non-Latin scripts: こんにちは 你好 مرحبا"),
    Line::from("Emojis: 🚀 🔥 🎉 💡"),
]);
```

## Working with Formatted Text

You can build complex text layouts by combining spans and lines:

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};

fn create_title(title: &str) -> Line {
    Line::from(vec![
        Span::styled("│ ", Style::default().fg(Color::Gray)),
        Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │", Style::default().fg(Color::Gray)),
    ])
}

fn create_key_value(key: &str, value: &str) -> Line {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(key, Style::default().fg(Color::Yellow)),
        Span::raw(": "),
        Span::raw(value),
    ])
}

let text = Text::from(vec![
    create_title("USER PROFILE"),
    Line::from(""),
    create_key_value("Name", "John Doe"),
    create_key_value("Email", "john.doe@example.com"),
    create_key_value("Role", "Administrator"),
    Line::from(""),
]);
```

## Advanced Text Styling

You can create text with gradients, blinking, and other effects:

```rust
// Gradient text
let colors = [
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::Cyan,
    Color::Blue,
    Color::Magenta,
];

let rainbow_text = "Rainbow text";
let rainbow_spans: Vec<Span> = rainbow_text
    .chars()
    .enumerate()
    .map(|(i, c)| {
        let color = colors[i % colors.len()];
        Span::styled(c.to_string(), Style::default().fg(color))
    })
    .collect();

let rainbow_line = Line::from(rainbow_spans);

// Blinking text
let blinking = Span::styled(
    "This text blinks",
    Style::default().add_modifier(Modifier::SLOW_BLINK),
);

// Bold and italic
let styled = Span::styled(
    "Bold and italic",
    Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
);
```

## Text Input Handling

For text inputs (like a text editor), you'll need to handle user input and update text:

```rust
// In your application state
struct AppState {
    input_text: String,
    cursor_position: usize,
}

// In your event handler
match key.code {
    KeyCode::Char(c) => {
        app_state.input_text.insert(app_state.cursor_position, c);
        app_state.cursor_position += 1;
    }
    KeyCode::Backspace => {
        if app_state.cursor_position > 0 {
            app_state.cursor_position -= 1;
            app_state.input_text.remove(app_state.cursor_position);
        }
    }
    KeyCode::Delete => {
        if app_state.cursor_position < app_state.input_text.len() {
            app_state.input_text.remove(app_state.cursor_position);
        }
    }
    KeyCode::Left => {
        if app_state.cursor_position > 0 {
            app_state.cursor_position -= 1;
        }
    }
    KeyCode::Right => {
        if app_state.cursor_position < app_state.input_text.len() {
            app_state.cursor_position += 1;
        }
    }
    // Handle other keys (Home, End, etc.)
}

// In your render function
let input = Paragraph::new(app_state.input_text.as_ref())
    .block(Block::default().title("Input").borders(Borders::ALL));

f.render_widget(input, input_area);

// Draw cursor (highlight the character at cursor_position)
if app_state.cursor_position < app_state.input_text.len() {
    // Use a cursor style to highlight the character at cursor position
    let cursor_position = app_state.cursor_position;
    f.buffer_mut().set_style(
        input_area.x + 1 + cursor_position as u16,
        input_area.y + 1,
        Style::default().fg(Color::Black).bg(Color::White),
    );
} else {
    // Draw cursor at the end of input
    f.buffer_mut().set_style(
        input_area.x + 1 + app_state.input_text.len() as u16,
        input_area.y + 1,
        Style::default().fg(Color::White).bg(Color::Yellow),
    );
}
```

## Multi-line Text Editing

For multi-line text editing, you'll need to track lines and cursor position:

```rust
struct TextEditorState {
    lines: Vec<String>,
    cursor: (usize, usize),  // (row, column)
}

// Rendering multi-line text
let text = Text::from(
    app_state.lines.iter().map(|line| Line::from(line.clone())).collect::<Vec<_>>()
);

let editor = Paragraph::new(text)
    .block(Block::default().title("Editor").borders(Borders::ALL));

f.render_widget(editor, editor_area);

// Draw cursor at the current position
let (row, col) = app_state.cursor;
f.buffer_mut().set_style(
    editor_area.x + 1 + col as u16,
    editor_area.y + 1 + row as u16,
    Style::default().fg(Color::Black).bg(Color::White),
);
```

## Text Size Calculation

Sometimes you need to know how much space text will take up:

```rust
fn get_text_width(text: &str) -> usize {
    text.chars().count()  // Simple approach for ASCII
    
    // For proper Unicode width calculation, use the unicode-width crate:
    // unicode_width::UnicodeWidthStr::width(text)
}

fn get_wrapped_height(text: &str, max_width: usize) -> usize {
    // Simple approximation for line wrapping
    let width = get_text_width(text);
    (width + max_width - 1) / max_width  // Ceiling division
}
```

## Best Practices for Text Handling

1. **Use Spans for styling parts of text**: This allows for more flexible styling than applying styles to entire paragraphs.
2. **Be mindful of terminal width**: Text that's too wide will be truncated or wrapped, depending on your settings.
3. **Handle Unicode properly**: For length calculations, use a Unicode-aware library like `unicode-width`.
4. **Consider text alignment**: Choose the appropriate alignment (left, center, right) for your interface.
5. **Implement scrolling for long text**: Allow users to scroll through text that doesn't fit in the viewport.
6. **Add visual cues for editable text**: Use a cursor or highlighting to show where text can be edited.
7. **Support selection for copy/paste**: Implement text selection for clipboard operations.

## Implementing Text Selection

For text editors, you'll want to implement text selection:

```rust
struct TextEditorState {
    lines: Vec<String>,
    cursor: (usize, usize),  // (row, column)
    selection_start: Option<(usize, usize)>,  // Starting point of selection
}

// When starting selection (e.g., on Shift+Arrow or mouse down)
app_state.selection_start = Some(app_state.cursor);

// When moving cursor with selection active
// The selection is from selection_start to current cursor

// When rendering, highlight the selected text
for (row_idx, line) in app_state.lines.iter().enumerate() {
    if let Some(selection_start) = app_state.selection_start {
        let (start_row, start_col) = selection_start;
        let (current_row, current_col) = app_state.cursor;
        
        // Determine the range for this row
        // (logic to calculate selection range for the current row)
        
        // Apply highlight style to the selected portion
        // (rendering code for highlighted selection)
    }
}
```

With these tools and techniques, you can build sophisticated text handling in your Ratatui applications, from simple displays to complex text editors with selection, copying, and pasting.