use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Padding, Paragraph, StatefulWidget, Widget},
};

use crate::modal::{InputEvent, Key};

/// Type of dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogType {
    /// Information dialog
    Info,
    /// Warning dialog
    Warning,
    /// Error dialog
    Error,
    /// Question dialog
    Question,
    /// Input dialog
    Input,
    /// Progress dialog
    Progress,
    /// File browser dialog
    FileBrowser,
    /// Multi-step wizard dialog
    Wizard,
}

impl DialogType {
    /// Get the color associated with this dialog type
    pub fn color(&self) -> Color {
        match self {
            DialogType::Info => Color::Blue,
            DialogType::Warning => Color::Yellow,
            DialogType::Error => Color::Red,
            DialogType::Question => Color::Green,
            DialogType::Input => Color::Cyan,
            DialogType::Progress => Color::Magenta,
            DialogType::FileBrowser => Color::DarkGray,
            DialogType::Wizard => Color::LightBlue,
        }
    }

    /// Get the title associated with this dialog type
    pub fn title(&self) -> &'static str {
        match self {
            DialogType::Info => "Information",
            DialogType::Warning => "Warning",
            DialogType::Error => "Error",
            DialogType::Question => "Question",
            DialogType::Input => "Input",
            DialogType::Progress => "Progress",
            DialogType::FileBrowser => "File Browser",
            DialogType::Wizard => "Wizard",
        }
    }
    
    /// Check if this dialog type is an input type
    pub fn is_input_type(&self) -> bool {
        *self == DialogType::Input
    }
}

/// Button in a dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DialogButton {
    /// Label of the button
    pub label: String,
    /// Whether the button is selected
    pub selected: bool,
    /// Value returned when the button is pressed
    pub value: String,
}

impl DialogButton {
    /// Create a new button
    pub fn new<S: Into<String>>(label: S, value: S) -> Self {
        Self {
            label: label.into(),
            selected: false,
            value: value.into(),
        }
    }

    /// Create a selected button
    pub fn selected<S: Into<String>>(label: S, value: S) -> Self {
        Self {
            label: label.into(),
            selected: true,
            value: value.into(),
        }
    }
}

/// File entry for file browser dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Name of the file or directory
    pub name: String,
    /// Path of the file or directory
    pub path: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Size of the file (if applicable)
    pub size: Option<u64>,
}

/// Step in a wizard dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WizardStep {
    /// Title of the step
    pub title: String,
    /// Content of the step
    pub content: String,
    /// Whether this step is complete
    pub is_complete: bool,
}

/// State for the dialog widget
#[derive(Debug, Clone)]
pub struct DialogState {
    /// Type of dialog
    pub dialog_type: DialogType,
    /// Title of the dialog
    pub title: String,
    /// Message to display
    pub message: String,
    /// Buttons
    pub buttons: Vec<DialogButton>,
    /// Current button index
    pub current_button: usize,
    /// Input value (for input dialogs)
    pub input_value: String,
    /// Input cursor position
    pub input_cursor: usize,
    /// Dialog result
    pub result: Option<String>,
    
    // Progress dialog specific fields
    /// Progress percentage (0-100)
    pub progress_percent: u8,
    /// Progress status message
    pub progress_status: String,
    /// Whether the progress is indeterminate
    pub progress_indeterminate: bool,
    
    // File browser specific fields
    /// Current directory
    pub current_directory: String,
    /// File entries in the current directory
    pub file_entries: Vec<FileEntry>,
    /// Selected file index
    pub selected_file_index: usize,
    /// Filter for displayed files (e.g., "*.rs")
    pub file_filter: Option<String>,
    
    // Wizard dialog specific fields
    /// Wizard steps
    pub wizard_steps: Vec<WizardStep>,
    /// Current wizard step index
    pub current_step_index: usize,
}

impl DialogState {
    /// Create a new dialog state
    pub fn new<S1: Into<String>, S2: Into<String>>(
        dialog_type: DialogType,
        title: S1,
        message: S2,
        buttons: Vec<DialogButton>,
    ) -> Self {
        let current_button = buttons.iter().position(|b| b.selected).unwrap_or(0);

        Self {
            dialog_type,
            title: title.into(),
            message: message.into(),
            buttons,
            current_button,
            input_value: String::new(),
            input_cursor: 0,
            result: None,
            // Initialize progress dialog fields
            progress_percent: 0,
            progress_status: String::new(),
            progress_indeterminate: false,
            // Initialize file browser fields
            current_directory: String::new(),
            file_entries: Vec::new(),
            selected_file_index: 0,
            file_filter: None,
            // Initialize wizard fields
            wizard_steps: Vec::new(),
            current_step_index: 0,
        }
    }

    /// Create a simple message dialog
    pub fn message<S1: Into<String>, S2: Into<String>>(
        dialog_type: DialogType,
        title: S1,
        message: S2,
    ) -> Self {
        Self::new(dialog_type, title, message, vec![DialogButton::selected("OK", "ok")])
    }

    /// Create a confirmation dialog
    pub fn confirm<S1: Into<String>, S2: Into<String>>(title: S1, message: S2) -> Self {
        Self::new(DialogType::Question, title, message, vec![
            DialogButton::selected("Yes", "yes"),
            DialogButton::new("No", "no"),
        ])
    }

    /// Create an input dialog
    pub fn input<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        title: S1,
        message: S2,
        default_value: S3,
    ) -> Self {
        let mut state = Self::new(DialogType::Input, title, message, vec![
            DialogButton::selected("OK", "ok"),
            DialogButton::new("Cancel", "cancel"),
        ]);
        state.input_value = default_value.into();
        state.input_cursor = state.input_value.len();
        state
    }
    
    /// Create a progress dialog
    pub fn progress<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        title: S1,
        message: S2,
        status: S3,
        indeterminate: bool,
    ) -> Self {
        let mut state = Self::new(
            DialogType::Progress,
            title,
            message,
            vec![DialogButton::new("Cancel", "cancel")],
        );
        state.progress_status = status.into();
        state.progress_indeterminate = indeterminate;
        state
    }
    
    /// Create a file browser dialog
    pub fn file_browser<S1: Into<String>, S2: Into<String>, S3: Into<String>>(
        title: S1,
        message: S2,
        initial_directory: S3,
        filter: Option<String>,
    ) -> Self {
        let mut state = Self::new(
            DialogType::FileBrowser,
            title,
            message,
            vec![
                DialogButton::selected("Open", "open"),
                DialogButton::new("Cancel", "cancel"),
            ],
        );
        
        state.current_directory = initial_directory.into();
        state.file_filter = filter;
        
        // We'll populate file_entries when rendering
        
        state
    }
    
    /// Create a wizard dialog
    pub fn wizard<S: Into<String>>(title: S, steps: Vec<WizardStep>) -> Self {
        let initial_content = if !steps.is_empty() {
            steps[0].content.clone()
        } else {
            String::new()
        };
        
        let mut state = Self::new(
            DialogType::Wizard,
            title,
            initial_content,
            vec![
                DialogButton::selected("Next", "next"),
                DialogButton::new("Previous", "previous"),
                DialogButton::new("Cancel", "cancel"),
            ],
        );
        
        state.wizard_steps = steps;
        state
    }
    
    /// Update progress for a progress dialog
    pub fn update_progress(&mut self, percent: u8, status: Option<String>) {
        self.progress_percent = percent.min(100);
        if let Some(status) = status {
            self.progress_status = status;
        }
    }
    
    /// Set whether the progress dialog shows indeterminate progress
    pub fn set_indeterminate(&mut self, indeterminate: bool) {
        self.progress_indeterminate = indeterminate;
    }
    
    /// Navigate to a directory in the file browser
    pub fn navigate_to_directory<S: Into<String>>(&mut self, directory: S) {
        self.current_directory = directory.into();
        self.selected_file_index = 0;
        // File entries will be populated when rendering
    }
    
    /// Go to the next wizard step
    pub fn next_wizard_step(&mut self) -> bool {
        if self.current_step_index < self.wizard_steps.len() - 1 {
            self.current_step_index += 1;
            self.message = self.wizard_steps[self.current_step_index].content.clone();
            
            // Update the buttons based on step position
            self.update_wizard_buttons();
            true
        } else {
            false
        }
    }
    
    /// Go to the previous wizard step
    pub fn previous_wizard_step(&mut self) -> bool {
        if self.current_step_index > 0 {
            self.current_step_index -= 1;
            self.message = self.wizard_steps[self.current_step_index].content.clone();
            
            // Update the buttons based on step position
            self.update_wizard_buttons();
            true
        } else {
            false
        }
    }
    
    /// Set the completion status of the current wizard step
    pub fn set_current_step_complete(&mut self, complete: bool) {
        if !self.wizard_steps.is_empty() && self.current_step_index < self.wizard_steps.len() {
            self.wizard_steps[self.current_step_index].is_complete = complete;
        }
    }
    
    /// Check if all wizard steps are complete
    pub fn are_all_steps_complete(&self) -> bool {
        !self.wizard_steps.is_empty() && self.wizard_steps.iter().all(|step| step.is_complete)
    }
    
    /// Update wizard buttons based on current step
    fn update_wizard_buttons(&mut self) {
        // Find button indices
        let next_idx = self.buttons.iter().position(|b| b.value == "next");
        let prev_idx = self.buttons.iter().position(|b| b.value == "previous");
        let finish_idx = self.buttons.iter().position(|b| b.value == "finish");
        
        // First step doesn't need Previous button
        if let Some(idx) = prev_idx {
            if self.current_step_index == 0 {
                self.buttons[idx].label = "".to_string();
            } else {
                self.buttons[idx].label = "Previous".to_string();
            }
        }
        
        // Last step has Finish instead of Next
        if let Some(idx) = next_idx {
            if self.current_step_index == self.wizard_steps.len() - 1 {
                self.buttons[idx].label = "Finish".to_string();
                self.buttons[idx].value = "finish".to_string();
            } else {
                self.buttons[idx].label = "Next".to_string();
                self.buttons[idx].value = "next".to_string();
            }
        }
    }

    /// Move to the next button
    pub fn next_button(&mut self) {
        self.current_button = (self.current_button + 1) % self.buttons.len();
    }

    /// Move to the previous button
    pub fn prev_button(&mut self) {
        self.current_button = if self.current_button == 0 {
            self.buttons.len() - 1
        } else {
            self.current_button - 1
        };
    }

    /// Handle a key event
    pub fn handle_event(&mut self, event: &InputEvent) -> bool {
        match event {
            InputEvent::Key(key) => self.handle_key(key),
            _ => false,
        }
    }

    /// Handle a key
    pub fn handle_key(&mut self, key: &Key) -> bool {
        match key.code {
            KeyCode::Left => {
                match self.dialog_type {
                    DialogType::FileBrowser => {
                        // In file browser, left navigates to parent directory
                        if self.current_directory != "/" {
                            let path = std::path::Path::new(&self.current_directory);
                            if let Some(parent) = path.parent() {
                                self.navigate_to_directory(parent.to_string_lossy().to_string());
                                return true;
                            }
                        }
                        false
                    },
                    _ => {
                        self.prev_button();
                        true
                    }
                }
            },
            KeyCode::Right => {
                match self.dialog_type {
                    DialogType::FileBrowser => {
                        // In file browser, right enters selected directory
                        if !self.file_entries.is_empty() && self.selected_file_index < self.file_entries.len() {
                            let selected = &self.file_entries[self.selected_file_index];
                            if selected.is_dir {
                                self.navigate_to_directory(&selected.path);
                                return true;
                            }
                        }
                        self.next_button();
                        true
                    },
                    _ => {
                        self.next_button();
                        true
                    }
                }
            },
            KeyCode::Up => {
                match self.dialog_type {
                    DialogType::FileBrowser => {
                        // Move file selection up
                        if self.selected_file_index > 0 {
                            self.selected_file_index -= 1;
                        }
                        true
                    },
                    _ => false,
                }
            },
            KeyCode::Down => {
                match self.dialog_type {
                    DialogType::FileBrowser => {
                        // Move file selection down
                        if !self.file_entries.is_empty() && self.selected_file_index < self.file_entries.len() - 1 {
                            self.selected_file_index += 1;
                        }
                        true
                    },
                    _ => false,
                }
            },
            KeyCode::Enter => {
                match self.dialog_type {
                    DialogType::Wizard => {
                        // Handle wizard navigation
                        let button_value = &self.buttons[self.current_button].value;
                        
                        if button_value == "next" {
                            self.next_wizard_step();
                            return true;
                        } else if button_value == "previous" {
                            self.previous_wizard_step();
                            return true;
                        } else if button_value == "finish" {
                            // Finish the wizard
                            self.result = Some("finish".to_string());
                            return true;
                        }
                        
                        // Otherwise treat as normal button
                        self.result = Some(button_value.clone());
                        true
                    },
                    DialogType::FileBrowser => {
                        // Check if we're on a directory entry
                        if !self.file_entries.is_empty() && self.selected_file_index < self.file_entries.len() {
                            let selected = &self.file_entries[self.selected_file_index];
                            if selected.is_dir {
                                // Navigate into directory
                                self.navigate_to_directory(&selected.path);
                                return true;
                            }
                        }
                        
                        // If button is selected, use that
                        if self.current_button < self.buttons.len() {
                            let button_value = &self.buttons[self.current_button].value;
                            if button_value == "open" && !self.file_entries.is_empty() && self.selected_file_index < self.file_entries.len() {
                                // Return the selected file path
                                let selected = &self.file_entries[self.selected_file_index];
                                self.result = Some(selected.path.clone());
                            } else {
                                self.result = Some(button_value.clone());
                            }
                            return true;
                        }
                        
                        false
                    },
                    DialogType::Progress => {
                        // Progress dialogs usually only have a Cancel button
                        self.result = Some("cancel".to_string());
                        true
                    },
                    _ => {
                        // Default behavior for other dialog types
                        self.result = Some(self.buttons[self.current_button].value.clone());
                        true
                    },
                }
            },
            KeyCode::Esc => {
                self.result = Some("cancel".to_string());
                true
            },
            _ => {
                match self.dialog_type {
                    DialogType::Input => self.handle_input_key(key),
                    _ => false,
                }
            },
        }
    }

    /// Handle a key for an input dialog
    fn handle_input_key(&mut self, key: &Key) -> bool {
        match key.code {
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    self.input_value.remove(self.input_cursor - 1);
                    self.input_cursor -= 1;
                }
                true
            },
            KeyCode::Delete => {
                if self.input_cursor < self.input_value.len() {
                    self.input_value.remove(self.input_cursor);
                }
                true
            },
            KeyCode::Left => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                }
                true
            },
            KeyCode::Right => {
                if self.input_cursor < self.input_value.len() {
                    self.input_cursor += 1;
                }
                true
            },
            KeyCode::Home => {
                self.input_cursor = 0;
                true
            },
            KeyCode::End => {
                self.input_cursor = self.input_value.len();
                true
            },
            KeyCode::Char(c) => {
                self.input_value.insert(self.input_cursor, c);
                self.input_cursor += 1;
                true
            },
            _ => false,
        }
    }
}

/// Dialog widget
pub struct Dialog<'a> {
    /// Block for the dialog
    block: Option<Block<'a>>,
    /// Style for the dialog
    style: Style,
    /// Width of the dialog (percentage of screen)
    width_percent: u16,
    /// Height of the dialog (percentage of screen)
    height_percent: u16,
}

impl<'a> Default for Dialog<'a> {
    fn default() -> Self {
        Self {
            block: None,
            style: Style::default(),
            width_percent: 60,
            height_percent: 40,
        }
    }
}

impl<'a> Dialog<'a> {
    /// Create a new dialog
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the block for the dialog
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set the style for the dialog
    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Set the width percentage
    pub fn width_percent(mut self, percent: u16) -> Self {
        self.width_percent = percent.clamp(20, 100);
        self
    }

    /// Set the height percentage
    pub fn height_percent(mut self, percent: u16) -> Self {
        self.height_percent = percent.clamp(20, 100);
        self
    }
}

impl<'a> StatefulWidget for Dialog<'a> {
    type State = DialogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate dialog dimensions
        let width = (area.width * self.width_percent) / 100;
        let height = (area.height * self.height_percent) / 100;

        // Ensure minimum dimensions
        let width = width.max(40);
        let height = height.max(10);

        // Calculate dialog position
        let x = area.left() + (area.width - width) / 2;
        let y = area.top() + (area.height - height) / 2;

        let dialog_area = Rect { x, y, width, height };

        // Create a block for the dialog
        let dialog_block = Block::default()
            .title(format!(" {} - {} ", state.dialog_type.title(), state.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(state.dialog_type.color()))
            .style(self.style);

        // Render the dialog block
        dialog_block.render(dialog_area, buf);

        // Get the inner area
        let inner_area = dialog_block.inner(dialog_area);

        // Create layouts for different parts of the dialog
        let chunks = match state.dialog_type {
            DialogType::Input => {
                // For input dialogs: message, input field, buttons
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(2), // Message area
                        Constraint::Length(3), // Input field
                        Constraint::Length(3), // Buttons
                    ])
                    .split(inner_area)
            },
            DialogType::Progress => {
                // For progress dialogs: message, progress bar, status, buttons
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Message area
                        Constraint::Length(1), // Progress bar
                        Constraint::Length(1), // Status text
                        Constraint::Min(1),    // Spacing
                        Constraint::Length(3), // Buttons
                    ])
                    .split(inner_area)
            },
            DialogType::FileBrowser => {
                // For file browser: message, file list, buttons
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(2), // Message/path area
                        Constraint::Min(5),    // File list
                        Constraint::Length(3), // Buttons
                    ])
                    .split(inner_area)
            },
            DialogType::Wizard => {
                // For wizard dialogs: steps list, content, buttons
                let horizontal = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(30), // Steps list
                        Constraint::Percentage(70), // Content
                    ])
                    .split(inner_area);
                
                // Content area is further split vertically
                let content_area = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(2), // Content
                        Constraint::Length(3), // Buttons
                    ])
                    .split(horizontal[1]);
                
                // Create a composite layout
                vec![
                    horizontal[0],     // Steps list
                    content_area[0],   // Content
                    content_area[1],   // Buttons
                ]
            },
            _ => {
                // For standard dialogs: message, buttons
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(2), // Message area
                        Constraint::Length(3), // Buttons
                    ])
                    .split(inner_area)
            },
        };

        // Render message (for all dialog types)
        match state.dialog_type {
            DialogType::FileBrowser => {
                // For file browser, show current directory
                let path_text = format!("Path: {}", state.current_directory);
                let message = Paragraph::new(path_text)
                    .alignment(Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                message.render(chunks[0], buf);
            },
            DialogType::Wizard => {
                // For wizard, render steps list in first chunk
                let mut step_texts = Vec::new();
                for (i, step) in state.wizard_steps.iter().enumerate() {
                    let prefix = if i == state.current_step_index { "> " } else { "  " };
                    let status = if step.is_complete { "[✓] " } else { "[ ] " };
                    let style = if i == state.current_step_index {
                        Style::default().fg(state.dialog_type.color()).add_modifier(Modifier::BOLD)
                    } else if step.is_complete {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default()
                    };
                    
                    step_texts.push(Line::from(Span::styled(
                        format!("{}{}{}", prefix, status, step.title), 
                        style,
                    )));
                }
                
                let steps_list = Paragraph::new(step_texts)
                    .block(Block::default().borders(Borders::RIGHT))
                    .alignment(Alignment::Left);
                steps_list.render(chunks[0], buf);
                
                // Render content in second chunk
                let message = Paragraph::new(state.message.clone())
                    .alignment(Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                message.render(chunks[1], buf);
            },
            _ => {
                // Standard message rendering for other dialog types
                let message = Paragraph::new(state.message.clone())
                    .alignment(Alignment::Left)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                message.render(chunks[0], buf);
            },
        }

        // Render specialized components based on dialog type
        match state.dialog_type {
            DialogType::Input => {
                // Render input field for input dialogs
                let input_block = Block::default()
                    .borders(Borders::ALL)
                    .title(" Input ")
                    .border_style(Style::default().fg(Color::Cyan));

                let input_area = input_block.inner(chunks[1]);
                input_block.render(chunks[1], buf);

                // Render input text with cursor
                let cursor_position = state.input_cursor.min(state.input_value.len());
                let (before_cursor, after_cursor) = state.input_value.split_at(cursor_position);

                let mut spans = Vec::new();
                spans.push(Span::raw(before_cursor));

                if after_cursor.is_empty() {
                    // Cursor at the end
                    spans.push(Span::styled(" ", Style::default().bg(Color::White)));
                } else {
                    // Cursor in the middle
                    let first_char = after_cursor.chars().next().unwrap();
                    let rest = &after_cursor[first_char.len_utf8()..];
                    spans.push(Span::styled(
                        first_char.to_string(),
                        Style::default().bg(Color::White).fg(Color::Black),
                    ));
                    spans.push(Span::raw(rest));
                }

                let input_text = Line::from(spans);
                buf.set_line(input_area.x, input_area.y, &input_text, input_area.width);
            },
            DialogType::Progress => {
                // Render progress bar
                let bar_area = chunks[1];
                let bar_width = bar_area.width as usize - 2; // Leave space for brackets
                
                if state.progress_indeterminate {
                    // Indeterminate progress bar (animated)
                    let elapsed = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis();
                    
                    let position = (elapsed / 100) as usize % (bar_width * 2);
                    let position = if position > bar_width { bar_width * 2 - position } else { position };
                    
                    let mut bar = String::with_capacity(bar_width + 2);
                    bar.push('[');
                    for i in 0..bar_width {
                        if i == position {
                            bar.push('█');
                        } else {
                            bar.push(' ');
                        }
                    }
                    bar.push(']');
                    
                    buf.set_string(
                        bar_area.x, 
                        bar_area.y, 
                        bar, 
                        Style::default().fg(state.dialog_type.color())
                    );
                } else {
                    // Determinate progress bar
                    let filled = (bar_width * state.progress_percent as usize) / 100;
                    
                    let mut bar = String::with_capacity(bar_width + 2);
                    bar.push('[');
                    for i in 0..bar_width {
                        if i < filled {
                            bar.push('█');
                        } else {
                            bar.push(' ');
                        }
                    }
                    bar.push(']');
                    
                    buf.set_string(
                        bar_area.x, 
                        bar_area.y, 
                        bar, 
                        Style::default().fg(state.dialog_type.color())
                    );
                    
                    // Also show percentage
                    let percent_text = format!(" {}% ", state.progress_percent);
                    let percent_x = bar_area.x + (bar_area.width - percent_text.len() as u16) / 2;
                    buf.set_string(
                        percent_x,
                        bar_area.y,
                        percent_text,
                        Style::default().fg(Color::White).bg(state.dialog_type.color())
                    );
                }
                
                // Show status text
                let status_text = Paragraph::new(state.progress_status.clone())
                    .alignment(Alignment::Center);
                status_text.render(chunks[2], buf);
            },
            DialogType::FileBrowser => {
                // Populate the file entries if needed (this would be done by the application in real use)
                if state.file_entries.is_empty() {
                    // This is just a placeholder - in a real app we'd read from the filesystem
                    // We're adding some sample entries for the example
                    if state.current_directory == "/" {
                        state.file_entries = vec![
                            FileEntry {
                                name: "home".to_string(),
                                path: "/home".to_string(),
                                is_dir: true,
                                size: None,
                            },
                            FileEntry {
                                name: "usr".to_string(),
                                path: "/usr".to_string(),
                                is_dir: true,
                                size: None,
                            },
                            FileEntry {
                                name: "var".to_string(),
                                path: "/var".to_string(),
                                is_dir: true,
                                size: None,
                            },
                            FileEntry {
                                name: "README.txt".to_string(),
                                path: "/README.txt".to_string(),
                                is_dir: false,
                                size: Some(1024),
                            },
                        ];
                    } else if state.current_directory == "/home" {
                        state.file_entries = vec![
                            FileEntry {
                                name: "user".to_string(),
                                path: "/home/user".to_string(),
                                is_dir: true,
                                size: None,
                            },
                            FileEntry {
                                name: "notes.txt".to_string(),
                                path: "/home/notes.txt".to_string(),
                                is_dir: false,
                                size: Some(2048),
                            },
                        ];
                    } else {
                        // For any other directory, we'll just show a sample file
                        state.file_entries = vec![
                            FileEntry {
                                name: "sample.txt".to_string(),
                                path: format!("{}/sample.txt", state.current_directory),
                                is_dir: false,
                                size: Some(512),
                            },
                        ];
                    }
                }
                
                // Render file list
                let file_list_area = chunks[1];
                let file_list_block = Block::default()
                    .borders(Borders::ALL)
                    .title(" Files ")
                    .border_style(Style::default().fg(state.dialog_type.color()));
                
                let file_list_inner = file_list_block.inner(file_list_area);
                file_list_block.render(file_list_area, buf);
                
                // Render file entries
                let visible_items = file_list_inner.height as usize;
                let start_idx = if state.selected_file_index >= visible_items {
                    state.selected_file_index - visible_items + 1
                } else {
                    0
                };
                
                for (i, entry) in state.file_entries.iter().enumerate().skip(start_idx).take(visible_items) {
                    if i >= start_idx + visible_items {
                        break;
                    }
                    
                    let y = file_list_inner.y + (i - start_idx) as u16;
                    
                    // Format the entry
                    let icon = if entry.is_dir { "📁 " } else { "📄 " };
                    let size = entry.size.map_or("".to_string(), |s| format!(" ({} bytes)", s));
                    let entry_text = format!("{}{}{}", icon, entry.name, size);
                    
                    // Style based on selection
                    let style = if i == state.selected_file_index {
                        Style::default().bg(state.dialog_type.color()).fg(Color::White)
                    } else {
                        Style::default()
                    };
                    
                    buf.set_string(file_list_inner.x, y, entry_text, style);
                }
            },
            _ => {}, // No additional rendering for other dialog types
        }

        // Determine the buttons area based on dialog type
        let buttons_chunk = match state.dialog_type {
            DialogType::Input => chunks[2],
            DialogType::Progress => chunks[4],
            DialogType::FileBrowser => chunks[2],
            DialogType::Wizard => chunks[2],
            _ => chunks[1],
        };

        // Render buttons (for all dialog types)
        // Skip rendering buttons with empty labels (used for wizard navigation)
        let visible_buttons: Vec<_> = state.buttons.iter()
            .enumerate()
            .filter(|(_, b)| !b.label.is_empty())
            .collect();
        
        let button_count = visible_buttons.len();
        
        if button_count > 0 {
            let button_width = 10;
            let total_width = button_count as u16 * button_width + (button_count as u16 - 1) * 2;
            let start_x = inner_area.x + (inner_area.width - total_width) / 2;
            
            for (button_idx, (i, button)) in visible_buttons.iter().enumerate() {
                let button_x = start_x + button_idx as u16 * (button_width + 2);
                let button_area = Rect {
                    x: button_x,
                    y: buttons_chunk.y + 1,
                    width: button_width,
                    height: 1,
                };
                
                let is_selected = *i == state.current_button;
                let button_style = if is_selected {
                    Style::default()
                        .bg(state.dialog_type.color())
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                
                let button_text = format!("[ {} ]", button.label);
                buf.set_string(button_area.x, button_area.y, button_text, button_style);
            }
        }
    }
}

impl<'a> Widget for Dialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut state =
            DialogState::message(DialogType::Info, "Dialog", "This is a dialog message");
        StatefulWidget::render(self, area, buf, &mut state);
    }
}

// Include tests module
#[path = "dialog_tests.rs"]
mod tests;
