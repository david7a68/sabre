use std::hash::Hash;

use glamour::Contains;
use glamour::Rect;

use crate::graphics::Texture;
use crate::ui::Pixels;
use crate::ui::text::TextLayoutId;

use super::Alignment;
use super::LayoutDirection;
use super::Size;
use super::UiBuilder;
use super::style::StateFlags;

mod button;
mod frame;
mod horizontal_separator;
mod image;
mod label;
mod surface;
mod text_edit;
mod vertical_separator;

pub use button::Button;
pub use frame::Frame;
pub use horizontal_separator::HorizontalSeparator;
pub use image::Image;
pub use label::Label;
pub use surface::Surface;
pub use text_edit::TextEdit;
pub use vertical_separator::VerticalSeparator;

use macros::*;

#[derive(Clone, Copy, Debug)]
pub struct Interaction {
    pub is_activated: bool,
    pub is_hovered: bool,
    pub is_focused: bool,
}

pub trait Container<'a>: Sized {
    fn builder_mut(&mut self) -> &mut UiBuilder<'a>;

    fn child<'this>(&'this mut self) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.builder_mut().child()
    }

    fn named_child<'this>(&'this mut self, name: impl Hash) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.builder_mut().named_child(name)
    }

    fn child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.builder_mut().child_direction(direction);
        self
    }

    fn with_child_direction(mut self, direction: LayoutDirection) -> Self {
        self.child_direction(direction);
        self
    }

    fn child_alignment(&mut self, major: Alignment, minor: Alignment) -> &mut Self {
        self.builder_mut().child_alignment(major, minor);
        self
    }

    fn with_child_alignment(mut self, major: Alignment, minor: Alignment) -> Self {
        self.child_alignment(major, minor);
        self
    }
}

impl<'a> Container<'a> for UiBuilder<'a> {
    fn builder_mut(&mut self) -> &mut UiBuilder<'a> {
        self
    }

    fn child<'this>(&'this mut self) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.child()
    }

    fn named_child<'this>(&'this mut self, name: impl Hash) -> UiBuilder<'this>
    where
        'a: 'this,
    {
        self.named_child(name)
    }
}

pub trait UiBuilderWidgetsExt<'a>: Container<'a> {
    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn frame<'this>(&'this mut self) -> frame::Frame<'this>
    where
        'a: 'this,
    {
        frame::Frame::new(self.builder_mut())
    }

    /// Creates an invisible, non-interactive layout widget for grouping other
    /// widgets together.
    fn with_frame(&mut self, callback: impl FnOnce(frame::Frame<'_>)) -> &mut Self {
        let container = self.frame();
        callback(container);
        self
    }

    fn image(&mut self, texture: &Texture, width: Size) {
        image::Image::new(self.builder_mut(), texture)
            .with_width(width)
            .finish()
    }

    fn surface<'this>(&'this mut self) -> surface::Surface<'this>
    where
        'a: 'this,
    {
        surface::Surface::new(self.builder_mut())
    }

    fn with_surface(&mut self, callback: impl FnOnce(surface::Surface<'_>)) -> &mut Self {
        let panel = self.surface();
        callback(panel);
        self
    }

    fn text_button(&mut self, label: &str) -> Interaction {
        button::Button::new(self.builder_mut(), Some(label)).finish()
    }

    fn text_edit<'this>(
        &'this mut self,
        initial_text: &str,
        width: f32,
    ) -> text_edit::TextEdit<'this>
    where
        'a: 'this,
    {
        text_edit::TextEdit::new(self.builder_mut(), Size::Fixed(width)).default_text(initial_text)
    }

    fn label<'this>(&'this mut self, text: &str) -> label::Label<'this>
    where
        'a: 'this,
    {
        label::Label::new(self.builder_mut(), text)
    }

    fn horizontal_separator<'this>(
        &'this mut self,
    ) -> horizontal_separator::HorizontalSeparator<'this>
    where
        'a: 'this,
    {
        horizontal_separator::HorizontalSeparator::new(self.builder_mut())
    }

    fn vertical_separator<'this>(&'this mut self) -> vertical_separator::VerticalSeparator<'this>
    where
        'a: 'this,
    {
        vertical_separator::VerticalSeparator::new(self.builder_mut())
    }
}

impl<'a, C: Container<'a>> UiBuilderWidgetsExt<'a> for C {}

/// Controls when a click is registered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClickBehavior {
    /// Click fires immediately when mouse is pressed (more responsive).
    #[default]
    OnPress,
    /// Click fires when mouse is released while still hovered (standard UI behavior).
    OnRelease,
}

impl Interaction {
    /// Compute interaction state for a widget.
    ///
    /// Returns the interaction result and whether the widget is currently active (being pressed).
    pub fn compute(
        builder: &UiBuilder<'_>,
        behavior: ClickBehavior,
        interest: StateFlags,
    ) -> (Self, StateFlags) {
        let was_focused = builder.is_focused();
        let (was_active, is_hovered) = builder
            .prev_state()
            .map(|s| (s.was_active, s.placement.contains(&builder.input.pointer)))
            .unwrap_or_default();

        let is_left_down = builder.input.mouse_state.is_left_down();
        let just_pressed = is_left_down && !was_active;
        let just_released = !is_left_down && was_active;

        let is_activated = match behavior {
            ClickBehavior::OnPress => is_hovered && just_pressed,
            ClickBehavior::OnRelease => is_hovered && just_released,
        };

        let mut state = StateFlags::NORMAL;
        if is_hovered {
            state |= StateFlags::HOVERED & interest;
        }
        if is_hovered && is_left_down {
            state |= StateFlags::PRESSED & interest;
        }
        if is_activated || ((is_hovered || !just_pressed) && was_focused) {
            state |= StateFlags::FOCUSED & interest;
        }

        (
            Self {
                is_activated,
                is_hovered,
                is_focused: state.contains(StateFlags::FOCUSED),
            },
            state,
        )
    }
}

#[derive(Default)]
pub struct WidgetState {
    pub placement: Rect<Pixels>,
    /// Whether the widget was being actively pressed last frame
    pub was_active: bool,

    pub text_layout: Option<TextLayoutId>,
}

mod macros {
    /// Macro to forward builder property methods from a widget struct to its
    /// internal UiBuilder.
    ///
    /// Each method is forwarded in two forms: `property` takes `&mut self` and
    /// returns `&mut Self` for chaining, and `with_property` takes `self` by
    /// value and returns `Self` for builder-style use.
    ///
    /// Supported properties:
    ///
    /// - color (with_color)
    /// - width (with_width)
    /// - height (with_height)
    /// - size (with_size)
    /// - padding (with_padding)
    macro_rules! forward_properties {
        ($($method:ident),+) => {
            $crate::ui::widget::macros::forward_properties!(@impl $($method),+);
        };
        (@impl color $(, $method:ident)*) => {
            pub fn color(&mut self, color: impl Into<$crate::graphics::Color>) -> &mut Self {
                self.builder.color(color.into());
                self
            }

            pub fn with_color(mut self, color: impl Into<$crate::graphics::Color>) -> Self {
                self.color(color);
                self
            }

            $crate::ui::widget::macros::forward_properties!(@impl $($method),*);
        };
        (@impl width $(, $method:ident)*) => {
            pub fn width(&mut self, width: impl Into<$crate::ui::Size>) -> &mut Self {
                self.builder.width(width);
                self
            }

            pub fn with_width(mut self, width: impl Into<$crate::ui::Size>) -> Self {
                self.width(width);
                self
            }

            $crate::ui::widget::macros::forward_properties!(@impl $($method),*);
        };
        (@impl height $(, $method:ident)*) => {
            pub fn height(&mut self, height: impl Into<$crate::ui::Size>) -> &mut Self {
                self.builder.height(height);
                self
            }

            pub fn with_height(mut self, height: impl Into<$crate::ui::Size>) -> Self {
                self.height(height);
                self
            }

            $crate::ui::widget::macros::forward_properties!(@impl $($method),*);
        };
        (@impl size $(, $method:ident)*) => {
            pub fn size(&mut self, width: impl Into<$crate::ui::Size>, height: impl Into<$crate::ui::Size>) -> &mut Self {
                self.builder.size(width, height);
                self
            }

            pub fn with_size(mut self, width: impl Into<$crate::ui::Size>, height: impl Into<$crate::ui::Size>) -> Self {
                self.size(width, height);
                self
            }

            $crate::ui::widget::macros::forward_properties!(@impl $($method),*);
        };
        (@impl padding $(, $method:ident)*) => {
            pub fn padding(&mut self, padding: $crate::ui::Padding) -> &mut Self {
                self.builder.padding(padding);
                self
            }

            pub fn with_padding(mut self, padding: $crate::ui::Padding) -> Self {
                self.padding(padding);
                self
            }

            $crate::ui::widget::macros::forward_properties!(@impl $($method),*);
        };
        (@impl ) => {};
    }

    pub(crate) use forward_properties;

    macro_rules! impl_container {
        ($type:ident < $($lt:lifetime),+>) => {
            impl <$($lt),+> $crate::ui::widget::Container<$($lt),+> for $type <$($lt),+> {
                fn builder_mut(&mut self) -> &mut UiBuilder<$($lt),+> {
                    &mut self.builder
                }
            }

            impl <$($lt),+> $type <$($lt),+> {
                pub fn child_direction(&mut self, direction: $crate::ui::LayoutDirection) -> &mut Self {
                    use $crate::ui::widget::Container;

                    self.builder_mut().child_direction(direction);
                    self
                }

                pub fn with_child_direction(mut self, direction: $crate::ui::LayoutDirection) -> Self {

                    self.child_direction(direction);
                    self
                }

                pub fn child_alignment(&mut self, major: $crate::ui::Alignment, minor: $crate::ui::Alignment) -> &mut Self {
                    use $crate::ui::widget::Container;

                    self.builder_mut().child_alignment(major, minor);
                    self
                }

                pub fn with_child_alignment(mut self, major: $crate::ui::Alignment, minor: $crate::ui::Alignment) -> Self {
                    self.child_alignment(major, minor);
                    self
                }
            }
        };
    }

    pub(crate) use impl_container;
}
