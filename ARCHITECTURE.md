# Cyrum Architecture

This document outlines the architecture of Cyrum, focusing on the components and how they interact, with special attention to the modal UI system currently using modalkit.

## Overall Structure

Cyrum is a Matrix chat client organized around the following key components:

1. **Base Types** (`src/base.rs`): Core data types and utilities used throughout the codebase
2. **Configuration** (`src/config.rs`): Settings management and configuration loading
3. **Keybindings** (`src/keybindings.rs`): Keyboard input handling and keymapping
4. **Commands** (`src/commands.rs`): Command definitions and execution
5. **Message Handling** (`src/message/`): Message processing, rendering, and storage
6. **Windows System** (`src/windows/`): UI components and window management
7. **Worker** (`src/worker.rs`): Background operations and Matrix SDK interface
8. **Main** (`src/main.rs`): Application entry point and event loop

## Modal System

Cyrum currently uses `modalkit` to provide its modal interface, similar to vim-style editing and navigation. Key components of the modal system:

### Keybinding Management

The keybinding system (`src/keybindings.rs`) integrates with modalkit to:
- Define default keybindings based on vim-style inputs
- Support custom keybindings from config
- Handle mode-specific behaviors (Normal, Insert, Visual)

```rust
// Setup keybindings from modalkit's vim environment
pub fn setup_keybindings() -> Keybindings {
    let mut ism = Keybindings::empty();
    let vim = VimBindings::default()
        .submit_on_enter()
        .cursor_open(MATRIX_ID_WORD.clone());
    vim.setup(&mut ism);
    // Additional custom bindings...
    ism
}
```

### Window System

The window system (`src/windows/`) is built around modalkit's window management:

1. **IambWindow Enum** (`src/windows/mod.rs`): 
   - A container for different window types (rooms, lists, etc.)
   - Implements modalkit traits like `Editable`, `Scrollable`, etc.
   - Delegates actions to the appropriate window type

2. **Room Windows** (`src/windows/room/`):
   - **ChatState** manages the state of a room window
   - **RoomFocus** tracks whether input focus is on the scrollback or message bar
   - Implements message editing, viewing, and composition

3. **Scrollback** (`src/windows/room/scrollback.rs`):
   - Manages room message history display
   - Handles scrolling, navigation, and cursor positioning
   - Implements vim-style motions and searches within the message history

## Modalkit Integration Points

The key integration points with modalkit that would need replacing include:

### 1. Input handling and modes

Modalkit provides vim-like modes (Normal, Insert, Visual) and handles keyboard input processing and mapping to actions. In `src/keybindings.rs`:

```rust
// ChatState needs alternative implementation for focus toggling
pub fn focus_toggle(&mut self) {
    self.focus = match self.focus {
        RoomFocus::Scrollback => RoomFocus::MessageBar,
        RoomFocus::MessageBar => RoomFocus::Scrollback,
    };
}
```

### 2. Window and Buffer Management

Modalkit manages windows, buffers, and cursor state. In `src/windows/mod.rs`:

```rust
// IambWindow would need reimplementation to handle:
impl Window<IambInfo> for IambWindow {
    fn id(&self) -> IambId { ... }
    fn get_tab_title(&self, store: &mut ProgramStore) -> Line { ... }
    fn get_win_title(&self, store: &mut ProgramStore) -> Line { ... }
    fn open(id: IambId, store: &mut ProgramStore) -> IambResult<Self> { ... }
    fn find(name: String, store: &mut ProgramStore) -> IambResult<Self> { ... }
}
```

### 3. Text Editing

Modalkit provides vim-style text editing in the message bar. In `src/windows/room/chat.rs`:

```rust
// TextBoxState comes from modalkit - would need replacement
pub struct ChatState {
    room_id: OwnedRoomId,
    room: MatrixRoom,
    tbox: TextBoxState<IambInfo>,  // modalkit textbox
    // ...other fields
}
```

### 4. Popup Dialogs and Prompts

Modalkit handles confirmations and other dialog popups through its dialog system. In `src/windows/room/chat.rs`:

```rust
// Confirmation dialogs use modalkit's prompt system
MessageAction::Cancel(skip_confirm) => {
    if skip_confirm {
        self.reset();
        return Ok(None);
    }

    self.reply_to = None;
    self.editing = None;

    let msg = "Would you like to clear the message bar?";
    let act = PromptAction::Abort(false);
    let prompt = PromptYesNo::new(msg, vec![Action::from(act)]);
    let prompt = Box::new(prompt);

    Err(UIError::NeedConfirm(prompt))
}
```

### 5. List Navigation

Modalkit provides list navigation with vim-like motions for room lists, member lists, etc. In `src/windows/mod.rs`:

```rust
// List widget state relies on modalkit
pub type DirectListState = ListState<DirectItem, IambInfo>;
pub type MemberListState = ListState<MemberItem, IambInfo>;
pub type RoomListState = ListState<RoomItem, IambInfo>;
// ...and so on
```

## Requirements for Replacing Modalkit

To replace modalkit with native ratatui v0.30 components, we need to implement:

1. **Mode System**: A state machine to track application modes (Normal, Insert, Visual)
2. **Keybinding Manager**: Map keys to actions based on current mode
3. **Text Editor**: A text editing component with vim-like capabilities
4. **Popup Dialog System**: Component for confirmations and user prompts
5. **Window Management**: System to track and switch between windows
6. **List Navigation**: Vim-style navigation for lists with cursor and scrolling

## Implementation Plan

1. Create a mode state tracking system
2. Build a text editing component based on ratatui's Paragraph and Block widgets
3. Implement a popup dialog component
4. Create a window management system to replace modalkit's window handling
5. Build a list component with vim-style navigation
6. Refactor each integration point to use the new native components

This will allow us to maintain the current features and UX while removing the dependency on modalkit.