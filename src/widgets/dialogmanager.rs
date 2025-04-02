use std::collections::VecDeque;
use std::{fmt, vec};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
};

use crate::modal::InputEvent;
use crate::widgets::dialog::{Dialog, DialogState};

/// A unique identifier for a dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DialogId(pub usize);

impl fmt::Display for DialogId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Dialog-{}", self.0)
    }
}

/// Result from a dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogResult {
    /// The ID of the dialog that produced this result
    pub id: DialogId,
    /// The value returned from the dialog
    pub value: String,
    /// Additional context for input dialogs
    pub input: Option<String>,
}

impl DialogResult {
    /// Create a new dialog result
    pub fn new(id: DialogId, value: String, input: Option<String>) -> Self {
        Self { id, value, input }
    }
}

/// A callback to be executed when a dialog is closed
pub type DialogCallback = Box<dyn FnOnce(&DialogResult) + Send + 'static>;

/// A dialog with its associated state
struct ManagedDialog {
    /// Dialog widget
    dialog: Dialog<'static>,
    /// Dialog state
    state: DialogState,
    /// Dialog ID
    id: DialogId,
    /// Whether the dialog is modal (blocks input to elements beneath it)
    is_modal: bool,
    /// Dialog z-index (higher values are rendered on top)
    z_index: usize,
    /// Callback to execute when the dialog is closed
    callback: Option<DialogCallback>,
}

/// A manager for multiple dialogs
pub struct DialogManager {
    /// Collection of dialogs
    dialogs: Vec<ManagedDialog>,
    /// Queue of dialog results from closed dialogs
    results: VecDeque<DialogResult>,
    /// Counter for generating unique dialog IDs
    next_id: usize,
}

impl Default for DialogManager {
    fn default() -> Self {
        Self {
            dialogs: Vec::new(),
            results: VecDeque::new(),
            next_id: 0,
        }
    }
}

impl DialogManager {
    /// Create a new dialog manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a dialog to the manager
    pub fn add_dialog(
        &mut self, 
        dialog: Dialog<'static>, 
        state: DialogState, 
        is_modal: bool, 
        z_index: Option<usize>,
        callback: Option<DialogCallback>,
    ) -> DialogId {
        let id = DialogId(self.next_id);
        self.next_id += 1;

        // Calculate z-index:
        // If provided, use that value, otherwise
        // use the current highest z-index + 1 (or 0 if no dialogs exist)
        let z_index = z_index.unwrap_or_else(|| {
            self.dialogs
                .iter()
                .map(|d| d.z_index)
                .max()
                .map(|max| max + 1)
                .unwrap_or(0)
        });

        let managed_dialog = ManagedDialog {
            dialog,
            state,
            id,
            is_modal,
            z_index,
            callback,
        };

        // Insert the dialog in z-order (sorted by z-index)
        let insert_pos = self.dialogs
            .iter()
            .position(|d| d.z_index > z_index)
            .unwrap_or(self.dialogs.len());
        
        self.dialogs.insert(insert_pos, managed_dialog);
        id
    }

    /// Remove a dialog from the manager
    pub fn remove_dialog(&mut self, id: DialogId) -> Option<DialogState> {
        let position = self.dialogs.iter().position(|d| d.id == id)?;
        
        // Execute the callback if one exists
        if let Some(dialog) = self.dialogs.get(position) {
            if let Some(result) = &dialog.state.result {
                let dialog_result = DialogResult::new(
                    id,
                    result.clone(),
                    if dialog.state.dialog_type.is_input_type() {
                        Some(dialog.state.input_value.clone())
                    } else {
                        None
                    },
                );

                // Execute the callback if present
                if let Some(callback) = dialog.callback.take() {
                    (callback)(&dialog_result);
                }

                // Add to results queue
                self.results.push_back(dialog_result);
            }
        }

        // Remove the dialog and return its state
        Some(self.dialogs.remove(position).state)
    }

    /// Get the state of a dialog
    pub fn get_dialog_state(&self, id: DialogId) -> Option<&DialogState> {
        self.dialogs.iter().find(|d| d.id == id).map(|d| &d.state)
    }

    /// Get mutable state of a dialog
    pub fn get_dialog_state_mut(&mut self, id: DialogId) -> Option<&mut DialogState> {
        self.dialogs.iter_mut().find(|d| d.id == id).map(|d| &mut d.state)
    }

    /// Check if a dialog has a result
    pub fn has_result(&self, id: DialogId) -> bool {
        self.dialogs
            .iter()
            .find(|d| d.id == id)
            .map(|d| d.state.result.is_some())
            .unwrap_or(false)
    }

    /// Get the next dialog result from the queue
    pub fn pop_result(&mut self) -> Option<DialogResult> {
        self.results.pop_front()
    }

    /// Get the number of active dialogs
    pub fn dialog_count(&self) -> usize {
        self.dialogs.len()
    }

    /// Check if there are any modal dialogs active
    pub fn has_modal_dialogs(&self) -> bool {
        self.dialogs.iter().any(|d| d.is_modal)
    }

    /// Get the top-most dialog (highest z-index)
    pub fn top_dialog(&self) -> Option<DialogId> {
        // The dialogs are already sorted by z-index in the vec
        self.dialogs.last().map(|d| d.id)
    }

    /// Get the top-most modal dialog, if any
    pub fn top_modal_dialog(&self) -> Option<DialogId> {
        self.dialogs.iter()
            .filter(|d| d.is_modal)
            .max_by_key(|d| d.z_index)
            .map(|d| d.id)
    }

    /// Handle an input event
    /// Returns true if the event was handled, false otherwise
    pub fn handle_event(&mut self, event: &InputEvent) -> bool {
        // First try the top-most modal dialog, if any
        if let Some(id) = self.top_modal_dialog() {
            if let Some(dialog) = self.dialogs.iter_mut().find(|d| d.id == id) {
                let handled = dialog.state.handle_event(event);
                
                // Check if the dialog was closed
                if handled && dialog.state.result.is_some() {
                    // Create the result object
                    let result = DialogResult::new(
                        id,
                        dialog.state.result.as_ref().unwrap().clone(),
                        if dialog.state.dialog_type.is_input_type() {
                            Some(dialog.state.input_value.clone())
                        } else {
                            None
                        },
                    );
                    
                    // Execute callback
                    if let Some(callback) = dialog.callback.take() {
                        (callback)(&result);
                    }
                    
                    // Add to results queue
                    self.results.push_back(result);
                }
                
                return handled;
            }
        }
        
        // If no modal dialog handled it, try all dialogs in reverse z-order
        for dialog in self.dialogs.iter_mut().rev() {
            let handled = dialog.state.handle_event(event);
            
            if handled {
                // Check if the dialog was closed
                if dialog.state.result.is_some() {
                    // Create the result object
                    let result = DialogResult::new(
                        dialog.id,
                        dialog.state.result.as_ref().unwrap().clone(),
                        if dialog.state.dialog_type.is_input_type() {
                            Some(dialog.state.input_value.clone())
                        } else {
                            None
                        },
                    );
                    
                    // Execute callback
                    if let Some(callback) = dialog.callback.take() {
                        (callback)(&result);
                    }
                    
                    // Add to results queue
                    self.results.push_back(result);
                }
                
                return true;
            }
        }
        
        false
    }

    /// Render all dialogs
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Render dialogs in z-order (lower z-index first, higher z-index on top)
        for dialog in &mut self.dialogs {
            dialog.dialog.clone().render(area, buf, &mut dialog.state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::dialog::{DialogButton, DialogType};
    use crate::modal::Key;
    use crossterm::event::KeyCode;

    #[test]
    fn test_dialog_manager_creation() {
        let manager = DialogManager::new();
        assert_eq!(manager.dialog_count(), 0);
        assert!(!manager.has_modal_dialogs());
        assert!(manager.top_dialog().is_none());
    }

    #[test]
    fn test_add_dialog() {
        let mut manager = DialogManager::new();
        
        let dialog = Dialog::default();
        let state = DialogState::message(DialogType::Info, "Test", "Test message");
        
        let id = manager.add_dialog(dialog, state, true, None, None);
        
        assert_eq!(manager.dialog_count(), 1);
        assert!(manager.has_modal_dialogs());
        assert_eq!(manager.top_dialog(), Some(id));
    }

    #[test]
    fn test_remove_dialog() {
        let mut manager = DialogManager::new();
        
        let dialog = Dialog::default();
        let state = DialogState::message(DialogType::Info, "Test", "Test message");
        
        let id = manager.add_dialog(dialog, state, true, None, None);
        assert_eq!(manager.dialog_count(), 1);
        
        let removed_state = manager.remove_dialog(id);
        assert!(removed_state.is_some());
        assert_eq!(manager.dialog_count(), 0);
    }

    #[test]
    fn test_dialog_z_ordering() {
        let mut manager = DialogManager::new();
        
        let dialog1 = Dialog::default();
        let state1 = DialogState::message(DialogType::Info, "Dialog 1", "Message 1");
        let id1 = manager.add_dialog(dialog1, state1, false, Some(1), None);
        
        let dialog2 = Dialog::default();
        let state2 = DialogState::message(DialogType::Warning, "Dialog 2", "Message 2");
        let id2 = manager.add_dialog(dialog2, state2, false, Some(3), None);
        
        let dialog3 = Dialog::default();
        let state3 = DialogState::message(DialogType::Error, "Dialog 3", "Message 3");
        let id3 = manager.add_dialog(dialog3, state3, false, Some(2), None);
        
        // The top dialog should be the one with the highest z-index
        assert_eq!(manager.top_dialog(), Some(id2));
    }

    #[test]
    fn test_handle_event() {
        let mut manager = DialogManager::new();
        
        let dialog = Dialog::default();
        let state = DialogState::confirm("Confirm", "Are you sure?");
        
        let id = manager.add_dialog(dialog, state, true, None, None);
        
        // Create an Enter key event to select the current button
        let key = Key { code: KeyCode::Enter, modifiers: crossterm::event::KeyModifiers::empty() };
        let event = InputEvent::Key(key);
        
        // Handle the event
        let handled = manager.handle_event(&event);
        assert!(handled);
        
        // The dialog should have a result
        assert!(manager.has_result(id));
        
        // There should be a result in the queue
        let result = manager.pop_result();
        assert!(result.is_some());
        
        let result = result.unwrap();
        assert_eq!(result.id, id);
        assert_eq!(result.value, "yes"); // Default for confirm dialog
    }

    #[test]
    fn test_modal_dialog_handling() {
        let mut manager = DialogManager::new();
        
        // Add a non-modal dialog
        let dialog1 = Dialog::default();
        let state1 = DialogState::message(DialogType::Info, "Non-modal", "Non-modal message");
        let id1 = manager.add_dialog(dialog1, state1, false, None, None);
        
        // Add a modal dialog
        let dialog2 = Dialog::default();
        let state2 = DialogState::message(DialogType::Warning, "Modal", "Modal message");
        let id2 = manager.add_dialog(dialog2, state2, true, None, None);
        
        assert!(manager.has_modal_dialogs());
        assert_eq!(manager.top_modal_dialog(), Some(id2));
        
        // Create an Enter key event
        let key = Key { code: KeyCode::Enter, modifiers: crossterm::event::KeyModifiers::empty() };
        let event = InputEvent::Key(key);
        
        // The event should be handled by the modal dialog
        let handled = manager.handle_event(&event);
        assert!(handled);
        
        // The modal dialog should have a result
        let result = manager.pop_result();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, id2);
        
        // Now there should be no modal dialogs
        assert!(!manager.has_modal_dialogs());
    }

    #[test]
    fn test_dialog_callback() {
        use std::sync::{Arc, Mutex};
        
        let callback_called = Arc::new(Mutex::new(false));
        let callback_called_clone = callback_called.clone();
        
        let callback = Box::new(move |result: &DialogResult| {
            let mut called = callback_called_clone.lock().unwrap();
            *called = true;
            assert_eq!(result.value, "ok");
        });
        
        let mut manager = DialogManager::new();
        
        let dialog = Dialog::default();
        let state = DialogState::message(DialogType::Info, "Test", "Test message");
        
        let id = manager.add_dialog(dialog, state, true, None, Some(callback));
        
        // Create an Enter key event
        let key = Key { code: KeyCode::Enter, modifiers: crossterm::event::KeyModifiers::empty() };
        let event = InputEvent::Key(key);
        
        // Handle the event
        manager.handle_event(&event);
        
        // The callback should have been called
        assert!(*callback_called.lock().unwrap());
    }
}