#[cfg(test)]
mod action_tests {
    use super::*;
    use crate::modal::{
        Action,
        ActionContextMap,
        ActionDispatcher,
        ActionError,
        ActionResult,
        GlobalAction,
        GlobalContext,
        TextAction,
        TextContext,
        WindowAction,
        WindowContext,
    };

    // Test action implementations
    #[derive(Debug)]
    struct TestGlobalAction {
        executed: bool,
    }

    impl TestGlobalAction {
        fn new() -> Self {
            Self { executed: false }
        }
    }

    impl GlobalAction for TestGlobalAction {
        fn execute_global(&self, _global: &mut GlobalContext) -> ActionResult {
            Ok(())
        }
    }

    impl_action!(TestGlobalAction, "Test global action");

    #[derive(Debug)]
    struct TestWindowAction;

    impl WindowAction for TestWindowAction {
        fn execute_window(
            &self,
            _window: &mut WindowContext,
            _global: &mut GlobalContext,
        ) -> ActionResult {
            Ok(())
        }
    }

    impl_action!(TestWindowAction, "Test window action");

    #[derive(Debug)]
    struct TestTextAction;

    impl TextAction for TestTextAction {
        fn execute_text(
            &self,
            _text: &mut TextContext,
            _window: &mut WindowContext,
            _global: &mut GlobalContext,
        ) -> ActionResult {
            Ok(())
        }
    }

    impl_action!(TestTextAction, "Test text action");

    #[derive(Debug)]
    struct TestUndoAction;

    impl Action for TestUndoAction {
        fn name(&self) -> &str {
            "undo_TestGlobalAction"
        }

        fn description(&self) -> &str {
            "Undo test global action"
        }

        fn execute(&self, _contexts: &mut ActionContextMap) -> ActionResult {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct TestFailAction;

    impl Action for TestFailAction {
        fn name(&self) -> &str {
            "TestFailAction"
        }

        fn description(&self) -> &str {
            "Action that always fails"
        }

        fn execute(&self, _contexts: &mut ActionContextMap) -> ActionResult {
            Err(ActionError::Failed("Test failure".to_string()))
        }
    }

    #[derive(Debug)]
    struct TestConfirmAction;

    impl Action for TestConfirmAction {
        fn name(&self) -> &str {
            "TestConfirmAction"
        }

        fn description(&self) -> &str {
            "Action that needs confirmation"
        }

        fn execute(&self, _contexts: &mut ActionContextMap) -> ActionResult {
            Ok(())
        }

        fn needs_confirmation(&self) -> bool {
            true
        }

        fn confirmation_message(&self) -> Option<String> {
            Some("Confirm this action?".to_string())
        }
    }

    #[test]
    fn test_action_dispatch() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestGlobalAction::new());

        // Execute action
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // Non-existent action
        let result = dispatcher.execute("NonExistentAction");
        assert!(matches!(result, Err(ActionError::Failed(_))));
    }

    #[test]
    fn test_context_actions() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestGlobalAction::new());
        dispatcher.register(TestWindowAction);
        dispatcher.register(TestTextAction);

        // Global action should work with default contexts
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // Window action should fail without window context
        let result = dispatcher.execute("TestWindowAction");
        assert!(matches!(result, Err(ActionError::ContextNotAvailable(_))));

        // Add window context
        dispatcher.add_context(WindowContext::new("test_window"));

        // Now window action should work
        let result = dispatcher.execute("TestWindowAction");
        assert!(result.is_ok());

        // Text action should still fail
        let result = dispatcher.execute("TestTextAction");
        assert!(matches!(result, Err(ActionError::ContextNotAvailable(_))));

        // Add text context
        dispatcher.add_context(TextContext::new("test_buffer"));

        // Now text action should work
        let result = dispatcher.execute("TestTextAction");
        assert!(result.is_ok());
    }

    #[test]
    fn test_hooks_and_validators() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestGlobalAction::new());

        // Add pre-hook that blocks action
        dispatcher.add_pre_hook(|name, _| {
            if name == "TestGlobalAction" {
                Err(ActionError::Failed("Pre-hook blocked action".to_string()))
            } else {
                Ok(())
            }
        });

        // Action should be blocked by pre-hook
        let result = dispatcher.execute("TestGlobalAction");
        assert!(matches!(result, Err(ActionError::Failed(_))));

        // Reset dispatcher
        let mut dispatcher = ActionDispatcher::new();
        dispatcher.register(TestGlobalAction::new());

        // Add validator that blocks action
        dispatcher.add_validator(|name, _| {
            if name == "TestGlobalAction" {
                Err("Validation failed".to_string())
            } else {
                Ok(())
            }
        });

        // Action should be blocked by validator
        let result = dispatcher.execute("TestGlobalAction");
        assert!(matches!(result, Err(ActionError::Failed(_))));

        // Reset dispatcher
        let mut dispatcher = ActionDispatcher::new();
        dispatcher.register(TestGlobalAction::new());

        // Add post-hook
        let mut post_hook_called = false;
        dispatcher.add_post_hook(move |name, _| {
            if name == "TestGlobalAction" {
                post_hook_called = true;
            }
            Ok(())
        });

        // Execute action
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // Post-hook should have been called
        assert!(post_hook_called);
    }

    #[test]
    fn test_confirmation() {
        let mut dispatcher = ActionDispatcher::new();

        // Register action that needs confirmation
        dispatcher.register(TestConfirmAction);

        // Action should be blocked and return NeedsConfirmation
        let result = dispatcher.execute("TestConfirmAction");
        assert!(matches!(result, Err(ActionError::NeedsConfirmation(_))));
    }

    #[test]
    fn test_undo_redo() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestGlobalAction::new());
        dispatcher.register(TestUndoAction);

        // Initially nothing to undo/redo
        assert!(!dispatcher.can_undo());
        assert!(!dispatcher.can_redo());

        // Execute action
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // Now we should be able to undo
        assert!(dispatcher.can_undo());
        assert!(!dispatcher.can_redo());

        // Undo the action
        let result = dispatcher.undo();
        assert!(result.is_ok());

        // Now we should be able to redo
        assert!(!dispatcher.can_undo());
        assert!(dispatcher.can_redo());

        // Redo the action
        let result = dispatcher.redo();
        assert!(result.is_ok());

        // Now we should be able to undo again
        assert!(dispatcher.can_undo());
        assert!(!dispatcher.can_redo());

        // Clear history
        dispatcher.clear_history();

        // Now nothing to undo/redo
        assert!(!dispatcher.can_undo());
        assert!(!dispatcher.can_redo());
    }

    #[test]
    fn test_action_history() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestGlobalAction::new());

        // Execute action
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // History should have one entry
        assert_eq!(dispatcher.history().len(), 1);
        assert_eq!(dispatcher.history()[0].name, "TestGlobalAction");

        // Set small history size
        dispatcher.set_max_history_size(2);

        // Execute action twice more
        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        let result = dispatcher.execute("TestGlobalAction");
        assert!(result.is_ok());

        // History should have only 2 entries
        assert_eq!(dispatcher.history().len(), 2);
    }

    #[test]
    fn test_failed_action() {
        let mut dispatcher = ActionDispatcher::new();

        // Register actions
        dispatcher.register(TestFailAction);

        // Execute action that fails
        let result = dispatcher.execute("TestFailAction");
        assert!(matches!(result, Err(ActionError::Failed(_))));

        // History should be empty since action failed
        assert_eq!(dispatcher.history().len(), 0);
    }
}

#[cfg(test)]
mod sequence_tests {
    use crate::modal::{
        InputHandler,
        Key,
        KeySequence,
        KeybindingManager,
        Mode,
        SequenceBinding,
        SequenceStatus,
    };
    use crossterm::event::KeyCode;
    use std::time::Duration;

    #[test]
    fn test_key_sequence_creation() {
        // Test from vec of keys
        let keys = vec![
            Key::simple(KeyCode::Char('g')),
            Key::simple(KeyCode::Char('g')),
        ];
        let sequence = KeySequence::new(keys);
        assert_eq!(sequence.len(), 2);
        assert_eq!(sequence.keys[0].code, KeyCode::Char('g'));
        assert_eq!(sequence.keys[1].code, KeyCode::Char('g'));

        // Test from str
        let sequence = KeySequence::from_str("gg");
        assert_eq!(sequence.len(), 2);
        assert_eq!(sequence.keys[0].code, KeyCode::Char('g'));
        assert_eq!(sequence.keys[1].code, KeyCode::Char('g'));

        // Test from codes
        let sequence = KeySequence::from_codes(vec![KeyCode::Char('d'), KeyCode::Char('d')]);
        assert_eq!(sequence.len(), 2);
        assert_eq!(sequence.keys[0].code, KeyCode::Char('d'));
        assert_eq!(sequence.keys[1].code, KeyCode::Char('d'));
    }

    #[test]
    fn test_key_sequence_comparison() {
        let seq_gg = KeySequence::from_str("gg");
        let seq_g = KeySequence::from_str("g");
        let seq_dd = KeySequence::from_str("dd");

        // Test prefix
        assert!(seq_g.is_prefix_of(&seq_gg));
        assert!(!seq_gg.is_prefix_of(&seq_g));
        assert!(!seq_dd.is_prefix_of(&seq_gg));

        // Test starts_with
        assert!(seq_gg.starts_with(&seq_g));
        assert!(!seq_g.starts_with(&seq_gg));
        assert!(!seq_gg.starts_with(&seq_dd));
    }

    #[test]
    fn test_input_handler_sequence_processing() {
        let mut handler = InputHandler::with_timeout(Duration::from_millis(100));

        // Process first key of sequence
        let status = handler.process_key(Key::simple(KeyCode::Char('g')));
        match status {
            SequenceStatus::Partial(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq.keys[0].code, KeyCode::Char('g'));
            },
            _ => panic!("Expected SequenceStatus::Partial"),
        }

        // Process second key to complete sequence
        let status = handler.process_key(Key::simple(KeyCode::Char('g')));
        match status {
            SequenceStatus::Partial(seq) => {
                assert_eq!(seq.len(), 2);
                assert_eq!(seq.keys[0].code, KeyCode::Char('g'));
                assert_eq!(seq.keys[1].code, KeyCode::Char('g'));
            },
            _ => panic!("Expected SequenceStatus::Partial"),
        }

        // Complete the sequence
        let status = handler.complete_sequence();
        match status {
            SequenceStatus::Complete(seq) => {
                assert_eq!(seq.len(), 2);
                assert_eq!(seq.keys[0].code, KeyCode::Char('g'));
                assert_eq!(seq.keys[1].code, KeyCode::Char('g'));
            },
            _ => panic!("Expected SequenceStatus::Complete"),
        }

        // Verify sequence is reset
        assert!(!handler.is_sequence_in_progress());
    }

    #[test]
    fn test_sequence_abort() {
        let mut handler = InputHandler::new();

        // Start a sequence
        handler.process_key(Key::simple(KeyCode::Char('d')));
        assert!(handler.is_sequence_in_progress());

        // Abort with Escape
        let status = handler.process_key(Key::simple(KeyCode::Esc));
        match status {
            SequenceStatus::Aborted(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq.keys[0].code, KeyCode::Char('d'));
            },
            _ => panic!("Expected SequenceStatus::Aborted"),
        }

        // Verify sequence is reset
        assert!(!handler.is_sequence_in_progress());
    }

    #[test]
    fn test_keybinding_manager_with_sequences() {
        let mut manager = KeybindingManager::new();

        // Add sequence bindings
        let binding_gg = SequenceBinding::from_str("gg", vec![Mode::Normal], "goto_start");
        let binding_dd = SequenceBinding::from_str("dd", vec![Mode::Normal], "delete_line");

        manager.add_sequence_binding(binding_gg);
        manager.add_sequence_binding(binding_dd);

        // Test sequence resolution
        let seq_gg = KeySequence::from_str("gg");
        let seq_dd = KeySequence::from_str("dd");
        let seq_g = KeySequence::from_str("g");

        // Test exact matches
        assert_eq!(manager.process_key_sequence(&seq_gg, Mode::Normal), Some("goto_start"));
        assert_eq!(manager.process_key_sequence(&seq_dd, Mode::Normal), Some("delete_line"));

        // Test partial match
        assert_eq!(manager.process_key_sequence(&seq_g, Mode::Normal), None);

        // Test prefix checking
        assert!(manager.is_sequence_prefix(&seq_g, Mode::Normal));

        // Test finding sequences that match prefix
        let matches = manager.find_matching_sequences(&seq_g, Mode::Normal);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].action_name, "goto_start");
    }

    #[test]
    fn test_ambiguous_sequences() {
        let mut manager = KeybindingManager::new();

        // Add potentially ambiguous sequences
        manager.add_sequence_binding(SequenceBinding::from_str(
            "gg",
            vec![Mode::Normal],
            "goto_start",
        ));
        manager.add_sequence_binding(SequenceBinding::from_str(
            "ga",
            vec![Mode::Normal],
            "goto_after",
        ));

        // Test with InputHandler for ambiguous resolution
        let mut handler = InputHandler::new();

        // Process first 'g'
        handler.process_key(Key::simple(KeyCode::Char('g')));

        // At this point, we have a partial match that could be either 'gg' or 'ga'
        let current = handler.current_sequence();
        let available_sequences = manager.get_available_sequences(Mode::Normal);

        // Check if current sequence is a prefix of any registered sequence
        let is_prefix = manager.is_sequence_prefix(&current, Mode::Normal);
        assert!(is_prefix);

        // Find all possible completions
        let matches = manager.find_matching_sequences(&current, Mode::Normal);
        assert_eq!(matches.len(), 2); // Should match both 'gg' and 'ga'
    }
}

#[cfg(test)]
mod key_parsing_tests {
    use crate::modal::{Key, Keybinding, KeybindingManager, Mode};
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn test_key_creation() {
        // Test simple key creation
        let key = Key::simple(KeyCode::Char('a'));
        assert_eq!(key.code, KeyCode::Char('a'));
        assert_eq!(key.modifiers, KeyModifiers::empty());

        // Test key with modifiers
        let key = Key::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key.code, KeyCode::Char('c'));
        assert_eq!(key.modifiers, KeyModifiers::CONTROL);

        // Test key with multiple modifiers
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        let key = Key::new(KeyCode::Char('d'), modifiers);
        assert_eq!(key.code, KeyCode::Char('d'));
        assert_eq!(key.modifiers, modifiers);
    }

    #[test]
    fn test_key_display() {
        // Test simple key display
        let key = Key::simple(KeyCode::Char('a'));
        assert_eq!(key.to_string(), "a");

        // Test function key display
        let key = Key::simple(KeyCode::F(5));
        assert_eq!(key.to_string(), "F5");

        // Test special key display
        let key = Key::simple(KeyCode::Enter);
        assert_eq!(key.to_string(), "CR");

        // Test key with control modifier
        let key = Key::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key.to_string(), "C-c");

        // Test key with multiple modifiers
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::ALT;
        let key = Key::new(KeyCode::Char('d'), modifiers);
        assert_eq!(key.to_string(), "C-A-d");
    }

    #[test]
    fn test_key_modifier_checks() {
        // Test has_ctrl
        let key = Key::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(key.has_ctrl());
        assert!(!key.has_shift());
        assert!(!key.has_alt());

        // Test has_shift
        let key = Key::new(KeyCode::Char('s'), KeyModifiers::SHIFT);
        assert!(!key.has_ctrl());
        assert!(key.has_shift());
        assert!(!key.has_alt());

        // Test has_alt
        let key = Key::new(KeyCode::Char('a'), KeyModifiers::ALT);
        assert!(!key.has_ctrl());
        assert!(!key.has_shift());
        assert!(key.has_alt());

        // Test multiple modifiers
        let modifiers = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        let key = Key::new(KeyCode::Char('d'), modifiers);
        assert!(key.has_ctrl());
        assert!(key.has_shift());
        assert!(!key.has_alt());
    }
}

#[cfg(test)]
mod binding_resolution_tests {
    use crate::modal::{Key, Keybinding, KeybindingManager, Mode};
    use crossterm::event::KeyCode;

    #[test]
    fn test_keybinding_resolution_by_mode() {
        let mut manager = KeybindingManager::new();

        // Create bindings for different modes
        let normal_binding = Keybinding::new(
            Key::simple(KeyCode::Char('h')),
            vec![Mode::Normal],
            "cursor_left_normal",
        );

        let visual_binding = Keybinding::new(
            Key::simple(KeyCode::Char('h')),
            vec![Mode::Visual],
            "cursor_left_visual",
        );

        let insert_binding = Keybinding::new(
            Key::simple(KeyCode::Char('h')),
            vec![Mode::Insert],
            "cursor_left_insert",
        );

        // Add bindings to manager
        manager.add_binding(normal_binding);
        manager.add_binding(visual_binding);
        manager.add_binding(insert_binding);

        // Test resolution by mode
        let key = Key::simple(KeyCode::Char('h'));
        assert_eq!(manager.process_key(&key, Mode::Normal), Some("cursor_left_normal"));
        assert_eq!(manager.process_key(&key, Mode::Visual), Some("cursor_left_visual"));
        assert_eq!(manager.process_key(&key, Mode::Insert), Some("cursor_left_insert"));
    }

    #[test]
    fn test_multi_mode_binding() {
        let mut manager = KeybindingManager::new();

        // Create a binding that works in multiple modes
        let multi_mode_binding = Keybinding::new(
            Key::simple(KeyCode::Esc),
            vec![Mode::Insert, Mode::Visual],
            "enter_normal_mode",
        );

        // Create a normal mode specific binding
        let normal_binding =
            Keybinding::new(Key::simple(KeyCode::Esc), vec![Mode::Normal], "cancel_operation");

        // Add bindings to manager
        manager.add_binding(multi_mode_binding);
        manager.add_binding(normal_binding);

        // Test resolution across modes
        let key = Key::simple(KeyCode::Esc);
        assert_eq!(manager.process_key(&key, Mode::Insert), Some("enter_normal_mode"));
        assert_eq!(manager.process_key(&key, Mode::Visual), Some("enter_normal_mode"));
        assert_eq!(manager.process_key(&key, Mode::Normal), Some("cancel_operation"));
    }

    #[test]
    fn test_multiple_bindings_same_key() {
        let mut manager = KeybindingManager::new();

        // Create multiple bindings for the same key in the same mode
        let binding1 =
            Keybinding::new(Key::simple(KeyCode::Char('x')), vec![Mode::Normal], "delete_char");

        let binding2 =
            Keybinding::new(Key::simple(KeyCode::Char('x')), vec![Mode::Normal], "cut_selection");

        // Add bindings to manager (binding2 should override binding1)
        manager.add_binding(binding1);
        manager.add_binding(binding2);

        // Get all bindings for the key
        let key = Key::simple(KeyCode::Char('x'));
        let bindings = manager.get_bindings_for_key(&key);

        // Should have two bindings
        assert_eq!(bindings.len(), 2);

        // First match should be returned when processing
        assert_eq!(manager.process_key(&key, Mode::Normal), Some("delete_char"));
    }

    #[test]
    fn test_no_binding_match() {
        let mut manager = KeybindingManager::new();

        // Add a binding
        let binding =
            Keybinding::new(Key::simple(KeyCode::Char('h')), vec![Mode::Normal], "cursor_left");

        manager.add_binding(binding);

        // Test key with no binding
        let key = Key::simple(KeyCode::Char('z'));
        assert_eq!(manager.process_key(&key, Mode::Normal), None);

        // Test key in mode with no binding
        let key = Key::simple(KeyCode::Char('h'));
        assert_eq!(manager.process_key(&key, Mode::Insert), None);
    }

    #[test]
    fn test_remove_binding() {
        let mut manager = KeybindingManager::new();

        // Add bindings
        let binding1 =
            Keybinding::new(Key::simple(KeyCode::Char('h')), vec![Mode::Normal], "cursor_left");

        let binding2 =
            Keybinding::new(Key::simple(KeyCode::Char('h')), vec![Mode::Visual], "selection_left");

        manager.add_binding(binding1);
        manager.add_binding(binding2);

        // Verify bindings exist
        let key = Key::simple(KeyCode::Char('h'));
        assert_eq!(manager.process_key(&key, Mode::Normal), Some("cursor_left"));
        assert_eq!(manager.process_key(&key, Mode::Visual), Some("selection_left"));

        // Remove binding for Normal mode
        manager.remove_binding(&key, "cursor_left");

        // Verify only Visual mode binding remains
        assert_eq!(manager.process_key(&key, Mode::Normal), None);
        assert_eq!(manager.process_key(&key, Mode::Visual), Some("selection_left"));

        // Remove binding for Visual mode
        manager.remove_binding(&key, "selection_left");

        // Verify no bindings remain
        assert_eq!(manager.process_key(&key, Mode::Normal), None);
        assert_eq!(manager.process_key(&key, Mode::Visual), None);
    }
}

#[cfg(test)]
mod sequence_timeout_tests {
    use crate::modal::{InputHandler, Key, KeySequence, SequenceStatus};
    use crossterm::event::KeyCode;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timeout_handling() {
        // Create handler with very short timeout for testing
        let mut handler = InputHandler::with_timeout(Duration::from_millis(10));

        // Start a sequence
        handler.process_key(Key::simple(KeyCode::Char('d')));

        // Sleep to ensure timeout
        thread::sleep(Duration::from_millis(20));

        // Verify timeout detection
        assert!(handler.has_sequence_timed_out());

        // Add another key should trigger timeout status
        let status = handler.process_key(Key::simple(KeyCode::Char('d')));
        match status {
            SequenceStatus::Timeout(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq.keys[0].code, KeyCode::Char('d'));
            },
            _ => panic!("Expected SequenceStatus::Timeout"),
        }

        // Verify sequence is reset after timeout
        assert!(!handler.is_sequence_in_progress());

        // New key should start a new sequence
        let status = handler.process_key(Key::simple(KeyCode::Char('y')));
        match status {
            SequenceStatus::Partial(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq.keys[0].code, KeyCode::Char('y'));
            },
            _ => panic!("Expected SequenceStatus::Partial"),
        }
    }

    #[test]
    fn test_timeout_configuration() {
        // Test default timeout
        let handler = InputHandler::new();
        assert_eq!(handler.sequence_timeout(), Duration::from_millis(500));

        // Test custom timeout
        let timeout = Duration::from_millis(250);
        let handler = InputHandler::with_timeout(timeout);
        assert_eq!(handler.sequence_timeout(), timeout);

        // Test changing timeout
        let mut handler = InputHandler::new();
        let new_timeout = Duration::from_millis(100);
        handler.set_sequence_timeout(new_timeout);
        assert_eq!(handler.sequence_timeout(), new_timeout);
    }

    #[test]
    fn test_sequence_reset() {
        let mut handler = InputHandler::new();

        // Start a sequence
        handler.process_key(Key::simple(KeyCode::Char('g')));
        assert!(handler.is_sequence_in_progress());

        // Reset the sequence
        handler.reset_sequence();
        assert!(!handler.is_sequence_in_progress());

        // Starting a new sequence should work normally
        let status = handler.process_key(Key::simple(KeyCode::Char('d')));
        match status {
            SequenceStatus::Partial(seq) => {
                assert_eq!(seq.len(), 1);
                assert_eq!(seq.keys[0].code, KeyCode::Char('d'));
            },
            _ => panic!("Expected SequenceStatus::Partial"),
        }
    }
}
