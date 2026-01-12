//! The Plinth styling system.
//!
//! This module provides a small, type-safe styling engine for UI components.
//! Styles are stored in a [`StyleRegistry`] and can be resolved for a
//! particular widget and interaction state (such as *normal*, *hovered*,
//! or *disabled*).
//!
//! # Overview
//!
//! - [`StyleRegistry`] owns all registered styles and is responsible for
//!   resolving concrete values for each property.
//! - [`StateFlags`] describes the logical state of a widget (e.g.
//!   `StateFlags::NORMAL`, `StateFlags::HOVERED`), and is used both when
//!   registering style properties and when resolving them.
//! - `PropertyKey` is an internal identifier that associates a strongly
//!   typed property (such as [`BackgroundColor`] or [`TextColor`]) with
//!   its storage in the registry.
//! - Individual style properties are represented by variants of
//!   [`StyleProperty`], which can be attached to styles for specific
//!   states.
//!
//! Typically, each visual component is associated with a style identifier
//! returned from the registry. At render time, the component asks the
//! registry to resolve the strongly typed properties it needs for its
//! current [`StateFlags`].
//!
//! # Basic usage
//!
//! ```no_run
//! use plinth::ui::style::{
//!     StyleRegistry, StateFlags, StyleProperty, BackgroundColor, TextColor,
//! };
//! use plinth::graphics::Color;
//!
//! let mut registry = StyleRegistry::new();
//!
//! // Register a simple style with separate background and text colors
//! let style_id = registry.register(
//!     None, // no parent style
//!     vec![
//!         (
//!             StateFlags::NORMAL,
//!             StyleProperty::BackgroundColor(Color::WHITE),
//!         ),
//!         (
//!             StateFlags::NORMAL,
//!             StyleProperty::TextColor(Color::BLACK),
//!         ),
//!     ],
//! ).unwrap();
//!
//! // Later, resolve strongly typed properties for a particular state:
//! let bg: Color = registry.resolve::<BackgroundColor>(style_id, StateFlags::NORMAL);
//! let text: Color = registry.resolve::<TextColor>(style_id, StateFlags::NORMAL);
//! ```
//!
//! The registry also supports:
//!
//! - State-specific overrides (e.g. different colors for `HOVERED` vs `NORMAL`).
//! - Inheritance between styles, allowing one style to build on another.
//! - Default values for properties that are not explicitly set.
//!
//! See the `properties` and `registry` submodules, as well as the tests in
//! this file, for more details on the available properties and resolution
//! behavior.

mod properties;
mod registry;
mod stateful_property;

// Public API
pub use properties::*;
pub use registry::*;
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

    use super::*;

    #[test]
    fn style_system_integration() {
        // Demonstrates the typical usage pattern: create registry, register
        // styles with inheritance and state overrides, then resolve properties.
        let mut registry = StyleRegistry::new();

        // Base button style
        let button_base = registry
            .register(
                None,
                vec![
                    (
                        StateFlags::NORMAL,
                        StyleProperty::BackgroundColor(Color::srgb(0.2, 0.2, 0.2, 1.0)),
                    ),
                    (StateFlags::NORMAL, StyleProperty::TextColor(Color::WHITE)),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::BackgroundColor(Color::srgb(0.3, 0.3, 0.3, 1.0)),
                    ),
                    (
                        StateFlags::PRESSED,
                        StyleProperty::BackgroundColor(Color::srgb(0.1, 0.1, 0.1, 1.0)),
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
                        StyleProperty::BackgroundColor(Color::srgb(0.0, 0.4, 0.8, 1.0)),
                    ),
                    (
                        StateFlags::HOVERED,
                        StyleProperty::BackgroundColor(Color::srgb(0.0, 0.5, 1.0, 1.0)),
                    ),
                ],
            )
            .unwrap();

        // Resolve properties for different states
        let base_normal: Color =
            registry.resolve::<BackgroundColor>(button_base, StateFlags::NORMAL);
        let base_hovered: Color =
            registry.resolve::<BackgroundColor>(button_base, StateFlags::HOVERED);
        let primary_normal: Color =
            registry.resolve::<BackgroundColor>(button_primary, StateFlags::NORMAL);
        let primary_text: Color = registry.resolve::<TextColor>(button_primary, StateFlags::NORMAL);

        // Base button colors
        assert_eq!(base_normal, Color::srgb(0.2, 0.2, 0.2, 1.0));
        assert_eq!(base_hovered, Color::srgb(0.3, 0.3, 0.3, 1.0));

        // Primary inherits text color, overrides background
        assert_eq!(primary_normal, Color::srgb(0.0, 0.4, 0.8, 1.0));
        assert_eq!(primary_text, Color::WHITE); // Inherited from base
    }
}
