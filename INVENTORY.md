# Modalkit Usage Inventory

This document inventories all uses of modalkit in the codebase, categorized by component type. This will guide the replacement of modalkit with our custom implementation.

## 1. Key Handling & Input

### Files:
- `src/keybindings.rs`
- `src/config.rs`

### Components:
- `modalkit::key::TerminalKey` - Terminal key representation
- `modalkit::keybindings::InputKey` - Input key abstraction
- `modalkit::env::vim::keybindings::InputStep` - Step in input processing
- `modalkit::env::vim::keybindings::VimBindings` - Vim-style keyboard bindings
- `modalkit::env::vim::VimMode` - Vim editor modes (Normal, Insert, Visual)
- `modalkit::env::CommonKeyClass` - Common key classes for input handling

### Required Functionality:
- Key event handling with modifier support
- Vim-style modal input (Normal, Insert, Visual modes)
- Key sequence binding to actions
- Custom key bindings from configuration
- Input context tracking

## 2. Actions Framework

### Files:
- `src/commands.rs`
- `src/base.rs`
- `src/windows/mod.rs`
- `src/windows/room/mod.rs`
- `src/windows/room/chat.rs`
- `src/windows/room/scrollback.rs`

### Components:
- `modalkit::actions::Action` - Base action trait
- `modalkit::actions::Editable` - Edit actions interface
- `modalkit::actions::EditorAction` - Editor-specific actions
- `modalkit::actions::Jumpable` - Navigation/jump actions
- `modalkit::actions::CursorAction` - Cursor movement actions
- `modalkit::actions::EditAction` - Text editing actions
- `modalkit::actions::PromptAction` - Dialog prompt actions
- `modalkit::actions::WindowAction` - Window management actions
- `modalkit::actions::InsertTextAction` - Text insertion actions
- `modalkit::actions::MacroAction` - Macro recording/replay

### Required Functionality:
- Action dispatch system to route actions to handlers
- Command parsing and execution
- Action context tracking
- Error handling for actions
- Undo/redo support

## 3. Text Editing

### Files:
- `src/windows/room/chat.rs`
- `src/windows/room/scrollback.rs`
- `src/message/mod.rs`

### Components:
- `modalkit::editing::rope::EditRope` - Text rope data structure
- `modalkit::editing::cursor::Cursor` - Cursor position tracking
- `modalkit::editing::cursor::CursorGroup` - Managing multiple cursors
- `modalkit::editing::cursor::CursorState` - Cursor state (position, selection)
- `modalkit::editing::completion::CompletionList` - Auto-completion support
- `modalkit::editing::context::Resolve` - Context resolution for editing
- `modalkit::editing::context::EditContext` - Editing context
- `modalkit::editing::history::HistoryList` - Command/edit history
- `modalkit::editing::store::RegisterError` - Register errors

### Required Functionality:
- Text buffer management with efficient operations
- Cursor handling (movement, selection)
- Unicode character handling
- History tracking for undo/redo
- Completion support
- Register/clipboard handling

## 4. UI Components

### Files:
- `src/windows/mod.rs`
- `src/windows/room/mod.rs`
- `src/windows/room/chat.rs`
- `src/windows/room/scrollback.rs`
- `src/windows/room/space.rs`
- `src/windows/welcome.rs`
- `src/main.rs`

### Components:
- `modalkit_ratatui::textbox::TextBox` - Text input widget
- `modalkit_ratatui::textbox::TextBoxState` - Text input state
- `modalkit_ratatui::list::List` - List widget
- `modalkit_ratatui::list::ListState` - List state
- `modalkit_ratatui::list::ListCursor` - Cursor in list
- `modalkit_ratatui::list::ListItem` - Item in list
- `modalkit_ratatui::cmdbar::CommandBarState` - Command bar state
- `modalkit_ratatui::screen::Screen` - Screen management
- `modalkit_ratatui::screen::ScreenState` - Screen state
- `modalkit_ratatui::screen::TabbedLayoutDescription` - Tabbed layout description
- `modalkit_ratatui::windows::WindowLayoutDescription` - Window layout description
- `modalkit_ratatui::windows::WindowLayoutState` - Window layout state
- `modalkit_ratatui::TermOffset` - Terminal offset
- `modalkit_ratatui::TerminalCursor` - Terminal cursor
- `modalkit_ratatui::Window` - Window abstraction
- `modalkit_ratatui::WindowOps` - Window operations
- `modalkit_ratatui::ScrollActions` - Scrolling actions
- `modalkit_ratatui::PromptActions` - Prompt actions
- `modalkit_ratatui::TerminalExtOps` - Terminal extension operations

### Required Functionality:
- Window management (creation, focus, layout)
- Text input handling with modal editing
- List widgets with selection and navigation
- Screen management and rendering
- Command bar for command input
- Dialog prompts for user interaction
- Scrolling for overflow content

## 5. Error Handling

### Files:
- `src/windows/room/mod.rs`
- `src/windows/room/scrollback.rs`
- `src/worker.rs`

### Components:
- `modalkit::errors::EditError` - Editing errors
- `modalkit::errors::EditResult` - Result type for editing
- `modalkit::errors::UIError` - UI-related errors
- `modalkit::errors::UIResult` - Result type for UI operations
- `modalkit::errors::CommandError` - Command errors
- `modalkit::errors::CommandResult` - Result type for commands

### Required Functionality:
- Error types for edit operations
- Error types for UI operations
- Result types with appropriate error handling
- Error context for debugging

## 6. Dialog System

### Files:
- `src/windows/room/mod.rs`
- `src/windows/room/chat.rs`

### Components:
- `modalkit::keybindings::dialog::PromptYesNo` - Yes/No confirmation dialog
- `modalkit::keybindings::dialog::MultiChoice` - Multiple choice dialog
- `modalkit::keybindings::dialog::MultiChoiceItem` - Item in multiple choice dialog

### Required Functionality:
- Dialog prompts with customizable options
- Confirmation dialogs (yes/no)
- Multiple choice selection dialogs
- Dialog result handling

## 7. Prelude & General Utilities

### Files:
- Various files

### Components:
- `modalkit::prelude::*` - Common imports
- `modalkit::prelude::EditInfo` - Edit information
- `modalkit::prelude::InfoMessage` - Information message
- `modalkit::prelude::OpenTarget` - Open target for navigation
- `modalkit::prelude::EditTarget` - Edit target
- `modalkit::prelude::SearchType` - Search type
- `modalkit::prelude::PositionList` - Position list
- `modalkit::prelude::MoveDir1D` - 1D movement direction
- `modalkit::prelude::MoveDir2D` - 2D movement direction
- `modalkit::prelude::TargetShape` - Target shape
- `modalkit::prelude::Register` - Register for clipboard
- `modalkit::prelude::ScrollSize` - Scroll size
- `modalkit::prelude::ScrollStyle` - Scroll style

### Required Functionality:
- Common types and traits used throughout the application
- Utility functions and constants
- Standardized interfaces

## Summary of Component Dependencies

1. **Action System**: Central to the application, handles dispatching user input to appropriate handlers
2. **Modal Editing**: Provides vim-style editing with different modes (Normal, Insert, Visual)
3. **Key Binding**: Maps key sequences to actions based on current mode
4. **Text Editing**: Handles text manipulation with cursor management
5. **Window Management**: Controls layout, tabs, and window relationships
6. **UI Components**: Widgets for text display, input, lists, etc.
7. **Dialog System**: User prompts and interaction components

The most critical components to replace are:
1. Modal state system (already implemented)
2. Keybinding system (already implemented)
3. Text editing component (already implemented)
4. Window management system (partially implemented, needs tabs integration)
5. Dialog system (already implemented)