#[cfg(test)]
#[path = "texteditor.rs"]
mod texteditor {
    pub use super::*;
}

#[cfg(test)]
mod tests {
    use super::texteditor::*;
    
    // Mock the Mode enum since we can't access the original due to build errors
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Mode {
        Normal,
        Insert,
        Visual,
        Command,
    }
    
    // Mock the ModalState struct
    #[derive(Debug, Clone)]
    pub struct ModalState {
        mode: Mode,
    }
    
    impl ModalState {
        pub fn default() -> Self {
            Self { mode: Mode::Normal }
        }
        
        pub fn mode(&self) -> Mode {
            self.mode
        }
    }
    
    use clipboard::ClipboardProvider;
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Style},
    };

    /// Helper function to create a test buffer for rendering
    fn create_test_buffer(width: u16, height: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, width, height))
    }

    #[test]
    fn test_cursor_position() {
        let pos1 = CursorPosition::new(5, 10);
        assert_eq!(pos1.line, 5);
        assert_eq!(pos1.column, 10);

        let pos2 = CursorPosition::default();
        assert_eq!(pos2.line, 0);
        assert_eq!(pos2.column, 0);

        let pos3 = pos1.beginning_of_line();
        assert_eq!(pos3.line, 5);
        assert_eq!(pos3.column, 0);

        let pos4 = pos1.end_of_line(20);
        assert_eq!(pos4.line, 5);
        assert_eq!(pos4.column, 20);
    }

    #[test]
    fn test_selection() {
        let start = CursorPosition::new(1, 5);
        let end = CursorPosition::new(3, 10);
        let sel = Selection::new(start, end);

        assert_eq!(sel.start, start);
        assert_eq!(sel.end, end);
        assert!(!sel.is_empty());

        let empty_sel = Selection::new(start, start);
        assert!(empty_sel.is_empty());

        // Test ordered() when start < end
        let (ordered_start, ordered_end) = sel.ordered();
        assert_eq!(ordered_start, start);
        assert_eq!(ordered_end, end);

        // Test ordered() when end < start
        let reversed_sel = Selection::new(end, start);
        let (ordered_start, ordered_end) = reversed_sel.ordered();
        assert_eq!(ordered_start, start);
        assert_eq!(ordered_end, end);
    }

    #[test]
    fn test_document_language() {
        assert_eq!(DocumentLanguage::from_filename("test.rs"), DocumentLanguage::Rust);
        assert_eq!(DocumentLanguage::from_filename("test.md"), DocumentLanguage::Markdown);
        assert_eq!(DocumentLanguage::from_filename("test.html"), DocumentLanguage::Html);
        assert_eq!(DocumentLanguage::from_filename("test.json"), DocumentLanguage::Json);
        assert_eq!(DocumentLanguage::from_filename("test.toml"), DocumentLanguage::Toml);
        assert_eq!(DocumentLanguage::from_filename("test.yaml"), DocumentLanguage::Yaml);
        assert_eq!(DocumentLanguage::from_filename("test.py"), DocumentLanguage::Python);
        assert_eq!(DocumentLanguage::from_filename("test.js"), DocumentLanguage::JavaScript);
        assert_eq!(DocumentLanguage::from_filename("test.ts"), DocumentLanguage::TypeScript);
        assert_eq!(DocumentLanguage::from_filename("test.cpp"), DocumentLanguage::Cpp);
        assert_eq!(DocumentLanguage::from_filename("test.go"), DocumentLanguage::Go);
        assert_eq!(DocumentLanguage::from_filename("test.unknown"), DocumentLanguage::PlainText);
        assert_eq!(DocumentLanguage::from_filename("test"), DocumentLanguage::PlainText);

        // Test syntect name mapping
        assert_eq!(DocumentLanguage::Rust.get_syntect_name(), "Rust");
        assert_eq!(DocumentLanguage::Markdown.get_syntect_name(), "Markdown");
        assert_eq!(DocumentLanguage::PlainText.get_syntect_name(), "Plain Text");
        assert_eq!(DocumentLanguage::Matrix.get_syntect_name(), "Markdown"); // Matrix uses Markdown highlighting
    }

    #[test]
    fn test_editor_state_creation() {
        // Test default constructor
        let state = TextEditorState::default();
        assert_eq!(state.lines, vec![String::new()]);
        assert_eq!(state.cursor, CursorPosition::default());
        assert_eq!(state.selection, None);
        assert_eq!(state.scroll_offset, (0, 0));
        assert_eq!(state.language, DocumentLanguage::PlainText);
        assert!(state.highlight_cache_dirty);
        assert!(state.undo_stack.is_empty());
        assert!(state.redo_stack.is_empty());

        // Test new constructor
        let state2 = TextEditorState::new();
        assert_eq!(state2.lines, vec![String::new()]);
        
        // Test with_language
        let state3 = TextEditorState::with_language(DocumentLanguage::Rust);
        assert_eq!(state3.language, DocumentLanguage::Rust);
        
        // Test from_file
        let content = "Line 1\nLine 2\nLine 3";
        let state4 = TextEditorState::from_file(content, "test.rs");
        assert_eq!(state4.lines, vec!["Line 1", "Line 2", "Line 3"]);
        assert_eq!(state4.language, DocumentLanguage::Rust);
    }

    #[test]
    fn test_content_management() {
        let mut state = TextEditorState::default();
        
        // Test set_content
        let content = "Line 1\nLine 2\nLine 3";
        state.set_content(content);
        assert_eq!(state.lines, vec!["Line 1", "Line 2", "Line 3"]);
        assert_eq!(state.cursor, CursorPosition::default());
        assert!(state.undo_stack.is_empty()); // Set content clears history
        
        // Test content() getter
        assert_eq!(state.content(), content);
        
        // Test current_line
        assert_eq!(state.current_line(), "Line 1");
        state.cursor.line = 1;
        assert_eq!(state.current_line(), "Line 2");
    }

    #[test]
    fn test_basic_editing() {
        let mut state = TextEditorState::default();
        
        // Test insert_char
        state.insert_char('a');
        assert_eq!(state.lines[0], "a");
        assert_eq!(state.cursor.column, 1);
        
        state.insert_char('b');
        assert_eq!(state.lines[0], "ab");
        assert_eq!(state.cursor.column, 2);
        
        // Test delete_char_backward
        state.delete_char_backward();
        assert_eq!(state.lines[0], "a");
        assert_eq!(state.cursor.column, 1);
        
        // Test delete_char_forward
        state.cursor.column = 0;
        state.delete_char_forward();
        assert_eq!(state.lines[0], "");
        assert_eq!(state.cursor.column, 0);
        
        // Test insert_newline
        state.insert_text("Hello world");
        state.cursor.column = 5; // After "Hello"
        state.insert_newline();
        assert_eq!(state.lines[0], "Hello");
        assert_eq!(state.lines[1], " world");
        assert_eq!(state.cursor.line, 1);
        assert_eq!(state.cursor.column, 0);
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = TextEditorState::default();
        state.set_content("Line 1\nLine 2\nLine 3");
        
        // Test move_cursor_to
        state.move_cursor_to(CursorPosition::new(1, 2));
        assert_eq!(state.cursor, CursorPosition::new(1, 2));
        
        // Test move_cursor_left
        state.move_cursor_left();
        assert_eq!(state.cursor, CursorPosition::new(1, 1));
        
        // Test move_cursor_right
        state.move_cursor_right();
        assert_eq!(state.cursor, CursorPosition::new(1, 2));
        
        // Test move_cursor_up
        state.move_cursor_up();
        assert_eq!(state.cursor, CursorPosition::new(0, 2));
        
        // Test move_cursor_down
        state.move_cursor_down();
        assert_eq!(state.cursor, CursorPosition::new(1, 2));
        
        // Test move_cursor_begin_of_line
        state.move_cursor_begin_of_line();
        assert_eq!(state.cursor, CursorPosition::new(1, 0));
        
        // Test move_cursor_end_of_line
        state.move_cursor_end_of_line();
        assert_eq!(state.cursor, CursorPosition::new(1, 6)); // "Line 2" length is 6
    }

    #[test]
    fn test_selection_operations() {
        let mut state = TextEditorState::default();
        state.set_content("Hello world\nLine 2\nLine 3");
        
        // Test start_selection
        state.cursor = CursorPosition::new(0, 0);
        state.start_selection();
        assert!(state.selection.is_some());
        let sel = state.selection.unwrap();
        assert_eq!(sel.start, CursorPosition::new(0, 0));
        assert_eq!(sel.end, CursorPosition::new(0, 0));
        
        // Test update_selection
        state.cursor.column = 5; // Move to end of "Hello"
        state.update_selection();
        let sel = state.selection.unwrap();
        assert_eq!(sel.start, CursorPosition::new(0, 0));
        assert_eq!(sel.end, CursorPosition::new(0, 5));
        
        // Test selected_text
        let selected = state.selected_text();
        assert_eq!(selected, Some("Hello".to_string()));
        
        // Test delete_selection
        state.delete_selection();
        assert_eq!(state.lines[0], " world");
        assert_eq!(state.cursor, CursorPosition::new(0, 0));
        assert_eq!(state.selection, None);
        
        // Test clear_selection
        state.start_selection();
        assert!(state.selection.is_some());
        state.clear_selection();
        assert_eq!(state.selection, None);
    }

    #[test]
    fn test_undo_redo() {
        let mut state = TextEditorState::default();
        
        // Insert text and track in undo stack
        state.insert_char('a');
        state.insert_char('b');
        state.insert_char('c');
        assert_eq!(state.lines[0], "abc");
        assert_eq!(state.undo_stack.len(), 3);
        
        // Test undo
        assert!(state.undo());
        assert_eq!(state.lines[0], "ab");
        assert_eq!(state.cursor.column, 2);
        assert_eq!(state.undo_stack.len(), 2);
        assert_eq!(state.redo_stack.len(), 1);
        
        // Test redo
        assert!(state.redo());
        assert_eq!(state.lines[0], "abc");
        assert_eq!(state.cursor.column, 3);
        assert_eq!(state.undo_stack.len(), 3);
        assert_eq!(state.redo_stack.len(), 0);
        
        // Test multi-character edits
        state.cursor.column = 0;
        let text = "Hello ";
        state.undo_stack.push(UndoOperation::Insert {
            position: state.cursor,
            text: text.to_string(),
        });
        state.insert_text(text);
        assert_eq!(state.lines[0], "Hello abc");
        
        assert!(state.undo());
        assert_eq!(state.lines[0], "abc");
    }

    #[test]
    fn test_find_replace() {
        let mut state = TextEditorState::default();
        state.set_content("Hello world\nHello test\nworld test");
        
        // Test find
        let matches = state.find("Hello", true);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].0, CursorPosition::new(0, 0));
        assert_eq!(matches[0].1, CursorPosition::new(0, 5));
        assert_eq!(matches[1].0, CursorPosition::new(1, 0));
        assert_eq!(matches[1].1, CursorPosition::new(1, 5));
        
        // Test find with case insensitivity
        let matches = state.find("hello", false);
        assert_eq!(matches.len(), 2);
        
        // Test replace_all
        let count = state.replace_all("Hello", "Hi", true);
        assert_eq!(count, 2);
        assert_eq!(state.lines[0], "Hi world");
        assert_eq!(state.lines[1], "Hi test");
        assert_eq!(state.lines[2], "world test");
        assert_eq!(state.undo_stack.len(), 2); // Two replace operations
    }

    #[test]
    fn test_clipboard_operations() {
        // Skip this test if clipboard is not available
        let clipboard_available = ClipboardProvider::new().is_ok();
        if !clipboard_available {
            return;
        }
        
        let mut state = TextEditorState::default();
        state.set_content("Hello world\nLine 2\nLine 3");
        
        // Set up selection
        state.cursor = CursorPosition::new(0, 0);
        state.start_selection();
        state.cursor.column = 5; // Select "Hello"
        state.update_selection();
        
        // Test copy
        assert!(state.copy());
        
        // Test cut
        assert!(state.cut());
        assert_eq!(state.lines[0], " world");
        
        // Test paste
        state.cursor.column = 0;
        assert!(state.paste());
        assert_eq!(state.lines[0], "Hello world");
    }

    #[test]
    fn test_ensure_cursor_visible() {
        let mut state = TextEditorState::default();
        state.set_content("Line 1\nLine 2\nLine 3\nLine 4\nLine 5");
        
        // Set cursor outside visible area
        state.cursor = CursorPosition::new(10, 5);
        
        // Create a small viewport area
        let area = Rect::new(0, 0, 20, 3);
        
        // Ensure cursor is visible (should adjust scroll offset)
        state.ensure_cursor_visible(area);
        
        // Scroll offset should be updated to make cursor visible
        assert!(state.scroll_offset.0 >= 8); // Should scroll down to show line 10
    }

    #[test]
    fn test_widget_rendering() {
        let mut state = TextEditorState::default();
        state.set_content("Line 1\nLine 2\nLine 3");
        
        let widget = TextEditor::default();
        let area = Rect::new(0, 0, 20, 5);
        let mut buffer = create_test_buffer(20, 5);
        
        // Render widget
        widget.render(area, &mut buffer, &mut state);
        
        // Basic rendering checks
        let content_cell = buffer.get(0, 1); // First character of first line
        assert!(!content_cell.symbol.is_empty());
        
        // Change mode and check status line
        state.modal_state.enter_insert_mode();
        widget.render(area, &mut buffer, &mut state);
        
        // Check for cursor
        let cursor_cell = buffer.get(0, 1); // Cursor should be at first position
        assert!(cursor_cell.style().bg != Color::Reset); // Cursor should have background color
    }

    #[test]
    fn test_syntax_highlighting() {
        let mut state = TextEditorState::with_language(DocumentLanguage::Rust);
        state.set_content("fn main() {\n    println!(\"Hello world\");\n}");
        
        // Get highlighted line (this uses syntect)
        let highlighted = state.get_highlighted_line(0);
        assert!(highlighted.is_some());
        let highlighted = highlighted.unwrap();
        
        // Syntax highlighting should break the line into styled segments
        assert!(highlighted.len() >= 2); // At least some styling should be applied
        
        // Test changing language
        state.set_language(DocumentLanguage::PlainText);
        assert!(state.highlight_cache_dirty); // Cache should be marked dirty
        
        // Plain text should just return a single segment
        let plain = state.get_highlighted_line(0);
        assert!(plain.is_some());
        let plain = plain.unwrap();
        assert_eq!(plain.len(), 1);
    }

    #[test]
    fn test_get_text_between() {
        let mut state = TextEditorState::default();
        state.set_content("Hello world\nLine 2\nLine 3");
        
        // Test within same line
        let text = state.get_text_between(
            CursorPosition::new(0, 0),
            CursorPosition::new(0, 5),
        );
        assert_eq!(text, "Hello");
        
        // Test across multiple lines
        let text = state.get_text_between(
            CursorPosition::new(0, 6),
            CursorPosition::new(1, 4),
        );
        assert_eq!(text, "world\nLine");
        
        // Test with reversed positions
        let text = state.get_text_between(
            CursorPosition::new(1, 4),
            CursorPosition::new(0, 6),
        );
        assert_eq!(text, "world\nLine");
    }

    #[test]
    fn test_insert_delete_text() {
        let mut state = TextEditorState::default();
        
        // Test insert_text at beginning
        state.insert_text("Hello");
        assert_eq!(state.lines[0], "Hello");
        assert_eq!(state.cursor.column, 5);
        
        // Test insert_text in middle
        state.cursor.column = 2;
        state.insert_text("XX");
        assert_eq!(state.lines[0], "HeXXllo");
        assert_eq!(state.cursor.column, 4);
        
        // Test insert multi-line text
        state.cursor.column = 0;
        state.insert_text("Line 1\nLine 2\n");
        assert_eq!(state.lines[0], "Line 1");
        assert_eq!(state.lines[1], "Line 2");
        assert_eq!(state.lines[2], "HeXXllo");
        assert_eq!(state.cursor.line, 2);
        assert_eq!(state.cursor.column, 0);
    }

    #[test]
    fn test_clamp_position() {
        let mut state = TextEditorState::default();
        state.set_content("Line 1\nLine 2\nLine 3");
        
        // Test valid position
        let pos = state.clamp_position(CursorPosition::new(1, 2));
        assert_eq!(pos, CursorPosition::new(1, 2));
        
        // Test out of bounds line
        let pos = state.clamp_position(CursorPosition::new(10, 0));
        assert_eq!(pos, CursorPosition::new(2, 0)); // Clamped to last line
        
        // Test out of bounds column
        let pos = state.clamp_position(CursorPosition::new(1, 20));
        assert_eq!(pos, CursorPosition::new(1, 6)); // Clamped to end of line ("Line 2" length is 6)
    }
}