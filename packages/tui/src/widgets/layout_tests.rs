#[cfg(test)]
mod tests {
    use super::super::layout::{LayoutConstraint, LayoutManager, LayoutNode, LayoutType};
    use super::super::tabs::TabsState;
    use ratatui::layout::Rect;
    use std::collections::HashMap;

    #[test]
    fn test_layout_node_creation() {
        // Test leaf node creation
        let leaf = LayoutNode::leaf("window1");
        match leaf {
            LayoutNode::Leaf { window_id } => {
                assert_eq!(window_id, "window1");
            }
            _ => panic!("Expected leaf node"),
        }

        // Test parent node creation
        let parent = LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Percentage(30),
                LayoutConstraint::Percentage(70),
            ],
            vec![
                LayoutNode::leaf("window1"),
                LayoutNode::leaf("window2"),
            ],
        );

        match parent {
            LayoutNode::Parent { layout_type, constraints, children } => {
                assert_eq!(layout_type, LayoutType::Horizontal);
                assert_eq!(constraints.len(), 2);
                assert_eq!(children.len(), 2);
            }
            _ => panic!("Expected parent node"),
        }
    }

    #[test]
    fn test_layout_computation() {
        // Create a simple layout
        let layout = LayoutNode::parent(
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

        // Create a test area
        let area = Rect::new(0, 0, 100, 50);
        
        // Create empty tab indices map
        let tab_indices = HashMap::new();

        // Compute the layout
        let computed = layout.compute_layout(area, &tab_indices);

        // Check the result
        assert_eq!(computed.len(), 2);
        assert_eq!(computed[0].0, "window1");
        assert_eq!(computed[1].0, "window2");

        // First window should be on the left half
        assert_eq!(computed[0].1.x, 0);
        assert_eq!(computed[0].1.y, 0);
        assert_eq!(computed[0].1.width, 50);
        assert_eq!(computed[0].1.height, 50);

        // Second window should be on the right half
        assert_eq!(computed[1].1.x, 50);
        assert_eq!(computed[1].1.y, 0);
        assert_eq!(computed[1].1.width, 50);
        assert_eq!(computed[1].1.height, 50);
    }

    #[test]
    fn test_vertical_layout() {
        // Create a vertical layout
        let layout = LayoutNode::parent(
            LayoutType::Vertical,
            vec![
                LayoutConstraint::Percentage(30),
                LayoutConstraint::Percentage(70),
            ],
            vec![
                LayoutNode::leaf("top"),
                LayoutNode::leaf("bottom"),
            ],
        );

        // Create a test area
        let area = Rect::new(0, 0, 100, 100);
        
        // Create empty tab indices map
        let tab_indices = HashMap::new();

        // Compute the layout
        let computed = layout.compute_layout(area, &tab_indices);

        // Check the result
        assert_eq!(computed.len(), 2);
        assert_eq!(computed[0].0, "top");
        assert_eq!(computed[1].0, "bottom");

        // Top window
        assert_eq!(computed[0].1.x, 0);
        assert_eq!(computed[0].1.y, 0);
        assert_eq!(computed[0].1.width, 100);
        assert_eq!(computed[0].1.height, 30);

        // Bottom window
        assert_eq!(computed[1].1.x, 0);
        assert_eq!(computed[1].1.y, 30);
        assert_eq!(computed[1].1.width, 100);
        assert_eq!(computed[1].1.height, 70);
    }

    #[test]
    fn test_nested_layout() {
        // Create a nested layout with a horizontal split containing a vertical split on the left
        let layout = LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Percentage(30),
                LayoutConstraint::Percentage(70),
            ],
            vec![
                LayoutNode::parent(
                    LayoutType::Vertical,
                    vec![
                        LayoutConstraint::Percentage(50),
                        LayoutConstraint::Percentage(50),
                    ],
                    vec![
                        LayoutNode::leaf("top-left"),
                        LayoutNode::leaf("bottom-left"),
                    ],
                ),
                LayoutNode::leaf("right"),
            ],
        );

        // Create a test area
        let area = Rect::new(0, 0, 100, 100);
        
        // Create empty tab indices map
        let tab_indices = HashMap::new();

        // Compute the layout
        let computed = layout.compute_layout(area, &tab_indices);

        // Check the result
        assert_eq!(computed.len(), 3);
        
        // Find each window by ID
        let top_left = computed.iter().find(|(id, _)| id == "top-left").unwrap();
        let bottom_left = computed.iter().find(|(id, _)| id == "bottom-left").unwrap();
        let right = computed.iter().find(|(id, _)| id == "right").unwrap();

        // Check top-left window
        assert_eq!(top_left.1.x, 0);
        assert_eq!(top_left.1.y, 0);
        assert_eq!(top_left.1.width, 30);
        assert_eq!(top_left.1.height, 50);

        // Check bottom-left window
        assert_eq!(bottom_left.1.x, 0);
        assert_eq!(bottom_left.1.y, 50);
        assert_eq!(bottom_left.1.width, 30);
        assert_eq!(bottom_left.1.height, 50);

        // Check right window
        assert_eq!(right.1.x, 30);
        assert_eq!(right.1.y, 0);
        assert_eq!(right.1.width, 70);
        assert_eq!(right.1.height, 100);
    }

    #[test]
    fn test_fixed_constraint() {
        // Create a layout with fixed and percentage constraints
        let layout = LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Fixed(20),
                LayoutConstraint::Percentage(100),
            ],
            vec![
                LayoutNode::leaf("fixed"),
                LayoutNode::leaf("remaining"),
            ],
        );

        // Create a test area
        let area = Rect::new(0, 0, 100, 50);
        
        // Create empty tab indices map
        let tab_indices = HashMap::new();

        // Compute the layout
        let computed = layout.compute_layout(area, &tab_indices);

        // Check the result
        assert_eq!(computed.len(), 2);
        
        // Fixed window should be exactly 20 wide
        let fixed = computed.iter().find(|(id, _)| id == "fixed").unwrap();
        assert_eq!(fixed.1.width, 20);
        
        // Remaining window should take the rest of the space
        let remaining = computed.iter().find(|(id, _)| id == "remaining").unwrap();
        assert_eq!(remaining.1.width, 80);
    }

    #[test]
    fn test_window_splitting() {
        // Create a simple layout manager with a single window
        let mut manager = LayoutManager::new();
        manager.set_root(LayoutNode::leaf("window1"));

        // Split the window horizontally
        let result = manager.split_window("window1", LayoutType::Horizontal, "window2");
        assert!(result);

        // Compute the layout
        let area = Rect::new(0, 0, 100, 50);
        let computed = manager.compute_layout(area);

        // Check the result
        assert_eq!(computed.len(), 2);
        assert!(computed.iter().any(|(id, _)| id == "window1"));
        assert!(computed.iter().any(|(id, _)| id == "window2"));

        // Now split window2 vertically
        let result = manager.split_window("window2", LayoutType::Vertical, "window3");
        assert!(result);

        // Compute the layout again
        let computed = manager.compute_layout(area);

        // Check the result
        assert_eq!(computed.len(), 3);
        assert!(computed.iter().any(|(id, _)| id == "window1"));
        assert!(computed.iter().any(|(id, _)| id == "window2"));
        assert!(computed.iter().any(|(id, _)| id == "window3"));
    }

    #[test]
    fn test_window_closing() {
        // Create a layout with three windows
        let mut manager = LayoutManager::new();
        manager.set_root(LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Percentage(50),
                LayoutConstraint::Percentage(50),
            ],
            vec![
                LayoutNode::leaf("window1"),
                LayoutNode::parent(
                    LayoutType::Vertical,
                    vec![
                        LayoutConstraint::Percentage(50),
                        LayoutConstraint::Percentage(50),
                    ],
                    vec![
                        LayoutNode::leaf("window2"),
                        LayoutNode::leaf("window3"),
                    ],
                ),
            ],
        ));

        // Close window2
        let result = manager.close_window("window2");
        assert!(result);

        // Compute the layout
        let area = Rect::new(0, 0, 100, 50);
        let computed = manager.compute_layout(area);

        // Check the result - should have window1 and window3
        assert_eq!(computed.len(), 2);
        assert!(computed.iter().any(|(id, _)| id == "window1"));
        assert!(computed.iter().any(|(id, _)| id == "window3"));
        assert!(!computed.iter().any(|(id, _)| id == "window2"));
    }

    #[test]
    fn test_tabbed_layout() {
        // Create a layout with tabs
        let mut manager = LayoutManager::new();
        manager.set_root(LayoutNode::parent(
            LayoutType::Tabbed,
            vec![
                LayoutConstraint::Percentage(100),
                LayoutConstraint::Percentage(100),
                LayoutConstraint::Percentage(100),
            ],
            vec![
                LayoutNode::leaf("tab1"),
                LayoutNode::leaf("tab2"),
                LayoutNode::leaf("tab3"),
            ],
        ));
        
        // Create a test area
        let area = Rect::new(0, 0, 100, 50);
        
        // First computation shows tab1 by default
        let computed = manager.compute_layout(area);
        
        // There should be 2 elements: the tab content and the tab bar
        assert_eq!(computed.len(), 2);
        
        // We should have tab1 content and a tab bar area
        assert!(computed.iter().any(|(id, _)| id == "tab1"));
        assert!(computed.iter().any(|(id, _)| id.starts_with("__TAB_BAR_")));
        
        // Add tab states explicitly 
        let tab_id = computed.iter()
            .find(|(id, _)| id.starts_with("__TAB_BAR_"))
            .map(|(id, _)| id.replace("__TAB_BAR_", "").replace("__", ""))
            .unwrap();
            
        manager.add_tab(&tab_id, "tab1");
        manager.add_tab(&tab_id, "tab2");
        manager.add_tab(&tab_id, "tab3");
        
        // Switch to tab2
        manager.set_active_tab(&tab_id, 1);
        
        // Compute the layout again
        let computed = manager.compute_layout(area);
        
        // Now we should see tab2 content
        assert!(computed.iter().any(|(id, _)| id == "tab2"));
        assert!(!computed.iter().any(|(id, _)| id == "tab1"));
        assert!(!computed.iter().any(|(id, _)| id == "tab3"));
        
        // Switch to tab3
        manager.set_active_tab(&tab_id, 2);
        
        // Compute the layout again
        let computed = manager.compute_layout(area);
        
        // Now we should see tab3 content
        assert!(computed.iter().any(|(id, _)| id == "tab3"));
        assert!(!computed.iter().any(|(id, _)| id == "tab1"));
        assert!(!computed.iter().any(|(id, _)| id == "tab2"));
    }

    #[test]
    fn test_layout_serialization() {
        // Create a layout
        let mut manager = LayoutManager::new();
        manager.set_root(LayoutNode::parent(
            LayoutType::Horizontal,
            vec![
                LayoutConstraint::Percentage(30),
                LayoutConstraint::Percentage(70),
            ],
            vec![
                LayoutNode::leaf("window1"),
                LayoutNode::leaf("window2"),
            ],
        ));

        // Serialize to JSON
        let json = manager.to_json().unwrap();

        // Deserialize from JSON
        let deserialized = LayoutManager::from_json(&json).unwrap();

        // Compute both layouts
        let area = Rect::new(0, 0, 100, 50);
        let original = manager.compute_layout(area);
        let round_trip = deserialized.compute_layout(area);

        // Check that they produce the same result
        assert_eq!(original.len(), round_trip.len());
        for i in 0..original.len() {
            assert_eq!(original[i].0, round_trip[i].0);
            assert_eq!(original[i].1, round_trip[i].1);
        }
    }
}