use clipboard::{ClipboardContext, ClipboardProvider};
use lazy_static::lazy_static;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, StatefulWidget, Widget},
};
use std::{collections::HashMap, time::Instant};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::modal::{ModalState, Mode};

/// Cursor position in the text editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    /// Line number (0-indexed)
    pub line: usize,
    /// Column number (0-indexed)
    pub column: usize,
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self { line: 0, column: 0 }
    }
}

impl CursorPosition {
    /// Create a new cursor position
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Move to the beginning of the line
    pub fn beginning_of_line(&self) -> Self {
        Self { line: self.line, column: 0 }
    }

    /// Move to the end of the line
    pub fn end_of_line(&self, line_length: usize) -> Self {
        Self { line: self.line, column: line_length }
    }
}

/// Selection range in the text editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Start position of selection
    pub start: CursorPosition,
    /// End position of selection
    pub end: CursorPosition,
}

impl Selection {
    /// Create a new selection
    pub fn new(start: CursorPosition, end: CursorPosition) -> Self {
        Self { start, end }
    }

    /// Check if the selection is empty
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Get the actual start and end positions for iterating (ensuring start <= end)
    pub fn ordered(&self) -> (CursorPosition, CursorPosition) {
        if self.start.line < self.end.line ||
            (self.start.line == self.end.line && self.start.column <= self.end.column)
        {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }
}

/// Represents a document language for syntax highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentLanguage {
    PlainText,
    Rust,
    Markdown,
    Html,
    Json,
    Toml,
    Yaml,
    Python,
    JavaScript,
    TypeScript,
    Cpp,
    Go,
    Matrix,
    // Add more as needed
}

impl DocumentLanguage {
    pub fn from_filename(filename: &str) -> Self {
        if let Some(extension) = filename.split('.').last() {
            match extension.to_lowercase().as_str() {
                "rs" => Self::Rust,
                "md" | "markdown" => Self::Markdown,
                "html" | "htm" => Self::Html,
                "json" => Self::Json,
                "toml" => Self::Toml,
                "yml" | "yaml" => Self::Yaml,
                "py" => Self::Python,
                "js" => Self::JavaScript,
                "ts" => Self::TypeScript,
                "cpp" | "cc" | "cxx" | "c++" | "h" | "hpp" => Self::Cpp,
                "go" => Self::Go,
                _ => Self::PlainText,
            }
        } else {
            Self::PlainText
        }
    }

    pub fn get_syntect_name(&self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::Markdown => "Markdown",
            Self::Html => "HTML",
            Self::Json => "JSON",
            Self::Toml => "TOML",
            Self::Yaml => "YAML",
            Self::Python => "Python",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::Cpp => "C++",
            Self::Go => "Go",
            Self::Matrix => "Markdown", // Use Markdown highlighting for Matrix content
        }
    }
}

/// Undo/Redo operation
#[derive(Debug, Clone)]
pub enum UndoOperation {
    Insert {
        position: CursorPosition,
        text: String,
    },
    Delete {
        position: CursorPosition,
        text: String,
        is_backward: bool,
    },
    Replace {
        start: CursorPosition,
        end: CursorPosition,
        old_text: String,
        new_text: String,
    },
}

/// State for the text editor widget
#[derive(Debug, Clone)]
pub struct TextEditorState {
    /// Content of the text editor as lines of text
    pub lines: Vec<String>,
    /// Current cursor position
    pub cursor: CursorPosition,
    /// Current selection (if any)
    pub selection: Option<Selection>,
    /// Current scroll offset
    pub scroll_offset: (usize, usize),
    /// Modal state for the editor
    pub modal_state: ModalState,
    /// Show line numbers
    pub show_line_numbers: bool,
    /// Read-only mode
    pub read_only: bool,
    /// Document language for syntax highlighting
    pub language: DocumentLanguage,
    /// Undo history
    pub undo_stack: Vec<UndoOperation>,
    /// Redo history
    pub redo_stack: Vec<UndoOperation>,
    /// Syntax highlighting cache
    pub highlight_cache: HashMap<usize, Vec<(Style, String)>>,
    /// Syntax cache is dirty (needs refresh)
    pub highlight_cache_dirty: bool,
    /// Timestamp when the cache was last updated
    pub last_highlight_update: Instant,
    /// Clipboard context
    pub clipboard: Option<ClipboardContext>,
    /// Completion suggestions
    pub completion_suggestions: Vec<String>,
    /// Completion query (current word being completed)
    pub completion_query: String,
    /// Completion active
    pub completion_active: bool,
    /// Selected completion index
    pub completion_selected: usize,
}

impl Default for TextEditorState {
    fn default() -> Self {
        let clipboard = ClipboardProvider::new().ok();

        Self {
            lines: vec![String::new()],
            cursor: CursorPosition::default(),
            selection: None,
            scroll_offset: (0, 0),
            modal_state: ModalState::default(),
            show_line_numbers: true,
            read_only: false,
            language: DocumentLanguage::PlainText,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            highlight_cache: HashMap::new(),
            highlight_cache_dirty: true,
            last_highlight_update: Instant::now(),
            clipboard,
        }
    }
}

// Shared syntect resources
lazy_static::lazy_static! {
    static ref SYNTAX_SET: SyntaxSet = SyntaxSet::load_defaults_newlines();
    static ref THEME_SET: ThemeSet = ThemeSet::load_defaults();
}

impl TextEditorState {
    /// Create a new text editor state
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new text editor with specific language
    pub fn with_language(language: DocumentLanguage) -> Self {
        let mut state = Self::default();
        state.language = language;
        state
    }

    /// Create a new text editor from a file
    pub fn from_file(content: &str, filename: &str) -> Self {
        let language = DocumentLanguage::from_filename(filename);
        let mut state = Self::with_language(language);
        state.set_content(content);
        state
    }

    /// Set content from a string (splitting by newlines)
    pub fn set_content(&mut self, content: &str) {
        self.lines = content.lines().map(String::from).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = CursorPosition::default();
        self.selection = None;
        self.scroll_offset = (0, 0);
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.highlight_cache.clear();
        self.highlight_cache_dirty = true;
    }

    /// Get content as a single string
    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    /// Get current line
    pub fn current_line(&self) -> &str {
        &self.lines[self.cursor.line]
    }

    /// Set the document language
    pub fn set_language(&mut self, language: DocumentLanguage) {
        self.language = language;
        self.highlight_cache.clear();
        self.highlight_cache_dirty = true;
    }

    /// Undo the last operation
    pub fn undo(&mut self) -> bool {
        if self.read_only {
            return false;
        }

        if let Some(operation) = self.undo_stack.pop() {
            self.redo_stack.push(operation.clone());

            match operation {
                UndoOperation::Insert { position, text } => {
                    // For insert, we need to delete the text
                    self.cursor = position;
                    let end_pos = self.get_position_after_text(position, &text);
                    self.selection = Some(Selection::new(position, end_pos));
                    self.delete_selection();
                    self.selection = None;
                },
                UndoOperation::Delete { position, text, is_backward } => {
                    // For delete, we need to insert the text back
                    self.cursor = position;
                    self.insert_text(&text);

                    // Adjust cursor position
                    if is_backward {
                        self.cursor = position;
                    }
                },
                UndoOperation::Replace { start, end, old_text, new_text } => {
                    // For replace, restore the old text
                    self.cursor = start;
                    self.selection = Some(Selection::new(start, end));
                    self.delete_selection();
                    self.selection = None;
                    self.insert_text(&old_text);
                },
            }

            self.highlight_cache_dirty = true;
            true
        } else {
            false
        }
    }

    /// Redo the last undone operation
    pub fn redo(&mut self) -> bool {
        if self.read_only {
            return false;
        }

        if let Some(operation) = self.redo_stack.pop() {
            self.undo_stack.push(operation.clone());

            match operation {
                UndoOperation::Insert { position, text } => {
                    // For insert, we need to insert the text again
                    self.cursor = position;
                    self.insert_text(&text);
                },
                UndoOperation::Delete { position, text, is_backward } => {
                    // For delete, we need to delete the text again
                    self.cursor = position;
                    let end_pos = self.get_position_after_text(position, &text);
                    self.selection = Some(Selection::new(position, end_pos));
                    self.delete_selection();
                    self.selection = None;

                    // Adjust cursor position
                    if !is_backward {
                        self.cursor = position;
                    }
                },
                UndoOperation::Replace { start, end, old_text, new_text } => {
                    // For replace, apply the new text again
                    self.cursor = start;
                    self.selection = Some(Selection::new(start, end));
                    self.delete_selection();
                    self.selection = None;
                    self.insert_text(&new_text);
                },
            }

            self.highlight_cache_dirty = true;
            true
        } else {
            false
        }
    }

    /// Copy selected text to clipboard
    pub fn copy(&mut self) -> bool {
        if let Some(text) = self.selected_text() {
            if let Some(clipboard) = &mut self.clipboard {
                if let Ok(()) = clipboard.set_contents(text) {
                    return true;
                }
            }
        }
        false
    }

    /// Cut selected text to clipboard
    pub fn cut(&mut self) -> bool {
        if self.read_only {
            return false;
        }

        if let Some(text) = self.selected_text() {
            if let Some(clipboard) = &mut self.clipboard {
                if let Ok(()) = clipboard.set_contents(text) {
                    // Record operation for undo
                    if let Some(selection) = self.selection {
                        let (start, end) = selection.ordered();
                        self.undo_stack.push(UndoOperation::Delete {
                            position: start,
                            text,
                            is_backward: false,
                        });
                        self.redo_stack.clear();
                    }

                    self.delete_selection();
                    self.highlight_cache_dirty = true;
                    return true;
                }
            }
        }
        false
    }

    /// Paste text from clipboard
    pub fn paste(&mut self) -> bool {
        if self.read_only {
            return false;
        }

        if let Some(clipboard) = &mut self.clipboard {
            if let Ok(text) = clipboard.get_contents() {
                // Record operation for undo
                let cursor_pos = self.cursor;
                self.undo_stack
                    .push(UndoOperation::Insert { position: cursor_pos, text: text.clone() });
                self.redo_stack.clear();

                // Delete selection if any
                if self.selection.is_some() {
                    self.delete_selection();
                }

                // Insert text
                self.insert_text(&text);
                self.highlight_cache_dirty = true;
                return true;
            }
        }
        false
    }

    /// Find a string in the document content
    pub fn find(&self, query: &str, case_sensitive: bool) -> Vec<(CursorPosition, CursorPosition)> {
        let mut results = Vec::new();

        if query.is_empty() {
            return results;
        }

        let query = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        for (line_idx, line) in self.lines.iter().enumerate() {
            let line_to_search = if case_sensitive {
                line.as_str()
            } else {
                &line.to_lowercase()
            };

            let mut start_idx = 0;
            while let Some(match_idx) = line_to_search[start_idx..].find(&query) {
                let match_start = start_idx + match_idx;
                let match_end = match_start + query.len();

                results.push((
                    CursorPosition::new(line_idx, match_start),
                    CursorPosition::new(line_idx, match_end),
                ));

                start_idx = match_start + 1;
            }
        }

        results
    }

    /// Replace all occurrences of a string
    pub fn replace_all(&mut self, find: &str, replace: &str, case_sensitive: bool) -> usize {
        if self.read_only {
            return 0;
        }

        let matches = self.find(find, case_sensitive);
        let count = matches.len();

        // Start from the end to avoid position shifts
        for (start, end) in matches.into_iter().rev() {
            // Record for undo
            let old_text = self.get_text_between(start, end);

            self.undo_stack.push(UndoOperation::Replace {
                start,
                end,
                old_text,
                new_text: replace.to_string(),
            });

            // Perform the replacement
            self.cursor = start;
            self.selection = Some(Selection::new(start, end));
            self.delete_selection();
            self.selection = None;
            self.insert_text(replace);
        }

        if count > 0 {
            self.redo_stack.clear();
            self.highlight_cache_dirty = true;
        }

        count
    }

    /// Get position after inserting text
    fn get_position_after_text(&self, start: CursorPosition, text: &str) -> CursorPosition {
        let mut pos = start;

        for c in text.chars() {
            if c == '\n' {
                pos.line += 1;
                pos.column = 0;
            } else {
                pos.column += 1;
            }
        }

        pos
    }

    /// Get text between two positions
    fn get_text_between(&self, start: CursorPosition, end: CursorPosition) -> String {
        let (start, end) =
            if start.line < end.line || (start.line == end.line && start.column <= end.column) {
                (start, end)
            } else {
                (end, start)
            };

        if start.line == end.line {
            // Within a single line
            let line = &self.lines[start.line];
            let graphemes = line.graphemes(true).collect::<Vec<_>>();
            graphemes[start.column..end.column.min(graphemes.len())].join("")
        } else {
            // Across multiple lines
            let mut result = String::new();

            // First line (partial)
            let first_line = &self.lines[start.line];
            let first_graphemes = first_line.graphemes(true).collect::<Vec<_>>();
            result.push_str(&first_graphemes[start.column.min(first_graphemes.len())..].join(""));
            result.push('\n');

            // Middle lines (complete)
            for line in &self.lines[(start.line + 1)..end.line] {
                result.push_str(line);
                result.push('\n');
            }

            // Last line (partial)
            let last_line = &self.lines[end.line];
            let last_graphemes = last_line.graphemes(true).collect::<Vec<_>>();
            result.push_str(&last_graphemes[..end.column.min(last_graphemes.len())].join(""));

            result
        }
    }

    /// Insert text at current cursor position
    pub fn insert_text(&mut self, text: &str) {
        if self.read_only {
            return;
        }

        // Split text into lines
        let mut lines = text.split('\n').collect::<Vec<_>>();

        if lines.is_empty() {
            return;
        }

        // Handle the first line (append to current line or insert at cursor)
        let current_line = &self.lines[self.cursor.line];
        let graphemes = current_line.graphemes(true).collect::<Vec<_>>();
        let column = self.cursor.column.min(graphemes.len());

        let (before, after) = graphemes.split_at(column);
        let before_str = before.join("");
        let mut new_line = before_str + lines[0];

        // If there's only one line in the text, append the rest of the current line
        if lines.len() == 1 {
            new_line += &after.join("");
            self.lines[self.cursor.line] = new_line;

            // Move cursor to the end of the inserted text
            self.cursor.column += lines[0].graphemes(true).count();
        } else {
            // Handle multi-line insertion
            self.lines[self.cursor.line] = new_line;

            // Add all intermediate lines
            for (i, line) in lines.iter().enumerate().skip(1).take(lines.len() - 2) {
                self.lines.insert(self.cursor.line + i, line.to_string());
            }

            // Add the last line + remainder of current line
            let last_line = lines[lines.len() - 1].to_string() + &after.join("");
            self.lines.insert(self.cursor.line + lines.len() - 1, last_line);

            // Move cursor to the end of the inserted text
            self.cursor.line += lines.len() - 1;
            self.cursor.column = lines[lines.len() - 1].graphemes(true).count();
        }

        self.highlight_cache_dirty = true;
    }

    /// Get syntax highlighted styles for a line
    pub fn get_highlighted_line(&mut self, line_idx: usize) -> Option<Vec<(Style, String)>> {
        // Return cached highlights if available and not dirty
        if !self.highlight_cache_dirty && self.highlight_cache.contains_key(&line_idx) {
            return self.highlight_cache.get(&line_idx).cloned();
        }

        // Check if we need to refresh the entire cache
        if self.highlight_cache_dirty {
            // Only refresh at most once per 100ms to avoid excessive CPU usage
            let now = Instant::now();
            if now.duration_since(self.last_highlight_update).as_millis() < 100 {
                // Return existing cache or fallback to plain text
                return self
                    .highlight_cache
                    .get(&line_idx)
                    .cloned()
                    .or_else(|| Some(vec![(Style::default(), self.lines[line_idx].clone())]));
            }

            // Clear the cache and mark it as clean
            self.highlight_cache.clear();
            self.highlight_cache_dirty = false;
            self.last_highlight_update = now;
        }

        // Check if we're using syntax highlighting
        if self.language == DocumentLanguage::PlainText {
            // Just return the line as-is
            let result = vec![(Style::default(), self.lines[line_idx].clone())];
            self.highlight_cache.insert(line_idx, result.clone());
            return Some(result);
        }

        // Try to find the syntax definition
        if let Ok(syntax) = SYNTAX_SET.find_syntax_by_name(self.language.get_syntect_name()) {
            // Highlight using syntect
            let mut highlighter =
                HighlightLines::new(syntax, &THEME_SET.themes["base16-ocean.dark"]);

            // Get the line
            let line = &self.lines[line_idx];

            // Highlight the line
            if let Ok(ranges) = highlighter.highlight_line(line, &SYNTAX_SET) {
                // Convert syntect styles to ratatui styles
                let styled_ranges = ranges
                    .iter()
                    .map(|(style, text)| {
                        let fg_color = style.foreground;
                        let ratatui_style =
                            Style::default().fg(Color::Rgb(fg_color.r, fg_color.g, fg_color.b));
                        (ratatui_style, text.to_string())
                    })
                    .collect::<Vec<_>>();

                // Cache and return the result
                self.highlight_cache.insert(line_idx, styled_ranges.clone());
                return Some(styled_ranges);
            }
        }

        // Fallback to plain text if highlighting fails
        let result = vec![(Style::default(), self.lines[line_idx].clone())];
        self.highlight_cache.insert(line_idx, result.clone());
        Some(result)
    }

    /// Insert character at current cursor position
    pub fn insert_char(&mut self, c: char) {
        if self.read_only {
            return;
        }

        if c == '\n' {
            self.insert_newline();
            return;
        }

        // Record for undo
        let cursor_pos = self.cursor;
        self.undo_stack
            .push(UndoOperation::Insert { position: cursor_pos, text: c.to_string() });
        self.redo_stack.clear();

        // Delete selection if any
        if self.selection.is_some() {
            self.delete_selection();
        }

        let line = &mut self.lines[self.cursor.line];
        let graphemes = line.graphemes(true).collect::<Vec<_>>();
        let column = self.cursor.column.min(graphemes.len());

        let mut new_line = String::new();
        for (i, g) in graphemes.iter().enumerate() {
            if i == column {
                new_line.push(c);
            }
            new_line.push_str(g);
        }

        if column == graphemes.len() {
            new_line.push(c);
        }

        self.lines[self.cursor.line] = new_line;
        self.cursor.column += 1;
        self.highlight_cache_dirty = true;
    }

    /// Delete character before the cursor
    pub fn delete_char_backward(&mut self) {
        if self.read_only {
            return;
        }

        // If there's a selection, delete it instead
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }

        if self.cursor.column == 0 {
            if self.cursor.line > 0 {
                // Join with previous line
                let current_line = self.lines.remove(self.cursor.line);
                let prev_line_end = self.lines[self.cursor.line - 1].graphemes(true).count();

                // Record for undo
                self.undo_stack.push(UndoOperation::Delete {
                    position: CursorPosition::new(self.cursor.line - 1, prev_line_end),
                    text: "\n".to_string(),
                    is_backward: true,
                });
                self.redo_stack.clear();

                self.cursor.line -= 1;
                self.cursor.column = prev_line_end;
                self.lines[self.cursor.line].push_str(&current_line);
                self.highlight_cache_dirty = true;
            }
            return;
        }

        let line = &mut self.lines[self.cursor.line];
        let graphemes = line.graphemes(true).collect::<Vec<_>>();
        let column = self.cursor.column.min(graphemes.len());

        if column > 0 {
            // Record for undo
            let deleted_char = graphemes[column - 1].to_string();
            self.undo_stack.push(UndoOperation::Delete {
                position: CursorPosition::new(self.cursor.line, column - 1),
                text: deleted_char,
                is_backward: true,
            });
            self.redo_stack.clear();

            let mut new_line = String::new();
            for (i, g) in graphemes.iter().enumerate() {
                if i != column - 1 {
                    new_line.push_str(g);
                }
            }
            self.lines[self.cursor.line] = new_line;
            self.cursor.column -= 1;
            self.highlight_cache_dirty = true;
        }
    }

    /// Delete character at the cursor
    pub fn delete_char_forward(&mut self) {
        if self.read_only {
            return;
        }

        // If there's a selection, delete it instead
        if self.selection.is_some() {
            self.delete_selection();
            return;
        }

        let line = &mut self.lines[self.cursor.line];
        let graphemes = line.graphemes(true).collect::<Vec<_>>();
        let column = self.cursor.column.min(graphemes.len());

        if column < graphemes.len() {
            // Record for undo
            let deleted_char = graphemes[column].to_string();
            self.undo_stack.push(UndoOperation::Delete {
                position: CursorPosition::new(self.cursor.line, column),
                text: deleted_char,
                is_backward: false,
            });
            self.redo_stack.clear();

            let mut new_line = String::new();
            for (i, g) in graphemes.iter().enumerate() {
                if i != column {
                    new_line.push_str(g);
                }
            }
            self.lines[self.cursor.line] = new_line;
            self.highlight_cache_dirty = true;
        } else if self.cursor.line < self.lines.len() - 1 {
            // At end of line, join with next line
            // Record for undo
            self.undo_stack.push(UndoOperation::Delete {
                position: CursorPosition::new(self.cursor.line, column),
                text: "\n".to_string(),
                is_backward: false,
            });
            self.redo_stack.clear();

            let next_line = self.lines.remove(self.cursor.line + 1);
            self.lines[self.cursor.line].push_str(&next_line);
            self.highlight_cache_dirty = true;
        }
    }

    /// Insert a newline at the cursor position
    pub fn insert_newline(&mut self) {
        if self.read_only {
            return;
        }

        // Record for undo
        let cursor_pos = self.cursor;
        self.undo_stack
            .push(UndoOperation::Insert { position: cursor_pos, text: "\n".to_string() });
        self.redo_stack.clear();

        // Delete selection if any
        if self.selection.is_some() {
            self.delete_selection();
        }

        let current_line = &self.lines[self.cursor.line];
        let graphemes = current_line.graphemes(true).collect::<Vec<_>>();
        let column = self.cursor.column.min(graphemes.len());

        let (before, after) = graphemes.split_at(column);
        let before_str = before.join("");
        let after_str = after.join("");

        self.lines[self.cursor.line] = before_str;
        self.lines.insert(self.cursor.line + 1, after_str);

        self.cursor.line += 1;
        self.cursor.column = 0;
        self.highlight_cache_dirty = true;
    }

    /// Move cursor to position
    pub fn move_cursor_to(&mut self, position: CursorPosition) {
        self.cursor = self.clamp_position(position);
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor.column > 0 {
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.lines[self.cursor.line].graphemes(true).count();
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        let line_len = self.lines[self.cursor.line].graphemes(true).count();
        if self.cursor.column < line_len {
            self.cursor.column += 1;
        } else if self.cursor.line < self.lines.len() - 1 {
            self.cursor.line += 1;
            self.cursor.column = 0;
        }
    }

    /// Move cursor up
    pub fn move_cursor_up(&mut self) {
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let line_len = self.lines[self.cursor.line].graphemes(true).count();
            self.cursor.column = self.cursor.column.min(line_len);
        }
    }

    /// Move cursor down
    pub fn move_cursor_down(&mut self) {
        if self.cursor.line < self.lines.len() - 1 {
            self.cursor.line += 1;
            let line_len = self.lines[self.cursor.line].graphemes(true).count();
            self.cursor.column = self.cursor.column.min(line_len);
        }
    }

    /// Move cursor to beginning of line
    pub fn move_cursor_begin_of_line(&mut self) {
        self.cursor.column = 0;
    }

    /// Move cursor to end of line
    pub fn move_cursor_end_of_line(&mut self) {
        let line_len = self.lines[self.cursor.line].graphemes(true).count();
        self.cursor.column = line_len;
    }

    /// Start selection at current cursor position
    pub fn start_selection(&mut self) {
        self.selection = Some(Selection::new(self.cursor, self.cursor));
    }

    /// Update selection end to current cursor position
    pub fn update_selection(&mut self) {
        if let Some(selection) = &mut self.selection {
            selection.end = self.cursor;
        }
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Delete selected text
    pub fn delete_selection(&mut self) {
        if self.read_only {
            return;
        }

        if let Some(selection) = self.selection.take() {
            let (start, end) = selection.ordered();

            // Get the selected text for undo
            let selected_text = self.get_text_between(start, end);

            // Record the operation for undo (unless it's already being handled by a caller)
            // - cut() and paste() record their own operations
            // - insert_char() and delete_char_*() handle their own cases
            let backtrace = std::backtrace::Backtrace::capture();
            let caller_name = backtrace.to_string();

            if !caller_name.contains("cut") &&
                !caller_name.contains("paste") &&
                !caller_name.contains("insert_char") &&
                !caller_name.contains("delete_char")
            {
                self.undo_stack.push(UndoOperation::Delete {
                    position: start,
                    text: selected_text.clone(),
                    is_backward: false,
                });
                self.redo_stack.clear();
            }

            if start.line == end.line {
                // Delete within a single line
                let line = &mut self.lines[start.line];
                let graphemes = line.graphemes(true).collect::<Vec<_>>();
                let mut new_line = String::new();

                for (i, g) in graphemes.iter().enumerate() {
                    if i < start.column || i >= end.column {
                        new_line.push_str(g);
                    }
                }

                self.lines[start.line] = new_line;
                self.cursor = start;
            } else {
                // Delete across multiple lines
                let start_line = &self.lines[start.line];
                let end_line = &self.lines[end.line];

                let start_graphemes = start_line.graphemes(true).collect::<Vec<_>>();
                let end_graphemes = end_line.graphemes(true).collect::<Vec<_>>();

                let mut new_line = String::new();

                // Keep beginning of first line
                for (i, g) in start_graphemes.iter().enumerate() {
                    if i < start.column {
                        new_line.push_str(g);
                    }
                }

                // Add end of last line
                for (i, g) in end_graphemes.iter().enumerate() {
                    if i >= end.column {
                        new_line.push_str(g);
                    }
                }

                // Replace first line with combined content
                self.lines[start.line] = new_line;

                // Remove lines in between
                self.lines.drain((start.line + 1)..=end.line);

                self.cursor = start;
            }

            self.highlight_cache_dirty = true;
        }
    }

    /// Get text within selection
    pub fn selected_text(&self) -> Option<String> {
        self.selection.map(|selection| {
            let (start, end) = selection.ordered();

            if start.line == end.line {
                // Selection within a single line
                let line = &self.lines[start.line];
                let graphemes = line.graphemes(true).collect::<Vec<_>>();
                graphemes[start.column..end.column.min(graphemes.len())].join("")
            } else {
                // Selection across multiple lines
                let mut result = String::new();

                // First line (partial)
                let first_line = &self.lines[start.line];
                let first_graphemes = first_line.graphemes(true).collect::<Vec<_>>();
                result
                    .push_str(&first_graphemes[start.column.min(first_graphemes.len())..].join(""));
                result.push('\n');

                // Middle lines (complete)
                for line in &self.lines[(start.line + 1)..end.line] {
                    result.push_str(line);
                    result.push('\n');
                }

                // Last line (partial)
                let last_line = &self.lines[end.line];
                let last_graphemes = last_line.graphemes(true).collect::<Vec<_>>();
                result.push_str(&last_graphemes[..end.column.min(last_graphemes.len())].join(""));

                result
            }
        })
    }

    /// Ensure position is within bounds of the text
    fn clamp_position(&self, pos: CursorPosition) -> CursorPosition {
        let line = pos.line.min(self.lines.len() - 1);
        let column = pos.column.min(self.lines[line].graphemes(true).count());
        CursorPosition::new(line, column)
    }

    /// Adjust viewport to ensure cursor is visible
    pub fn ensure_cursor_visible(&mut self, area: Rect) {
        let line_numbers_width = if self.show_line_numbers {
            self.lines.len().to_string().len() + 1
        } else {
            0
        };

        let viewport_height = area.height as usize;
        let viewport_width = area.width.saturating_sub(line_numbers_width as u16) as usize;

        // Vertical scrolling
        if self.cursor.line < self.scroll_offset.0 {
            self.scroll_offset.0 = self.cursor.line;
        } else if self.cursor.line >= self.scroll_offset.0 + viewport_height {
            self.scroll_offset.0 = self.cursor.line.saturating_sub(viewport_height) + 1;
        }

        // Horizontal scrolling
        if self.cursor.column < self.scroll_offset.1 {
            self.scroll_offset.1 = self.cursor.column;
        } else if self.cursor.column >= self.scroll_offset.1 + viewport_width {
            self.scroll_offset.1 = self.cursor.column.saturating_sub(viewport_width) + 1;
        }
    }
}

/// Text editor widget with modal editing support
pub struct TextEditor<'a> {
    /// Block for styling the widget
    pub block: Option<Block<'a>>,
    /// Style for normal text
    pub style: Style,
    /// Style for the cursor
    pub cursor_style: Style,
    /// Style for selected text
    pub selection_style: Style,
    /// Style for line numbers
    pub line_number_style: Style,
}

// Include tests module
#[path = "texteditor_tests.rs"]
mod tests;

impl<'a> Default for TextEditor<'a> {
    fn default() -> Self {
        Self {
            block: None,
            style: Style::default(),
            cursor_style: Style::default().bg(Color::White).fg(Color::Black),
            selection_style: Style::default().bg(Color::DarkGray),
            line_number_style: Style::default().fg(Color::DarkGray),
        }
    }
}

impl<'a> TextEditor<'a> {
    /// Create a new text editor widget
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the block for the widget
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for normal text
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the style for the cursor
    pub fn cursor_style(mut self, style: Style) -> Self {
        self.cursor_style = style;
        self
    }

    /// Set the style for selected text
    pub fn selection_style(mut self, style: Style) -> Self {
        self.selection_style = style;
        self
    }

    /// Set the style for line numbers
    pub fn line_number_style(mut self, style: Style) -> Self {
        self.line_number_style = style;
        self
    }
}

impl<'a> StatefulWidget for TextEditor<'a> {
    type State = TextEditorState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Apply block if provided
        let render_area = match self.block {
            Some(b) => {
                let inner_area = b.inner(area);
                b.render(area, buf);
                inner_area
            },
            None => area,
        };

        if render_area.width == 0 || render_area.height == 0 {
            return;
        }

        state.ensure_cursor_visible(render_area);

        let line_numbers_width = if state.show_line_numbers {
            state.lines.len().to_string().len() + 1
        } else {
            0
        };

        let text_area = Rect {
            x: render_area.x + line_numbers_width as u16,
            y: render_area.y,
            width: render_area.width.saturating_sub(line_numbers_width as u16),
            height: render_area.height,
        };

        // Render visible lines
        for i in 0..render_area.height as usize {
            let line_idx = state.scroll_offset.0 + i;
            if line_idx >= state.lines.len() {
                break;
            }

            let y = render_area.y + i as u16;

            // Render line number if enabled
            if state.show_line_numbers {
                let line_number = line_idx + 1;
                let line_number_str =
                    format!("{:>width$}", line_number, width = line_numbers_width - 1);
                buf.set_string(render_area.x, y, line_number_str, self.line_number_style);
            }

            // Get highlighted line
            let line_spans = if state.language != DocumentLanguage::PlainText {
                // Use syntax highlighting
                state
                    .get_highlighted_line(line_idx)
                    .unwrap_or_else(|| vec![(self.style, state.lines[line_idx].clone())])
            } else {
                // No syntax highlighting - just use the line as is
                vec![(self.style, state.lines[line_idx].clone())]
            };

            // Apply selection highlighting if needed
            let mut rendered_spans = Vec::new();

            if let Some(selection) = state.selection {
                let (sel_start, sel_end) = selection.ordered();

                // Determine if this line has any selection
                let has_selection = line_idx >= sel_start.line && line_idx <= sel_end.line;

                if has_selection {
                    // Calculate selection boundaries on this line
                    let sel_start_col = if line_idx == sel_start.line {
                        sel_start.column
                    } else {
                        0
                    };
                    let sel_end_col = if line_idx == sel_end.line {
                        sel_end.column
                    } else {
                        usize::MAX
                    };

                    // Process each highlighted span
                    for (style, text) in line_spans {
                        let mut current_col = 0;
                        let graphemes = text.graphemes(true).collect::<Vec<_>>();

                        let mut current_span = String::new();
                        let mut current_style = style;

                        for grapheme in graphemes {
                            let is_selected =
                                current_col >= sel_start_col && current_col < sel_end_col;

                            let span_style = if is_selected {
                                self.selection_style.patch(style)
                            } else {
                                style
                            };

                            // If style changes, finish the current span
                            if span_style != current_style || current_span.is_empty() {
                                if !current_span.is_empty() {
                                    rendered_spans.push(Span::styled(current_span, current_style));
                                    current_span = String::new();
                                }
                                current_style = span_style;
                            }

                            current_span.push_str(grapheme);
                            current_col += 1;
                        }

                        // Add the final span
                        if !current_span.is_empty() {
                            rendered_spans.push(Span::styled(current_span, current_style));
                        }
                    }
                } else {
                    // No selection on this line, just use the highlighted spans
                    for (style, text) in line_spans {
                        rendered_spans.push(Span::styled(
                            text.graphemes(true)
                                .skip(state.scroll_offset.1)
                                .take(text_area.width as usize)
                                .collect::<String>(),
                            style,
                        ));
                    }
                }
            } else {
                // No selection, just use the highlighted spans
                for (style, text) in line_spans {
                    let visible_text = text
                        .graphemes(true)
                        .skip(state.scroll_offset.1)
                        .take(text_area.width as usize)
                        .collect::<String>();

                    if !visible_text.is_empty() {
                        rendered_spans.push(Span::styled(visible_text, style));
                    }
                }
            }

            // Render the line
            let line_text = Line::from(rendered_spans);
            buf.set_line(text_area.x, y, &line_text, text_area.width);

            // Render cursor (if on this line)
            if state.cursor.line == line_idx &&
                state.cursor.column >= state.scroll_offset.1 &&
                (state.cursor.column - state.scroll_offset.1) < text_area.width as usize
            {
                let cursor_x = text_area.x + (state.cursor.column - state.scroll_offset.1) as u16;

                // Determine cursor character
                let cursor_char =
                    if state.cursor.column < state.lines[line_idx].graphemes(true).count() {
                        state.lines[line_idx]
                            .graphemes(true)
                            .nth(state.cursor.column)
                            .unwrap_or(" ")
                    } else {
                        " "
                    };

                // Different cursor style based on mode
                let cursor_style = match state.modal_state.mode() {
                    Mode::Normal => Style::default().fg(Color::Black).bg(Color::Gray),
                    Mode::Insert => Style::default().fg(Color::Black).bg(Color::Green),
                    Mode::Visual => Style::default().fg(Color::Black).bg(Color::Blue),
                    _ => self.cursor_style,
                };

                // Set cursor cell
                buf.set_string(cursor_x, y, cursor_char, cursor_style);
            }
        }

        // Render status line at the bottom
        if render_area.height > 1 {
            let status_y = render_area.y + render_area.height - 1;

            // Mode indicator
            let mode_text = match state.modal_state.mode() {
                Mode::Normal => "NORMAL",
                Mode::Insert => "INSERT",
                Mode::Visual => "VISUAL",
                _ => "COMMAND",
            };

            // File info
            let file_info = format!(
                "{} | {}",
                state.language.get_syntect_name(),
                if state.read_only { "READ ONLY" } else { "EDIT" }
            );

            // Position info
            let position_info = format!("{}:{}", state.cursor.line + 1, state.cursor.column + 1);

            // Render mode indicator (left)
            buf.set_string(
                render_area.x,
                status_y,
                mode_text,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            );

            // Render file info (middle)
            let file_info_x = render_area.x + (render_area.width - file_info.len() as u16) / 2;
            buf.set_string(file_info_x, status_y, file_info, Style::default().fg(Color::White));

            // Render position (right)
            let position_x = render_area.x + render_area.width - position_info.len() as u16;
            buf.set_string(position_x, status_y, position_info, Style::default().fg(Color::White));
        }
    }
}

impl<'a> Widget for TextEditor<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state = TextEditorState::default();
        StatefulWidget::render(self, area, buf, &mut state);
    }
}
