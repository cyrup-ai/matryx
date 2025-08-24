#[cfg(test)]
mod tests {
    use super::*;
    use crate::modal::Key;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::Style,
    };

    /// Helper function to create a test buffer for rendering
    fn create_test_buffer(width: u16, height: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, width, height))
    }

    #[test]
    fn test_dialog_type() {
        assert_eq!(DialogType::Info.title(), "Information");
        assert_eq!(DialogType::Warning.title(), "Warning");
        assert_eq!(DialogType::Error.title(), "Error");
        assert_eq!(DialogType::Question.title(), "Question");
        assert_eq!(DialogType::Input.title(), "Input");
        assert_eq!(DialogType::Progress.title(), "Progress");
        assert_eq!(DialogType::FileBrowser.title(), "File Browser");
        assert_eq!(DialogType::Wizard.title(), "Wizard");
        
        assert!(DialogType::Input.is_input_type());
        assert!(!DialogType::Info.is_input_type());
    }

    #[test]
    fn test_dialog_button() {
        let button = DialogButton::new("OK", "ok");
        assert_eq!(button.label, "OK");
        assert_eq!(button.value, "ok");
        assert!(!button.selected);

        let selected_button = DialogButton::selected("Cancel", "cancel");
        assert_eq!(selected_button.label, "Cancel");
        assert_eq!(selected_button.value, "cancel");
        assert!(selected_button.selected);
    }

    #[test]
    fn test_dialog_state_creation() {
        // Test message dialog
        let state = DialogState::message(DialogType::Info, "Test", "Test message");
        assert_eq!(state.dialog_type, DialogType::Info);
        assert_eq!(state.title, "Test");
        assert_eq!(state.message, "Test message");
        assert_eq!(state.buttons.len(), 1);
        assert_eq!(state.buttons[0].label, "OK");
        assert_eq!(state.buttons[0].value, "ok");
        
        // Test confirmation dialog
        let state = DialogState::confirm("Confirm", "Are you sure?");
        assert_eq!(state.dialog_type, DialogType::Question);
        assert_eq!(state.title, "Confirm");
        assert_eq!(state.message, "Are you sure?");
        assert_eq!(state.buttons.len(), 2);
        assert_eq!(state.buttons[0].label, "Yes");
        assert_eq!(state.buttons[0].value, "yes");
        assert_eq!(state.buttons[1].label, "No");
        assert_eq!(state.buttons[1].value, "no");
        
        // Test input dialog
        let state = DialogState::input("Input", "Enter value:", "default");
        assert_eq!(state.dialog_type, DialogType::Input);
        assert_eq!(state.title, "Input");
        assert_eq!(state.message, "Enter value:");
        assert_eq!(state.input_value, "default");
        assert_eq!(state.input_cursor, 7); // Length of "default"
        
        // Test progress dialog
        let state = DialogState::progress("Progress", "Working...", "Starting", false);
        assert_eq!(state.dialog_type, DialogType::Progress);
        assert_eq!(state.title, "Progress");
        assert_eq!(state.message, "Working...");
        assert_eq!(state.progress_status, "Starting");
        assert!(!state.progress_indeterminate);
        
        // Test file browser dialog
        let state = DialogState::file_browser("Open", "Select file:", "/home", Some("*.txt".to_string()));
        assert_eq!(state.dialog_type, DialogType::FileBrowser);
        assert_eq!(state.title, "Open");
        assert_eq!(state.message, "Select file:");
        assert_eq!(state.current_directory, "/home");
        assert_eq!(state.file_filter, Some("*.txt".to_string()));
        
        // Test wizard dialog
        let steps = vec![
            WizardStep {
                title: "Step 1".to_string(),
                content: "Step 1 content".to_string(),
                is_complete: false,
            },
            WizardStep {
                title: "Step 2".to_string(),
                content: "Step 2 content".to_string(),
                is_complete: false,
            },
        ];
        
        let state = DialogState::wizard("Setup", steps.clone());
        assert_eq!(state.dialog_type, DialogType::Wizard);
        assert_eq!(state.title, "Setup");
        assert_eq!(state.message, "Step 1 content"); // First step content
        assert_eq!(state.wizard_steps.len(), 2);
        assert_eq!(state.wizard_steps[0].title, "Step 1");
        assert_eq!(state.wizard_steps[1].title, "Step 2");
    }

    #[test]
    fn test_dialog_navigation() {
        // Test button navigation
        let mut state = DialogState::confirm("Test", "Message");
        assert_eq!(state.current_button, 0); // "Yes" is selected
        
        state.next_button();
        assert_eq!(state.current_button, 1); // "No" is selected
        
        state.next_button();
        assert_eq!(state.current_button, 0); // Wraps around to "Yes"
        
        state.prev_button();
        assert_eq!(state.current_button, 1); // Back to "No"
        
        // Test wizard navigation
        let steps = vec![
            WizardStep {
                title: "Step 1".to_string(),
                content: "Step 1 content".to_string(),
                is_complete: false,
            },
            WizardStep {
                title: "Step 2".to_string(),
                content: "Step 2 content".to_string(),
                is_complete: false,
            },
            WizardStep {
                title: "Step 3".to_string(),
                content: "Step 3 content".to_string(),
                is_complete: false,
            },
        ];
        
        let mut state = DialogState::wizard("Test", steps);
        assert_eq!(state.current_step_index, 0);
        assert_eq!(state.message, "Step 1 content");
        
        // Navigate forward
        assert!(state.next_wizard_step());
        assert_eq!(state.current_step_index, 1);
        assert_eq!(state.message, "Step 2 content");
        
        // Navigate backward
        assert!(state.previous_wizard_step());
        assert_eq!(state.current_step_index, 0);
        assert_eq!(state.message, "Step 1 content");
        
        // Cannot go back before first step
        assert!(!state.previous_wizard_step());
        assert_eq!(state.current_step_index, 0);
        
        // Go to last step
        assert!(state.next_wizard_step());
        assert!(state.next_wizard_step());
        assert_eq!(state.current_step_index, 2);
        
        // Cannot go past last step
        assert!(!state.next_wizard_step());
        assert_eq!(state.current_step_index, 2);
    }

    #[test]
    fn test_progress_dialog() {
        let mut state = DialogState::progress("Test", "Working...", "Starting", false);
        assert_eq!(state.progress_percent, 0);
        
        // Update progress
        state.update_progress(50, Some("Halfway done".to_string()));
        assert_eq!(state.progress_percent, 50);
        assert_eq!(state.progress_status, "Halfway done");
        
        // Update just the percentage
        state.update_progress(75, None);
        assert_eq!(state.progress_percent, 75);
        assert_eq!(state.progress_status, "Halfway done"); // Unchanged
        
        // Test indeterminate mode
        assert!(!state.progress_indeterminate);
        state.set_indeterminate(true);
        assert!(state.progress_indeterminate);
    }

    #[test]
    fn test_wizard_step_completion() {
        let steps = vec![
            WizardStep {
                title: "Step 1".to_string(),
                content: "Step 1 content".to_string(),
                is_complete: false,
            },
            WizardStep {
                title: "Step 2".to_string(),
                content: "Step 2 content".to_string(),
                is_complete: false,
            },
        ];
        
        let mut state = DialogState::wizard("Test", steps);
        assert!(!state.are_all_steps_complete());
        
        // Complete current step
        state.set_current_step_complete(true);
        assert!(!state.are_all_steps_complete()); // Still one incomplete step
        
        // Move to next step and complete it
        state.next_wizard_step();
        state.set_current_step_complete(true);
        
        // Now all steps should be complete
        assert!(state.are_all_steps_complete());
    }

    #[test]
    fn test_dialog_key_handling() {
        // Test basic key handling
        let mut state = DialogState::confirm("Test", "Message");
        
        // Right key should move to next button
        let key = Key { code: KeyCode::Right, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.current_button, 1);
        
        // Left key should move to previous button
        let key = Key { code: KeyCode::Left, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.current_button, 0);
        
        // Enter key should select the button
        let key = Key { code: KeyCode::Enter, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.result, Some("yes".to_string()));
        
        // Escape key should cancel
        let mut state = DialogState::confirm("Test", "Message");
        let key = Key { code: KeyCode::Esc, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.result, Some("cancel".to_string()));
        
        // Test input dialog key handling
        let mut state = DialogState::input("Test", "Message", "");
        
        // Character key should insert the character
        let key = Key { code: KeyCode::Char('a'), modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.input_value, "a");
        assert_eq!(state.input_cursor, 1);
        
        // Backspace should delete the character
        let key = Key { code: KeyCode::Backspace, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.input_value, "");
        assert_eq!(state.input_cursor, 0);
        
        // Test wizard dialog key handling
        let steps = vec![
            WizardStep {
                title: "Step 1".to_string(),
                content: "Step 1 content".to_string(),
                is_complete: false,
            },
            WizardStep {
                title: "Step 2".to_string(),
                content: "Step 2 content".to_string(),
                is_complete: false,
            },
        ];
        
        let mut state = DialogState::wizard("Test", steps);
        
        // Find the "next" button
        let next_button_idx = state.buttons.iter().position(|b| b.value == "next").unwrap();
        state.current_button = next_button_idx;
        
        // Enter should go to next step
        let key = Key { code: KeyCode::Enter, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.current_step_index, 1);
        
        // Find the "previous" button
        let prev_button_idx = state.buttons.iter().position(|b| b.value == "previous").unwrap();
        state.current_button = prev_button_idx;
        
        // Enter should go to previous step
        let key = Key { code: KeyCode::Enter, modifiers: KeyModifiers::empty() };
        assert!(state.handle_key(&key));
        assert_eq!(state.current_step_index, 0);
    }

    #[test]
    fn test_dialog_rendering() {
        // Create a dialog and buffer for rendering
        let dialog = Dialog::default();
        let mut buffer = create_test_buffer(80, 24);
        
        // Test rendering a message dialog
        let mut state = DialogState::message(DialogType::Info, "Test", "Test message");
        dialog.clone().render(Rect::new(0, 0, 80, 24), &mut buffer, &mut state);
        
        // Test rendering an input dialog
        let mut state = DialogState::input("Input", "Enter value:", "default");
        dialog.clone().render(Rect::new(0, 0, 80, 24), &mut buffer, &mut state);
        
        // Test rendering a progress dialog
        let mut state = DialogState::progress("Progress", "Working...", "Starting", false);
        state.progress_percent = 50;
        dialog.clone().render(Rect::new(0, 0, 80, 24), &mut buffer, &mut state);
        
        // Test rendering a file browser dialog
        let mut state = DialogState::file_browser("Open", "Select file:", "/", None);
        dialog.clone().render(Rect::new(0, 0, 80, 24), &mut buffer, &mut state);
        
        // Test rendering a wizard dialog
        let steps = vec![
            WizardStep {
                title: "Step 1".to_string(),
                content: "Step 1 content".to_string(),
                is_complete: true,
            },
            WizardStep {
                title: "Step 2".to_string(),
                content: "Step 2 content".to_string(),
                is_complete: false,
            },
        ];
        
        let mut state = DialogState::wizard("Test", steps);
        dialog.render(Rect::new(0, 0, 80, 24), &mut buffer, &mut state);
    }
}