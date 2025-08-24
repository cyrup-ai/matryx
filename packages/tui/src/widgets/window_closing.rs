use std::sync::mpsc::{channel, Receiver, Sender};

use crate::widgets::{
    dialog::{Dialog, DialogButton, DialogState, DialogType},
    dialogmanager::{DialogId, DialogManager, DialogResult},
};

/// Result of a window closing operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowCloseResult {
    /// User confirmed window closing
    Confirmed(String),
    /// User denied window closing
    Denied(String),
    /// Dialog was canceled
    Canceled,
}

/// Window closing confirmation handler
pub struct WindowCloser {
    /// Dialog manager for confirmation dialogs
    dialog_manager: DialogManager,
    /// Sender for window closing results
    sender: Sender<WindowCloseResult>,
    /// Receiver for window closing results
    receiver: Receiver<WindowCloseResult>,
    /// Map of dialog IDs to window IDs
    dialog_window_map: std::collections::HashMap<DialogId, String>,
}

impl Default for WindowCloser {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            dialog_manager: DialogManager::default(),
            sender,
            receiver,
            dialog_window_map: std::collections::HashMap::new(),
        }
    }
}

impl WindowCloser {
    /// Create a new window closer
    pub fn new() -> Self {
        Self::default()
    }

    /// Confirm window closing with a dialog
    pub fn confirm_close_window<S: Into<String>>(&mut self, window_id: S, title: Option<S>) -> DialogId {
        let window_id = window_id.into();
        let window_title = title.map(|t| t.into()).unwrap_or_else(|| window_id.clone());

        // Create confirmation dialog
        let dialog = Dialog::default()
            .title("Close Window")
            .width_percent(40)
            .height_percent(20);

        let message = format!("Are you sure you want to close \"{}\"?", window_title);
        let state = DialogState::confirmation(
            "Close Window",
            &message,
            &[
                DialogButton::new("Yes", true, true),
                DialogButton::new("No", false, false),
                DialogButton::new("Cancel", false, false),
            ],
        );

        // Create a closure to handle the dialog result
        let sender = self.sender.clone();
        let window_id_clone = window_id.clone();
        let callback = move |result: DialogResult| {
            if let Some(button_index) = result.button_index {
                match button_index {
                    0 => {
                        // User confirmed closing
                        let _ = sender.send(WindowCloseResult::Confirmed(window_id_clone.clone()));
                    }
                    1 => {
                        // User denied closing
                        let _ = sender.send(WindowCloseResult::Denied(window_id_clone.clone()));
                    }
                    _ => {
                        // User canceled
                        let _ = sender.send(WindowCloseResult::Canceled);
                    }
                }
            } else {
                // Dialog was closed without selecting a button
                let _ = sender.send(WindowCloseResult::Canceled);
            }
        };

        // Add the dialog to the manager
        let id = self.dialog_manager.add_dialog(dialog, state, true, Some(callback), None);
        self.dialog_window_map.insert(id.clone(), window_id);

        id
    }

    /// Check for window closing results
    pub fn check_results(&mut self) -> Option<WindowCloseResult> {
        if let Ok(result) = self.receiver.try_recv() {
            Some(result)
        } else {
            None
        }
    }

    /// Handle events and update dialogs
    pub fn handle_event(&mut self, event: &crate::modal::InputEvent, modal_state: &mut crate::modal::ModalState) -> bool {
        self.dialog_manager.handle_event(event, modal_state)
    }

    /// Render dialogs
    pub fn render(&mut self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        self.dialog_manager.render(area, buf);
    }

    /// Process dialog results
    pub fn process_dialog_results(&mut self) {
        self.dialog_manager.process_results();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modal::{InputEvent, Key, ModalState};
    use ratatui::{buffer::Buffer, layout::Rect};

    #[test]
    fn test_window_closer_create_dialog() {
        let mut closer = WindowCloser::new();
        
        // Create confirmation dialog
        let dialog_id = closer.confirm_close_window("test-window", Some("Test Window"));
        
        // Check that dialog was created
        assert!(closer.dialog_window_map.contains_key(&dialog_id));
        assert_eq!(closer.dialog_window_map.get(&dialog_id), Some(&"test-window".to_string()));
    }

    #[test]
    fn test_window_closer_handling() {
        let mut closer = WindowCloser::new();
        let mut modal_state = ModalState::default();
        
        // Create confirmation dialog
        let _dialog_id = closer.confirm_close_window("test-window", Some("Test Window"));
        
        // Create a test buffer
        let area = Rect::new(0, 0, 80, 24);
        let mut buffer = Buffer::empty(area);
        
        // Render the dialog
        closer.render(area, &mut buffer);
        
        // Check that buffer contains dialog content
        let content = buffer.content.iter().map(|&c| char::from_u32(c as u32).unwrap_or(' ')).collect::<String>();
        assert!(content.contains("Close Window"));
        
        // Simulate "Yes" button press
        // First press Tab to select the "Yes" button
        let key_event = InputEvent::Key(Key {
            code: crossterm::event::KeyCode::Tab,
            modifiers: crossterm::event::KeyModifiers::empty(),
        });
        closer.handle_event(&key_event, &mut modal_state);
        
        // Then press Enter to activate the button
        let key_event = InputEvent::Key(Key {
            code: crossterm::event::KeyCode::Enter,
            modifiers: crossterm::event::KeyModifiers::empty(),
        });
        closer.handle_event(&key_event, &mut modal_state);
        
        // Process dialog results
        closer.process_dialog_results();
        
        // Check for window closing result
        if let Some(result) = closer.check_results() {
            assert_eq!(result, WindowCloseResult::Confirmed("test-window".to_string()));
        } else {
            panic!("Expected window closing result");
        }
    }
}