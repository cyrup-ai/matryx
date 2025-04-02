# Clipboard Implementation in Ratatui Applications

## Overview

Ratatui is a Rust library for building terminal user interfaces (TUIs), but it doesn't provide built-in clipboard functionality. Terminal applications have unique challenges when implementing clipboard operations due to the terminal environment's limitations and cross-platform considerations.

This document outlines approaches to implementing clipboard operations in Ratatui applications, based on research of existing implementations and best practices.

## Challenges with Clipboard in Terminal Applications

1. **No Standard API**: Ratatui doesn't provide a standardized clipboard API.
2. **Terminal Limitations**: Terminal environments have inherent limitations for clipboard access.
3. **Mouse Capture Interference**: When a terminal application captures mouse events, it can interfere with the terminal's built-in text selection and clipboard functionality.
4. **Cross-Platform Compatibility**: Clipboard behavior varies across operating systems.
5. **Permissions**: Some environments restrict clipboard access for security reasons.

## Implementation Approaches

### 1. External Clipboard Crates

Several Rust crates provide clipboard functionality that can be integrated with Ratatui applications:

#### `clipboard` Crate
A simple cross-platform clipboard library that works on Windows, macOS, and Linux (X11).

```rust
use clipboard::{ClipboardContext, ClipboardProvider};

fn copy_to_clipboard(text: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut ctx: ClipboardContext = ClipboardProvider::new()?;
    ctx.set_contents(text.to_owned())?;
    Ok(())
}

fn paste_from_clipboard() -> Result<String, Box<dyn std::error::Error>> {
    let mut ctx: ClipboardContext = ClipboardProvider::new()?;
    Ok(ctx.get_contents()?)
}
```

#### `arboard` Crate
A more modern clipboard library with additional features:

```rust
use arboard::Clipboard;

fn copy_to_clipboard(text: &str) -> Result<(), arboard::Error> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(text)?;
    Ok(())
}

fn paste_from_clipboard() -> Result<String, arboard::Error> {
    let mut clipboard = Clipboard::new()?;
    clipboard.get_text()
}
```

### 2. Trait-Based Approach

The `rat-text` crate uses a trait-based approach to allow for flexible clipboard implementation:

```rust
// Clipboard trait to link to some clipboard implementation
pub trait Clipboard {
    fn set_contents(&mut self, contents: String) -> Result<(), Box<dyn std::error::Error>>;
    fn get_contents(&mut self) -> Result<String, Box<dyn std::error::Error>>;
}

// Example implementation using the clipboard crate
struct SystemClipboard {
    context: clipboard::ClipboardContext,
}

impl SystemClipboard {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            context: clipboard::ClipboardProvider::new()?,
        })
    }
}

impl Clipboard for SystemClipboard {
    fn set_contents(&mut self, contents: String) -> Result<(), Box<dyn std::error::Error>> {
        self.context.set_contents(contents)?;
        Ok(())
    }

    fn get_contents(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(self.context.get_contents()?)
    }
}
```

This approach allows for:
- Easy substitution of different clipboard implementations
- Testing with mock clipboard implementations
- Fallback mechanisms when clipboard access is restricted

### 3. Internal Clipboard Buffer

For situations where system clipboard access is unavailable or restricted:

```rust
struct InternalClipboard {
    buffer: String,
}

impl InternalClipboard {
    fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }
    
    fn copy(&mut self, text: &str) {
        self.buffer = text.to_owned();
    }
    
    fn paste(&self) -> &str {
        &self.buffer
    }
}
```

### 4. Hybrid Approach

Combining internal buffers with system clipboard access when available:

```rust
enum ClipboardImpl {
    System(SystemClipboard),
    Internal(InternalClipboard),
}

impl ClipboardImpl {
    fn new() -> Self {
        match SystemClipboard::new() {
            Ok(clipboard) => ClipboardImpl::System(clipboard),
            Err(_) => ClipboardImpl::Internal(InternalClipboard::new()),
        }
    }
    
    fn copy(&mut self, text: &str) {
        match self {
            ClipboardImpl::System(clipboard) => {
                let _ = clipboard.set_contents(text.to_owned());
            }
            ClipboardImpl::Internal(clipboard) => {
                clipboard.copy(text);
            }
        }
    }
    
    fn paste(&mut self) -> String {
        match self {
            ClipboardImpl::System(clipboard) => {
                clipboard.get_contents().unwrap_or_default()
            }
            ClipboardImpl::Internal(clipboard) => {
                clipboard.paste().to_owned()
            }
        }
    }
}
```

## Best Practices for Ratatui Applications

1. **Handle Mouse Capture Carefully**:
   - Consider toggling mouse capture off during text selection
   - Provide keyboard shortcuts for clipboard operations

2. **Implement Standard Key Bindings**:
   - Ctrl+C, Ctrl+X, Ctrl+V for copy, cut, and paste
   - Consider vi-like bindings (y, d, p) for applications with vi keybindings

3. **Provide Visual Feedback**:
   - Highlight selected text
   - Indicate when text has been copied

4. **Error Handling**:
   - Gracefully handle clipboard access failures
   - Provide fallback mechanisms

5. **Selection Management**:
   - Implement a clear selection model with start and end points
   - Support both keyboard and mouse selection methods

## Integration with TextEditor Widget

For a TextEditor widget in a Ratatui application, clipboard functionality typically involves:

1. **Selection State**:
   ```rust
   pub struct Selection {
       start: (usize, usize), // (row, column)
       end: (usize, usize),   // (row, column)
   }
   
   pub struct TextEditorState {
       pub lines: Vec<String>,
       pub cursor: (usize, usize),
       pub selection: Option<Selection>,
       // other fields...
   }
   ```

2. **Clipboard Operations**:
   ```rust
   impl TextEditorState {
       pub fn copy(&self, clipboard: &mut impl Clipboard) -> Result<(), Box<dyn std::error::Error>> {
           if let Some(selection) = &self.selection {
               let text = self.get_selected_text(selection);
               clipboard.set_contents(text)?;
           }
           Ok(())
       }
       
       pub fn cut(&mut self, clipboard: &mut impl Clipboard) -> Result<(), Box<dyn std::error::Error>> {
           if let Some(selection) = &self.selection {
               let text = self.get_selected_text(selection);
               clipboard.set_contents(text)?;
               self.delete_selection(selection);
               self.selection = None;
           }
           Ok(())
       }
       
       pub fn paste(&mut self, clipboard: &mut impl Clipboard) -> Result<(), Box<dyn std::error::Error>> {
           if let Some(selection) = &self.selection {
               self.delete_selection(selection);
               self.selection = None;
           }
           
           let text = clipboard.get_contents()?;
           self.insert_text_at_cursor(&text);
           Ok(())
       }
       
       // Helper methods...
   }
   ```

3. **Event Handling**:
   ```rust
   fn handle_key_event(&mut self, key: KeyEvent, clipboard: &mut impl Clipboard) -> Result<(), Box<dyn std::error::Error>> {
       match key {
           // Copy
           KeyEvent { code: KeyCode::Char('c'), modifiers: KeyModifiers::CONTROL, .. } => {
               self.copy(clipboard)?;
           }
           
           // Cut
           KeyEvent { code: KeyCode::Char('x'), modifiers: KeyModifiers::CONTROL, .. } => {
               self.cut(clipboard)?;
           }
           
           // Paste
           KeyEvent { code: KeyCode::Char('v'), modifiers: KeyModifiers::CONTROL, .. } => {
               self.paste(clipboard)?;
           }
           
           // Other key handling...
           _ => {}
       }
       Ok(())
   }
   ```

## Example Implementations

1. **clipboard-history-tui**: Uses the `clipboard` crate with a custom wrapper.
2. **rat-text**: Implements a trait-based approach for flexible clipboard integration.
3. **viuer**: Handles terminal limitations for image display and clipboard operations.

## Conclusion

Implementing clipboard functionality in Ratatui applications requires consideration of terminal limitations and cross-platform behavior. By using external clipboard crates with appropriate abstraction (like the trait-based approach), it's possible to provide robust clipboard operations while maintaining flexibility and testability.

For the Cyrum project, a hybrid approach combining system clipboard access with an internal buffer would provide the best user experience across different environments.