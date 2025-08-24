//! Welcome Window
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Widget},
};

use crate::base::maxtryxBufferId;
use crate::maxtryx_window::{
    ActionResult, MatrixWindow, EditableWindow, EditableWindowState, ScrollDirection, WindowComponent,
};
use crate::modal::{
    MatrixAction, EditorAction, InputEvent, ModalState, WindowAction, WindowActionTrait,
};
use crate::widgets::texteditor::{DocumentLanguage, TextEditorState};

const WELCOME_TEXT: &str = include_str!("welcome.md");

/// State for the welcome window
#[derive(Debug, Clone)]
pub struct WelcomeState {
    /// Text editor state for the welcome content
    text_editor: TextEditorState,
    /// Modal state for the window
    modal: ModalState,
    /// Window ID
    id: String,
    /// Is the window focused
    focused: bool,
}

impl WelcomeState {
    /// Create a new welcome state
    pub fn new() -> Self {
        let mut text_editor = TextEditorState::new();
        text_editor.set_content(WELCOME_TEXT);
        text_editor.set_language(DocumentLanguage::Markdown);
        text_editor.set_read_only(true);
        
        Self {
            text_editor,
            modal: ModalState::default(),
            id: maxtryxBufferId::Welcome.to_string(),
            focused: false,
        }
    }
}

impl EditableWindowState for WelcomeState {
    fn text_editor_state(&self) -> Option<&TextEditorState> {
        Some(&self.text_editor)
    }
    
    fn text_editor_state_mut(&mut self) -> Option<&mut TextEditorState> {
        Some(&mut self.text_editor)
    }
    
    fn modal_state(&self) -> &ModalState {
        &self.modal
    }
    
    fn modal_state_mut(&mut self) -> &mut ModalState {
        &mut self.modal
    }
}

/// Welcome window component
pub struct WelcomeWindow {
    /// Window state
    state: WelcomeState,
}

impl WelcomeWindow {
    /// Create a new welcome window
    pub fn new() -> Self {
        Self {
            state: WelcomeState::new(),
        }
    }
}

impl WindowComponent for WelcomeWindow {
    fn handle_event(&mut self, event: &InputEvent, modal_state: &mut ModalState) -> bool {
        // Handle basic navigation in read-only mode
        if let Some(text_editor) = self.state.text_editor_state_mut() {
            match event {
                InputEvent::Key(key) => {
                    match key.code {
                        crossterm::event::KeyCode::Up => {
                            text_editor.move_cursor_up();
                            true
                        },
                        crossterm::event::KeyCode::Down => {
                            text_editor.move_cursor_down();
                            true
                        },
                        crossterm::event::KeyCode::Left => {
                            text_editor.move_cursor_left();
                            true
                        },
                        crossterm::event::KeyCode::Right => {
                            text_editor.move_cursor_right();
                            true
                        },
                        crossterm::event::KeyCode::Home => {
                            text_editor.move_cursor_begin_of_line();
                            true
                        },
                        crossterm::event::KeyCode::End => {
                            text_editor.move_cursor_end_of_line();
                            true
                        },
                        crossterm::event::KeyCode::PageUp => {
                            // Scroll up by 10 lines
                            for _ in 0..10 {
                                text_editor.move_cursor_up();
                            }
                            true
                        },
                        crossterm::event::KeyCode::PageDown => {
                            // Scroll down by 10 lines
                            for _ in 0..10 {
                                text_editor.move_cursor_down();
                            }
                            true
                        },
                        _ => false,
                    }
                },
                _ => false,
            }
        } else {
            false
        }
    }
    
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        // Create a block for the window
        let block = Block::default()
            .title(self.window_title())
            .borders(Borders::ALL)
            .border_style(
                if self.is_focused() {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Gray)
                }
            );
            
        // Render contents using text editor
        if let Some(text_editor) = self.state.text_editor_state_mut() {
            let inner_area = block.inner(area);
            
            // Render the block
            block.render(area, buf);
            
            // Render the text editor content
            let editor_widget = crate::widgets::texteditor::TextEditor::default()
                .style(Style::default());
                
            editor_widget.render(inner_area, buf, text_editor);
        } else {
            // Fallback rendering if text editor is not available
            let inner_area = block.inner(area);
            
            // Render the block
            block.render(area, buf);
            
            // Render a simple paragraph as fallback
            let text = Text::from(WELCOME_TEXT);
            let paragraph = Paragraph::new(text)
                .style(Style::default());
                
            paragraph.render(inner_area, buf);
        }
    }
    
    fn can_focus(&self) -> bool {
        true
    }
    
    fn set_focus(&mut self, focused: bool) {
        self.state.focused = focused;
    }
    
    fn is_focused(&self) -> bool {
        self.state.focused
    }
    
    fn title(&self) -> &str {
        "Welcome"
    }
}

impl MatrixWindow for WelcomeWindow {
    fn id(&self) -> &str {
        &self.state.id
    }
    
    fn tab_title(&self) -> String {
        "Welcome".to_string()
    }
    
    fn window_title(&self) -> String {
        "Welcome to Matrix".to_string()
    }
    
    fn duplicate(&self) -> Self 
    where
        Self: Sized
    {
        Self::new()
    }
    
    fn close(&mut self) -> bool {
        true // Always allow closing
    }
    
    fn save(&mut self, _path: Option<&str>) -> ActionResult {
        Ok(None) // Nothing to save (read-only)
    }
    
    fn get_completions(&self) -> Option<Vec<String>> {
        None // No completions in welcome window
    }
    
    fn get_cursor_word(&self) -> Option<String> {
        None // No word selection
    }
    
    fn get_selected_text(&self) -> Option<String> {
        self.state.text_editor.selected_text()
    }
    
    fn execute_action(&mut self, action: EditorAction) -> ActionResult {
        // Handle basic editor actions for navigation
        match action {
            EditorAction::Movement(movement) => {
                use crate::modal::MovementAction;
                
                match movement {
                    MovementAction::Up => self.state.text_editor.move_cursor_up(),
                    MovementAction::Down => self.state.text_editor.move_cursor_down(),
                    MovementAction::Left => self.state.text_editor.move_cursor_left(),
                    MovementAction::Right => self.state.text_editor.move_cursor_right(),
                    MovementAction::LineBegin => self.state.text_editor.move_cursor_begin_of_line(),
                    MovementAction::LineEnd => self.state.text_editor.move_cursor_end_of_line(),
                    _ => return Ok(None), // Ignore other movement actions
                }
                
                Ok(Some(Box::new(true)))
            },
            _ => Ok(None), // Ignore other actions
        }
    }
    
    fn execute_maxtryx_action(&mut self, _action: MatrixAction) -> ActionResult {
        Ok(None) // No Matrix-specific actions in welcome window
    }
}

impl WindowActionTrait for WelcomeWindow {
    fn handle_window_action(&mut self, action: WindowAction) -> ActionResult {
        // Handle window actions
        match action {
            WindowAction::Scroll(direction) => {
                // Convert to our ScrollDirection enum
                use crate::modal::ScrollDirection as ModalScroll;
                
                let dir = match direction {
                    ModalScroll::Up => ScrollDirection::Up,
                    ModalScroll::Down => ScrollDirection::Down,
                    ModalScroll::PageUp => ScrollDirection::PageUp,
                    ModalScroll::PageDown => ScrollDirection::PageDown,
                    ModalScroll::Top => ScrollDirection::Top,
                    ModalScroll::Bottom => ScrollDirection::Bottom,
                    _ => return Ok(None),
                };
                
                self.scroll(dir, 1)
            },
            _ => Ok(None),
        }
    }
}

impl EditableWindow for WelcomeWindow {
    fn state(&self) -> &dyn EditableWindowState {
        &self.state
    }
    
    fn state_mut(&mut self) -> &mut dyn EditableWindowState {
        &mut self.state
    }
    
    fn toggle_focus(&mut self) {
        // No components to toggle focus between in welcome window
    }
    
    fn scroll(&mut self, direction: ScrollDirection, amount: usize) -> ActionResult {
        // Handle scrolling
        match direction {
            ScrollDirection::Up => {
                for _ in 0..amount {
                    self.state.text_editor.move_cursor_up();
                }
            },
            ScrollDirection::Down => {
                for _ in 0..amount {
                    self.state.text_editor.move_cursor_down();
                }
            },
            ScrollDirection::PageUp => {
                for _ in 0..10 {
                    self.state.text_editor.move_cursor_up();
                }
            },
            ScrollDirection::PageDown => {
                for _ in 0..10 {
                    self.state.text_editor.move_cursor_down();
                }
            },
            ScrollDirection::Top => {
                self.state.text_editor.move_cursor_to(
                    crate::widgets::texteditor::CursorPosition::new(0, 0)
                );
            },
            ScrollDirection::Bottom => {
                let last_line = self.state.text_editor.lines.len().saturating_sub(1);
                self.state.text_editor.move_cursor_to(
                    crate::widgets::texteditor::CursorPosition::new(last_line, 0)
                );
            },
            _ => return Ok(None),
        }
        
        Ok(Some(Box::new(true)))
    }
}