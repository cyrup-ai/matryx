# Widgets

Widgets are the building blocks of a ratatui application. Each widget is responsible for rendering a specific UI element.

## Core Widgets

### Block

The Block widget is used to create a container with optional borders, title, and padding.

```rust
use ratatui::widgets::{Block, Borders};

let block = Block::default()
    .title("Block Example")
    .borders(Borders::ALL);
```

### Paragraph

The Paragraph widget is used to display text with optional styling and alignment.

```rust
use ratatui::widgets::Paragraph;
use ratatui::style::{Style, Color};
use ratatui::text::{Text, Span};

let text = Text::from(vec![
    Span::styled("This is a ", Style::default()),
    Span::styled("styled", Style::default().fg(Color::Red)),
    Span::styled(" paragraph.", Style::default()),
]);

let paragraph = Paragraph::new(text)
    .block(Block::default().title("Paragraph").borders(Borders::ALL))
    .wrap(Wrap { trim: true });
```

### List

The List widget displays a sequence of items as a list.

```rust
use ratatui::widgets::{List, ListItem};
use ratatui::style::{Style, Color, Modifier};

let items = vec![
    ListItem::new("Item 1"),
    ListItem::new("Item 2"),
    ListItem::new("Item 3"),
];

let list = List::new(items)
    .block(Block::default().title("List").borders(Borders::ALL))
    .highlight_style(Style::default().add_modifier(Modifier::BOLD));
```

### Table

The Table widget displays data in a tabular format.

```rust
use ratatui::widgets::{Table, Row, Cell};
use ratatui::style::{Style, Color};
use ratatui::layout::Constraint;

let rows = vec![
    Row::new(vec![
        Cell::from("Row 1, Col 1"),
        Cell::from("Row 1, Col 2"),
    ]),
    Row::new(vec![
        Cell::from("Row 2, Col 1"),
        Cell::from("Row 2, Col 2"),
    ]),
];

let table = Table::new(rows)
    .header(Row::new(vec!["Header 1", "Header 2"]).style(Style::default().fg(Color::Yellow)))
    .block(Block::default().title("Table").borders(Borders::ALL))
    .widths(&[Constraint::Percentage(50), Constraint::Percentage(50)]);
```

### Chart

The Chart widget displays data as a line, bar, or scatter plot.

```rust
use ratatui::widgets::{Chart, Dataset, Axis};
use ratatui::style::{Style, Color};
use ratatui::symbols;

let data = vec![(0.0, 5.0), (1.0, 6.0), (2.0, 7.0), (3.0, 8.0), (4.0, 4.0)];

let datasets = vec![
    Dataset::default()
        .name("Data 1")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .data(&data),
];

let chart = Chart::new(datasets)
    .block(Block::default().title("Chart").borders(Borders::ALL))
    .x_axis(Axis::default().bounds([0.0, 5.0]))
    .y_axis(Axis::default().bounds([0.0, 10.0]));
```

### Gauge

The Gauge widget displays a progress indicator.

```rust
use ratatui::widgets::Gauge;
use ratatui::style::{Style, Color};

let gauge = Gauge::default()
    .block(Block::default().title("Gauge").borders(Borders::ALL))
    .gauge_style(Style::default().fg(Color::Blue))
    .percent(75);
```

### Sparkline

The Sparkline widget displays data as a small line chart.

```rust
use ratatui::widgets::Sparkline;
use ratatui::style::{Style, Color};

let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1];

let sparkline = Sparkline::default()
    .block(Block::default().title("Sparkline").borders(Borders::ALL))
    .data(&data)
    .style(Style::default().fg(Color::Green));
```

### Barchart

The Barchart widget displays data as a series of bars.

```rust
use ratatui::widgets::BarChart;
use ratatui::style::{Style, Color};

let data = vec![
    ("B1", 9),
    ("B2", 12),
    ("B3", 5),
    ("B4", 8),
    ("B5", 4),
];

let barchart = BarChart::default()
    .block(Block::default().title("BarChart").borders(Borders::ALL))
    .data(&data)
    .bar_width(4)
    .bar_gap(1)
    .bar_style(Style::default().fg(Color::Yellow))
    .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));
```

### Tabs

The Tabs widget displays a collection of selectable tabs.

```rust
use ratatui::widgets::Tabs;
use ratatui::style::{Style, Color};

let titles = vec!["Tab 1", "Tab 2", "Tab 3"];

let tabs = Tabs::new(titles)
    .block(Block::default().title("Tabs").borders(Borders::ALL))
    .select(1)
    .style(Style::default().fg(Color::White))
    .highlight_style(Style::default().fg(Color::Yellow));
```

### Calendar

The Calendar widget displays a monthly calendar.

```rust
use ratatui::widgets::Calendar;
use chrono::{Local, NaiveDate};

let date = Local::now().date_naive();

let calendar = Calendar::new([date.year(), date.month() as u32])
    .block(Block::default().title("Calendar").borders(Borders::ALL));
```

## Custom Widgets

You can create custom widgets by implementing the Widget trait.

```rust
use ratatui::widgets::Widget;
use ratatui::layout::Rect;
use ratatui::buffer::Buffer;

struct CustomWidget {
    // fields for your widget
}

impl Widget for CustomWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Custom rendering logic
    }
}
```

## Composition

Widgets can be composed to create complex interfaces. Use layouts to arrange widgets on the screen.

```rust
use ratatui::layout::{Layout, Direction, Constraint};
use ratatui::widgets::{Block, Borders, Paragraph};

terminal.draw(|f| {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ].as_ref())
        .split(f.size());
    
    let paragraph1 = Paragraph::new("Top paragraph")
        .block(Block::default().title("Block 1").borders(Borders::ALL));
    
    let paragraph2 = Paragraph::new("Bottom paragraph")
        .block(Block::default().title("Block 2").borders(Borders::ALL));
    
    f.render_widget(paragraph1, chunks[0]);
    f.render_widget(paragraph2, chunks[1]);
})?;
```

## StatefulWidgets

Some widgets can maintain state. These implement the StatefulWidget trait.

```rust
use ratatui::widgets::{List, ListItem, ListState};

let items = vec![
    ListItem::new("Item 1"),
    ListItem::new("Item 2"),
    ListItem::new("Item 3"),
];

let list = List::new(items)
    .block(Block::default().title("List").borders(Borders::ALL));

let mut state = ListState::default();
state.select(Some(1)); // Select the second item

f.render_stateful_widget(list, chunks[0], &mut state);
```