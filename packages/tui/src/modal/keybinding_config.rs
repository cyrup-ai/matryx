use crossterm::event::{KeyCode, KeyModifiers};
use serde::de::{Error as SerdeError, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use toml;

use super::{Key, KeySequence, Keybinding, KeybindingManager, Mode, SequenceBinding};

/// Deserializer for a mode string
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ModeSet(pub Vec<Mode>);
pub struct ModeSetVisitor;

impl<'de> Visitor<'de> for ModeSetVisitor {
    type Value = ModeSet;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .write_str("a valid mode or mode combination (e.g. \"normal\" or \"normal|visual\")")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: SerdeError,
    {
        let mut modes = vec![];

        for mode in value.split('|') {
            let mode = match mode.to_ascii_lowercase().as_str() {
                "insert" | "i" => Mode::Insert,
                "normal" | "n" => Mode::Normal,
                "visual" | "v" => Mode::Visual,
                _ => return Err(E::custom("Could not parse into a valid mode")),
            };

            modes.push(mode);
        }

        Ok(ModeSet(modes))
    }
}

impl<'de> Deserialize<'de> for ModeSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ModeSetVisitor)
    }
}

/// Deserializer for a key or key combination
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConfigKey(pub Key);
pub struct ConfigKeyVisitor;

impl<'de> Visitor<'de> for ConfigKeyVisitor {
    type Value = ConfigKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid key or key combination (e.g. \"j\" or \"C-x\")")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: SerdeError,
    {
        let (base, modifiers) = if value.contains('-') {
            let parts: Vec<&str> = value.split('-').collect();
            if parts.len() != 2 {
                return Err(E::custom("Invalid key format, expected format like 'C-x'"));
            }

            let mod_part = parts[0];
            let key_part = parts[1];

            let mut modifiers = KeyModifiers::empty();
            for c in mod_part.chars() {
                match c.to_ascii_uppercase() {
                    'C' => modifiers.insert(KeyModifiers::CONTROL),
                    'A' => modifiers.insert(KeyModifiers::ALT),
                    'S' => modifiers.insert(KeyModifiers::SHIFT),
                    _ => return Err(E::custom(format!("Unknown modifier: {}", c))),
                }
            }

            (key_part, modifiers)
        } else {
            (value, KeyModifiers::empty())
        };

        let code = match base {
            "Space" => KeyCode::Char(' '),
            "Enter" | "CR" => KeyCode::Enter,
            "Esc" => KeyCode::Esc,
            "Tab" => KeyCode::Tab,
            "BackTab" => KeyCode::BackTab,
            "Backspace" => KeyCode::Backspace,
            "Delete" | "Del" => KeyCode::Delete,
            "Insert" | "Ins" => KeyCode::Insert,
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "PageUp" | "PgUp" => KeyCode::PageUp,
            "PageDown" | "PgDn" => KeyCode::PageDown,
            s if s.len() == 1 => {
                let c = s.chars().next().unwrap();
                KeyCode::Char(c)
            },
            s if s.starts_with('F') && s.len() > 1 => {
                let n = s[1..]
                    .parse::<u8>()
                    .map_err(|_| E::custom(format!("Invalid function key: {}", s)))?;
                KeyCode::F(n)
            },
            _ => return Err(E::custom(format!("Unknown key: {}", base))),
        };

        Ok(ConfigKey(Key { code, modifiers }))
    }
}

impl<'de> Deserialize<'de> for ConfigKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ConfigKeyVisitor)
    }
}

/// Deserializer for a key sequence string
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConfigKeySequence(pub KeySequence);
pub struct ConfigKeySequenceVisitor;

impl<'de> Visitor<'de> for ConfigKeySequenceVisitor {
    type Value = ConfigKeySequence;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid key sequence (e.g. \"gg\" or \"dd\")")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: SerdeError,
    {
        if value.is_empty() {
            return Err(E::custom("Key sequence cannot be empty"));
        }

        let sequence = KeySequence::from_str(value);
        Ok(ConfigKeySequence(sequence))
    }
}

impl<'de> Deserialize<'de> for ConfigKeySequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ConfigKeySequenceVisitor)
    }
}

type KeyBindingMap = HashMap<ConfigKey, String>;
type KeySequenceMap = HashMap<ConfigKeySequence, String>;
type ModeBindingsMap = HashMap<ModeSet, KeyBindingMap>;
type ModeSequenceMap = HashMap<ModeSet, KeySequenceMap>;

#[derive(Debug, Deserialize)]
pub struct KeybindingConfig {
    #[serde(default)]
    pub bindings: ModeBindingsMap,
    #[serde(default)]
    pub sequences: ModeSequenceMap,
}

impl KeybindingConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        let config: KeybindingConfig = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn apply_to_manager(&self, manager: &mut KeybindingManager) {
        // Apply regular keybindings
        for (mode_set, bindings) in &self.bindings {
            let modes = mode_set.0.clone();

            for (key_config, action) in bindings {
                let key = key_config.0;
                let binding = Keybinding::with_description(
                    key,
                    modes.clone(),
                    action.clone(),
                    format!("Custom binding for {}", action),
                );
                manager.add_binding(binding);
            }
        }

        // Apply sequence keybindings
        for (mode_set, sequences) in &self.sequences {
            let modes = mode_set.0.clone();

            for (seq_config, action) in sequences {
                let sequence = seq_config.0.clone();
                let binding = SequenceBinding::with_description(
                    sequence,
                    modes.clone(),
                    action.clone(),
                    format!("Custom sequence for {}", action),
                );
                manager.add_sequence_binding(binding);
            }
        }
    }
}

/// Function to setup default keybindings for all modes
pub fn setup_default_keybindings(manager: &mut KeybindingManager) {
    // Normal mode single key bindings
    let normal_bindings = vec![
        Keybinding::with_description(
            Key::simple(KeyCode::Char('h')),
            vec![Mode::Normal],
            "cursor_left",
            "Move cursor left",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('j')),
            vec![Mode::Normal],
            "cursor_down",
            "Move cursor down",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('k')),
            vec![Mode::Normal],
            "cursor_up",
            "Move cursor up",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('l')),
            vec![Mode::Normal],
            "cursor_right",
            "Move cursor right",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('i')),
            vec![Mode::Normal],
            "enter_insert_mode",
            "Enter insert mode",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('v')),
            vec![Mode::Normal],
            "enter_visual_mode",
            "Enter visual mode",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('0')),
            vec![Mode::Normal],
            "goto_line_start",
            "Go to start of line",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('$')),
            vec![Mode::Normal],
            "goto_line_end",
            "Go to end of line",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('w')),
            vec![Mode::Normal],
            "word_forward",
            "Move forward one word",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('b')),
            vec![Mode::Normal],
            "word_backward",
            "Move backward one word",
        ),
    ];

    // Insert mode bindings
    let insert_bindings = vec![
        Keybinding::with_description(
            Key::simple(KeyCode::Esc),
            vec![Mode::Insert],
            "enter_normal_mode",
            "Enter normal mode",
        ),
        Keybinding::with_description(
            Key::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            vec![Mode::Insert],
            "enter_normal_mode",
            "Enter normal mode",
        ),
    ];

    // Visual mode bindings
    let visual_bindings = vec![
        Keybinding::with_description(
            Key::simple(KeyCode::Esc),
            vec![Mode::Visual],
            "enter_normal_mode",
            "Enter normal mode",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('h')),
            vec![Mode::Visual],
            "cursor_left",
            "Extend selection left",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('j')),
            vec![Mode::Visual],
            "cursor_down",
            "Extend selection down",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('k')),
            vec![Mode::Visual],
            "cursor_up",
            "Extend selection up",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('l')),
            vec![Mode::Visual],
            "cursor_right",
            "Extend selection right",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('y')),
            vec![Mode::Visual],
            "yank",
            "Yank (copy) selection",
        ),
        Keybinding::with_description(
            Key::simple(KeyCode::Char('d')),
            vec![Mode::Visual],
            "delete",
            "Delete selection",
        ),
    ];

    // Add all bindings to the manager
    manager.add_bindings(normal_bindings);
    manager.add_bindings(insert_bindings);
    manager.add_bindings(visual_bindings);

    // Add default sequence bindings
    let sequence_bindings = vec![
        SequenceBinding::from_str("gg", vec![Mode::Normal], "goto_start"),
        SequenceBinding::from_str("G", vec![Mode::Normal], "goto_end"),
        SequenceBinding::from_str("dd", vec![Mode::Normal], "delete_line"),
        SequenceBinding::from_str("yy", vec![Mode::Normal], "yank_line"),
        SequenceBinding::from_str("p", vec![Mode::Normal], "paste_after"),
        SequenceBinding::from_str("P", vec![Mode::Normal], "paste_before"),
        SequenceBinding::from_str("zz", vec![Mode::Normal], "center_cursor"),
    ];

    manager.add_sequence_bindings(sequence_bindings);
}
