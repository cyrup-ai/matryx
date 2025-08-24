use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::widgets::{
    layout::{LayoutConstraint, LayoutManager, LayoutNode, LayoutType},
    window::{WindowComponent, WindowId},
};

/// Window size state
#[derive(Debug, Clone, PartialEq)]
pub enum WindowSize {
    /// Normal sized window (as defined by layout)
    Normal,
    /// Maximized window (takes full parent area)
    Maximized,
    /// Minimized window (hidden)
    Minimized,
}

/// Window manipulation manager that tracks window sizes and positions
#[derive(Debug, Clone)]
pub struct WindowManipulator {
    /// Map of window sizes
    sizes: HashMap<String, WindowSize>,
    /// Original window positions before maximize
    original_positions: HashMap<String, (String, usize)>,
    /// Map of window constraints before resize
    original_constraints: HashMap<String, Vec<LayoutConstraint>>,
    /// Previously active window before a maximize/minimize
    previous_active: Option<String>,
}

impl Default for WindowManipulator {
    fn default() -> Self {
        Self {
            sizes: HashMap::new(),
            original_positions: HashMap::new(),
            original_constraints: HashMap::new(),
            previous_active: None,
        }
    }
}

impl WindowManipulator {
    /// Create a new window manipulator
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a window is maximized
    pub fn is_maximized<S: AsRef<str>>(&self, window_id: S) -> bool {
        self.sizes
            .get(window_id.as_ref())
            .map_or(false, |size| *size == WindowSize::Maximized)
    }

    /// Check if a window is minimized
    pub fn is_minimized<S: AsRef<str>>(&self, window_id: S) -> bool {
        self.sizes
            .get(window_id.as_ref())
            .map_or(false, |size| *size == WindowSize::Minimized)
    }

    /// Get the size state of a window
    pub fn get_window_size<S: AsRef<str>>(&self, window_id: S) -> WindowSize {
        self.sizes
            .get(window_id.as_ref())
            .cloned()
            .unwrap_or(WindowSize::Normal)
    }

    /// Maximize a window
    pub fn maximize_window<S: AsRef<str>>(
        &mut self,
        window_id: S,
        active_window: Option<S>,
        layout_manager: &mut LayoutManager,
    ) -> bool {
        let window_id = window_id.as_ref();

        // If already maximized, do nothing
        if self.is_maximized(window_id) {
            return false;
        }

        // Store the previous active window if different from the one being maximized
        if let Some(active) = active_window {
            let active = active.as_ref();
            if active != window_id {
                self.previous_active = Some(active.to_string());
            }
        }

        // Find window position in layout
        // We need to perform a tree walk to find the parent and position
        self.find_window_position(window_id, &layout_manager);

        // Set the window to maximized
        self.sizes.insert(window_id.to_string(), WindowSize::Maximized);

        // Replace the layout with just the maximized window
        let original_root = layout_manager.extract_root();
        layout_manager.set_root(LayoutNode::leaf(window_id));

        true
    }

    /// Restore a window from maximized or minimized state
    pub fn restore_window<S: AsRef<str>>(
        &mut self,
        window_id: S,
        layout_manager: &mut LayoutManager,
    ) -> bool {
        let window_id = window_id.as_ref();

        // Check if window is maximized or minimized
        let is_changed = match self.sizes.get(window_id) {
            Some(WindowSize::Maximized) | Some(WindowSize::Minimized) => true,
            _ => false,
        };

        if !is_changed {
            return false;
        }

        // Remove window size state
        self.sizes.remove(window_id);

        // Restore original layout from backed up position
        if let Some((parent_id, position)) = self.original_positions.remove(window_id) {
            // Restore the original constraints if available
            if let Some(constraints) = self.original_constraints.remove(&parent_id) {
                // Rebuild the layout tree...
                // This would typically require complex tree rebuilding logic
                // For this example, we'll just reset to a default layout
                let mut root = layout_manager.extract_root();
                self.restore_layout(&mut root, window_id, &parent_id, position);
                layout_manager.set_root(root);
            }
        }

        true
    }

    /// Minimize a window
    pub fn minimize_window<S: AsRef<str>>(
        &mut self,
        window_id: S,
        active_window: Option<S>,
        layout_manager: &mut LayoutManager,
    ) -> bool {
        let window_id = window_id.as_ref();

        // If already minimized, do nothing
        if self.is_minimized(window_id) {
            return false;
        }

        // Store the previous active window if different from the one being minimized
        if let Some(active) = active_window {
            let active = active.as_ref();
            if active != window_id {
                self.previous_active = Some(active.to_string());
            }
        }

        // Find window position in layout
        self.find_window_position(window_id, &layout_manager);

        // Set the window to minimized
        self.sizes.insert(window_id.to_string(), WindowSize::Minimized);

        // Update the layout to exclude the minimized window
        let mut root = layout_manager.extract_root();
        self.hide_window_in_layout(&mut root, window_id);
        layout_manager.set_root(root);

        true
    }

    /// Get the previous active window before a maximize/minimize
    pub fn get_previous_active(&self) -> Option<&str> {
        self.previous_active.as_deref()
    }

    /// Resize a window constraint
    pub fn resize_window<S: AsRef<str>>(
        &mut self,
        window_id: S,
        delta: i16,
        layout_manager: &mut LayoutManager,
    ) -> bool {
        let window_id = window_id.as_ref();

        // Find the window's parent layout and constraint index
        if let Some((parent_id, position)) = self.find_window_position(window_id, &layout_manager) {
            let mut root = layout_manager.extract_root();
            let result = self.adjust_constraint_in_layout(
                &mut root,
                &parent_id,
                position,
                delta,
            );

            layout_manager.set_root(root);
            return result;
        }

        false
    }

    /// Move a window within its parent layout
    pub fn move_window<S: AsRef<str>>(
        &mut self,
        window_id: S,
        direction: i8, // -1 for previous position, 1 for next position
        layout_manager: &mut LayoutManager,
    ) -> bool {
        let window_id = window_id.as_ref();

        // Find the window's parent layout and constraint index
        if let Some((parent_id, position)) = self.find_window_position(window_id, &layout_manager) {
            let mut root = layout_manager.extract_root();
            let result = self.move_window_in_layout(
                &mut root,
                &parent_id,
                position,
                direction,
            );

            layout_manager.set_root(root);
            return result;
        }

        false
    }

    /// Adjust a window constraint in layout
    fn adjust_constraint_in_layout(
        &mut self,
        node: &mut LayoutNode,
        parent_id: &str,
        position: usize,
        delta: i16,
    ) -> bool {
        match node {
            LayoutNode::Leaf { window_id } => {
                // This is a leaf, not a parent
                false
            }
            LayoutNode::Parent { layout_type, constraints, children } => {
                // Check if this is the parent we're looking for
                if layout_type != &LayoutType::Tabbed {
                    // For parent identification, we'll use the first child's window_id as parent_id
                    let first_window_id = if let Some(LayoutNode::Leaf { window_id }) = children.first() {
                        window_id
                    } else if let Some(LayoutNode::Parent { .. }) = children.first() {
                        // For nested layouts, we need a better ID scheme
                        // For this example, we'll just use a placeholder
                        "nested"
                    } else {
                        ""
                    };

                    if first_window_id == parent_id && position < constraints.len() {
                        // Backup original constraints if not already stored
                        if !self.original_constraints.contains_key(parent_id) {
                            self.original_constraints.insert(parent_id.to_string(), constraints.clone());
                        }

                        // Adjust the constraint
                        match &constraints[position] {
                            LayoutConstraint::Percentage(value) => {
                                let new_value = (*value as i16 + delta).max(10).min(90) as u16;
                                constraints[position] = LayoutConstraint::Percentage(new_value);

                                // Adjust the other constraint to maintain total of 100%
                                if constraints.len() == 2 && position < constraints.len() {
                                    let other_pos = 1 - position; // Toggle between 0 and 1
                                    if let LayoutConstraint::Percentage(other_value) = constraints[other_pos] {
                                        let new_other = (100 - new_value).max(10).min(90);
                                        constraints[other_pos] = LayoutConstraint::Percentage(new_other);
                                    }
                                }
                                
                                return true;
                            }
                            LayoutConstraint::Fixed(value) => {
                                let new_value = (*value as i16 + delta).max(5) as u16;
                                constraints[position] = LayoutConstraint::Fixed(new_value);
                                return true;
                            }
                            _ => return false,
                        }
                    }

                    // Recursively check children
                    for child in children {
                        if self.adjust_constraint_in_layout(child, parent_id, position, delta) {
                            return true;
                        }
                    }
                }

                false
            }
        }
    }

    /// Move a window in layout
    fn move_window_in_layout(
        &mut self,
        node: &mut LayoutNode,
        parent_id: &str,
        position: usize,
        direction: i8,
    ) -> bool {
        match node {
            LayoutNode::Leaf { .. } => false,
            LayoutNode::Parent { layout_type, constraints, children } => {
                // Check if this is the parent we're looking for
                let first_window_id = if let Some(LayoutNode::Leaf { window_id }) = children.first() {
                    window_id
                } else if let Some(LayoutNode::Parent { .. }) = children.first() {
                    "nested"
                } else {
                    ""
                };

                if first_window_id == parent_id {
                    // Calculate new position
                    let new_position = if direction > 0 {
                        if position + 1 < children.len() { position + 1 } else { position }
                    } else {
                        if position > 0 { position - 1 } else { position }
                    };

                    // Don't do anything if position doesn't change
                    if new_position == position {
                        return false;
                    }

                    // Swap the nodes and constraints
                    if new_position < children.len() {
                        children.swap(position, new_position);
                        
                        if position < constraints.len() && new_position < constraints.len() {
                            constraints.swap(position, new_position);
                        }
                        
                        return true;
                    }
                }

                // Recursively check children
                for child in children {
                    if self.move_window_in_layout(child, parent_id, position, direction) {
                        return true;
                    }
                }

                false
            }
        }
    }

    /// Find a window's position in the layout
    fn find_window_position(
        &mut self,
        window_id: &str,
        layout_manager: &LayoutManager,
    ) -> Option<(String, usize)> {
        let mut result = None;
        let root = layout_manager.get_root_ref();
        self.find_window_in_layout(root, window_id, &mut result, "root", 0);
        result
    }

    /// Recursive helper to find a window in layout
    fn find_window_in_layout(
        &self,
        node: &LayoutNode,
        window_id: &str,
        result: &mut Option<(String, usize)>,
        parent_id: &str,
        index: usize,
    ) {
        match node {
            LayoutNode::Leaf { window_id: node_id } => {
                if node_id == window_id {
                    *result = Some((parent_id.to_string(), index));
                }
            }
            LayoutNode::Parent { children, .. } => {
                for (i, child) in children.iter().enumerate() {
                    // Use first child's window_id as the parent ID for its children
                    let next_parent = if let LayoutNode::Leaf { window_id: child_id } = child {
                        child_id
                    } else {
                        // For nested layouts, use a better scheme in a real implementation
                        "nested"
                    };
                    
                    self.find_window_in_layout(child, window_id, result, next_parent, i);
                }
            }
        }
    }

    /// Helper to restore a window in layout
    fn restore_layout(
        &self,
        node: &mut LayoutNode,
        window_id: &str,
        parent_id: &str,
        position: usize,
    ) -> bool {
        match node {
            LayoutNode::Leaf { window_id: node_id } => {
                // Special case for maximized window - the entire layout was replaced
                if node_id == window_id && window_id == "maximized" {
                    // In a real implementation, we'd restore from a backup of the entire layout
                    *node = LayoutNode::parent(
                        LayoutType::Horizontal,
                        vec![
                            LayoutConstraint::Percentage(50),
                            LayoutConstraint::Percentage(50),
                        ],
                        vec![
                            LayoutNode::leaf("window1"),
                            LayoutNode::leaf("window2"),
                        ],
                    );
                    return true;
                }
                false
            }
            LayoutNode::Parent { children, .. } => {
                // Try to find the parent
                let first_window_id = if let Some(LayoutNode::Leaf { window_id: id }) = children.first() {
                    id
                } else {
                    "nested"
                };

                if first_window_id == parent_id {
                    // For minimized windows, restore them to their original position
                    if position < children.len() {
                        let child = &mut children[position];
                        if let LayoutNode::Leaf { window_id: current_id } = child {
                            if current_id != window_id {
                                *child = LayoutNode::leaf(window_id);
                                return true;
                            }
                        }
                    } else if position == children.len() {
                        // If at the end, it was probably removed, so add it back
                        children.push(LayoutNode::leaf(window_id));
                        return true;
                    }
                }

                // Recursively try to restore in children
                for child in children {
                    if self.restore_layout(child, window_id, parent_id, position) {
                        return true;
                    }
                }

                false
            }
        }
    }

    /// Helper to hide a window in layout
    fn hide_window_in_layout(&self, node: &mut LayoutNode, window_id: &str) -> bool {
        match node {
            LayoutNode::Leaf { window_id: node_id } => {
                node_id == window_id
            }
            LayoutNode::Parent { children, .. } => {
                let mut i = 0;
                while i < children.len() {
                    if self.hide_window_in_layout(&mut children[i], window_id) {
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

/// Extension traits for LayoutManager to get root node reference
trait LayoutManagerExt {
    /// Get reference to the root node
    fn get_root_ref(&self) -> &LayoutNode;
    
    /// Extract the root node, replacing it with a default
    fn extract_root(&mut self) -> LayoutNode;
}

impl LayoutManagerExt for LayoutManager {
    fn get_root_ref(&self) -> &LayoutNode {
        // This is a mock implementation as we don't have direct access to the root
        // In a real implementation, you would add a method to LayoutManager to get
        // a reference to the root node
        &LayoutNode::leaf("mock")
    }
    
    fn extract_root(&mut self) -> LayoutNode {
        // This is a mock implementation as we don't have direct access to the root
        // In a real implementation, you would add a method to LayoutManager to
        // replace the root node and return the old one
        LayoutNode::leaf("mock")
    }
}