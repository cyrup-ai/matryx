pub mod dialog;
pub mod dialogmanager;
pub mod layout;
pub mod tabs;
pub mod texteditor;
pub mod window;
pub mod window_manipulation;
pub mod window_closing;

pub use dialog::{Dialog, DialogButton, DialogState, DialogType};
pub use dialogmanager::{DialogId, DialogManager, DialogResult};
pub use layout::{LayoutConstraint, LayoutManager, LayoutNode, LayoutNodeId, LayoutType};
pub use tabs::{Tabs, TabsState, tabs};
pub use texteditor::{CursorPosition, Selection, TextEditor, TextEditorState};
pub use window::{Window, WindowComponent, WindowId, WindowManager, WindowState};
pub use window_manipulation::{WindowManipulator, WindowSize};
pub use window_closing::{WindowCloser, WindowCloseResult};

// Re-export the window components from maxtryx_window.rs 
pub use crate::window_manager::WindowManager as MatrixWindowManager;
