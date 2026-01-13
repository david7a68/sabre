use std::sync::Arc;

use crate::graphics::Color;
use crate::graphics::text::FontStyle;
use crate::graphics::text::TextAlignment;
use crate::ui::Alignment;
use crate::ui::LayoutDirection;
use crate::ui::Size;
use crate::ui::layout::Padding;

use crate::ui::style::StatefulProperty;
use crate::ui::style::registry::PropertyKey;

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct StateFlags: u8 {
        const HOVERED  = 0b00000001;
        const PRESSED  = 0b00000010;
        const SELECTED = 0b00000100;
        const DISABLED = 0b00001000;
        const FOCUSED  = 0b00010000;
        const CHECKED  = 0b00100000;
        const INVALID  = 0b01000000;
        const EXPANDED = 0b10000000;

        const NORMAL = 0;
    }
}

macros::declare_style! {
    pub struct Style {
        background_color: BackgroundColor(Color) = Color::WHITE,

        // layout styles
        padding: Padding(use Padding) = Padding::default(),
        child_major_alignment: ChildMajorAlignment(Alignment) = Alignment::Start,
        child_minor_alignment: ChildMinorAlignment(Alignment) = Alignment::Center,
        child_spacing: ChildSpacing(f32) = 4.0,
        child_direction: ChildDirection(LayoutDirection) = LayoutDirection::Horizontal,
        width: Width(Size) = Size::Fit { min: 20.0, max: f32::MAX },
        height: Height(Size) = Size::Fit { min: 10.0, max: f32::MAX },

        // text styles
        font: Font(Arc<crate::graphics::text::Font>) = Arc::new(crate::graphics::text::Font::default()),
        font_size: FontSize(u16) = 14,
        font_style: FontStyle(use FontStyle) = FontStyle::Normal,
        font_weight: FontWeight(u16) = 400,
        strikethrough_color: StrikethroughColor(Color) = Color::BLACK,
        strikethrough_offset: StrikethroughOffset(f32) = 0.0,
        text_align: TextAlignment(use TextAlignment) = TextAlignment::Start,
        text_color: TextColor(Color) = Color::BLACK,
        underline_color: UnderlineColor(Color) = Color::BLACK,
        underline_offset: UnderlineOffset(f32) = 0.0,
    }
}

mod macros {
    /// Generates:
    /// - `Style` struct with StatefulProperty<T> for each property
    /// - `StyleProperty` enum for setting properties
    /// - Zero-sized type keys for type-safe resolution
    ///
    /// Syntax:
    /// - `field: Key(Type) = default` - creates new ZST key type
    /// - `field: Key(use Type) = default` - implements trait on existing type (no struct)
    macro_rules! declare_style {
        // Entry point: start token munching
        (
            $vis:vis struct $style_name:ident {
                $($rest:tt)*
            }
        ) => {
            $crate::ui::style::properties::macros::declare_style!(@munch
                vis: [$vis]
                name: [$style_name]
                new: []
                use: []
                rest: [$($rest)*]
            );
        };

        // Munch a "use" variant: field: Key(use Type) = default,
        (@munch
            vis: [$vis:vis]
            name: [$style_name:ident]
            new: [$(($new_field:ident, $new_key:ident, $new_content:ty, $new_default:expr))*]
            use: [$(($use_field:ident, $use_key:ident, $use_content:ty, $use_default:expr))*]
            rest: [$field:ident : $key:ident (use $content:ty) = $default:expr, $($rest:tt)*]
        ) => {
            $crate::ui::style::properties::macros::declare_style!(@munch
                vis: [$vis]
                name: [$style_name]
                new: [$(($new_field, $new_key, $new_content, $new_default))*]
                use: [$(($use_field, $use_key, $use_content, $use_default))* ($field, $key, $content, $default)]
                rest: [$($rest)*]
            );
        };

        // Munch a "new" variant: field: Key(Type) = default,
        (@munch
            vis: [$vis:vis]
            name: [$style_name:ident]
            new: [$(($new_field:ident, $new_key:ident, $new_content:ty, $new_default:expr))*]
            use: [$(($use_field:ident, $use_key:ident, $use_content:ty, $use_default:expr))*]
            rest: [$field:ident : $key:ident ($content:ty) = $default:expr, $($rest:tt)*]
        ) => {
            $crate::ui::style::properties::macros::declare_style!(@munch
                vis: [$vis]
                name: [$style_name]
                new: [$(($new_field, $new_key, $new_content, $new_default))* ($field, $key, $content, $default)]
                use: [$(($use_field, $use_key, $use_content, $use_default))*]
                rest: [$($rest)*]
            );
        };

        // Terminal case: empty rest, emit the final code
        (@munch
            vis: [$vis:vis]
            name: [$style_name:ident]
            new: [$(($new_field:ident, $new_key:ident, $new_content:ty, $new_default:expr))*]
            use: [$(($use_field:ident, $use_key:ident, $use_content:ty, $use_default:expr))*]
            rest: []
        ) => {
            // The resolved style struct with cached lookups
            #[derive(Clone, Debug)]
            $vis struct $style_name {
                $(
                    pub(crate) $new_field: StatefulProperty<$new_content>,
                )*
                $(
                    pub(crate) $use_field: StatefulProperty<$use_content>,
                )*
            }

            impl Default for $style_name {
                fn default() -> Self {
                    Self {
                        $(
                            $new_field: StatefulProperty::new($new_default),
                        )*
                        $(
                            $use_field: StatefulProperty::new($use_default),
                        )*
                    }
                }
            }

            impl $style_name {
                /// Apply a single property override for the given state.
                pub(crate) fn apply(&mut self, flags: StateFlags, prop: StyleProperty) {
                    match prop {
                        $(
                            StyleProperty::$new_key(value) => {
                                self.$new_field.set(flags, value);
                            }
                        )*
                        $(
                            StyleProperty::$use_key(value) => {
                                self.$use_field.set(flags, value);
                            }
                        )*
                    }
                }

                /// Apply multiple property overrides.
                pub(crate) fn apply_all(&mut self, properties: impl IntoIterator<Item = (StateFlags, StyleProperty)>) {
                    for (flags, prop) in properties {
                        self.apply(flags, prop);
                    }
                }
            }

            // Enum for dynamically specifying property values
            #[derive(Clone, Debug, PartialEq)]
            $vis enum StyleProperty {
                $(
                    $new_key($new_content),
                )*
                $(
                    $use_key($use_content),
                )*
            }

            // New zero-sized type keys
            $(
                $vis struct $new_key;

                impl PropertyKey for $new_key {
                    type Value = $new_content;

                    fn get(style: &$style_name, state: StateFlags) -> Self::Value {
                        style.$new_field.get(state)
                    }
                }

                impl $crate::sealed::Sealed for $new_key {}
            )*

            // Trait impl for existing types (no struct definition)
            $(
                impl PropertyKey for $use_key {
                    type Value = $use_content;

                    fn get(style: &$style_name, state: StateFlags) -> Self::Value {
                        style.$use_field.get(state)
                    }
                }

                impl $crate::sealed::Sealed for $use_key {}
            )*
        };
    }

    pub(crate) use declare_style;
}
