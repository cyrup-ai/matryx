use std::any::Any;
use std::collections::HashMap;
use std::fmt;

/// Result type for actions
pub type ActionResult = Result<(), ActionError>;

/// Error type for actions
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    /// Action not applicable in current context
    NotApplicable,
    /// Action failed to execute
    Failed(String),
    /// Action requires confirmation
    NeedsConfirmation(String),
    /// Context not available
    ContextNotAvailable(String),
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActionError::NotApplicable => write!(f, "Action not applicable in current context"),
            ActionError::Failed(msg) => write!(f, "Action failed: {}", msg),
            ActionError::NeedsConfirmation(msg) => write!(f, "Confirmation needed: {}", msg),
            ActionError::ContextNotAvailable(ctx) => {
                write!(f, "Required context not available: {}", ctx)
            },
        }
    }
}

impl std::error::Error for ActionError {}

/// Trait for providing context to actions
pub trait ActionContext: Any + Send + Sync {
    /// Get the context type ID
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }

    /// Convert to Any for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Convert to mutable Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get the context name
    fn name(&self) -> &str;
}

/// Collection of contexts for action execution
pub struct ActionContextMap {
    contexts: HashMap<std::any::TypeId, Box<dyn ActionContext>>,
}

impl Default for ActionContextMap {
    fn default() -> Self {
        Self { contexts: HashMap::new() }
    }
}

impl ActionContextMap {
    /// Create a new context map
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a context
    pub fn insert<T: ActionContext + 'static>(&mut self, context: T) {
        self.contexts.insert(std::any::TypeId::of::<T>(), Box::new(context));
    }

    /// Get a context by type
    pub fn get<T: ActionContext + 'static>(&self) -> Option<&T> {
        self.contexts
            .get(&std::any::TypeId::of::<T>())
            .and_then(|ctx| ctx.as_any().downcast_ref::<T>())
    }

    /// Get a mutable context by type
    pub fn get_mut<T: ActionContext + 'static>(&mut self) -> Option<&mut T> {
        self.contexts
            .get_mut(&std::any::TypeId::of::<T>())
            .and_then(|ctx| ctx.as_any_mut().downcast_mut::<T>())
    }

    /// Check if a context type is available
    pub fn has<T: ActionContext + 'static>(&self) -> bool {
        self.contexts.contains_key(&std::any::TypeId::of::<T>())
    }

    /// Remove a context by type
    pub fn remove<T: ActionContext + 'static>(&mut self) -> Option<Box<dyn ActionContext>> {
        self.contexts.remove(&std::any::TypeId::of::<T>())
    }

    /// Clear all contexts
    pub fn clear(&mut self) {
        self.contexts.clear();
    }
}

/// Global context for application-wide state
#[derive(Debug)]
pub struct GlobalContext {
    name: String,
}

impl GlobalContext {
    /// Create a new global context
    pub fn new() -> Self {
        Self { name: "Global".to_string() }
    }
}

impl ActionContext for GlobalContext {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Window context for window-specific state
#[derive(Debug)]
pub struct WindowContext {
    name: String,
    window_id: String,
}

impl WindowContext {
    /// Create a new window context
    pub fn new<S: Into<String>>(window_id: S) -> Self {
        let id = window_id.into();
        Self { name: format!("Window:{}", id), window_id: id }
    }

    /// Get the window ID
    pub fn window_id(&self) -> &str {
        &self.window_id
    }
}

impl ActionContext for WindowContext {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Text context for text editing state
#[derive(Debug)]
pub struct TextContext {
    name: String,
    buffer_id: String,
}

impl TextContext {
    /// Create a new text context
    pub fn new<S: Into<String>>(buffer_id: S) -> Self {
        let id = buffer_id.into();
        Self { name: format!("Text:{}", id), buffer_id: id }
    }

    /// Get the buffer ID
    pub fn buffer_id(&self) -> &str {
        &self.buffer_id
    }
}

impl ActionContext for TextContext {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Trait for actions that can be performed in the application
pub trait Action: std::fmt::Debug {
    /// Returns the name of the action
    fn name(&self) -> &str;

    /// Returns a description of the action
    fn description(&self) -> &str;

    /// Execute the action with contexts
    fn execute(&self, contexts: &mut ActionContextMap) -> ActionResult;

    /// Returns true if the action is applicable in the current context
    fn is_applicable(&self, contexts: &ActionContextMap) -> bool {
        true
    }

    /// Returns true if the action requires confirmation before execution
    fn needs_confirmation(&self) -> bool {
        false
    }

    /// Returns a confirmation message if the action requires confirmation
    fn confirmation_message(&self) -> Option<String> {
        None
    }

    /// Returns a list of required context types
    fn required_contexts(&self) -> Vec<&'static str> {
        Vec::new()
    }
}

/// Macro to easily implement common action traits
#[macro_export]
macro_rules! impl_action {
    ($name:ident, $description:expr) => {
        fn name(&self) -> &str {
            stringify!($name)
        }

        fn description(&self) -> &str {
            $description
        }
    };
}

/// Action that can be executed in a global context
pub trait GlobalAction: Action {
    /// Execute the action with just the global context
    fn execute_global(&self, global: &mut GlobalContext) -> ActionResult;
}

impl<T: GlobalAction> Action for T {
    fn execute(&self, contexts: &mut ActionContextMap) -> ActionResult {
        if let Some(global) = contexts.get_mut::<GlobalContext>() {
            self.execute_global(global)
        } else {
            Err(ActionError::ContextNotAvailable("Global".to_string()))
        }
    }

    fn is_applicable(&self, contexts: &ActionContextMap) -> bool {
        contexts.has::<GlobalContext>()
    }

    fn required_contexts(&self) -> Vec<&'static str> {
        vec!["Global"]
    }
}

/// Action that can be executed in a window context
pub trait WindowAction: Action {
    /// Execute the action with window context
    fn execute_window(
        &self,
        window: &mut WindowContext,
        global: &mut GlobalContext,
    ) -> ActionResult;
}

impl<T: WindowAction> Action for T {
    fn execute(&self, contexts: &mut ActionContextMap) -> ActionResult {
        let global = contexts
            .get_mut::<GlobalContext>()
            .ok_or_else(|| ActionError::ContextNotAvailable("Global".to_string()))?;

        let window = contexts
            .get_mut::<WindowContext>()
            .ok_or_else(|| ActionError::ContextNotAvailable("Window".to_string()))?;

        self.execute_window(window, global)
    }

    fn is_applicable(&self, contexts: &ActionContextMap) -> bool {
        contexts.has::<GlobalContext>() && contexts.has::<WindowContext>()
    }

    fn required_contexts(&self) -> Vec<&'static str> {
        vec!["Global", "Window"]
    }
}

/// Action that can be executed in a text editing context
pub trait TextAction: Action {
    /// Execute the action with text context
    fn execute_text(
        &self,
        text: &mut TextContext,
        window: &mut WindowContext,
        global: &mut GlobalContext,
    ) -> ActionResult;
}

impl<T: TextAction> Action for T {
    fn execute(&self, contexts: &mut ActionContextMap) -> ActionResult {
        let global = contexts
            .get_mut::<GlobalContext>()
            .ok_or_else(|| ActionError::ContextNotAvailable("Global".to_string()))?;

        let window = contexts
            .get_mut::<WindowContext>()
            .ok_or_else(|| ActionError::ContextNotAvailable("Window".to_string()))?;

        let text = contexts
            .get_mut::<TextContext>()
            .ok_or_else(|| ActionError::ContextNotAvailable("Text".to_string()))?;

        self.execute_text(text, window, global)
    }

    fn is_applicable(&self, contexts: &ActionContextMap) -> bool {
        contexts.has::<GlobalContext>() &&
            contexts.has::<WindowContext>() &&
            contexts.has::<TextContext>()
    }

    fn required_contexts(&self) -> Vec<&'static str> {
        vec!["Global", "Window", "Text"]
    }
}

/// Action execution hook
pub type ActionHook = Box<dyn Fn(&str, &ActionContextMap) -> ActionResult + Send + Sync>;

/// Action history entry
#[derive(Debug, Clone)]
pub struct ActionHistoryEntry {
    /// Name of the action
    pub name: String,
    /// Timestamp when the action was executed
    pub timestamp: std::time::SystemTime,
    /// Whether the action can be undone
    pub can_undo: bool,
    /// Whether the action is a redo operation
    pub is_redo: bool,
}

/// Action validator
pub type ActionValidator = Box<dyn Fn(&str, &ActionContextMap) -> Result<(), String> + Send + Sync>;

/// Generic action dispatcher
pub struct ActionDispatcher {
    /// List of registered actions
    actions: Vec<Box<dyn Action>>,
    /// Context map for action execution
    contexts: ActionContextMap,
    /// Pre-execution hooks
    pre_hooks: Vec<ActionHook>,
    /// Post-execution hooks
    post_hooks: Vec<ActionHook>,
    /// Action history for undo/redo
    history: Vec<ActionHistoryEntry>,
    /// Current position in history for undo/redo
    history_position: usize,
    /// Maximum history size
    max_history_size: usize,
    /// Action validators
    validators: Vec<ActionValidator>,
}

impl Default for ActionDispatcher {
    fn default() -> Self {
        let mut contexts = ActionContextMap::new();
        contexts.insert(GlobalContext::new());

        Self {
            actions: Vec::new(),
            contexts,
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
            history: Vec::new(),
            history_position: 0,
            max_history_size: 100,
            validators: Vec::new(),
        }
    }
}

impl ActionDispatcher {
    /// Create a new action dispatcher
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an action
    pub fn register<A: Action + 'static>(&mut self, action: A) {
        self.actions.push(Box::new(action));
    }

    /// Add a context
    pub fn add_context<T: ActionContext + 'static>(&mut self, context: T) {
        self.contexts.insert(context);
    }

    /// Get a context by type
    pub fn context<T: ActionContext + 'static>(&self) -> Option<&T> {
        self.contexts.get::<T>()
    }

    /// Get a mutable context by type
    pub fn context_mut<T: ActionContext + 'static>(&mut self) -> Option<&mut T> {
        self.contexts.get_mut::<T>()
    }

    /// Add a pre-execution hook
    pub fn add_pre_hook<F>(&mut self, hook: F)
    where
        F: Fn(&str, &ActionContextMap) -> ActionResult + Send + Sync + 'static,
    {
        self.pre_hooks.push(Box::new(hook));
    }

    /// Add a post-execution hook
    pub fn add_post_hook<F>(&mut self, hook: F)
    where
        F: Fn(&str, &ActionContextMap) -> ActionResult + Send + Sync + 'static,
    {
        self.post_hooks.push(Box::new(hook));
    }

    /// Add an action validator
    pub fn add_validator<F>(&mut self, validator: F)
    where
        F: Fn(&str, &ActionContextMap) -> Result<(), String> + Send + Sync + 'static,
    {
        self.validators.push(Box::new(validator));
    }

    /// Set the maximum history size
    pub fn set_max_history_size(&mut self, size: usize) {
        self.max_history_size = size;

        // Trim history if needed
        if self.history.len() > self.max_history_size {
            let diff = self.history.len() - self.max_history_size;
            self.history.drain(0..diff);
            self.history_position = self.history_position.saturating_sub(diff);
        }
    }

    /// Get the action history
    pub fn history(&self) -> &[ActionHistoryEntry] {
        &self.history
    }

    /// Clear the action history
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.history_position = 0;
    }

    /// Execute an action by name
    pub fn execute(&mut self, name: &str) -> ActionResult {
        if let Some(action) = self.find_action(name) {
            if !action.is_applicable(&self.contexts) {
                return Err(ActionError::NotApplicable);
            }

            // Check for required contexts
            for ctx_name in action.required_contexts() {
                if (ctx_name == "Global" && !self.contexts.has::<GlobalContext>()) ||
                    (ctx_name == "Window" && !self.contexts.has::<WindowContext>()) ||
                    (ctx_name == "Text" && !self.contexts.has::<TextContext>())
                {
                    return Err(ActionError::ContextNotAvailable(ctx_name.to_string()));
                }
            }

            // Run validators
            for validator in &self.validators {
                if let Err(reason) = validator(name, &self.contexts) {
                    return Err(ActionError::Failed(reason));
                }
            }

            if action.needs_confirmation() {
                if let Some(msg) = action.confirmation_message() {
                    return Err(ActionError::NeedsConfirmation(msg));
                }
            }

            // Run pre-hooks
            for hook in &self.pre_hooks {
                hook(name, &self.contexts)?;
            }

            // Execute the action
            let result = action.execute(&mut self.contexts);

            // Only run post-hooks and add to history if action was successful
            if result.is_ok() {
                // Run post-hooks
                for hook in &self.post_hooks {
                    hook(name, &self.contexts)?;
                }

                // Add to history
                self.add_to_history(name, false);
            }

            result
        } else {
            Err(ActionError::Failed(format!("Action '{}' not found", name)))
        }
    }

    /// Add an action to the history
    fn add_to_history(&mut self, name: &str, is_redo: bool) {
        // Trim future history if we're not at the end
        if self.history_position < self.history.len() {
            self.history.truncate(self.history_position);
        }

        // Add new entry
        self.history.push(ActionHistoryEntry {
            name: name.to_string(),
            timestamp: std::time::SystemTime::now(),
            can_undo: true, // By default assume actions can be undone
            is_redo,
        });

        // Update position
        self.history_position = self.history.len();

        // Trim if exceeding max size
        if self.history.len() > self.max_history_size {
            self.history.remove(0);
            self.history_position -= 1;
        }
    }

    /// Check if undo is available
    pub fn can_undo(&self) -> bool {
        self.history_position > 0
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.history_position < self.history.len()
    }

    /// Undo the last action
    pub fn undo(&mut self) -> ActionResult {
        if !self.can_undo() {
            return Err(ActionError::Failed("Nothing to undo".to_string()));
        }

        self.history_position -= 1;
        let entry = &self.history[self.history_position];

        if !entry.can_undo {
            return Err(ActionError::Failed(format!("Action '{}' cannot be undone", entry.name)));
        }

        // Find the inverse action
        let inverse_name = format!("undo_{}", entry.name);
        if let Some(action) = self.find_action(&inverse_name) {
            // Execute inverse without adding to history
            action.execute(&mut self.contexts)
        } else {
            Err(ActionError::Failed(format!("No undo action found for '{}'", entry.name)))
        }
    }

    /// Redo a previously undone action
    pub fn redo(&mut self) -> ActionResult {
        if !self.can_redo() {
            return Err(ActionError::Failed("Nothing to redo".to_string()));
        }

        let entry = &self.history[self.history_position];
        let name = entry.name.clone();
        self.history_position += 1;

        // Execute the action
        if let Some(action) = self.find_action(&name) {
            // Run pre-hooks
            for hook in &self.pre_hooks {
                hook(&name, &self.contexts)?;
            }

            // Execute the action
            let result = action.execute(&mut self.contexts);

            // Only run post-hooks if action was successful
            if result.is_ok() {
                // Run post-hooks
                for hook in &self.post_hooks {
                    hook(&name, &self.contexts)?;
                }

                // Mark as redo in history
                self.add_to_history(&name, true);
            }

            result
        } else {
            Err(ActionError::Failed(format!("Action '{}' not found", name)))
        }
    }

    /// Find an action by name
    pub fn find_action(&self, name: &str) -> Option<&Box<dyn Action>> {
        self.actions.iter().find(|action| action.name() == name)
    }

    /// Get all registered actions
    pub fn actions(&self) -> &[Box<dyn Action>] {
        &self.actions
    }

    /// Get applicable actions for the current context
    pub fn applicable_actions(&self) -> Vec<&Box<dyn Action>> {
        self.actions
            .iter()
            .filter(|action| action.is_applicable(&self.contexts))
            .collect()
    }
}
