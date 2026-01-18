mod properties;
mod registry;
mod stateful_property;

// Public API
pub use properties::*;
pub(crate) use registry::*;
pub(crate) use stateful_property::StatefulProperty;

#[cfg(test)]
mod tests {
    //! Integration tests demonstrating the public style API.
    //!
    //! Unit tests for individual components live in their respective modules:
    //! - `registry::tests` for StyleRegistry
    //! - `stateful_property::tests` for StatefulProperty
    //! - `properties::tests` for Style and property definitions (if any)

    use crate::graphics::Color;
    use crate::graphics::Paint;

    use super::*;

    #[test]
    fn style_system_integration() {
        // Demonstrates the typical usage pattern: create registry, register
        // styles with inheritance and state overrides, then resolve properties.
        let mut registry = StyleRegistry::default();

        // Base button style
        let button_base = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(Color::srgb(0.2, 0.2, 0.2, 1.0))),
                    ),
                    (StateFlags::NORMAL, StyleProperty::TextColor(Color::WHITE)),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(Color::srgb(0.3, 0.3, 0.3, 1.0))),
                    ),
                    (
                        StateFlags::PRESSED,
                        StyleProperty::Background(Paint::solid(Color::srgb(0.1, 0.1, 0.1, 1.0))),
                    ),
                ],
            )
            .unwrap();

        // Primary button variant inherits from base, overrides colors
        let button_primary = registry
            .register(
                Some(button_base),
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::Background(Paint::solid(Color::srgb(0.0, 0.4, 0.8, 1.0))),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::Background(Paint::solid(Color::srgb(0.0, 0.5, 1.0, 1.0))),
                    ),
                ],
            )
            .unwrap();

        // Resolve properties for different states
        let base_normal: Paint =
            registry.resolve::<Background>(button_base, StateFlags::NORMAL);
        let base_hovered: Paint =
            registry.resolve::<Background>(button_base, StateFlags::HOVERED);
        let primary_normal: Paint =
            registry.resolve::<Background>(button_primary, StateFlags::NORMAL);
        let primary_text: Color = registry.resolve::<TextColor>(button_primary, StateFlags::NORMAL);

        // Base button colors
        assert_eq!(base_normal, Paint::solid(Color::srgb(0.2, 0.2, 0.2, 1.0)));
        assert_eq!(base_hovered, Paint::solid(Color::srgb(0.3, 0.3, 0.3, 1.0)));

        // Primary inherits text color, overrides background
        assert_eq!(primary_normal, Paint::solid(Color::srgb(0.0, 0.4, 0.8, 1.0)));
        assert_eq!(primary_text, Color::WHITE); // Inherited from base
    }
}
