use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};

use crate::modal::{
    ActionResult, MatrixAction, EditorAction, InputEvent, ModalState,
    WindowActionTrait,
};
use crate::widgets::texteditor::TextEditorState;

// Matrix SDK wrapper imports
use matryx_api::{
    client::MatrixClient,
    room::MatrixRoom,
    error::Result as MatrixResult,
    future::{MatrixFuture, MatrixStream},
};

/// Trait for components that can be used in windows
pub trait WindowComponent {
    /// Handle an input event
    fn handle_event(&mut self, event: &InputEvent, modal_state: &mut ModalState) -> bool;

    /// Render the component
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    /// Check if the component can receive focus
    fn can_focus(&self) -> bool {
        true
    }

    /// Set focus state
    fn set_focus(&mut self, focused: bool);

    /// Check if the component is focused
    fn is_focused(&self) -> bool;

    /// Get the component title
    fn title(&self) -> &str;
}

/// Trait for windows in the Matrix UI
pub trait MatrixWindow: WindowComponent {
    /// Get the window ID
    fn id(&self) -> &str;
    
    /// Get the window title for the tab bar
    fn tab_title(&self) -> String;
    
    /// Get the window title for the window bar
    fn window_title(&self) -> String;
    
    /// Create a duplicate of this window
    fn duplicate(&self) -> Self
    where
        Self: Sized;
    
    /// Close the window
    fn close(&mut self) -> bool;
    
    /// Save window content
    fn save(&mut self, path: Option<&str>) -> ActionResult;
    
    /// Get completion options if available
    fn get_completions(&self) -> Option<Vec<String>>;
    
    /// Get word at cursor
    fn get_cursor_word(&self) -> Option<String>;
    
    /// Get selected text
    fn get_selected_text(&self) -> Option<String>;
    
    /// Execute an editor action
    fn execute_action(&mut self, action: EditorAction) -> ActionResult;
    
    /// Execute a Matrix-specific action
    fn execute_maxtryx_action(&mut self, action: MatrixAction) -> ActionResult;
}

/// State for windows that can be edited
pub trait EditableWindowState {
    /// Get the text editor state if available
    fn text_editor_state(&self) -> Option<&TextEditorState>;
    
    /// Get mutable text editor state if available
    fn text_editor_state_mut(&mut self) -> Option<&mut TextEditorState>;
    
    /// Get the modal state
    fn modal_state(&self) -> &ModalState;
    
    /// Get mutable modal state
    fn modal_state_mut(&mut self) -> &mut ModalState;
}

/// Window that supports editing
pub trait EditableWindow: MatrixWindow + WindowActionTrait {
    /// Get the window state
    fn state(&self) -> &dyn EditableWindowState;
    
    /// Get mutable window state
    fn state_mut(&mut self) -> &mut dyn EditableWindowState;
    
    /// Toggle focus between components
    fn toggle_focus(&mut self);
    
    /// Scroll the window content
    fn scroll(&mut self, direction: ScrollDirection, amount: usize) -> ActionResult;
}

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// Scroll up
    Up,
    /// Scroll down
    Down,
    /// Scroll to top
    Top,
    /// Scroll to bottom
    Bottom,
    /// Scroll to cursor
    ToCursor,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
}

/// Trait for matrix-specific window operations that integrates with MatrixClient wrappers
pub trait MatrixWindow: EditableWindow {
    /// Get the MatrixClient associated with this window
    fn client(&self) -> &matryx_api::client::MatrixClient;
    
    /// Get the current room as a MatrixRoom if in a room view
    fn current_room(&self) -> Option<matryx_api::room::MatrixRoom>;
    
    /// Get current room ID if in a room
    fn current_room_id(&self) -> Option<&str>;
    
    /// Get the current space ID if in a space view
    fn current_space_id(&self) -> Option<&str>;
    
    /// Handle general Matrix actions
    fn handle_matrix_action(&mut self, action: &MatrixAction) -> ActionResult;
    
    /// Handle room-specific actions
    fn handle_room_action(&mut self, room_id: &str, action: &str) -> ActionResult;
    
    /// Handle message-specific actions
    fn handle_message_action(&mut self, message_id: &str, action: &str) -> ActionResult;
    
    /// Handle sending a text message
    fn send_message(&mut self, content: &str) -> ActionResult;
    
    /// Handle sending a markdown message
    fn send_markdown(&mut self, content: &str) -> ActionResult;
    
    /// Handle sending a reaction to a message
    fn send_reaction(&mut self, message_id: &str, reaction: &str) -> ActionResult;
    
    /// Handle redacting (deleting) a message
    fn redact_message(&mut self, message_id: &str, reason: Option<&str>) -> ActionResult;
    
    /// Handle editing a message
    fn edit_message(&mut self, message_id: &str, new_content: &str) -> ActionResult;
    
    /// Handle replying to a message
    fn reply_to_message(&mut self, message_id: &str, content: &str) -> ActionResult;
    
    /// Handle uploading a file
    fn upload_file(&mut self, path: &str) -> ActionResult;
    
    /// Handle uploading an image
    fn upload_image(&mut self, data: &[u8], mime_type: &str) -> ActionResult;
    
    /// Handle downloading a file
    fn download_file(&mut self, message_id: &str, path: Option<&str>) -> ActionResult;
    
    /// Get timeline events for the current room
    fn get_timeline(&mut self, limit: u32) -> ActionResult;
    
    /// Get a thread timeline from a parent message
    fn get_thread(&mut self, parent_message_id: &str, limit: u32) -> ActionResult;
    
    /// Handle room membership operations (join, leave, invite)
    fn handle_membership(&mut self, action: &MembershipAction) -> ActionResult;
    
    /// Send a typing notification
    fn send_typing_notification(&mut self, typing: bool) -> ActionResult;
    
    /// Mark messages as read up to a certain event
    fn mark_as_read(&mut self, event_id: &str) -> ActionResult;
}

/// Actions specific to Matrix operations
#[derive(Debug, Clone)]
pub enum MatrixAction {
    /// Navigate to a room
    GoToRoom(String),
    /// Navigate to a space
    GoToSpace(String),
    /// Join a room
    JoinRoom(String),
    /// Leave a room
    LeaveRoom(String),
    /// Invite a user to a room
    InviteUser { room_id: String, user_id: String },
    /// Create a direct message room
    CreateDM(String),
    /// Create a new room
    CreateRoom { name: String, topic: Option<String>, is_direct: bool },
    /// Search messages
    SearchMessages(String),
    /// Toggle room encryption
    ToggleEncryption(String),
    /// Show device verification
    ShowVerification(String),
    /// Toggle room notifications
    ToggleNotifications { room_id: String, muted: bool },
}

/// Actions for room membership operations
#[derive(Debug, Clone)]
pub enum MembershipAction {
    /// Join a room
    Join(String),
    /// Leave a room
    Leave(String),
    /// Invite a user to a room
    Invite { room_id: String, user_id: String },
    /// Kick a user from a room
    Kick { room_id: String, user_id: String, reason: Option<String> },
    /// Ban a user from a room
    Ban { room_id: String, user_id: String, reason: Option<String> },
    /// Unban a user from a room
    Unban { room_id: String, user_id: String },
}

/// Adapter trait to help converting between MatrixWindow and MatrixWindow traits.
/// Implement this trait for any window that needs to support Matrix operations.
pub trait MatrixWindowAdapter {
    /// Get the Matrix client from the window's context
    fn get_client(&self) -> &MatrixClient;
    
    /// Get current room ID in string format if in a room view
    fn get_room_id(&self) -> Option<String>;
    
    /// Get current space ID in string format if in a space view 
    fn get_space_id(&self) -> Option<String>;
    
    /// Transform a MatrixAction to MatrixAction for processing
    fn matrix_to_maxtryx_action(&self, action: &MatrixAction) -> Option<MatrixAction>;
    
    /// Process a Matrix future operation and convert its result for display
    fn process_matrix_future<T>(&self, future: MatrixFuture<T>) -> ActionResult 
    where 
        T: 'static + Send;
}

/// Default implementations for common Matrix window operations
pub trait MatrixWindowDefault: MatrixWindow + MatrixWindowAdapter {
    /// Default implementation for handling Matrix actions
    fn default_handle_matrix_action(&self, action: &MatrixAction) -> ActionResult {
        // Convert MatrixAction to MatrixAction
        match self.matrix_to_maxtryx_action(action) {
            Some(maxtryx_action) => {
                // Execute the MatrixAction
                self.execute_maxtryx_action(maxtryx_action)
            }
            None => {
                // Handle the action directly based on type
                match action {
                    MatrixAction::GoToRoom(room_id) => {
                        // Implementation would depend on application's navigation logic
                        Ok(None)
                    }
                    // Handle other action types...
                    _ => Ok(None),
                }
            }
        }
    }
    
    /// Default implementation for sending a text message
    fn default_send_message(&self, content: &str) -> ActionResult {
        if let Some(room_id) = self.get_room_id() {
            if let Ok(ruma_id) = room_id.parse::<ruma::RoomId>() {
                let content = content.to_string();
                let client = self.get_client();
                
                if let Some(room) = client.get_room(&ruma_id) {
                    let future = room.send_text_message(&content, None);
                    return self.process_matrix_future(future);
                }
            }
        }
        
        Ok(None)
    }
    
    /// Default implementation for sending a markdown message
    fn default_send_markdown(&self, content: &str) -> ActionResult {
        if let Some(room_id) = self.get_room_id() {
            if let Ok(ruma_id) = room_id.parse::<ruma::RoomId>() {
                let content = content.to_string();
                let client = self.get_client();
                
                if let Some(room) = client.get_room(&ruma_id) {
                    let future = room.send_markdown_message(&content, None);
                    return self.process_matrix_future(future);
                }
            }
        }
        
        Ok(None)
    }
}

/// Window registry to manage all windows
pub struct WindowRegistry {
    /// Map of window IDs to windows
    windows: std::collections::HashMap<String, Box<dyn MatrixWindow>>,
    /// List of window IDs in z-order (front to back)
    z_order: Vec<String>,
    /// Currently active window ID
    active_window: Option<String>,
}

impl WindowRegistry {
    /// Create a new window registry
    pub fn new() -> Self {
        Self {
            windows: std::collections::HashMap::new(),
            z_order: Vec::new(),
            active_window: None,
        }
    }
    
    /// Register a window
    pub fn register<W: MatrixWindow + 'static>(&mut self, window: W) {
        let id = window.id().to_string();
        self.windows.insert(id.clone(), Box::new(window));
        self.z_order.push(id.clone());
        
        // Set as active if first window
        if self.active_window.is_none() {
            self.active_window = Some(id);
        }
    }
    
    /// Remove a window
    pub fn remove(&mut self, id: &str) -> Option<Box<dyn MatrixWindow>> {
        let window = self.windows.remove(id);
        
        // Update z-order
        self.z_order.retain(|wid| wid != id);
        
        // Update active window
        if self.active_window.as_deref() == Some(id) {
            self.active_window = self.z_order.first().cloned();
        }
        
        window
    }
    
    /// Get a window by ID
    pub fn get(&self, id: &str) -> Option<&dyn MatrixWindow> {
        self.windows.get(id).map(|w| w.as_ref())
    }
    
    /// Get a mutable window by ID
    pub fn get_mut(&mut self, id: &str) -> Option<&mut dyn MatrixWindow> {
        self.windows.get_mut(id).map(|w| w.as_mut())
    }
    
    /// Activate a window
    pub fn activate(&mut self, id: &str) -> bool {
        if !self.windows.contains_key(id) {
            return false;
        }
        
        // Move window to front of z-order
        self.z_order.retain(|wid| wid != id);
        self.z_order.insert(0, id.to_string());
        
        // Set as active window
        self.active_window = Some(id.to_string());
        
        true
    }
    
    /// Get the active window
    pub fn active_window(&self) -> Option<&dyn MatrixWindow> {
        self.active_window
            .as_ref()
            .and_then(|id| self.windows.get(id))
            .map(|w| w.as_ref())
    }
    
    /// Get the active window (mutable)
    pub fn active_window_mut(&mut self) -> Option<&mut dyn MatrixWindow> {
        if let Some(id) = &self.active_window {
            self.windows.get_mut(id).map(|w| w.as_mut())
        } else {
            None
        }
    }
    
    /// Handle an input event
    pub fn handle_event(&mut self, event: &InputEvent) -> bool {
        // Try active window first
        if let Some(id) = &self.active_window {
            if let Some(window) = self.windows.get_mut(id) {
                let mut modal_state = ModalState::default();
                if window.handle_event(event, &mut modal_state) {
                    return true;
                }
            }
        }
        
        // Check for window switching keys in normal mode
        false
    }
    
    /// Render all windows
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Render from back to front
        for id in self.z_order.iter().rev() {
            if let Some(window) = self.windows.get_mut(id) {
                window.render(area, buf);
            }
        }
    }
    
    /// Get all window IDs
    pub fn window_ids(&self) -> Vec<&str> {
        self.windows.keys().map(|id| id.as_str()).collect()
    }
    
    /// Get all window IDs in z-order
    pub fn z_order(&self) -> Vec<&str> {
        self.z_order.iter().map(|id| id.as_str()).collect()
    }
    
    /// Close all windows
    pub fn close_all(&mut self) {
        self.windows.clear();
        self.z_order.clear();
        self.active_window = None;
    }
}

/// A default implementation of a stateful window
pub struct Window<T> {
    /// Window ID
    id: String,
    /// Window title
    title: String,
    /// Window state
    state: T,
    /// Whether the window is focused
    focused: bool,
}

impl<T> Window<T> {
    /// Create a new window
    pub fn new<S: Into<String>>(id: S, title: S, state: T) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            state,
            focused: false,
        }
    }
    
    /// Get the window ID
    pub fn id(&self) -> &str {
        &self.id
    }
    
    /// Get the window title
    pub fn title(&self) -> &str {
        &self.title
    }
    
    /// Get the window state
    pub fn state(&self) -> &T {
        &self.state
    }
    
    /// Get mutable window state
    pub fn state_mut(&mut self) -> &mut T {
        &mut self.state
    }
    
    /// Set window focus
    pub fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
    
    /// Check if window is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }
}