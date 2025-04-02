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
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;

// Import from cyrum
use cyrum::modal::{InputEvent, Key};
use cyrum::widgets::{Dialog, DialogButton, DialogId, DialogManager, DialogResult, DialogState, DialogType};

struct App {
    dialog_manager: DialogManager,
    last_result: Option<String>,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        Self {
            dialog_manager: DialogManager::new(),
            last_result: None,
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
            KeyCode::Char('i') => {
                // Show info dialog
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::message(
                    DialogType::Info, 
                    "Information", 
                    "This is an information dialog message."
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('w') => {
                // Show warning dialog
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::message(
                    DialogType::Warning, 
                    "Warning", 
                    "This is a warning dialog message."
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('e') => {
                // Show error dialog
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::message(
                    DialogType::Error, 
                    "Error", 
                    "This is an error dialog message."
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('c') => {
                // Show confirmation dialog
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::confirm(
                    "Confirmation", 
                    "Do you want to proceed with this action?"
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('t') => {
                // Show input dialog
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::input(
                    "Input", 
                    "Please enter some text:", 
                    "Default text"
                );
                self.dialog_manager.add_dialog(dialog, state, true, None, None);
            },
            KeyCode::Char('m') => {
                // Show multiple dialogs with different z-indices
                let dialog1 = Dialog::default().width_percent(60).height_percent(40);
                let state1 = DialogState::message(
                    DialogType::Info, 
                    "Dialog 1", 
                    "This is the bottom dialog (z-index 0)."
                );
                self.dialog_manager.add_dialog(dialog1, state1, false, Some(0), None);
                
                let dialog2 = Dialog::default().width_percent(50).height_percent(30);
                let state2 = DialogState::message(
                    DialogType::Warning, 
                    "Dialog 2", 
                    "This is the middle dialog (z-index 1)."
                );
                self.dialog_manager.add_dialog(dialog2, state2, false, Some(1), None);
                
                let dialog3 = Dialog::default().width_percent(40).height_percent(20);
                let state3 = DialogState::message(
                    DialogType::Error, 
                    "Dialog 3", 
                    "This is the top dialog (z-index 2)."
                );
                self.dialog_manager.add_dialog(dialog3, state3, true, Some(2), None);
            },
            KeyCode::Char('b') => {
                // Show dialog with callback
                let dialog = Dialog::default().width_percent(50).height_percent(30);
                let state = DialogState::confirm(
                    "Callback Example", 
                    "This dialog has a callback function."
                );
                
                // Create a callback
                let callback = Box::new(|result: &DialogResult| {
                    println!("Dialog callback executed: {:?}", result.value);
                });
                
                self.dialog_manager.add_dialog(dialog, state, true, None, Some(callback));
            },
            _ => {},
        }
    }
    
    fn handle_dialog_result(&mut self, result: DialogResult) {
        // Store the result for display
        if let Some(input) = result.input {
            self.last_result = Some(format!("Dialog result: {} (Input: {})", result.value, input));
        } else {
            self.last_result = Some(format!("Dialog result: {}", result.value));
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
Dialog Manager Example

Press the following keys to show different dialogs:
- i: Show info dialog
- w: Show warning dialog
- e: Show error dialog
- c: Show confirmation dialog
- t: Show text input dialog
- m: Show multiple overlapping dialogs
- b: Show dialog with callback
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

        // Input handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
                
                if app.should_quit {
                    break;
                }
            }
        }
    }

    // Cleanup
    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    Ok(())
}