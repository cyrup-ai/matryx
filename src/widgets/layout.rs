use std::fmt;
use serde::{Deserialize, Serialize};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use crate::widgets::tabs::TabsState;

/// Layout constraint type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutConstraint {
    /// Percentage constraint (0-100)
    Percentage(u16),
    /// Fixed size constraint in cells
    Fixed(u16),
    /// Minimum size constraint
    Min(u16),
    /// Maximum size constraint
    Max(u16),
}

impl From<LayoutConstraint> for Constraint {
    fn from(constraint: LayoutConstraint) -> Self {
        match constraint {
            LayoutConstraint::Percentage(value) => Constraint::Percentage(value),
            LayoutConstraint::Fixed(value) => Constraint::Length(value),
            LayoutConstraint::Min(value) => Constraint::Min(value),
            LayoutConstraint::Max(value) => Constraint::Max(value),
        }
    }
}

/// Layout type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutType {
    /// Vertical layout (top to bottom)
    Vertical,
    /// Horizontal layout (left to right)
    Horizontal,
    /// Tabbed layout (stacked with tab bar)
    Tabbed,
}

impl From<LayoutType> for Direction {
    fn from(layout_type: LayoutType) -> Self {
        match layout_type {
            LayoutType::Vertical => Direction::Vertical,
            LayoutType::Horizontal => Direction::Horizontal,
            // Tabbed layout is handled specially during rendering
            LayoutType::Tabbed => Direction::Vertical,
        }
    }
}

/// Node identifier for layout tree
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayoutNodeId(String);

impl LayoutNodeId {
    /// Create a new layout node ID
    pub fn new<S: Into<String>>(id: S) -> Self {
        Self(id.into())
    }

    /// Get the string value of the layout node ID
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayoutNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for LayoutNodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for LayoutNodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Layout node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutNode {
    /// Leaf node containing a window ID
    Leaf { window_id: String },
    /// Parent node with children
    Parent {
        /// Layout type
        layout_type: LayoutType,
        /// Layout constraints for children
        constraints: Vec<LayoutConstraint>,
        /// Child nodes
        children: Vec<LayoutNode>,
    },
}

impl LayoutNode {
    /// Create a new leaf node
    pub fn leaf<S: Into<String>>(window_id: S) -> Self {
        Self::Leaf {
            window_id: window_id.into(),
        }
    }

    /// Create a new parent node
    pub fn parent(layout_type: LayoutType, constraints: Vec<LayoutConstraint>, children: Vec<LayoutNode>) -> Self {
        Self::Parent {
            layout_type,
            constraints,
            children,
        }
    }

    /// Compute the layout for this node
    pub fn compute_layout(&self, area: Rect, tab_indices: &std::collections::HashMap<String, usize>) -> Vec<(String, Rect)> {
        match self {
            Self::Leaf { window_id } => vec![(window_id.clone(), area)],
            Self::Parent { layout_type, constraints, children } => {
                // For tabbed layout, we only show the active tab and the tab bar
                if *layout_type == LayoutType::Tabbed {
                    if children.is_empty() {
                        return Vec::new();
                    }

                    // Generate a tab ID for this parent node
                    let tab_id = format!("tab_{:p}", self);
                    
                    // Determine active tab index, default to 0 if not set
                    let active_tab = tab_indices.get(&tab_id).cloned().unwrap_or(0);
                    let active_tab = active_tab.min(children.len() - 1);

                    // Reserve space for tab bar (1 row)
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(1), // Tab bar
                            Constraint::Min(0),    // Content
                        ])
                        .split(area);

                    // Compute layout for the active tab
                    let active_child = &children[active_tab];
                    
                    // Store active tab info in result
                    let mut result = active_child.compute_layout(chunks[1], tab_indices);
                    
                    // Add special indicator for tab bar area
                    result.push((format!("__TAB_BAR_{}__", tab_id), chunks[0]));
                    
                    return result;
                }

                // Convert layout constraints
                let constraints: Vec<Constraint> = constraints
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect();

                // Create layout
                let chunks = Layout::default()
                    .direction((*layout_type).into())
                    .constraints(constraints)
                    .split(area);

                // Compute layout for each child
                children
                    .iter()
                    .zip(chunks.iter())
                    .flat_map(|(child, &chunk)| child.compute_layout(chunk, tab_indices))
                    .collect()
            }
        }
    }

    /// Split this node in the given direction
    pub fn split(&mut self, window_id: &str, layout_type: LayoutType, new_window_id: &str) -> bool {
        match self {
            Self::Leaf { window_id: current_window_id } => {
                if current_window_id == window_id {
                    // Replace leaf with a parent node
                    let original_window_id = current_window_id.clone();
                    *self = Self::Parent {
                        layout_type: layout_type.clone(),
                        constraints: vec![
                            LayoutConstraint::Percentage(50),
                            LayoutConstraint::Percentage(50),
                        ],
                        children: vec![
                            Self::Leaf { window_id: original_window_id },
                            Self::Leaf { window_id: new_window_id.to_string() },
                        ],
                    };
                    true
                } else {
                    false
                }
            }
            Self::Parent { children, .. } => {
                // Try to split a child node
                for child in children {
                    if child.split(window_id, layout_type.clone(), new_window_id) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Find a window in the layout tree
    pub fn find_window(&self, window_id: &str) -> Option<Rect> {
        match self {
            Self::Leaf { window_id: current_window_id } => {
                if current_window_id == window_id {
                    Some(Rect::default()) // Will be replaced during compute_layout
                } else {
                    None
                }
            }
            Self::Parent { children, .. } => {
                // Try to find the window in children
                for child in children {
                    if let Some(_) = child.find_window(window_id) {
                        return Some(Rect::default()); // Will be replaced during compute_layout
                    }
                }
                None
            }
        }
    }

    /// Close a window in the layout tree
    pub fn close_window(&mut self, window_id: &str) -> bool {
        match self {
            Self::Leaf { window_id: current_window_id } => {
                current_window_id == window_id
            }
            Self::Parent { children, .. } => {
                // Try to close a window in children
                let mut i = 0;
                while i < children.len() {
                    if children[i].close_window(window_id) {
                        // Remove the child
                        children.remove(i);
                        return true;
                    }
                    i += 1;
                }
                false
            }
        }
    }
}

/// Layout manager for window arrangements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutManager {
    /// Root layout node
    root: LayoutNode,
    /// Tab states for tabbed layouts
    tab_states: std::collections::HashMap<String, TabsState>,
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self {
            root: LayoutNode::Leaf { window_id: "default".to_string() },
            tab_states: std::collections::HashMap::new(),
        }
    }
}

impl LayoutManager {
    /// Create a new layout manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the root layout node
    pub fn set_root(&mut self, root: LayoutNode) -> &mut Self {
        self.root = root;
        self
    }

    /// Compute the layout for all windows
    pub fn compute_layout(&self, area: Rect) -> Vec<(String, Rect)> {
        // Create a map of tab indices from the TabsState objects
        let tab_indices: std::collections::HashMap<String, usize> = self.tab_states
            .iter()
            .map(|(id, state)| (id.clone(), state.selected))
            .collect();
            
        self.root.compute_layout(area, &tab_indices)
    }

    /// Split a window in the layout
    pub fn split_window<S, T>(
        &mut self,
        window_id: S,
        layout_type: LayoutType,
        new_window_id: T,
    ) -> bool
    where
        S: AsRef<str>,
        T: AsRef<str>,
    {
        let result = self.root.split(window_id.as_ref(), layout_type, new_window_id.as_ref());
        
        // If created a tabbed layout, initialize its TabsState
        if result && layout_type == LayoutType::Tabbed {
            let tab_id = format!("tab_{:p}", self);
            let tabs = TabsState::new(vec![
                window_id.as_ref().to_string(),
                new_window_id.as_ref().to_string(),
            ]);
            self.tab_states.insert(tab_id, tabs);
        }
        
        result
    }

    /// Close a window in the layout
    pub fn close_window<S>(&mut self, window_id: S) -> bool
    where
        S: AsRef<str>,
    {
        // Find and update any TabsState that contains this window
        for (tab_id, tabs) in self.tab_states.iter_mut() {
            for (idx, title) in tabs.titles.iter().enumerate() {
                if title == window_id.as_ref() {
                    tabs.remove_tab(idx);
                    break;
                }
            }
            
            // Remove the TabsState if it's now empty
            if tabs.is_empty() {
                self.tab_states.remove(tab_id);
            }
        }
        
        self.root.close_window(window_id.as_ref())
    }

    /// Find a window in the layout
    pub fn find_window<S>(&self, window_id: S) -> Option<Rect>
    where
        S: AsRef<str>,
    {
        // Create a map of tab indices from the TabsState objects
        let tab_indices: std::collections::HashMap<String, usize> = self.tab_states
            .iter()
            .map(|(id, state)| (id.clone(), state.selected))
            .collect();
            
        let result = self.root.compute_layout(Rect::default(), &tab_indices);
        result.iter()
            .find(|(id, _)| id == window_id.as_ref())
            .map(|(_, rect)| *rect)
    }

    /// Get the tab state for a tabbed layout
    pub fn get_tab_state<S>(&self, tab_id: S) -> Option<&TabsState>
    where
        S: AsRef<str>,
    {
        self.tab_states.get(tab_id.as_ref())
    }

    /// Get mutable tab state for a tabbed layout
    pub fn get_tab_state_mut<S>(&mut self, tab_id: S) -> Option<&mut TabsState>
    where
        S: AsRef<str>,
    {
        self.tab_states.get_mut(tab_id.as_ref())
    }

    /// Set the active tab for a tabbed layout
    pub fn set_active_tab<S>(&mut self, tab_id: S, tab_index: usize) -> &mut Self
    where
        S: AsRef<str>,
    {
        if let Some(tabs) = self.tab_states.get_mut(tab_id.as_ref()) {
            tabs.select(tab_index);
        }
        self
    }

    /// Get the active tab index for a tabbed layout
    pub fn get_active_tab<S>(&self, tab_id: S) -> Option<usize>
    where
        S: AsRef<str>,
    {
        self.tab_states.get(tab_id.as_ref()).map(|tabs| tabs.selected)
    }
    
    /// Add a tab to a tabbed layout
    pub fn add_tab<S, T>(&mut self, tab_id: S, window_id: T) -> &mut Self
    where
        S: AsRef<str>,
        T: AsRef<str>,
    {
        if let Some(tabs) = self.tab_states.get_mut(tab_id.as_ref()) {
            tabs.add_tab(window_id.as_ref().to_string());
        } else {
            let tabs = TabsState::new(vec![window_id.as_ref().to_string()]);
            self.tab_states.insert(tab_id.as_ref().to_string(), tabs);
        }
        self
    }

    /// Remove a tab from a tabbed layout
    pub fn remove_tab<S>(&mut self, tab_id: S, tab_index: usize) -> &mut Self
    where
        S: AsRef<str>,
    {
        if let Some(tabs) = self.tab_states.get_mut(tab_id.as_ref()) {
            tabs.remove_tab(tab_index);
            
            // Remove the TabsState if it's now empty
            if tabs.is_empty() {
                self.tab_states.remove(tab_id.as_ref());
            }
        }
        self
    }

    /// Move a tab in a tabbed layout
    pub fn move_tab<S>(&mut self, tab_id: S, from_index: usize, to_index: usize) -> &mut Self
    where
        S: AsRef<str>,
    {
        if let Some(tabs) = self.tab_states.get_mut(tab_id.as_ref()) {
            tabs.move_tab(from_index, to_index);
        }
        self
    }

    /// Serialize the layout to JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize the layout from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}