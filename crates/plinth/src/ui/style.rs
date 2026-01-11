//! The Plinth Styling system.
//!

mod properties;
mod registry;

// Public API
pub use properties::*;
pub use registry::*;

#[cfg(test)]
mod tests {
    use crate::graphics::Color;

    use super::*;

    // Helper to create colors from 0-255 RGB values
    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::srgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    #[test]
    fn test_basic_type_safe_resolution() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(255, 0, 0)),
                ),
                (StateFlags::NORMAL, StyleProperty::TextColor(rgb(0, 0, 255))),
            ],
        );

        let bg: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        let text: Color = registry.resolve::<TextColor>(style, StateFlags::NORMAL);

        assert_eq!(bg, rgb(255, 0, 0));
        assert_eq!(text, rgb(0, 0, 255));
    }

    #[test]
    fn test_state_specific_resolution() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(150, 150, 150)),
                ),
                (
                    StateFlags::PRESSED,
                    StyleProperty::BackgroundColor(rgb(200, 200, 200)),
                ),
            ],
        );

        let normal: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        let hovered: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        let active: Color = registry.resolve::<BackgroundColor>(style, StateFlags::PRESSED);

        assert_eq!(normal, rgb(100, 100, 100));
        assert_eq!(hovered, rgb(150, 150, 150));
        assert_eq!(active, rgb(200, 200, 200));
    }

    #[test]
    fn test_default_fallback() {
        let mut registry = StyleRegistry::new();

        // Register style with no properties
        let style = registry.register(None, vec![]);

        // Should return default values
        let bg: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        let text: Color = registry.resolve::<TextColor>(style, StateFlags::NORMAL);

        assert_eq!(bg, Color::WHITE);
        assert_eq!(text, Color::BLACK);
    }

    #[test]
    fn test_inheritance() {
        let mut registry = StyleRegistry::new();

        // Parent style with background
        let parent = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(50, 50, 50)),
                ),
                (
                    StateFlags::NORMAL,
                    StyleProperty::TextColor(rgb(255, 255, 255)),
                ),
            ],
        );

        // Child style only overrides background
        let child = registry.register(
            Some(parent),
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        let bg: Color = registry.resolve::<BackgroundColor>(child, StateFlags::NORMAL);
        let text: Color = registry.resolve::<TextColor>(child, StateFlags::NORMAL);

        // Background should be from child
        assert_eq!(bg, rgb(100, 100, 100));
        // Text color should be inherited from parent
        assert_eq!(text, rgb(255, 255, 255));
    }

    #[test]
    fn test_exact_match_priority() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED | StateFlags::PRESSED,
                    StyleProperty::BackgroundColor(rgb(200, 200, 200)),
                ),
            ],
        );

        // Exact match for HOVERED | PRESSED should return the more specific one
        let color: Color =
            registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED | StateFlags::PRESSED);
        assert_eq!(color, rgb(200, 200, 200));

        // HOVERED alone should match the first one
        let color: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        assert_eq!(color, rgb(100, 100, 100));
    }

    #[test]
    fn test_best_subset_match() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::PRESSED,
                    StyleProperty::BackgroundColor(rgb(150, 150, 150)),
                ),
            ],
        );

        // Query with HOVERED | PRESSED should match one of them (subset match)
        // Since both have same bit count, it should match the first one found
        let color: Color =
            registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED | StateFlags::PRESSED);

        // Should match one of the subset matches
        assert!(color == rgb(100, 100, 100) || color == rgb(150, 150, 150));
    }

    #[test]
    fn test_most_specific_subset_match() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(50, 50, 50)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED | StateFlags::PRESSED,
                    StyleProperty::BackgroundColor(rgb(150, 150, 150)),
                ),
            ],
        );

        // Query with HOVERED | PRESSED should match the most specific (2 bits)
        let color: Color =
            registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED | StateFlags::PRESSED);

        // Should match HOVERED | PRESSED (2 bits) rather than just HOVERED (1 bit)
        assert_eq!(color, rgb(150, 150, 150));
    }

    #[test]
    fn test_unregister() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        // Should resolve successfully
        let color: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(color, rgb(100, 100, 100));

        // Unregister the style
        registry.unregister(style);

        // Now should return default (since style doesn't exist)
        let color: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(color, Color::WHITE);
    }

    #[test]
    fn test_multiple_properties_same_state() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::TextColor(rgb(255, 255, 255)),
                ),
            ],
        );

        let bg: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        let text: Color = registry.resolve::<TextColor>(style, StateFlags::HOVERED);

        assert_eq!(bg, rgb(100, 100, 100));
        assert_eq!(text, rgb(255, 255, 255));
    }

    #[test]
    fn test_try_resolve_returns_none_for_missing_style() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        // Property exists
        let bg: Option<Color> = registry.try_resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(bg, Some(rgb(100, 100, 100)));

        // Unregister the style
        registry.unregister(style);

        // try_resolve should return None for non-existent style
        let bg: Option<Color> = registry.try_resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(bg, None);
    }

    #[test]
    fn test_state_fallback_to_normal() {
        let mut registry = StyleRegistry::new();

        // Only define NORMAL state
        let style = registry.register(
            None,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(50, 50, 50)),
            )],
        );

        // NORMAL should match
        let normal: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(normal, rgb(50, 50, 50));

        // HOVERED should fall back to NORMAL (NORMAL is contained in all states)
        let hovered: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        assert_eq!(hovered, rgb(50, 50, 50));

        // PRESSED should also fall back to NORMAL
        let active: Color = registry.resolve::<BackgroundColor>(style, StateFlags::PRESSED);
        assert_eq!(active, rgb(50, 50, 50));
    }

    #[test]
    fn test_hovered_does_not_match_normal_query() {
        let mut registry = StyleRegistry::new();

        // Only define HOVERED state, not NORMAL
        let style = registry.register(
            None,
            vec![(
                StateFlags::HOVERED,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        // NORMAL query should NOT match HOVERED (HOVERED is not a subset of NORMAL)
        // Should return default
        let normal: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(normal, Color::WHITE);

        // HOVERED should match
        let hovered: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        assert_eq!(hovered, rgb(100, 100, 100));
    }

    #[test]
    fn test_compound_state_selected_and_hovered() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(50, 50, 50)),
                ),
                (
                    StateFlags::SELECTED,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(150, 150, 150)),
                ),
                (
                    StateFlags::SELECTED | StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(200, 200, 200)),
                ),
            ],
        );

        // Individual states
        let normal: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(normal, rgb(50, 50, 50));

        let selected: Color = registry.resolve::<BackgroundColor>(style, StateFlags::SELECTED);
        assert_eq!(selected, rgb(100, 100, 100));

        let hovered: Color = registry.resolve::<BackgroundColor>(style, StateFlags::HOVERED);
        assert_eq!(hovered, rgb(150, 150, 150));

        // Compound state should match the most specific
        let selected_hovered: Color =
            registry.resolve::<BackgroundColor>(style, StateFlags::SELECTED | StateFlags::HOVERED);
        assert_eq!(selected_hovered, rgb(200, 200, 200));
    }

    #[test]
    fn test_style_update_regenerates() {
        let mut registry = StyleRegistry::new();

        let style = registry.register(
            None,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        // Initial value
        let color: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(color, rgb(100, 100, 100));

        // Update the style
        registry.update(
            style,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(200, 200, 200)),
            )],
        );

        // Should reflect new value
        let color: Color = registry.resolve::<BackgroundColor>(style, StateFlags::NORMAL);
        assert_eq!(color, rgb(200, 200, 200));
    }

    #[test]
    fn test_parent_update_regenerates_children() {
        let mut registry = StyleRegistry::new();

        let parent = registry.register(
            None,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(50, 50, 50)),
            )],
        );

        let child = registry.register(
            Some(parent),
            vec![(
                StateFlags::NORMAL,
                StyleProperty::TextColor(rgb(255, 255, 255)),
            )],
        );

        // Child inherits parent's background
        let bg: Color = registry.resolve::<BackgroundColor>(child, StateFlags::NORMAL);
        assert_eq!(bg, rgb(50, 50, 50));

        // Update parent
        registry.update(
            parent,
            vec![(
                StateFlags::NORMAL,
                StyleProperty::BackgroundColor(rgb(100, 100, 100)),
            )],
        );

        // Child should have regenerated with new parent value
        let bg: Color = registry.resolve::<BackgroundColor>(child, StateFlags::NORMAL);
        assert_eq!(bg, rgb(100, 100, 100));

        // Child's own property should be preserved
        let text: Color = registry.resolve::<TextColor>(child, StateFlags::NORMAL);
        assert_eq!(text, rgb(255, 255, 255));
    }

    #[test]
    fn test_direct_style_access() {
        let mut registry = StyleRegistry::new();

        let style_id = registry.register(
            None,
            vec![
                (
                    StateFlags::NORMAL,
                    StyleProperty::BackgroundColor(rgb(100, 100, 100)),
                ),
                (
                    StateFlags::HOVERED,
                    StyleProperty::BackgroundColor(rgb(150, 150, 150)),
                ),
            ],
        );

        // Get the resolved style directly
        let style = registry.get(style_id).unwrap();

        // Access properties directly on the Style struct
        assert_eq!(
            style.background_color.get(StateFlags::NORMAL),
            rgb(100, 100, 100)
        );
        assert_eq!(
            style.background_color.get(StateFlags::HOVERED),
            rgb(150, 150, 150)
        );
    }
}
