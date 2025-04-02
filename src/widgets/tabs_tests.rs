#[cfg(test)]
mod tests {
    use super::super::tabs::{Tabs, TabsState};
    use ratatui::{
        buffer::Buffer,
        layout::Rect,
        style::{Color, Modifier, Style},
    };

    #[test]
    fn test_tabs_state() {
        // Create a new tabs state
        let mut state = TabsState::new(vec![
            "Tab 1".to_string(),
            "Tab 2".to_string(),
            "Tab 3".to_string(),
        ]);

        // Check initial state
        assert_eq!(state.selected, 0);
        assert_eq!(state.selected_title(), Some(&"Tab 1".to_string()));
        assert_eq!(state.len(), 3);
        assert!(!state.is_empty());

        // Test next/prev navigation
        state.next();
        assert_eq!(state.selected, 1);
        assert_eq!(state.selected_title(), Some(&"Tab 2".to_string()));

        state.next();
        assert_eq!(state.selected, 2);
        assert_eq!(state.selected_title(), Some(&"Tab 3".to_string()));

        state.next(); // Wrap around
        assert_eq!(state.selected, 0);
        assert_eq!(state.selected_title(), Some(&"Tab 1".to_string()));

        state.prev();
        assert_eq!(state.selected, 2);
        assert_eq!(state.selected_title(), Some(&"Tab 3".to_string()));

        state.prev();
        assert_eq!(state.selected, 1);
        assert_eq!(state.selected_title(), Some(&"Tab 2".to_string()));

        // Test direct selection
        state.select(0);
        assert_eq!(state.selected, 0);
        assert_eq!(state.selected_title(), Some(&"Tab 1".to_string()));

        // Test invalid selection (index out of bounds)
        state.select(10); // Should not change state
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_tabs_state_operations() {
        // Create a new tabs state
        let mut state = TabsState::new(vec![
            "Tab 1".to_string(),
            "Tab 2".to_string(),
        ]);

        // Test add_tab
        state.add_tab("Tab 3".to_string());
        assert_eq!(state.len(), 3);
        assert_eq!(state.titles[2], "Tab 3");

        // Test remove_tab
        state.remove_tab(1);
        assert_eq!(state.len(), 2);
        assert_eq!(state.titles[0], "Tab 1");
        assert_eq!(state.titles[1], "Tab 3");

        // Test selected adjustment after removal
        state.select(1);
        assert_eq!(state.selected, 1);
        state.remove_tab(1);
        assert_eq!(state.len(), 1);
        assert_eq!(state.selected, 0); // Selected should be adjusted

        // Test move_tab
        state.add_tab("Tab 2".to_string());
        state.add_tab("Tab 3".to_string());
        assert_eq!(state.titles, vec!["Tab 1", "Tab 2", "Tab 3"]);

        state.select(0);
        state.move_tab(0, 2);
        assert_eq!(state.titles, vec!["Tab 2", "Tab 3", "Tab 1"]);
        assert_eq!(state.selected, 2); // Selected should move with the tab

        state.move_tab(1, 0);
        assert_eq!(state.titles, vec!["Tab 3", "Tab 2", "Tab 1"]);
        assert_eq!(state.selected, 2); // Selected should remain at "Tab 1"
    }

    #[test]
    fn test_empty_tabs() {
        // Create an empty tabs state
        let mut state = TabsState::new(vec![]);

        // Operations should be safe on empty tabs
        assert_eq!(state.selected, 0);
        assert_eq!(state.selected_title(), None);
        assert_eq!(state.len(), 0);
        assert!(state.is_empty());

        // Navigation should be safe on empty tabs
        state.next();
        assert_eq!(state.selected, 0);

        state.prev();
        assert_eq!(state.selected, 0);

        // Other operations should be safe
        state.select(5);
        assert_eq!(state.selected, 0);

        state.remove_tab(0);
        assert_eq!(state.len(), 0);

        state.move_tab(0, 1);
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_tabs_rendering() {
        // Create a new tabs widget and state
        let tabs = Tabs::default()
            .style(Style::default().fg(Color::Gray))
            .selected_style(
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_selected(true)
            .show_borders(true);

        let mut state = TabsState::new(vec![
            "Tab 1".to_string(),
            "Tab 2".to_string(),
            "Tab 3".to_string(),
        ]);

        // Set the selected tab
        state.select(1);

        // Create a buffer to render into
        let area = Rect::new(0, 0, 50, 1);
        let mut buffer = Buffer::empty(area);

        // Render the tabs
        tabs.render(area, &mut buffer, &mut state);

        // Check the rendered output
        // This is a basic check to ensure something was rendered
        let rendered = buffer.content.iter().any(|&c| c != 0);
        assert!(rendered);

        // Check tab boundaries are visible
        let content: String = buffer.content
            .chunks(4)
            .map(|chunk| char::from_u32(chunk[0] as u32).unwrap_or(' '))
            .collect();
        
        assert!(content.contains('|')); // Should contain tab boundaries
    }

    #[test]
    fn test_tabs_overflow() {
        // Create a new tabs widget and state with many tabs
        let tabs = Tabs::default()
            .style(Style::default())
            .selected_style(Style::default())
            .highlight_selected(true)
            .show_borders(true);

        let mut state = TabsState::new((1..=20).map(|i| format!("Tab {}", i)).collect());

        // Create a small buffer to render into
        let area = Rect::new(0, 0, 20, 1); // Not enough space for all tabs
        let mut buffer = Buffer::empty(area);

        // Render the tabs
        tabs.render(area, &mut buffer, &mut state);

        // Check the rendered output
        // This is a basic check to ensure something was rendered
        let rendered = buffer.content.iter().any(|&c| c != 0);
        assert!(rendered);

        // Check for ellipsis indicating overflow
        let content: String = buffer.content
            .chunks(4)
            .map(|chunk| char::from_u32(chunk[0] as u32).unwrap_or(' '))
            .collect();
        
        assert!(content.contains('.')); // Should contain ellipsis
    }
}