# Layout System in Ratatui

Ratatui provides a powerful layout system that helps you organize your widgets on the screen. The layout system is based on constraints and can create complex arrangements with minimal code.

## Basic Layout

The core of the layout system is the `Layout` struct, which splits an area into multiple parts:

```rust
use ratatui::layout::{Layout, Direction, Constraint};

let chunks = Layout::default()
    .direction(Direction::Vertical)
    .margin(1)
    .constraints([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .split(f.size());
```

This creates a layout with three rows:
- A top row taking 10% of the available height
- A middle row taking 80% of the available height
- A bottom row taking 10% of the available height

Each resulting rectangle can be used to place widgets:

```rust
let header = Block::default().title("Header").borders(Borders::ALL);
let content = Block::default().title("Content").borders(Borders::ALL);
let footer = Block::default().title("Footer").borders(Borders::ALL);

f.render_widget(header, chunks[0]);
f.render_widget(content, chunks[1]);
f.render_widget(footer, chunks[2]);
```

## Direction

The `Direction` enum controls whether the layout splits horizontally or vertically:

- `Direction::Horizontal`: Splits the area into columns (left to right)
- `Direction::Vertical`: Splits the area into rows (top to bottom)

```rust
// Two columns (left and right)
let horizontal_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);

// Two rows (top and bottom)
let vertical_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);
```

## Constraints

Constraints define how space is allocated within a layout. Ratatui provides several types of constraints:

### Length (Exact Size)

Specifies an exact number of rows or columns:

```rust
use ratatui::layout::Constraint;

Constraint::Length(10) // Exactly 10 rows/columns
```

### Percentage

Allocates a percentage of the available space:

```rust
Constraint::Percentage(25) // 25% of available space
```

### Ratio

Allocates space based on relative proportions:

```rust
// In a 3:2:1 ratio
Constraint::Ratio(3, 6)  // 3/6 = 1/2 of the space
Constraint::Ratio(2, 6)  // 2/6 = 1/3 of the space
Constraint::Ratio(1, 6)  // 1/6 of the space
```

### Min

Specifies a minimum size:

```rust
Constraint::Min(5) // At least 5 rows/columns
```

### Max

Specifies a maximum size:

```rust
Constraint::Max(20) // At most 20 rows/columns
```

### Fill

Uses all remaining space after other constraints are satisfied:

```rust
// Header (fixed 3 rows), content (all remaining space), footer (fixed 3 rows)
[
    Constraint::Length(3),
    Constraint::Fill(1),
    Constraint::Length(3),
]
```

## Nested Layouts

Layouts can be nested to create complex arrangements:

```rust
// First, divide the screen into three rows
let vertical_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),  // Header
        Constraint::Fill(1),    // Content area
        Constraint::Length(3),  // Footer
    ])
    .split(f.size());

// Then, divide the content area into two columns
let horizontal_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(30), // Sidebar
        Constraint::Percentage(70), // Main content
    ])
    .split(vertical_chunks[1]);

// Render widgets in each area
f.render_widget(header_widget, vertical_chunks[0]);
f.render_widget(sidebar_widget, horizontal_chunks[0]);
f.render_widget(main_widget, horizontal_chunks[1]);
f.render_widget(footer_widget, vertical_chunks[2]);
```

## Layout with Margin

Margins add space around the edges of a layout:

```rust
let inner_chunks = Layout::default()
    .direction(Direction::Vertical)
    .margin(1)  // 1 cell margin on all sides
    .constraints([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(outer_area);
```

You can also set different margins for each side:

```rust
use ratatui::layout::Margin;

let chunks = Layout::default()
    .direction(Direction::Vertical)
    .margin(Margin {
        top: 1,
        right: 2,
        bottom: 1,
        left: 2,
    })
    .constraints([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ])
    .split(area);
```

## Alignment

Use `Layout::horizontal_margin` and `Layout::vertical_margin` to create centered layouts:

```rust
// Create a centered box that's 80% of the width and 80% of the height
let centered = Layout::default()
    .direction(Direction::Vertical)
    .horizontal_margin((f.size().width as f64 * 0.1) as u16)
    .vertical_margin((f.size().height as f64 * 0.1) as u16)
    .constraints([Constraint::Fill(1)])
    .split(f.size())[0];
```

## Layout Templates

For common layout patterns, you can create helper functions:

```rust
fn create_centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Create a centered rect using up certain percentage of the available rect
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Usage:
let popup_rect = create_centered_rect(60, 20, f.size());
f.render_widget(popup, popup_rect);
```

## Grid Layout

For grid-like layouts, you can use nested layouts:

```rust
// Create a 3x3 grid
let rows = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

let row1 = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[0]);

let row2 = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[1]);

let row3 = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[2]);

// Now you can access each cell: row1[0], row1[1], row1[2], row2[0], etc.
```

## Rect Utilities

The `layout` module provides utility functions for working with `Rect` objects:

```rust
use ratatui::layout::{Rect, Layout, Direction, Constraint};

// Center a rect within another rect
let centered = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Percentage((100 - height_percent) / 2),
        Constraint::Percentage(height_percent),
        Constraint::Percentage((100 - height_percent) / 2),
    ])
    .split(r)[1];
```

## Practical Examples

### Three-panel layout (sidebar, main, details):

```rust
let chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(20),
        Constraint::Percentage(50),
        Constraint::Percentage(30),
    ])
    .split(f.size());

f.render_widget(sidebar, chunks[0]);
f.render_widget(main_content, chunks[1]);
f.render_widget(details_panel, chunks[2]);
```

### Dashboard with multiple panels:

```rust
// First, divide into header and body
let main_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .split(f.size());

// Then divide body into multiple dashboard panels
let dashboard_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(main_chunks[1]);

// Divide each dashboard row into panels
let top_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 2),
        Constraint::Ratio(1, 2),
    ])
    .split(dashboard_chunks[0]);

let middle_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(dashboard_chunks[1]);

let bottom_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 1),
    ])
    .split(dashboard_chunks[2]);

// Render widgets in each panel
f.render_widget(header, main_chunks[0]);
f.render_widget(panel1, top_chunks[0]);
f.render_widget(panel2, top_chunks[1]);
f.render_widget(panel3, middle_chunks[0]);
f.render_widget(panel4, middle_chunks[1]);
f.render_widget(panel5, middle_chunks[2]);
f.render_widget(panel6, bottom_chunks[0]);
```

### Popup dialog:

```rust
// Get the background area
let background = f.size();

// Create a centered dialog
let area = centered_rect(60, 20, background);

// Render the dialog
f.render_widget(Clear, area); // Clear the background
f.render_widget(dialog, area);

// Helper function for creating a centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
```

The layout system in Ratatui is flexible enough to create almost any arrangement of widgets you need for your terminal UI application.