use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode,
    enable_raw_mode,
    EnterAlternateScreen,
    LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

// Import from maxtryx
use maxtryx::modal::{InputEvent, Key};
use maxtryx::widgets::{
    Dialog, DialogButton, DialogId, DialogManager, DialogResult, DialogState, DialogType,
    FileEntry, WizardStep
};

struct App {
    dialog_manager: DialogManager,
    last_result: Option<String>,
    progress_dialog_id: Option<DialogId>,
    progress_value: u8,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            dialog_manager: DialogManager::new(),
            last_result: None,
            progress_dialog_id: None,
            progress_value: 0,
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        // Convert crossterm key to our Key type
        let modal_key = Key {
            code: key.code,
            modifiers: key.modifiers,
        };
        let input_event = InputEvent::Key(modal_key);
        
        // Check if the dialog manager handles this event
        if self.dialog_manager.handle_event(&input_event) {
            // Event was handled by a dialog
            
            // Check for dialog results
            if let Some(result) = self.dialog_manager.pop_result() {
                self.handle_dialog_result(result);
            }
            
            return;
        }
        
        // Handle global keys
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            },
            KeyCode::Char('p') => {
                // Show progress dialog
                let dialog = Dialog::default().width_percent(60).height_percent(20);
                let state = DialogState::progress(
                    "File Upload",
                    "Uploading files...",
                    "Starting upload...",
                    false,
                );
                let id = self.dialog_manager.add_dialog(dialog, state, true, None, None);
                self.progress_dialog_id = Some(id);
                self.progress_value = 0;
            },
            KeyCode::Char('i') => {
                // Show indeterminate progress dialog
                let dialog = Dialog::default().width_percent(60).height_percent(20);
                let mut state = DialogState::progress(
                    "Background Task",
                    "Processing data...",
                    "This may take a while...",
                    true, // indeterminate
                );
                let id = self.dialog_manager.add_dialog(dialog, state, true, None, None);
                self.progress_dialog_id = Some(id);
            },
            KeyCode::Char('f') => {
                // Show file browser dialog
                let dialog = Dialog::default().width_percent(70).height_percent(60);
                let state = DialogState::file_browser(
                    "Open File",
                    "Select a file to open:",
                    "/", // start at root directory
                    None, // no filter
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('w') => {
                // Show wizard dialog
                let dialog = Dialog::default().width_percent(80).height_percent(70);
                
                let steps = vec![
                    WizardStep {
                        title: "Welcome".to_string(),
                        content: "Welcome to the setup wizard!\n\nThis wizard will guide you through the setup process. Click Next to continue.".to_string(),
                        is_complete: true,
                    },
                    WizardStep {
                        title: "Configuration".to_string(),
                        content: "Configure your settings here.\n\nYou would normally see configuration options in this section.".to_string(),
                        is_complete: false,
                    },
                    WizardStep {
                        title: "Installation".to_string(),
                        content: "Choose installation options.\n\nThis is where you would select components to install.".to_string(),
                        is_complete: false,
                    },
                    WizardStep {
                        title: "Finish".to_string(),
                        content: "Setup complete!\n\nClick Finish to close the wizard.".to_string(),
                        is_complete: false,
                    },
                ];
                
                let state = DialogState::wizard("Setup Wizard", steps);
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('e') => {
                // Show error dialog with formatting
                let dialog = Dialog::default().width_percent(60).height_percent(40);
                let error_message = "An error occurred while processing your request.\n\nError details:\n- Connection timeout\n- Server returned status code 503\n- Unable to establish secure connection\n\nPlease try again later or contact support.";
                let state = DialogState::message(DialogType::Error, "Error", error_message);
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            _ => {},
        }
    }
    
    fn update(&mut self) {
        // Update progress dialog if active
        if let Some(id) = self.progress_dialog_id {
            if let Some(state) = self.dialog_manager.get_dialog_state_mut(id) {
                if !state.progress_indeterminate {
                    // Update progress value
                    self.progress_value = (self.progress_value + 1).min(100);
                    state.update_progress(self.progress_value, Some(format!("Uploaded {}%", self.progress_value)));
                    
                    // If progress reaches 100%, close the dialog after a short delay
                    if self.progress_value >= 100 {
                        state.update_progress(100, Some("Upload complete!".to_string()));
                        if self.progress_value >= 105 { // Add a small delay
                            self.progress_dialog_id = None;
                            state.result = Some("ok".to_string());
                        }
                    }
                }
            }
        }
    }
    
    fn handle_dialog_result(&mut self, result: DialogResult) {
        // Store the result for display
        match result.id {
            id if Some(id) == self.progress_dialog_id => {
                self.progress_dialog_id = None;
                self.last_result = Some(format!("Progress dialog closed: {}", result.value));
            },
            _ => {
                // Handle other dialog results
                if let Some(input) = result.input {
                    self.last_result = Some(format!("Dialog result: {} (Input: {})", result.value, input));
                } else {
                    self.last_result = Some(format!("Dialog result: {}", result.value));
                }
            }
        }
    }
}

fn main() -> Result<(), io::Error> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app
    let mut app = App::new();

    loop {
        terminal.draw(|f| {
            let area = f.size();
            
            // Draw background with instructions
            let help_text = "
Specialized Dialog Examples

Press the following keys to show different specialized dialogs:
- p: Show progress dialog with percentage
- i: Show indeterminate progress dialog
- f: Show file browser dialog
- w: Show multi-step wizard dialog
- e: Show error dialog with formatting
- Ctrl+Q: Quit
            ";
            
            let instructions = Paragraph::new(help_text)
                .block(Block::default().borders(Borders::ALL).title("Instructions"));
            f.render_widget(instructions, Rect {
                x: 1,
                y: 1,
                width: area.width - 2,
                height: 15,
            });
            
            // Display last result if any
            if let Some(result) = &app.last_result {
                let result_para = Paragraph::new(result.clone())
                    .block(Block::default().borders(Borders::ALL).title("Last Dialog Result"))
                    .style(Style::default().fg(Color::Green));
                
                f.render_widget(result_para, Rect {
                    x: 1,
                    y: 17, 
                    width: area.width - 2,
                    height: 3,
                });
            }
            
            // Render dialog manager
            app.dialog_manager.render(area, f.buffer_mut());
        })?;

        // Check for events (with timeout to allow progress updates)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
                
                if app.should_quit {
                    break;
                }
            }
        }
        
        // Update application state (for progress dialog)
        app.update();
    }

    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}