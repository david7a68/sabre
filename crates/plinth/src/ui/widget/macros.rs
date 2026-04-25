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
