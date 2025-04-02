pub mod action;
pub mod actions;
pub mod input;
pub mod keybinding;
pub mod keybinding_config;
pub mod state;
#[cfg(test)]
mod state_tests;
#[cfg(test)]
mod tests;

// Direction types
/// One-dimensional movement direction (Next or Previous)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDir1D {
    /// Next item
    Next,
    /// Previous item
    Previous,
}

/// Two-dimensional movement direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDir2D {
    /// Move up
    Up,
    /// Move down
    Down,
    /// Move left
    Left,
    /// Move right
    Right,
}

/// Scroll style for scrolling content
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScrollStyle {
    /// Scroll up by a number of lines
    Up(usize),
    /// Scroll down by a number of lines
    Down(usize),
    /// Scroll left by a number of columns
    Left(usize),
    /// Scroll right by a number of columns
    Right(usize),
    /// Scroll to the start of the content
    Home,
    /// Scroll to the end of the content
    End,
    /// Scroll by page
    Page(MoveDir1D),
    /// Scroll to make the cursor visible
    Cursor,
    /// Scroll to center the cursor
    Center,
}

/// Word style for cursor word retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordStyle {
    /// A small word (alphanumeric characters)
    Small,
    /// A medium word (alphanumeric + underscore)
    Medium,
    /// A large word (non-whitespace characters)
    Large,
}

/// Position list type for jump operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionList {
    /// Jump list for navigating cursor positions
    Jump,
    /// Change list for navigating edit positions
    Change,
    /// Tag list for navigating marked positions
    Tag,
}

pub use action::{
    Action,
    ActionContext,
    ActionContextMap,
    ActionDispatcher,
    ActionError,
    ActionHistoryEntry,
    ActionHook,
    ActionResult,
    ActionValidator,
    GlobalAction,
    GlobalContext,
    TextAction,
    TextContext,
    WindowAction as WindowActionTrait,
    WindowContext,
};
pub use actions::{
    ApplicationAction, CyrumAction, DialogAction, EditAction, EditorAction, MatrixAction,
    MovementAction, SearchAction, WindowAction,
};
pub use input::{InputEvent, InputHandler, Key, KeySequence, SequenceStatus};
pub use keybinding::{Keybinding, KeybindingManager, SequenceBinding};
pub use keybinding_config::{setup_default_keybindings, KeybindingConfig};
pub use state::{CursorPosition, CursorState, CursorStyle, ModalState, Mode, ModeData, Selection};
