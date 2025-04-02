use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use std::fmt;
use std::time::{Duration, Instant};

/// Represents a key event with modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// The key code
    pub code: KeyCode,
    /// Modifier keys (ctrl, shift, alt)
    pub modifiers: KeyModifiers,
}

impl Key {
    /// Create a new Key instance
    pub fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    /// Create a Key instance from a KeyEvent
    pub fn from_key_event(event: KeyEvent) -> Self {
        Self { code: event.code, modifiers: event.modifiers }
    }

    /// Create a simple Key with no modifiers
    pub fn simple(code: KeyCode) -> Self {
        Self { code, modifiers: KeyModifiers::empty() }
    }

    /// Check if the key has Ctrl modifier
    pub fn has_ctrl(&self) -> bool {
        self.modifiers.contains(KeyModifiers::CONTROL)
    }

    /// Check if the key has Shift modifier
    pub fn has_shift(&self) -> bool {
        self.modifiers.contains(KeyModifiers::SHIFT)
    }

    /// Check if the key has Alt modifier
    pub fn has_alt(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();

        if self.has_ctrl() {
            parts.push("C");
        }
        if self.has_alt() {
            parts.push("A");
        }
        if self.has_shift() {
            parts.push("S");
        }

        let key_str = match self.code {
            KeyCode::Backspace => "BS".to_string(),
            KeyCode::Enter => "CR".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "S-Tab".to_string(),
            KeyCode::Delete => "Del".to_string(),
            KeyCode::Insert => "Ins".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::F(n) => format!("F{}", n),
            _ => format!("{:?}", self.code),
        };

        if parts.is_empty() {
            write!(f, "{}", key_str)
        } else {
            write!(f, "{}-{}", parts.join("-"), key_str)
        }
    }
}

/// Represents a sequence of keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySequence {
    /// The keys in the sequence
    pub keys: Vec<Key>,
}

impl KeySequence {
    /// Create a new key sequence from a vector of keys
    pub fn new(keys: Vec<Key>) -> Self {
        Self { keys }
    }

    /// Create a key sequence from a vector of key codes
    pub fn from_codes(codes: Vec<KeyCode>) -> Self {
        Self { keys: codes.into_iter().map(Key::simple).collect() }
    }

    /// Create a key sequence from a string (e.g., "gg", "dd")
    pub fn from_str(s: &str) -> Self {
        Self {
            keys: s.chars().map(|c| Key::simple(KeyCode::Char(c))).collect(),
        }
    }

    /// Check if this sequence is a prefix of another sequence
    pub fn is_prefix_of(&self, other: &KeySequence) -> bool {
        if self.keys.len() > other.keys.len() {
            return false;
        }

        self.keys.iter().zip(other.keys.iter()).all(|(a, b)| a == b)
    }

    /// Check if this sequence starts with another sequence
    pub fn starts_with(&self, prefix: &KeySequence) -> bool {
        prefix.is_prefix_of(self)
    }

    /// Get the length of the sequence
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Check if the sequence is empty
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl From<Key> for KeySequence {
    fn from(key: Key) -> Self {
        Self { keys: vec![key] }
    }
}

impl fmt::Display for KeySequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key_strs: Vec<String> = self.keys.iter().map(|k| k.to_string()).collect();
        write!(f, "{}", key_strs.join(""))
    }
}

/// Represents different types of input events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// Keyboard input
    Key(Key),
    /// Mouse input
    Mouse(MouseEvent),
    /// Resize event
    Resize(u16, u16),
    /// Paste event
    Paste(String),
    /// Focus gained event
    FocusGained,
    /// Focus lost event
    FocusLost,
    /// Key sequence completed
    KeySequence(KeySequence),
}

impl From<Event> for InputEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::Key(key) => InputEvent::Key(Key::from_key_event(key)),
            Event::Mouse(mouse) => InputEvent::Mouse(mouse),
            Event::Resize(width, height) => InputEvent::Resize(width, height),
            Event::Paste(text) => InputEvent::Paste(text),
            Event::FocusGained => InputEvent::FocusGained,
            Event::FocusLost => InputEvent::FocusLost,
        }
    }
}

/// Status of a key sequence being processed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SequenceStatus {
    /// No sequence is being processed
    None,
    /// A sequence is being processed and is partially matched
    Partial(KeySequence),
    /// A sequence has been completed
    Complete(KeySequence),
    /// A sequence has timed out
    Timeout(KeySequence),
    /// A sequence was aborted (e.g., by pressing escape)
    Aborted(KeySequence),
}

/// Input handler for processing input events
pub struct InputHandler {
    /// Last input event received
    last_event: Option<InputEvent>,
    /// Current key sequence being processed
    current_sequence: Vec<Key>,
    /// The timestamp of the last key in the sequence
    last_key_time: Option<Instant>,
    /// Timeout duration for key sequences
    sequence_timeout: Duration,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self {
            last_event: None,
            current_sequence: Vec::new(),
            last_key_time: None,
            sequence_timeout: Duration::from_millis(500), // Default timeout: 500ms
        }
    }
}

impl InputHandler {
    /// Create a new input handler
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new input handler with a custom sequence timeout
    pub fn with_timeout(timeout: Duration) -> Self {
        Self { sequence_timeout: timeout, ..Self::default() }
    }

    /// Process an input event
    pub fn process_event(&mut self, event: Event) -> InputEvent {
        let input_event = InputEvent::from(event);
        self.last_event = Some(input_event.clone());
        input_event
    }

    /// Process a key event and update the current sequence
    pub fn process_key(&mut self, key: Key) -> SequenceStatus {
        // Check for timeout
        if let Some(last_time) = self.last_key_time {
            if last_time.elapsed() > self.sequence_timeout && !self.current_sequence.is_empty() {
                let timed_out_sequence = std::mem::replace(&mut self.current_sequence, Vec::new());
                self.last_key_time = None;
                return SequenceStatus::Timeout(KeySequence::new(timed_out_sequence));
            }
        }

        // Special case for Escape key - abort the sequence
        if key.code == KeyCode::Esc && !self.current_sequence.is_empty() {
            let aborted_sequence = std::mem::replace(&mut self.current_sequence, Vec::new());
            self.last_key_time = None;
            return SequenceStatus::Aborted(KeySequence::new(aborted_sequence));
        }

        // Add the key to the current sequence
        self.current_sequence.push(key);
        self.last_key_time = Some(Instant::now());

        // Return the partial sequence status
        SequenceStatus::Partial(KeySequence::new(self.current_sequence.clone()))
    }

    /// Check if the current sequence matches any of the provided sequences
    /// Returns the matching sequence or None if no match
    pub fn check_sequence_match(&self, sequences: &[KeySequence]) -> Option<KeySequence> {
        if self.current_sequence.is_empty() {
            return None;
        }

        let current = KeySequence::new(self.current_sequence.clone());

        // Check for exact matches
        for seq in sequences {
            if &current == seq {
                return Some(seq.clone());
            }
        }

        // Check if current is a prefix of any sequence
        let is_prefix = sequences.iter().any(|seq| current.is_prefix_of(seq));

        if !is_prefix {
            // If not a prefix, reset the sequence
            None
        } else {
            // If it's a prefix, continue collecting keys
            None
        }
    }

    /// Complete the current sequence and reset
    pub fn complete_sequence(&mut self) -> SequenceStatus {
        if self.current_sequence.is_empty() {
            return SequenceStatus::None;
        }

        let completed_sequence = std::mem::replace(&mut self.current_sequence, Vec::new());
        self.last_key_time = None;
        SequenceStatus::Complete(KeySequence::new(completed_sequence))
    }

    /// Abort the current sequence and reset
    pub fn abort_sequence(&mut self) -> SequenceStatus {
        if self.current_sequence.is_empty() {
            return SequenceStatus::None;
        }

        let aborted_sequence = std::mem::replace(&mut self.current_sequence, Vec::new());
        self.last_key_time = None;
        SequenceStatus::Aborted(KeySequence::new(aborted_sequence))
    }

    /// Reset the current sequence
    pub fn reset_sequence(&mut self) {
        self.current_sequence.clear();
        self.last_key_time = None;
    }

    /// Check if a sequence is in progress
    pub fn is_sequence_in_progress(&self) -> bool {
        !self.current_sequence.is_empty()
    }

    /// Get the current sequence
    pub fn current_sequence(&self) -> KeySequence {
        KeySequence::new(self.current_sequence.clone())
    }

    /// Check if the current sequence has timed out
    pub fn has_sequence_timed_out(&self) -> bool {
        if let Some(last_time) = self.last_key_time {
            !self.current_sequence.is_empty() && last_time.elapsed() > self.sequence_timeout
        } else {
            false
        }
    }

    /// Set the sequence timeout duration
    pub fn set_sequence_timeout(&mut self, timeout: Duration) {
        self.sequence_timeout = timeout;
    }

    /// Get the sequence timeout duration
    pub fn sequence_timeout(&self) -> Duration {
        self.sequence_timeout
    }

    /// Get the last input event
    pub fn last_event(&self) -> Option<&InputEvent> {
        self.last_event.as_ref()
    }
}
