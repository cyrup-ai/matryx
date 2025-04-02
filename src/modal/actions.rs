use std::fmt;

/// Editor movement action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MovementAction {
    /// Move cursor left
    Left,
    /// Move cursor right
    Right,
    /// Move cursor up
    Up,
    /// Move cursor down
    Down,
    /// Move cursor to start of line
    LineStart,
    /// Move cursor to end of line
    LineEnd,
    /// Move cursor to top of buffer
    BufferTop,
    /// Move cursor to bottom of buffer
    BufferBottom,
    /// Move cursor to next word
    NextWord,
    /// Move cursor to previous word
    PrevWord,
    /// Move cursor to beginning of word
    WordStart,
    /// Move cursor to end of word
    WordEnd,
    /// Move cursor to next paragraph
    NextParagraph,
    /// Move cursor to previous paragraph
    PrevParagraph,
    /// Move cursor to matching bracket
    MatchingBracket,
    /// Page up
    PageUp,
    /// Page down
    PageDown,
    /// Scroll up
    ScrollUp,
    /// Scroll down
    ScrollDown,
    /// Scroll to top
    ScrollTop,
    /// Scroll to middle
    ScrollMiddle,
    /// Scroll to bottom
    ScrollBottom,
    /// Scroll to cursor
    ScrollToCursor,
    /// Jump to line by number
    JumpToLine(usize),
    /// Jump to next occurrence of character
    JumpToChar(char),
    /// Jump to previous occurrence of character
    JumpToPrevChar(char),
    /// Jump to match 
    JumpToMatch,
}

impl fmt::Display for MovementAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MovementAction::Left => write!(f, "left"),
            MovementAction::Right => write!(f, "right"),
            MovementAction::Up => write!(f, "up"),
            MovementAction::Down => write!(f, "down"),
            MovementAction::LineStart => write!(f, "line_start"),
            MovementAction::LineEnd => write!(f, "line_end"),
            MovementAction::BufferTop => write!(f, "buffer_top"),
            MovementAction::BufferBottom => write!(f, "buffer_bottom"),
            MovementAction::NextWord => write!(f, "next_word"),
            MovementAction::PrevWord => write!(f, "prev_word"),
            MovementAction::WordStart => write!(f, "word_start"),
            MovementAction::WordEnd => write!(f, "word_end"),
            MovementAction::NextParagraph => write!(f, "next_paragraph"),
            MovementAction::PrevParagraph => write!(f, "prev_paragraph"),
            MovementAction::MatchingBracket => write!(f, "matching_bracket"),
            MovementAction::PageUp => write!(f, "page_up"),
            MovementAction::PageDown => write!(f, "page_down"),
            MovementAction::ScrollUp => write!(f, "scroll_up"),
            MovementAction::ScrollDown => write!(f, "scroll_down"),
            MovementAction::ScrollTop => write!(f, "scroll_top"),
            MovementAction::ScrollMiddle => write!(f, "scroll_middle"),
            MovementAction::ScrollBottom => write!(f, "scroll_bottom"),
            MovementAction::ScrollToCursor => write!(f, "scroll_to_cursor"),
            MovementAction::JumpToLine(n) => write!(f, "jump_to_line({})", n),
            MovementAction::JumpToChar(c) => write!(f, "jump_to_char({})", c),
            MovementAction::JumpToPrevChar(c) => write!(f, "jump_to_prev_char({})", c),
            MovementAction::JumpToMatch => write!(f, "jump_to_match"),
        }
    }
}

/// Text editing action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditAction {
    /// Insert text
    Insert(String),
    /// Delete previous character
    DeleteBack,
    /// Delete next character
    DeleteForward,
    /// Delete line
    DeleteLine,
    /// Delete word
    DeleteWord,
    /// Delete to end of line
    DeleteToEndOfLine,
    /// Delete to start of line
    DeleteToStartOfLine,
    /// Delete word backward
    DeleteWordBackward,
    /// Delete word forward
    DeleteWordForward,
    /// Join lines
    JoinLines,
    /// Indent line
    Indent,
    /// Outdent line
    Outdent,
    /// Swap characters
    SwapCharacters,
    /// Swap lines
    SwapLines,
    /// Swap words
    SwapWords,
    /// To uppercase
    ToUppercase,
    /// To lowercase
    ToLowercase,
    /// Toggle case
    ToggleCase,
    /// Complete word
    CompleteWord,
    /// Expand snippet
    ExpandSnippet,
    /// New line above
    NewLineAbove,
    /// New line below
    NewLineBelow,
    /// Duplicate line
    DuplicateLine,
    /// Undo
    Undo,
    /// Redo
    Redo,
    /// Yank (copy) selection
    Yank,
    /// Paste after cursor
    PasteAfter,
    /// Paste before cursor
    PasteBefore,
    /// Replace character
    ReplaceChar(char),
    /// Replace selection
    ReplaceSelection(String),
    /// Replace line
    ReplaceLine(String),
    /// Replace word
    ReplaceWord(String),
}

impl fmt::Display for EditAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditAction::Insert(text) => {
                if text.len() > 10 {
                    write!(f, "insert({}...)", &text[..10])
                } else {
                    write!(f, "insert({})", text)
                }
            }
            EditAction::DeleteBack => write!(f, "delete_back"),
            EditAction::DeleteForward => write!(f, "delete_forward"),
            EditAction::DeleteLine => write!(f, "delete_line"),
            EditAction::DeleteWord => write!(f, "delete_word"),
            EditAction::DeleteToEndOfLine => write!(f, "delete_to_end_of_line"),
            EditAction::DeleteToStartOfLine => write!(f, "delete_to_start_of_line"),
            EditAction::DeleteWordBackward => write!(f, "delete_word_backward"),
            EditAction::DeleteWordForward => write!(f, "delete_word_forward"),
            EditAction::JoinLines => write!(f, "join_lines"),
            EditAction::Indent => write!(f, "indent"),
            EditAction::Outdent => write!(f, "outdent"),
            EditAction::SwapCharacters => write!(f, "swap_characters"),
            EditAction::SwapLines => write!(f, "swap_lines"),
            EditAction::SwapWords => write!(f, "swap_words"),
            EditAction::ToUppercase => write!(f, "to_uppercase"),
            EditAction::ToLowercase => write!(f, "to_lowercase"),
            EditAction::ToggleCase => write!(f, "toggle_case"),
            EditAction::CompleteWord => write!(f, "complete_word"),
            EditAction::ExpandSnippet => write!(f, "expand_snippet"),
            EditAction::NewLineAbove => write!(f, "new_line_above"),
            EditAction::NewLineBelow => write!(f, "new_line_below"),
            EditAction::DuplicateLine => write!(f, "duplicate_line"),
            EditAction::Undo => write!(f, "undo"),
            EditAction::Redo => write!(f, "redo"),
            EditAction::Yank => write!(f, "yank"),
            EditAction::PasteAfter => write!(f, "paste_after"),
            EditAction::PasteBefore => write!(f, "paste_before"),
            EditAction::ReplaceChar(c) => write!(f, "replace_char({})", c),
            EditAction::ReplaceSelection(text) => {
                if text.len() > 10 {
                    write!(f, "replace_selection({}...)", &text[..10])
                } else {
                    write!(f, "replace_selection({})", text)
                }
            }
            EditAction::ReplaceLine(text) => {
                if text.len() > 10 {
                    write!(f, "replace_line({}...)", &text[..10])
                } else {
                    write!(f, "replace_line({})", text)
                }
            }
            EditAction::ReplaceWord(text) => {
                if text.len() > 10 {
                    write!(f, "replace_word({}...)", &text[..10])
                } else {
                    write!(f, "replace_word({})", text)
                }
            }
        }
    }
}

/// Search action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    /// Search forward
    SearchForward(String),
    /// Search backward
    SearchBackward(String),
    /// Find next occurrence
    FindNext,
    /// Find previous occurrence
    FindPrev,
    /// Replace next occurrence
    ReplaceNext(String),
    /// Replace all occurrences
    ReplaceAll(String),
    /// Cancel search
    CancelSearch,
}

impl fmt::Display for SearchAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SearchAction::SearchForward(query) => write!(f, "search_forward({})", query),
            SearchAction::SearchBackward(query) => write!(f, "search_backward({})", query),
            SearchAction::FindNext => write!(f, "find_next"),
            SearchAction::FindPrev => write!(f, "find_prev"),
            SearchAction::ReplaceNext(text) => write!(f, "replace_next({})", text),
            SearchAction::ReplaceAll(text) => write!(f, "replace_all({})", text),
            SearchAction::CancelSearch => write!(f, "cancel_search"),
        }
    }
}

/// Window action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowAction {
    /// Split window horizontally
    SplitHorizontal,
    /// Split window vertically
    SplitVertical,
    /// Close window
    CloseWindow,
    /// Focus next window
    FocusNextWindow,
    /// Focus previous window
    FocusPrevWindow,
    /// Focus window by index
    FocusWindowByIndex(usize),
    /// Move window split left
    MoveSplitLeft,
    /// Move window split right
    MoveSplitRight,
    /// Move window split up
    MoveSplitUp,
    /// Move window split down
    MoveSplitDown,
    /// Maximize window
    MaximizeWindow,
    /// Restore window
    RestoreWindow,
    /// Create new tab
    NewTab,
    /// Close current tab
    CloseTab,
    /// Focus next tab
    NextTab,
    /// Focus previous tab
    PrevTab,
    /// Focus tab by index
    FocusTabByIndex(usize),
    /// Move tab to the left
    MoveTabLeft,
    /// Move tab to the right
    MoveTabRight,
}

impl fmt::Display for WindowAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WindowAction::SplitHorizontal => write!(f, "split_horizontal"),
            WindowAction::SplitVertical => write!(f, "split_vertical"),
            WindowAction::CloseWindow => write!(f, "close_window"),
            WindowAction::FocusNextWindow => write!(f, "focus_next_window"),
            WindowAction::FocusPrevWindow => write!(f, "focus_prev_window"),
            WindowAction::FocusWindowByIndex(idx) => write!(f, "focus_window_by_index({})", idx),
            WindowAction::MoveSplitLeft => write!(f, "move_split_left"),
            WindowAction::MoveSplitRight => write!(f, "move_split_right"),
            WindowAction::MoveSplitUp => write!(f, "move_split_up"),
            WindowAction::MoveSplitDown => write!(f, "move_split_down"),
            WindowAction::MaximizeWindow => write!(f, "maximize_window"),
            WindowAction::RestoreWindow => write!(f, "restore_window"),
            WindowAction::NewTab => write!(f, "new_tab"),
            WindowAction::CloseTab => write!(f, "close_tab"),
            WindowAction::NextTab => write!(f, "next_tab"),
            WindowAction::PrevTab => write!(f, "prev_tab"),
            WindowAction::FocusTabByIndex(idx) => write!(f, "focus_tab_by_index({})", idx),
            WindowAction::MoveTabLeft => write!(f, "move_tab_left"),
            WindowAction::MoveTabRight => write!(f, "move_tab_right"),
        }
    }
}

/// Dialog action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogAction {
    /// Show dialog
    ShowDialog,
    /// Hide dialog
    HideDialog,
    /// Dialog confirm
    Confirm,
    /// Dialog cancel
    Cancel,
    /// Dialog select next item
    SelectNext,
    /// Dialog select previous item
    SelectPrev,
    /// Dialog select item by index
    SelectItem(usize),
}

impl fmt::Display for DialogAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogAction::ShowDialog => write!(f, "show_dialog"),
            DialogAction::HideDialog => write!(f, "hide_dialog"),
            DialogAction::Confirm => write!(f, "confirm"),
            DialogAction::Cancel => write!(f, "cancel"),
            DialogAction::SelectNext => write!(f, "select_next"),
            DialogAction::SelectPrev => write!(f, "select_prev"),
            DialogAction::SelectItem(idx) => write!(f, "select_item({})", idx),
        }
    }
}

/// Application action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationAction {
    /// Quit application
    Quit,
    /// Save current file
    Save,
    /// Save all files
    SaveAll,
    /// Open file
    Open(String),
    /// Open file dialog
    OpenFileDialog,
    /// Create new file
    New,
    /// Toggle settings
    ToggleSetting(String),
    /// Execute command
    ExecuteCommand(String),
    /// Reload configuration
    ReloadConfig,
    /// Set mode
    SetMode(String),
    /// Show help
    ShowHelp,
    /// Show status message
    ShowStatus(String),
    /// Show error message
    ShowError(String),
}

impl fmt::Display for ApplicationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicationAction::Quit => write!(f, "quit"),
            ApplicationAction::Save => write!(f, "save"),
            ApplicationAction::SaveAll => write!(f, "save_all"),
            ApplicationAction::Open(path) => write!(f, "open({})", path),
            ApplicationAction::OpenFileDialog => write!(f, "open_file_dialog"),
            ApplicationAction::New => write!(f, "new"),
            ApplicationAction::ToggleSetting(setting) => write!(f, "toggle_setting({})", setting),
            ApplicationAction::ExecuteCommand(cmd) => write!(f, "execute_command({})", cmd),
            ApplicationAction::ReloadConfig => write!(f, "reload_config"),
            ApplicationAction::SetMode(mode) => write!(f, "set_mode({})", mode),
            ApplicationAction::ShowHelp => write!(f, "show_help"),
            ApplicationAction::ShowStatus(msg) => write!(f, "show_status({})", msg),
            ApplicationAction::ShowError(msg) => write!(f, "show_error({})", msg),
        }
    }
}

/// Combined action enum that encompasses all action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorAction {
    /// Movement action
    Movement(MovementAction),
    /// Edit action
    Edit(EditAction),
    /// Search action
    Search(SearchAction),
    /// Window action
    Window(WindowAction),
    /// Dialog action
    Dialog(DialogAction),
    /// Application action
    Application(ApplicationAction),
    /// Custom action with name and parameters
    Custom(String, Vec<String>),
}

impl fmt::Display for EditorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditorAction::Movement(action) => write!(f, "Movement({})", action),
            EditorAction::Edit(action) => write!(f, "Edit({})", action),
            EditorAction::Search(action) => write!(f, "Search({})", action),
            EditorAction::Window(action) => write!(f, "Window({})", action),
            EditorAction::Dialog(action) => write!(f, "Dialog({})", action),
            EditorAction::Application(action) => write!(f, "Application({})", action),
            EditorAction::Custom(name, params) => {
                write!(f, "Custom({}, [{}])", name, params.join(", "))
            }
        }
    }
}

/// Matrix-specific action types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatrixAction {
    /// Switch to room
    SwitchRoom(String),
    /// Send message
    SendMessage(String),
    /// Edit message
    EditMessage(String, String),
    /// Delete message
    DeleteMessage(String),
    /// React to message
    React(String, String),
    /// Upload file
    UploadFile(String),
    /// Download file
    DownloadFile(String, String),
    /// Toggle room list
    ToggleRoomList,
    /// Toggle member list
    ToggleMemberList,
    /// Focus chat input
    FocusChatInput,
    /// Focus scrollback
    FocusScrollback,
    /// Toggle typing notification
    ToggleTyping,
    /// Mark room as read
    MarkAsRead(String),
    /// Join room
    JoinRoom(String),
    /// Leave room
    LeaveRoom(String),
    /// Invite user to room
    InviteUser(String, String),
    /// Kick user from room
    KickUser(String, String),
    /// Ban user from room
    BanUser(String, String),
    /// Set room topic
    SetRoomTopic(String, String),
    /// Set room name
    SetRoomName(String, String),
    /// Create room
    CreateRoom(String),
    /// Search messages
    SearchMessages(String),
}

impl fmt::Display for MatrixAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatrixAction::SwitchRoom(room_id) => write!(f, "switch_room({})", room_id),
            MatrixAction::SendMessage(msg) => {
                if msg.len() > 10 {
                    write!(f, "send_message({}...)", &msg[..10])
                } else {
                    write!(f, "send_message({})", msg)
                }
            }
            MatrixAction::EditMessage(event_id, msg) => {
                if msg.len() > 10 {
                    write!(f, "edit_message({}, {}...)", event_id, &msg[..10])
                } else {
                    write!(f, "edit_message({}, {})", event_id, msg)
                }
            }
            MatrixAction::DeleteMessage(event_id) => write!(f, "delete_message({})", event_id),
            MatrixAction::React(event_id, reaction) => write!(f, "react({}, {})", event_id, reaction),
            MatrixAction::UploadFile(path) => write!(f, "upload_file({})", path),
            MatrixAction::DownloadFile(url, path) => write!(f, "download_file({}, {})", url, path),
            MatrixAction::ToggleRoomList => write!(f, "toggle_room_list"),
            MatrixAction::ToggleMemberList => write!(f, "toggle_member_list"),
            MatrixAction::FocusChatInput => write!(f, "focus_chat_input"),
            MatrixAction::FocusScrollback => write!(f, "focus_scrollback"),
            MatrixAction::ToggleTyping => write!(f, "toggle_typing"),
            MatrixAction::MarkAsRead(room_id) => write!(f, "mark_as_read({})", room_id),
            MatrixAction::JoinRoom(room_id) => write!(f, "join_room({})", room_id),
            MatrixAction::LeaveRoom(room_id) => write!(f, "leave_room({})", room_id),
            MatrixAction::InviteUser(room_id, user_id) => write!(f, "invite_user({}, {})", room_id, user_id),
            MatrixAction::KickUser(room_id, user_id) => write!(f, "kick_user({}, {})", room_id, user_id),
            MatrixAction::BanUser(room_id, user_id) => write!(f, "ban_user({}, {})", room_id, user_id),
            MatrixAction::SetRoomTopic(room_id, topic) => write!(f, "set_room_topic({}, {})", room_id, topic),
            MatrixAction::SetRoomName(room_id, name) => write!(f, "set_room_name({}, {})", room_id, name),
            MatrixAction::CreateRoom(name) => write!(f, "create_room({})", name),
            MatrixAction::SearchMessages(query) => write!(f, "search_messages({})", query),
        }
    }
}

/// Combined action enum that encompasses all action types including Matrix-specific actions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CyrumAction {
    /// Editor action
    Editor(EditorAction),
    /// Matrix action
    Matrix(MatrixAction),
}

impl fmt::Display for CyrumAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CyrumAction::Editor(action) => write!(f, "Editor({})", action),
            CyrumAction::Matrix(action) => write!(f, "Matrix({})", action),
        }
    }
}

/// Easily create Movement actions
pub mod movement {
    use super::*;

    pub fn left() -> EditorAction {
        EditorAction::Movement(MovementAction::Left)
    }

    pub fn right() -> EditorAction {
        EditorAction::Movement(MovementAction::Right)
    }

    pub fn up() -> EditorAction {
        EditorAction::Movement(MovementAction::Up)
    }

    pub fn down() -> EditorAction {
        EditorAction::Movement(MovementAction::Down)
    }

    pub fn line_start() -> EditorAction {
        EditorAction::Movement(MovementAction::LineStart)
    }

    pub fn line_end() -> EditorAction {
        EditorAction::Movement(MovementAction::LineEnd)
    }

    pub fn buffer_top() -> EditorAction {
        EditorAction::Movement(MovementAction::BufferTop)
    }

    pub fn buffer_bottom() -> EditorAction {
        EditorAction::Movement(MovementAction::BufferBottom)
    }
}

/// Easily create Edit actions
pub mod edit {
    use super::*;

    pub fn insert(text: &str) -> EditorAction {
        EditorAction::Edit(EditAction::Insert(text.to_string()))
    }

    pub fn delete_back() -> EditorAction {
        EditorAction::Edit(EditAction::DeleteBack)
    }

    pub fn delete_forward() -> EditorAction {
        EditorAction::Edit(EditAction::DeleteForward)
    }

    pub fn delete_line() -> EditorAction {
        EditorAction::Edit(EditAction::DeleteLine)
    }

    pub fn undo() -> EditorAction {
        EditorAction::Edit(EditAction::Undo)
    }

    pub fn redo() -> EditorAction {
        EditorAction::Edit(EditAction::Redo)
    }

    pub fn yank() -> EditorAction {
        EditorAction::Edit(EditAction::Yank)
    }

    pub fn paste_after() -> EditorAction {
        EditorAction::Edit(EditAction::PasteAfter)
    }

    pub fn paste_before() -> EditorAction {
        EditorAction::Edit(EditAction::PasteBefore)
    }
}

/// Easily create Window actions
pub mod window {
    use super::*;

    pub fn split_horizontal() -> EditorAction {
        EditorAction::Window(WindowAction::SplitHorizontal)
    }

    pub fn split_vertical() -> EditorAction {
        EditorAction::Window(WindowAction::SplitVertical)
    }

    pub fn close_window() -> EditorAction {
        EditorAction::Window(WindowAction::CloseWindow)
    }

    pub fn focus_next_window() -> EditorAction {
        EditorAction::Window(WindowAction::FocusNextWindow)
    }

    pub fn focus_prev_window() -> EditorAction {
        EditorAction::Window(WindowAction::FocusPrevWindow)
    }

    pub fn new_tab() -> EditorAction {
        EditorAction::Window(WindowAction::NewTab)
    }

    pub fn close_tab() -> EditorAction {
        EditorAction::Window(WindowAction::CloseTab)
    }

    pub fn next_tab() -> EditorAction {
        EditorAction::Window(WindowAction::NextTab)
    }

    pub fn prev_tab() -> EditorAction {
        EditorAction::Window(WindowAction::PrevTab)
    }
}

/// Easily create Matrix actions
pub mod matrix {
    use super::*;

    pub fn switch_room(room_id: &str) -> CyrumAction {
        CyrumAction::Matrix(MatrixAction::SwitchRoom(room_id.to_string()))
    }

    pub fn send_message(msg: &str) -> CyrumAction {
        CyrumAction::Matrix(MatrixAction::SendMessage(msg.to_string()))
    }

    pub fn focus_chat_input() -> CyrumAction {
        CyrumAction::Matrix(MatrixAction::FocusChatInput)
    }

    pub fn focus_scrollback() -> CyrumAction {
        CyrumAction::Matrix(MatrixAction::FocusScrollback)
    }

    pub fn toggle_room_list() -> CyrumAction {
        CyrumAction::Matrix(MatrixAction::ToggleRoomList)
    }
}