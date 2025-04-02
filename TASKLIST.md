# Modalkit Replacement Task List

This document tracks the progress of replacing modalkit with native ratatui components, following the implementation plan in IMPLEMENTATION_PLAN.md.

## Phase 1: Core Framework Implementation

### 1. Modal State System (`src/modal/mod.rs` and `src/modal/state.rs`)
1. [x] Create module structure: 
   - [x] Create `src/modal/` directory
   - [x] Create `src/modal/mod.rs` to export the module components
   - [x] Create `src/modal/state.rs` for the state management

2. [x] Implement basic mode types:
   - [x] Create `Mode` enum in `state.rs` with variants: `Normal`, `Insert`, `Visual`
   - [x] Add method to determine if a mode allows input (`allows_input()`)
   - [x] Add method to determine if a mode allows selection (`allows_selection()`)

3. [x] Implement state management:
   - [x] Define `ModalState` struct with current mode
   - [x] Implement `new()` constructor defaulting to Normal mode
   - [x] Add mode transition methods: `enter_normal_mode()`, `enter_insert_mode()`, `enter_visual_mode()`
   - [x] Add previous mode tracking and restoration

4. [x] Add cursor state handling:
   - [x] Add cursor position tracking to ModalState via CursorPosition and CursorState
   - [x] Implement methods to update cursor based on mode (cursor style changes per mode)
   - [x] Create utility methods for cursor visibility and style in different modes

5. [x] Implement mode persistence:
   - [x] Add serialization support for ModalState with serde
   - [x] Create methods to save/restore state between sessions
   - [x] Add support for mode-specific state data

6. [x] Create unit tests for mode system:
   - [x] Test mode transitions
   - [x] Test selection tracking in visual mode
   - [x] Test cursor behavior per mode
   - [x] Test serialization/restoration

### 2. Input and Keybinding System (`src/modal/input.rs` and `src/modal/keybinding.rs`)
1. [x] Create module structure:
   - [x] Extend `src/modal/` directory 
   - [x] Update `src/modal/mod.rs` to export input components
   - [x] Create `src/modal/input.rs` for key definitions and event handling
   - [x] Create `src/modal/keybinding.rs` for keybinding maps

2. [x] Define key input types:
   - [x] Create `Key` type to represent a single keypress (using crossterm's KeyEvent as base)
   - [x] Add formatting for key display and debugging
   - [x] Implement key modifiers (ctrl, shift, alt) detection
   - [x] Create `InputEvent` enum for different input types (keys, mouse, etc.)

3. [x] Implement keybinding infrastructure:
   - [x] Create `Keybinding` struct to associate keys with actions
   - [x] Define `KeybindingManager` to organize bindings by mode
   - [x] Implement lookup methods to find actions for keypresses
   - [x] Add support for mode-specific binding resolution

4. [x] Add key sequence handling:
   - [x] Implement partial match tracking for multi-key sequences
   - [x] Add timeout handling for incomplete sequences
   - [x] Create sequence abort/reset functionality
   - [x] Handle ambiguous sequence resolution

5. [x] Implement configuration loading:
   - [x] Create functions to load keybindings from config
   - [x] Add support for user-defined keymaps
   - [x] Implement default keybindings for each mode

6. [x] Create tests for key input system:
   - [x] Test key parsing and normalization
   - [x] Test binding resolution in different modes
   - [x] Test sequence matching and timeout logic
   - [x] Test configuration loading

### 3. Action Framework (`src/modal/action.rs`)
1. [x] Create module structure:
   - [x] Extend `src/modal/` directory
   - [x] Update `src/modal/mod.rs` to export action components  
   - [x] Create `src/modal/action.rs` with core definitions

2. [x] Define action type system:
   - [x] Create `Action` trait for common interface
   - [x] Define `ActionError` for error handling
   - [x] Add `ActionResult` type alias
   - [x] Create impl_action! macro for shared implementations

3. [x] Implement action dispatch mechanism:
   - [x] Create `ActionDispatcher` to route actions to handlers
   - [x] Implement handler registration system
   - [x] Add action execution pipeline
   - [x] Create return value handling for action results

4. [x] Create action context system:
   - [x] Define `ActionContext` trait to provide execution environment
   - [x] Implement context for different execution scopes (global, window, text)
   - [x] Add context switching/nesting capabilities with `ActionContextMap`

5. [x] Add action execution lifecycle:
   - [x] Implement pre/post action hooks
   - [x] Add action validation mechanisms
   - [x] Create undo/redo support
   - [x] Implement action history tracking

6. [x] Create action system tests:
   - [x] Test action dispatch to correct handlers
   - [x] Test context-based execution
   - [x] Test pre/post hooks functionality
   - [x] Test undo/redo capabilities

## Phase 2: UI Components Implementation

### 1. Text Editing Widget (`src/widgets/texteditor.rs`)
1. [x] Create module structure:
   - [x] Create `src/widgets/` directory
   - [x] Create `src/widgets/mod.rs` to export components
   - [x] Create `src/widgets/texteditor.rs` for the text editor widget

2. [x] Implement basic widget structure:
   - [x] Create `TextEditor` widget implementing `StatefulWidget`
   - [x] Define `TextEditorState` for state management
   - [x] Implement rendering logic with cursor display
   - [x] Add styling options (borders, colors, text styles)

3. [x] Add text content management:
   - [x] Create efficient text storage structure (line-based)
   - [x] Implement text insertion and deletion
   - [x] Add proper Unicode grapheme handling
   - [x] Support multi-line editing

4. [x] Implement cursor functionality:
   - [x] Add cursor positioning and movement (up, down, left, right)
   - [x] Implement line start/end navigation
   - [x] Add document start/end navigation
   - [x] Support screen-relative scrolling

5. [x] Add modal editing features:
   - [x] Integrate with Modal system from Phase 1
   - [x] Add visual mode text selection
   - [x] Implement selection operations

6. [x] Implement advanced text operations:
   - [x] Add clipboard operations (copy, cut, paste)
   - [x] Implement find/replace functionality
   - [x] Create text indentation and formatting helpers
   - [x] Add undo/redo history tracking

7. [x] Add optimization and extensions:
   - [x] Implement syntax highlighting capability
   - [x] Add keyboard shortcuts for common operations
   - [x] Optimize text rendering for large documents
   - [x] Create content completion support

8. [x] Create comprehensive tests:
   - [x] Test basic text editing operations
   - [x] Test cursor movement and positioning
   - [x] Test modal editing functions
   - [x] Test advanced operations and history

### 2. Dialog System (`src/widgets/dialog.rs`)
1. [x] Create module structure:
   - [x] Create `src/widgets/dialog.rs` for dialog widgets
   - [x] Define dialog component exports in `src/widgets/mod.rs`

2. [x] Implement basic dialog foundation:
   - [x] Create `Dialog` base widget implementing `StatefulWidget`
   - [x] Define `DialogState` for tracking dialog state
   - [x] Implement overlay rendering with proper layout
   - [x] Add positioning and sizing options

3. [x] Create dialog type variations:
   - [x] Create `DialogType` enum with various types (Info, Warning, Error, etc.)
   - [x] Implement `ConfirmDialog` functionality for yes/no prompts
   - [x] Create `MessageDialog` functionality for information display
   - [x] Develop `InputDialog` functionality for text input

4. [x] Add dialog styling and appearance:
   - [x] Create customizable borders and styles
   - [x] Implement color schemes for different dialog types
   - [x] Add title bars and formatting options
   - [x] Support for buttons and input fields

5. [x] Implement input handling:
   - [x] Add keyboard navigation for options
   - [x] Create button selection and activation
   - [x] Implement text input for input dialogs
   - [x] Add event handling and result callbacks

6. [x] Create dialog management system:
   - [x] Implement dialog stack for handling multiple dialogs
   - [x] Add z-ordering capabilities
   - [x] Create modal dialog blocking functionality
   - [x] Implement dialog result callbacks

7. [x] Add specialized dialog features:
   - [x] Create progress dialogs with percentage display
   - [x] Implement file browser dialog
   - [x] Add error dialogs with formatting
   - [x] Create multi-step wizard dialogues

8. [x] Create comprehensive tests:
   - [x] Test dialog rendering and display
   - [x] Test input handling and navigation
   - [x] Test different dialog types
   - [x] Test dialog stack management

### 3. Window Management (`src/widgets/window.rs`)
1. [x] Create module structure:
   - [x] Create `src/widgets/window.rs` for window components
   - [x] Update `src/widgets/mod.rs` to export window components

2. [x] Implement window abstractions:
   - [x] Define `WindowComponent` trait for component interface
   - [x] Create `Window` struct implementing `StatefulWidget`
   - [x] Define `WindowState` for state tracking
   - [x] Implement window event handling

3. [x] Create window manager:
   - [x] Implement `WindowManager` class
   - [x] Add window registration/tracking via `WindowId`
   - [x] Create window focus management
   - [x] Add z-order handling for overlapping windows

4. [x] Add window navigation:
   - [x] Implement focus changing between windows
   - [x] Add keyboard navigation shortcuts via function keys
   - [x] Create window cycling functionality
   - [x] Support for window activation/deactivation

5. [x] Implement window layouts:
   - [x] Create layout algorithms (vertical, horizontal, tabbed)
   - [x] Implement window splitting capabilities
   - [x] Add percentage and fixed-size constraints
   - [x] Create layout serialization/deserialization

6. [x] Create window manipulation features:
   - [x] Add window resizing capabilities
   - [x] Implement window moving within layouts
   - [x] Create maximize/minimize functionality
   - [x] Add window closing with confirmations

7. [x] Implement tabbed interface:
   - [x] Create tab bar rendering - Implemented in src/widgets/tabs.rs with support for multiple rendering styles
   - [x] Implement tab switching - Added tab selection and navigation methods to TabsState
   - [x] Add tab reordering capabilities - Added move_tab feature to reorder tabs with proper selection handling
   - [x] Create new tab/close tab functionality - Implemented add_tab and remove_tab methods with proper state management

8. [x] Create comprehensive tests:
   - [x] Test window rendering and focus - Tests in layout_tests.rs verify proper window rendering
   - [x] Test layout algorithms and constraints - Added tests for horizontal, vertical and nested layouts
   - [x] Test window manipulation operations - Added tests for split, close and resize operations
   - [x] Test tabbed interface functionality - Added specific tests for tabbed layout in tabs_tests.rs and layout_tests.rs

## Phase 3: Integration and Migration

### 1. Modalkit Dependency Removal (Already completed)
1. [x] Remove modalkit dependencies from Cargo.toml
2. [x] Add new required dependencies (crossterm, etc.)
3. [x] Update feature definitions to remove modalkit references

### 2. Matrix SDK Wrapper Integration (New!)
1. [x] Create `MatrixFuture` and `MatrixStream` utilities:
   - [x] Implement `MatrixFuture<T>` as a wrapper for futures without using Box<dyn Future>
   - [x] Implement `MatrixStream<T>` for stream types without using Box<dyn Stream>
   - [x] Create utilities for spawning tasks and handling channels

2. [x] Develop core Matrix wrapper components:
   - [x] Create `CyrumClient` wrapper for matrix_sdk::Client
   - [x] Implement `CyrumRoom` for room operations
   - [x] Create `CyrumRoomMember` for member operations
   - [x] Implement `CyrumMedia` for file operations
   - [x] Create `CyrumSync` for sync operations
   - [x] Implement `CyrumEncryption` for E2EE functionality
   - [x] Create `CyrumNotifications` for notification settings

3. [x] Create Matrix window type (`MatrixWindow` trait) - *Moved earlier for logical dependency order*:
   - [x] Define MatrixWindow trait with Matrix-specific operations
   - [x] Add methods for room actions, message display, and timeline navigation
   - [x] Create adapters between CyrumWindow and MatrixWindow
   - [x] Implement shared Matrix action types for window operations

4. [x] Refactor worker.rs to use Matrix SDK wrappers:
   - [x] Replace direct matrix_sdk::Client usage with CyrumClient
   - [x] Update room operations to use CyrumRoom
   - [x] Refactor sync operations to use CyrumSync
   - [x] Update event handling to use MatrixStream subscriptions
   - [x] Modify encryption operations to use CyrumEncryption

### 3. Import Replacement (`find . -name "*.rs" | xargs grep -l "modalkit"`)
1. [x] Create comprehensive inventory of modalkit usage:
   - [x] Run grep to find all files importing modalkit - Found 13 files with modalkit imports
   - [x] Document each use case and required functionality - Created detailed INVENTORY.md with component analysis
   - [x] Categorize imports by component (key handling, window management, etc.) - Organized into 7 functional categories

2. [x] Update basic type imports:
   - [x] Replace modalkit action types with custom Action enums - Created comprehensive actions.rs with EditorAction and CyrumAction enums
   - [x] Update key handling code to use crossterm types - Input handling with crossterm implemented in input.rs
   - [x] Replace modalkit event types with custom events - Added InputEvent enum in input.rs

3. [ ] Update window system imports (incremental approach):
   - [x] Replace Window trait implementations in src/windows/mod.rs - Created CyrumWindow enum and related functionality
   - [x] Create window component interfaces and base types - Implemented in src/cyrum_window.rs with WindowComponent, CyrumWindow, EditableWindow, and MatrixWindow traits
   - [x] Implement window management utility functions - Created WindowManager in src/window_manager.rs with tab management
   - [x] Implement placeholder components:
     - [x] Complete list state implementations (DirectListState, MemberListState, etc.) - Created a generic ListState<T> implementation with full scrolling and selection support and implementated item types (RoomItem, DirectItem, SpaceItem, GenericChatItem and VerifyItem)
     - [x] Implement drawing functionality for different list types - Added draw_list helper method to CyrumWindow with comprehensive rendering logic using ratatui primitives
     - [x] Add proper event handling in WindowManager for tab switching - Implemented key handling for tab navigation with keyboard shortcuts (Tab, Ctrl+H/L, F-keys) and implemented action dispatch
   - [x] Update welcome.rs with new window traits - Completely reimplemented welcome window using CyrumWindow and EditableWindow traits with TextEditorState for content display
   - [ ] Update room module implementations:
     - [ ] Create MatrixRoomState with CyrumRoom integration - Implement RoomState that uses CyrumRoom 
     - [ ] Update room/mod.rs to use new window system - Need to implement RoomState with CyrumWindow traits
     - [ ] Update matrix-specific window implementations:
       - [ ] Update chat.rs to use MatrixWindow trait and CyrumRoom
       - [ ] Update scrollback.rs to use text rendering from TextEditorState
       - [ ] Update space.rs to use ListState mechanism
   - [ ] Modify window layout references and tab container usage

4. [ ] Update editing component imports:
   - [ ] Replace textbox references
   - [ ] Update text selection and cursor code
   - [ ] Modify mode-specific editing functions

5. [ ] Update dialog system imports:
   - [ ] Replace dialog components
   - [ ] Update prompt implementations
   - [ ] Modify confirmation dialog usage

### 4. Matrix Client Integration
1. [x] Update matrix client integration with CyrumClient:
   - [x] Refactor worker.rs to process requests through CyrumClient
   - [x] Update requester system to use MatrixFuture instead of Box<dyn Future>
   - [x] Convert event handlers to use MatrixStream subscriptions
   - [x] Implement proper error handling with detailed Matrix error types

2. [ ] Implement matrix window components:
   - [ ] Create RoomChatWindow using CyrumRoom
   - [ ] Implement SpaceWindow with CyrumClient room hierarchy
   - [ ] Update room list displays to use CyrumClient
   - [ ] Add message composition with CyrumRoom

3. [ ] Enhance user experience with Matrix features:
   - [ ] Add typing notifications via CyrumRoom
   - [ ] Implement read receipts with CyrumRoom
   - [ ] Add reaction support via CyrumRoom
   - [ ] Implement file upload/download with CyrumMedia

4. [ ] Improve E2EE support:
   - [ ] Integrate CyrumEncryption into verification flows
   - [ ] Add key backup UI with CyrumEncryption
   - [ ] Implement device verification with CyrumEncryption
   - [ ] Add cross-signing support via CyrumEncryption

## Phase 4: Testing & Refinement

### 1. Functional Testing
1. [ ] Test modal editing system:
   - [ ] Verify normal mode navigation and commands
   - [ ] Test insert mode text editing capabilities
   - [ ] Confirm visual mode selection functionality
   - [ ] Check command mode operations

2. [ ] Validate window and layout system:
   - [ ] Test window creation and management
   - [ ] Verify split screen functionality
   - [ ] Check tab navigation and management
   - [ ] Test window focus handling

3. [ ] Assess dialog system:
   - [ ] Test confirmation dialogs
   - [ ] Verify input dialog functionality
   - [ ] Check selection dialog operations
   - [ ] Confirm dialog stacking and focus

4. [ ] Test Matrix client integration:
   - [ ] Verify room navigation with CyrumClient
   - [ ] Test message composition with CyrumRoom
   - [ ] Check notification handling with CyrumNotifications
   - [ ] Confirm encryption operations with CyrumEncryption

### 2. Performance Optimization
1. [ ] Measure baseline performance:
   - [ ] Benchmark rendering performance
   - [ ] Measure memory usage
   - [ ] Test CPU utilization
   - [ ] Assess startup time

2. [ ] Optimize rendering:
   - [ ] Implement partial screen updates
   - [ ] Optimize text rendering for large documents
   - [ ] Add rendering caching where appropriate
   - [ ] Reduce redraw frequency

3. [ ] Improve memory management:
   - [ ] Optimize text buffer implementation
   - [ ] Reduce clone operations where possible
   - [ ] Implement memory pooling for frequent allocations
   - [ ] Add lazy loading for large content

4. [ ] Enhance responsiveness:
   - [ ] Implement background processing for heavy operations
   - [ ] Optimize event handling pipeline
   - [ ] Add input debouncing
   - [ ] Improve rendering synchronization

### 3. Documentation and Usability
1. [ ] Update user documentation:
   - [ ] Refresh user manual with new features
   - [ ] Update keybinding reference
   - [ ] Create new user guides for complex operations
   - [ ] Add troubleshooting section for common issues

2. [ ] Create developer documentation:
   - [ ] Document architecture and component relationships
   - [ ] Create API reference for new components
   - [ ] Add examples for common customization scenarios
   - [ ] Write contributing guidelines

3. [ ] Improve error handling:
   - [ ] Implement better error messages
   - [ ] Add recovery mechanisms for common failures
   - [ ] Create logging improvements
   - [ ] Add diagnostic tools for troubleshooting

4. [ ] Enhance user experience:
   - [ ] Add visual indicators for mode changes
   - [ ] Improve status line information
   - [ ] Add tooltips and hints for complex operations
   - [ ] Create contextual help system

## Completed Items

- [x] Update dependency imports for matrix-sdk 0.10.0
- [x] Update module paths (`matrix_auth` → `authentication::matrix`)
- [x] Adapt Matrix authentication method calls to new API
- [x] Create example implementations for key components:
  - [x] Dialog system (`examples/dialog.rs`)
  - [x] Modal text editor (`examples/textbox.rs`)
  - [x] Window management (`examples/window_manager.rs`)
- [x] Implement Matrix SDK wrappers in crates/cyrup-matrix
  - [x] CyrumClient wrapper around matrix_sdk::Client
  - [x] CyrumRoom wrapper for room operations
  - [x] CyrumMedia wrapper for media operations
  - [x] CyrumEncryption wrapper for E2EE functions
  - [x] CyrumSync wrapper for sync operations
  - [x] MatrixFuture and MatrixStream utilities

## Dependencies
- [x] Update ratatui to 0.30.0-alpha.2
- [x] Update ratatui-image to 5.0.0
- [x] Update matrix-sdk to 0.10.0
- [x] Remove modalkit dependencies from Cargo.toml
- [x] Update feature definitions (removing modalkit/clipboard)
- [x] Add crossterm dependency as replacement