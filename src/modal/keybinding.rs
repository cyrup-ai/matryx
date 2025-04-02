use crate::modal::{Action, ActionResult, Key, KeySequence, Mode};
use std::collections::HashMap;

/// Represents a keybinding that maps a key or key sequence to an action
#[derive(Debug, Clone)]
pub struct Keybinding {
    /// The key that triggers the action
    pub key: Key,
    /// The modes in which this keybinding is active
    pub modes: Vec<Mode>,
    /// The name of the action to execute
    pub action_name: String,
    /// Description of the keybinding
    pub description: Option<String>,
}

impl Keybinding {
    /// Create a new keybinding
    pub fn new<S: Into<String>>(key: Key, modes: Vec<Mode>, action_name: S) -> Self {
        Self {
            key,
            modes,
            action_name: action_name.into(),
            description: None,
        }
    }

    /// Create a new keybinding with a description
    pub fn with_description<S1: Into<String>, S2: Into<String>>(
        key: Key,
        modes: Vec<Mode>,
        action_name: S1,
        description: S2,
    ) -> Self {
        Self {
            key,
            modes,
            action_name: action_name.into(),
            description: Some(description.into()),
        }
    }

    /// Check if the keybinding is active in the given mode
    pub fn is_active_in_mode(&self, mode: Mode) -> bool {
        self.modes.contains(&mode)
    }
}

/// Represents a keybinding that maps a key sequence to an action
#[derive(Debug, Clone)]
pub struct SequenceBinding {
    /// The key sequence that triggers the action
    pub sequence: KeySequence,
    /// The modes in which this keybinding is active
    pub modes: Vec<Mode>,
    /// The name of the action to execute
    pub action_name: String,
    /// Description of the keybinding
    pub description: Option<String>,
}

impl SequenceBinding {
    /// Create a new sequence binding
    pub fn new<S: Into<String>>(sequence: KeySequence, modes: Vec<Mode>, action_name: S) -> Self {
        Self {
            sequence,
            modes,
            action_name: action_name.into(),
            description: None,
        }
    }

    /// Create a new sequence binding with a description
    pub fn with_description<S1: Into<String>, S2: Into<String>>(
        sequence: KeySequence,
        modes: Vec<Mode>,
        action_name: S1,
        description: S2,
    ) -> Self {
        Self {
            sequence,
            modes,
            action_name: action_name.into(),
            description: Some(description.into()),
        }
    }

    /// Create a new sequence binding from a string
    pub fn from_str<S1: Into<String>>(
        sequence_str: &str,
        modes: Vec<Mode>,
        action_name: S1,
    ) -> Self {
        Self::new(KeySequence::from_str(sequence_str), modes, action_name)
    }

    /// Check if the sequence binding is active in the given mode
    pub fn is_active_in_mode(&self, mode: Mode) -> bool {
        self.modes.contains(&mode)
    }
}

/// Manages keybindings and triggers actions based on key input
pub struct KeybindingManager {
    /// Map of keys to keybindings
    bindings: HashMap<Key, Vec<Keybinding>>,
    /// Map of key sequences to sequence bindings
    sequence_bindings: Vec<SequenceBinding>,
}

impl Default for KeybindingManager {
    fn default() -> Self {
        Self {
            bindings: HashMap::new(),
            sequence_bindings: Vec::new(),
        }
    }
}

impl KeybindingManager {
    /// Create a new keybinding manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a keybinding
    pub fn add_binding(&mut self, binding: Keybinding) {
        self.bindings.entry(binding.key).or_insert_with(Vec::new).push(binding);
    }

    /// Add multiple keybindings
    pub fn add_bindings(&mut self, bindings: Vec<Keybinding>) {
        for binding in bindings {
            self.add_binding(binding);
        }
    }

    /// Add a sequence binding
    pub fn add_sequence_binding(&mut self, binding: SequenceBinding) {
        self.sequence_bindings.push(binding);
    }

    /// Add multiple sequence bindings
    pub fn add_sequence_bindings(&mut self, bindings: Vec<SequenceBinding>) {
        for binding in bindings {
            self.add_sequence_binding(binding);
        }
    }

    /// Get keybindings for a specific key
    pub fn get_bindings_for_key(&self, key: &Key) -> Vec<&Keybinding> {
        self.bindings
            .get(key)
            .map(|bindings| bindings.iter().collect())
            .unwrap_or_default()
    }

    /// Get keybindings active in a specific mode
    pub fn get_bindings_for_mode(&self, mode: Mode) -> Vec<&Keybinding> {
        self.bindings
            .values()
            .flatten()
            .filter(|binding| binding.is_active_in_mode(mode))
            .collect()
    }

    /// Get all sequence bindings
    pub fn get_sequence_bindings(&self) -> &[SequenceBinding] {
        &self.sequence_bindings
    }

    /// Get sequence bindings active in a specific mode
    pub fn get_sequence_bindings_for_mode(&self, mode: Mode) -> Vec<&SequenceBinding> {
        self.sequence_bindings
            .iter()
            .filter(|binding| binding.is_active_in_mode(mode))
            .collect()
    }

    /// Get all sequence patterns available in current mode
    pub fn get_available_sequences(&self, mode: Mode) -> Vec<KeySequence> {
        self.sequence_bindings
            .iter()
            .filter(|binding| binding.is_active_in_mode(mode))
            .map(|binding| binding.sequence.clone())
            .collect()
    }

    /// Process a key input in the given mode, returning the action name if a binding is found
    pub fn process_key(&self, key: &Key, mode: Mode) -> Option<&str> {
        self.bindings.get(key).and_then(|bindings| {
            bindings
                .iter()
                .find(|binding| binding.is_active_in_mode(mode))
                .map(|binding| binding.action_name.as_str())
        })
    }

    /// Process a key sequence in the given mode, returning the action name if a binding is found
    pub fn process_key_sequence(&self, sequence: &KeySequence, mode: Mode) -> Option<&str> {
        self.sequence_bindings
            .iter()
            .find(|binding| binding.is_active_in_mode(mode) && &binding.sequence == sequence)
            .map(|binding| binding.action_name.as_str())
    }

    /// Check if a sequence is a prefix of any registered sequence binding
    pub fn is_sequence_prefix(&self, sequence: &KeySequence, mode: Mode) -> bool {
        self.sequence_bindings
            .iter()
            .filter(|binding| binding.is_active_in_mode(mode))
            .any(|binding| sequence.is_prefix_of(&binding.sequence))
    }

    /// Find sequence bindings that have the given sequence as prefix
    pub fn find_matching_sequences(
        &self,
        sequence: &KeySequence,
        mode: Mode,
    ) -> Vec<&SequenceBinding> {
        self.sequence_bindings
            .iter()
            .filter(|binding| {
                binding.is_active_in_mode(mode) &&
                    (sequence.is_prefix_of(&binding.sequence) || &binding.sequence == sequence)
            })
            .collect()
    }

    /// Process a key and execute the corresponding action if found
    pub fn process_key_and_execute<T: Action>(
        &self,
        key: &Key,
        mode: Mode,
        action_handler: &T,
    ) -> Option<ActionResult> {
        self.process_key(key, mode).map(|_| action_handler.execute())
    }

    /// Process a key sequence and execute the corresponding action if found
    pub fn process_sequence_and_execute<T: Action>(
        &self,
        sequence: &KeySequence,
        mode: Mode,
        action_handler: &T,
    ) -> Option<ActionResult> {
        self.process_key_sequence(sequence, mode).map(|_| action_handler.execute())
    }

    /// Remove a keybinding
    pub fn remove_binding(&mut self, key: &Key, action_name: &str) {
        if let Some(bindings) = self.bindings.get_mut(key) {
            bindings.retain(|binding| binding.action_name != action_name);
            if bindings.is_empty() {
                self.bindings.remove(key);
            }
        }
    }

    /// Remove a sequence binding
    pub fn remove_sequence_binding(&mut self, sequence: &KeySequence, action_name: &str) {
        self.sequence_bindings.retain(|binding| {
            !(binding.sequence == *sequence && binding.action_name == action_name)
        });
    }

    /// Clear all keybindings
    pub fn clear_bindings(&mut self) {
        self.bindings.clear();
        self.sequence_bindings.clear();
    }
}
