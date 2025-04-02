#[cfg(test)]
mod tests {
    use super::*;
    use crate::modal::state::{CursorPosition, CursorStyle, ModalState, Mode, Selection};
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_mode_transitions() {
        let mut state = ModalState::new();

        // Default mode should be Normal
        assert!(state.is_normal_mode());
        assert!(!state.is_insert_mode());
        assert!(!state.is_visual_mode());

        // Enter insert mode
        state.enter_insert_mode();
        assert!(!state.is_normal_mode());
        assert!(state.is_insert_mode());
        assert!(!state.is_visual_mode());

        // Enter visual mode
        state.enter_visual_mode();
        assert!(!state.is_normal_mode());
        assert!(!state.is_insert_mode());
        assert!(state.is_visual_mode());

        // Return to previous mode (insert)
        let result = state.return_to_previous_mode();
        assert!(result);
        assert!(!state.is_normal_mode());
        assert!(state.is_insert_mode());
        assert!(!state.is_visual_mode());

        // Return to previous mode (normal)
        let result = state.return_to_previous_mode();
        assert!(result);
        assert!(state.is_normal_mode());
        assert!(!state.is_insert_mode());
        assert!(!state.is_visual_mode());

        // No more previous modes
        let result = state.return_to_previous_mode();
        assert!(!result);
    }

    #[test]
    fn test_cursor_styles() {
        let mut state = ModalState::new();

        // Normal mode should have block cursor
        assert_eq!(state.cursor().style(), CursorStyle::Block);

        // Insert mode should have bar cursor
        state.enter_insert_mode();
        assert_eq!(state.cursor().style(), CursorStyle::Bar);

        // Visual mode should have underline cursor
        state.enter_visual_mode();
        assert_eq!(state.cursor().style(), CursorStyle::Underline);

        // Manually change cursor style
        state.cursor_mut().set_style(CursorStyle::Block);
        assert_eq!(state.cursor().style(), CursorStyle::Block);

        // Update cursor style based on current mode
        state.update_cursor_style();
        assert_eq!(state.cursor().style(), CursorStyle::Underline);
    }

    #[test]
    fn test_cursor_positioning() {
        let mut state = ModalState::new();

        // Initial position should be 0,0
        assert_eq!(state.cursor_position(), CursorPosition::new(0, 0));

        // Move cursor
        state.move_cursor_to(5, 10);
        assert_eq!(state.cursor_position(), CursorPosition::new(5, 10));

        // Set position using CursorPosition
        let pos = CursorPosition::new(7, 15);
        state.set_cursor_position(pos);
        assert_eq!(state.cursor_position(), pos);

        // Move using cursor_mut
        state.cursor_mut().move_to(3, 8);
        assert_eq!(state.cursor_position(), CursorPosition::new(3, 8));
    }

    #[test]
    fn test_selection() {
        let mut state = ModalState::new();

        // No selection initially
        assert!(!state.has_selection());
        assert_eq!(state.selection(), None);

        // Enter visual mode and check selection
        state.enter_visual_mode();
        assert!(state.has_selection());

        let sel = state.selection().unwrap();
        assert_eq!(sel.start, CursorPosition::new(0, 0));
        assert_eq!(sel.end, CursorPosition::new(0, 0));

        // Move cursor and check that selection is updated
        state.move_cursor_to(2, 5);
        let sel = state.selection().unwrap();
        assert_eq!(sel.start, CursorPosition::new(0, 0));
        assert_eq!(sel.end, CursorPosition::new(2, 5));

        // Clear selection
        state.clear_selection();
        assert!(!state.has_selection());

        // Start new selection
        state.start_selection();
        assert!(state.has_selection());

        // Update selection
        state.move_cursor_to(3, 7);
        state.update_selection();
        let sel = state.selection().unwrap();
        assert_eq!(sel.end, CursorPosition::new(3, 7));

        // Exit visual mode should clear selection
        state.enter_normal_mode();
        assert!(!state.has_selection());
    }

    #[test]
    fn test_mode_data() {
        let mut state = ModalState::new();

        // Set data in normal mode
        state.set_mode_data("test", "normal_value");

        // Enter insert mode and set data
        state.enter_insert_mode();
        state.set_mode_data("test", "insert_value");

        // Get data from current mode
        assert_eq!(state.get_mode_data("test"), None); // This will return None in our placeholder implementation

        // Enter visual mode and check data from other modes
        state.enter_visual_mode();
        assert_eq!(state.get_mode_data_for_mode(Mode::Normal, "test"), None);
        assert_eq!(state.get_mode_data_for_mode(Mode::Insert, "test"), None);
    }

    #[test]
    fn test_serialization() {
        let mut state = ModalState::new();
        state.enter_insert_mode();
        state.move_cursor_to(5, 10);

        // Serialize to string
        let serialized = state.save_to_string().expect("Failed to serialize state");

        // Deserialize from string
        let deserialized =
            ModalState::load_from_string(&serialized).expect("Failed to deserialize state");

        // Check that state was preserved
        assert_eq!(deserialized.mode(), Mode::Insert);
        assert_eq!(deserialized.cursor_position(), CursorPosition::new(5, 10));
    }

    #[test]
    fn test_file_persistence() {
        let mut state = ModalState::new();
        state.enter_visual_mode();
        state.move_cursor_to(3, 7);

        // Create temporary file
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let temp_path = temp_file.path().to_owned();

        // Save state to file
        state.save_to_file(&temp_path).expect("Failed to save state to file");

        // Load state from file
        let loaded_state =
            ModalState::load_from_file(&temp_path).expect("Failed to load state from file");

        // Check that state was preserved
        assert_eq!(loaded_state.mode(), Mode::Visual);
        assert_eq!(loaded_state.cursor_position(), CursorPosition::new(3, 7));
        assert!(loaded_state.has_selection());
    }
}
