use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, StatefulWidget, Widget},
};

/// Tab state for the tabs widget
#[derive(Debug, Clone)]
pub struct TabsState {
    /// Tab titles
    pub titles: Vec<String>,
    /// Index of the selected tab
    pub selected: usize,
}

impl TabsState {
    /// Create a new tab state
    pub fn new(titles: Vec<String>) -> Self {
        Self {
            titles,
            selected: 0,
        }
    }

    /// Get the selected tab title
    pub fn selected_title(&self) -> Option<&String> {
        self.titles.get(self.selected)
    }

    /// Select the next tab
    pub fn next(&mut self) {
        if !self.titles.is_empty() {
            self.selected = (self.selected + 1) % self.titles.len();
        }
    }

    /// Select the previous tab
    pub fn prev(&mut self) {
        if !self.titles.is_empty() {
            self.selected = if self.selected > 0 {
                self.selected - 1
            } else {
                self.titles.len() - 1
            };
        }
    }

    /// Select a tab by index
    pub fn select(&mut self, index: usize) {
        if index < self.titles.len() {
            self.selected = index;
        }
    }

    /// Check if the tab state is empty
    pub fn is_empty(&self) -> bool {
        self.titles.is_empty()
    }

    /// Get the number of tabs
    pub fn len(&self) -> usize {
        self.titles.len()
    }

    /// Add a new tab
    pub fn add_tab(&mut self, title: String) {
        self.titles.push(title);
    }

    /// Remove a tab by index
    pub fn remove_tab(&mut self, index: usize) {
        if index < self.titles.len() {
            self.titles.remove(index);
            if self.selected >= self.titles.len() && !self.titles.is_empty() {
                self.selected = self.titles.len() - 1;
            }
        }
    }

    /// Move a tab from one position to another
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from < self.titles.len() && to < self.titles.len() {
            let title = self.titles.remove(from);
            self.titles.insert(to, title);
            
            // Update selected index if necessary
            if self.selected == from {
                self.selected = to;
            } else if from < self.selected && to >= self.selected {
                self.selected -= 1;
            } else if from > self.selected && to <= self.selected {
                self.selected += 1;
            }
        }
    }
}

/// Tabs widget with a tab bar
pub struct Tabs<'a> {
    /// Block for styling the tabs
    pub block: Option<Block<'a>>,
    /// Style for the tabs
    pub style: Style,
    /// Style for the selected tab
    pub selected_style: Style,
    /// Whether to highlight the selected tab
    pub highlight_selected: bool,
    /// Whether to show the tab borders
    pub show_borders: bool,
}

impl<'a> Default for Tabs<'a> {
    fn default() -> Self {
        Self {
            block: None,
            style: Style::default(),
            selected_style: Style::default().add_modifier(Modifier::BOLD),
            highlight_selected: true,
            show_borders: true,
        }
    }
}

impl<'a> Tabs<'a> {
    /// Create a new tabs widget
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the block for the tabs
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for the tabs
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the style for the selected tab
    pub fn selected_style(mut self, style: Style) -> Self {
        self.selected_style = style;
        self
    }

    /// Set whether to highlight the selected tab
    pub fn highlight_selected(mut self, highlight: bool) -> Self {
        self.highlight_selected = highlight;
        self
    }

    /// Set whether to show the tab borders
    pub fn show_borders(mut self, show: bool) -> Self {
        self.show_borders = show;
        self
    }
}

impl<'a> StatefulWidget for Tabs<'a> {
    type State = TabsState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let area = match self.block {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            }
            None => area,
        };

        if area.height < 1 || state.titles.is_empty() {
            return;
        }

        // Create tab spans
        let mut spans = Vec::new();
        let mut total_width = 0;
        let available_width = area.width as usize;

        // Add tabs until we run out of space
        for (i, title) in state.titles.iter().enumerate() {
            let (prefix, suffix) = if self.show_borders {
                ("|", "")
            } else {
                ("", " ")
            };

            let tab_width = title.len() + prefix.len() + suffix.len();
            
            // Check if we have space for this tab
            if total_width + tab_width > available_width && !spans.is_empty() {
                // No more space, add ellipsis and stop
                if total_width + 3 <= available_width {
                    spans.push(Span::raw("..."));
                }
                break;
            }

            // Add the tab with proper styling
            let style = if i == state.selected && self.highlight_selected {
                self.selected_style
            } else {
                self.style
            };

            spans.push(Span::styled(prefix, style));
            spans.push(Span::styled(title.clone(), style));
            spans.push(Span::styled(suffix, style));
            
            total_width += tab_width;
        }

        // Create the tab line
        let line = Line::from(spans);
        
        // Render the tabs
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

/// Creates a Tabs widget with preset styles
pub fn tabs() -> Tabs<'static> {
    Tabs::default()
        .style(Style::default().fg(Color::Gray))
        .selected_style(
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_selected(true)
}