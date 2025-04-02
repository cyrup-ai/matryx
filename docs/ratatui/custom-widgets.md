# Creating Custom Widgets in Ratatui

Ratatui provides a flexible system for creating custom widgets to fit your application's needs. This document covers the process of building custom widgets from scratch and extending existing widgets.

## Widget Trait Basics

All widgets in Ratatui implement the `Widget` trait, which requires a single method: `render`. Here's a simple example:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

struct Label {
    text: String,
    style: Style,
}

impl Label {
    pub fn new<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
        }
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for Label {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        
        // Simple rendering - just display the text at the start of the area
        buf.set_string(area.x, area.y, &self.text, self.style);
    }
}
```

## StatefulWidget Trait

For widgets that need to maintain state between renders, use the `StatefulWidget` trait:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{StatefulWidget, Widget},
};

struct Counter {
    label: String,
    style: Style,
}

struct CounterState {
    count: u32,
}

impl Counter {
    pub fn new<T: Into<String>>(label: T) -> Self {
        Self {
            label: label.into(),
            style: Style::default(),
        }
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl StatefulWidget for Counter {
    type State = CounterState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        
        let display = format!("{}: {}", self.label, state.count);
        buf.set_string(area.x, area.y, display, self.style);
    }
}

// Usage:
// let counter = Counter::new("Items");
// let mut state = CounterState { count: 5 };
// f.render_stateful_widget(counter, area, &mut state);
```

## Builder Pattern

Many Ratatui widgets use the builder pattern for a fluid API. Here's how to implement it:

```rust
struct Button {
    text: String,
    style: Style,
    active_style: Style,
    is_active: bool,
}

impl Button {
    pub fn new<T: Into<String>>(text: T) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
            active_style: Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            is_active: false,
        }
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    
    pub fn active_style(mut self, style: Style) -> Self {
        self.active_style = style;
        self
    }
    
    pub fn active(mut self, is_active: bool) -> Self {
        self.is_active = is_active;
        self
    }
}

impl Widget for Button {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        
        let style = if self.is_active { self.active_style } else { self.style };
        let display = format!("[ {} ]", self.text);
        
        buf.set_string(
            area.x,
            area.y,
            display,
            style,
        );
    }
}

// Usage:
// let button = Button::new("Save")
//     .style(Style::default().fg(Color::White))
//     .active_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
//     .active(is_selected);
// f.render_widget(button, area);
```

## Extending Existing Widgets

You can extend existing widgets by wrapping them:

```rust
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

struct BorderedParagraph {
    text: String,
    title: String,
    style: Style,
}

impl BorderedParagraph {
    pub fn new<T: Into<String>, U: Into<String>>(text: T, title: U) -> Self {
        Self {
            text: text.into(),
            title: title.into(),
            style: Style::default(),
        }
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for BorderedParagraph {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .style(self.style);
        
        let inner_area = block.inner(area);
        
        block.render(area, buf);
        
        Paragraph::new(self.text)
            .style(self.style)
            .render(inner_area, buf);
    }
}
```

## Text Editor Widget

Here's an example of a simple text editor widget:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, StatefulWidget, Widget},
};

pub struct TextEditor {
    block: Option<Block>,
    style: Style,
}

pub struct TextEditorState {
    pub content: String,
    pub cursor_position: usize,
}

impl TextEditor {
    pub fn new() -> Self {
        Self {
            block: None,
            style: Style::default(),
        }
    }
    
    pub fn block(mut self, block: Block) -> Self {
        self.block = Some(block);
        self
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl StatefulWidget for TextEditor {
    type State = TextEditorState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Get area to render the editor
        let editor_area = match self.block {
            Some(block) => {
                let inner_area = block.inner(area);
                block.render(area, buf);
                inner_area
            }
            None => area,
        };
        
        if editor_area.width == 0 || editor_area.height == 0 {
            return;
        }
        
        // Render text
        let content = &state.content;
        buf.set_string(
            editor_area.x,
            editor_area.y,
            content,
            self.style,
        );
        
        // Render cursor (if within visible area)
        if state.cursor_position <= content.len() {
            // Simple cursor implementation - just use a different background color
            let cursor_x = editor_area.x + state.cursor_position as u16;
            let cursor_y = editor_area.y;
            
            // Get the character at cursor position or space if at the end
            let cursor_char = if state.cursor_position < content.len() {
                content.chars().nth(state.cursor_position).unwrap()
            } else {
                ' '
            };
            
            buf.set_string(
                cursor_x,
                cursor_y,
                cursor_char.to_string(),
                Style::default().fg(Color::Black).bg(Color::White),
            );
        }
    }
}
```

## Handling Text Input and Selection

A more complete text editor widget would handle text input and selection:

```rust
pub struct TextEditorState {
    pub lines: Vec<String>,
    pub cursor: (usize, usize),  // (row, column)
    pub selection: Option<((usize, usize), (usize, usize))>,  // (start, end)
    pub scroll_offset: (usize, usize),  // (row, column)
}

impl TextEditorState {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            selection: None,
            scroll_offset: (0, 0),
        }
    }
    
    pub fn from_text(text: &str) -> Self {
        Self {
            lines: text.split('\n').map(|s| s.to_string()).collect(),
            cursor: (0, 0),
            selection: None,
            scroll_offset: (0, 0),
        }
    }
    
    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }
    
    // Cursor movement methods
    pub fn move_cursor_left(&mut self) {
        let (row, col) = self.cursor;
        if col > 0 {
            self.cursor = (row, col - 1);
        } else if row > 0 {
            self.cursor = (row - 1, self.lines[row - 1].len());
        }
    }
    
    pub fn move_cursor_right(&mut self) {
        let (row, col) = self.cursor;
        if col < self.lines[row].len() {
            self.cursor = (row, col + 1);
        } else if row < self.lines.len() - 1 {
            self.cursor = (row + 1, 0);
        }
    }
    
    pub fn move_cursor_up(&mut self) {
        let (row, col) = self.cursor;
        if row > 0 {
            let new_col = col.min(self.lines[row - 1].len());
            self.cursor = (row - 1, new_col);
        }
    }
    
    pub fn move_cursor_down(&mut self) {
        let (row, col) = self.cursor;
        if row < self.lines.len() - 1 {
            let new_col = col.min(self.lines[row + 1].len());
            self.cursor = (row + 1, new_col);
        }
    }
    
    // Edit methods
    pub fn insert_char(&mut self, c: char) {
        let (row, col) = self.cursor;
        if c == '\n' {
            // Handle new line
            let current_line = &self.lines[row];
            let new_line = current_line[col..].to_string();
            self.lines[row] = current_line[..col].to_string();
            self.lines.insert(row + 1, new_line);
            self.cursor = (row + 1, 0);
        } else {
            // Insert character
            let line = &mut self.lines[row];
            line.insert(col, c);
            self.cursor = (row, col + 1);
        }
    }
    
    pub fn delete_char(&mut self) {
        let (row, col) = self.cursor;
        if col > 0 {
            // Delete character before cursor
            let line = &mut self.lines[row];
            line.remove(col - 1);
            self.cursor = (row, col - 1);
        } else if row > 0 {
            // Merge with previous line
            let line = self.lines.remove(row);
            let prev_line_len = self.lines[row - 1].len();
            self.lines[row - 1].push_str(&line);
            self.cursor = (row - 1, prev_line_len);
        }
    }
    
    // Selection methods
    pub fn start_selection(&mut self) {
        self.selection = Some((self.cursor, self.cursor));
    }
    
    pub fn update_selection(&mut self) {
        if let Some((start, _)) = self.selection {
            self.selection = Some((start, self.cursor));
        }
    }
    
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }
    
    pub fn get_selected_text(&self) -> String {
        if let Some((start, end)) = self.selection {
            let (start_row, start_col) = start;
            let (end_row, end_col) = end;
            
            // Ensure start <= end
            let (start_row, start_col, end_row, end_col) = if start_row < end_row || (start_row == end_row && start_col <= end_col) {
                (start_row, start_col, end_row, end_col)
            } else {
                (end_row, end_col, start_row, start_col)
            };
            
            if start_row == end_row {
                // Selection is on a single line
                return self.lines[start_row][start_col..end_col].to_string();
            } else {
                // Selection spans multiple lines
                let mut result = String::new();
                
                // First line
                result.push_str(&self.lines[start_row][start_col..]);
                result.push('\n');
                
                // Middle lines
                for row in start_row + 1..end_row {
                    result.push_str(&self.lines[row]);
                    result.push('\n');
                }
                
                // Last line
                result.push_str(&self.lines[end_row][..end_col]);
                
                return result;
            }
        }
        
        String::new()
    }
    
    // Clipboard operations
    pub fn cut_selection(&mut self) -> String {
        let text = self.get_selected_text();
        if !text.is_empty() {
            self.delete_selection();
        }
        text
    }
    
    pub fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection {
            let (start_row, start_col) = start;
            let (end_row, end_col) = end;
            
            // Ensure start <= end
            let (start_row, start_col, end_row, end_col) = if start_row < end_row || (start_row == end_row && start_col <= end_col) {
                (start_row, start_col, end_row, end_col)
            } else {
                (end_row, end_col, start_row, start_col)
            };
            
            if start_row == end_row {
                // Delete within a single line
                let line = &mut self.lines[start_row];
                let before = line[..start_col].to_string();
                let after = line[end_col..].to_string();
                *line = before + &after;
            } else {
                // Delete across multiple lines
                let first_line_start = self.lines[start_row][..start_col].to_string();
                let last_line_end = self.lines[end_row][end_col..].to_string();
                
                // Combine first and last line
                self.lines[start_row] = first_line_start + &last_line_end;
                
                // Remove middle lines
                self.lines.drain(start_row + 1..=end_row);
            }
            
            self.cursor = (start_row, start_col);
            self.clear_selection();
        }
    }
    
    pub fn paste_text(&mut self, text: &str) {
        if let Some(_) = self.selection {
            self.delete_selection();
        }
        
        let (row, col) = self.cursor;
        let lines: Vec<&str> = text.split('\n').collect();
        
        if lines.len() == 1 {
            // Simple paste on a single line
            let line = &mut self.lines[row];
            let before = line[..col].to_string();
            let after = line[col..].to_string();
            *line = before + lines[0] + &after;
            self.cursor = (row, col + lines[0].len());
        } else {
            // Paste multiple lines
            let current_line = &self.lines[row];
            let new_first_line = current_line[..col].to_string() + lines[0];
            let new_last_line = lines.last().unwrap().to_string() + &current_line[col..];
            
            // Replace current line with first line of pasted text
            self.lines[row] = new_first_line;
            
            // Insert middle lines
            for (i, line) in lines.iter().enumerate().skip(1).take(lines.len() - 2) {
                self.lines.insert(row + i, line.to_string());
            }
            
            // Insert last line
            if lines.len() > 1 {
                self.lines.insert(row + lines.len() - 1, new_last_line);
            }
            
            // Update cursor position
            self.cursor = (row + lines.len() - 1, if lines.len() > 1 { lines.last().unwrap().len() } else { col + lines[0].len() });
        }
    }
}

// Then implement the TextEditor widget to render this state
```

## Complex Widgets: Form Elements

Here's an example of a form field widget:

```rust
enum FormFieldType {
    Text,
    Password,
    Number,
}

struct FormField {
    label: String,
    field_type: FormFieldType,
    width: u16,
    style: Style,
    focused_style: Style,
    is_focused: bool,
}

struct FormFieldState {
    value: String,
    cursor_position: usize,
}

impl FormField {
    pub fn new<T: Into<String>>(label: T) -> Self {
        Self {
            label: label.into(),
            field_type: FormFieldType::Text,
            width: 20,
            style: Style::default(),
            focused_style: Style::default().fg(Color::Yellow),
            is_focused: false,
        }
    }
    
    pub fn password(mut self) -> Self {
        self.field_type = FormFieldType::Password;
        self
    }
    
    pub fn number(mut self) -> Self {
        self.field_type = FormFieldType::Number;
        self
    }
    
    pub fn width(mut self, width: u16) -> Self {
        self.width = width;
        self
    }
    
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
    
    pub fn focused_style(mut self, style: Style) -> Self {
        self.focused_style = style;
        self
    }
    
    pub fn focused(mut self, is_focused: bool) -> Self {
        self.is_focused = is_focused;
        self
    }
}

impl StatefulWidget for FormField {
    type State = FormFieldState;
    
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if area.width == 0 || area.height == 0 || area.width < self.label.len() as u16 + 1 + self.width {
            return;
        }
        
        let style = if self.is_focused { self.focused_style } else { self.style };
        
        // Render label
        buf.set_string(area.x, area.y, &self.label, style);
        
        // Calculate field area
        let field_x = area.x + self.label.len() as u16 + 1;
        let field_width = self.width;
        
        // Render field border
        let border_style = style;
        buf.set_string(field_x, area.y, "┌", border_style);
        buf.set_string(field_x + field_width + 1, area.y, "┐", border_style);
        buf.set_string(field_x, area.y + 2, "└", border_style);
        buf.set_string(field_x + field_width + 1, area.y + 2, "┘", border_style);
        
        for x in field_x + 1..field_x + field_width + 1 {
            buf.set_string(x, area.y, "─", border_style);
            buf.set_string(x, area.y + 2, "─", border_style);
        }
        
        buf.set_string(field_x, area.y + 1, "│", border_style);
        buf.set_string(field_x + field_width + 1, area.y + 1, "│", border_style);
        
        // Render field content
        let display_value = match self.field_type {
            FormFieldType::Text => state.value.clone(),
            FormFieldType::Password => "*".repeat(state.value.len()),
            FormFieldType::Number => state.value.clone(),
        };
        
        // Calculate visible portion of text
        let visible_width = field_width as usize;
        let value_len = display_value.len();
        
        // Determine what portion of text to display
        let start_index = if state.cursor_position >= visible_width {
            state.cursor_position - visible_width + 1
        } else {
            0
        };
        
        let visible_text = if start_index < value_len {
            let end_index = (start_index + visible_width).min(value_len);
            &display_value[start_index..end_index]
        } else {
            ""
        };
        
        buf.set_string(field_x + 1, area.y + 1, visible_text, style);
        
        // Render cursor
        if self.is_focused {
            let cursor_x = field_x + 1 + (state.cursor_position - start_index) as u16;
            if cursor_x <= field_x + field_width {
                buf.set_style(Rect::new(cursor_x, area.y + 1, 1, 1), style.add_modifier(Modifier::REVERSED));
            }
        }
    }
}

// Usage:
// let field = FormField::new("Username:")
//     .width(20)
//     .style(Style::default())
//     .focused_style(Style::default().fg(Color::Yellow))
//     .focused(is_focused);
// let mut state = FormFieldState { value: "user123".to_string(), cursor_position: 7 };
// f.render_stateful_widget(field, area, &mut state);
```

## Using Custom Widgets

Once you've created your custom widgets, you can use them like any built-in widget:

```rust
// For regular widgets
let label = Label::new("Hello, world!")
    .style(Style::default().fg(Color::Yellow));
f.render_widget(label, chunks[0]);

// For stateful widgets
let mut editor_state = TextEditorState::new();
editor_state.lines = vec!["Hello, world!".to_string()];
editor_state.cursor = (0, 5);

let editor = TextEditor::new()
    .block(Block::default().title("Editor").borders(Borders::ALL))
    .style(Style::default());
f.render_stateful_widget(editor, chunks[1], &mut editor_state);
```

## Best Practices for Custom Widgets

1. **Immutable Builder Pattern**: Make widget constructors and method chains return `self` for a fluid API.
2. **Handle Edge Cases**: Always check if the rendering area is valid (non-zero width and height).
3. **Consistent APIs**: Follow existing Ratatui widget patterns for consistency.
4. **Stateful vs. Stateless**: Choose the appropriate trait based on whether your widget needs to maintain state.
5. **Composition**: Use existing widgets and combine them when possible.
6. **Minimal Redrawing**: Only draw what's needed to improve performance.
7. **Clear Documentation**: Document how your widget works and provide examples.

## Handling User Input

Custom widgets often need to handle user input. This is typically done outside the widget itself:

```rust
// In your application's main event loop
if let Event::Key(key) = event::read()? {
    match key.code {
        KeyCode::Char(c) => {
            if text_editor_state.is_focused {
                text_editor_state.insert_char(c);
            }
        },
        KeyCode::Backspace => {
            if text_editor_state.is_focused {
                text_editor_state.delete_char();
            }
        },
        // ... other key handlers
    }
}
```

By following these patterns, you can create custom widgets that seamlessly integrate with the rest of your Ratatui application.