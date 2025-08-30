use std::hash::Hash;
use std::time::Duration;

use crate::graphics::Color;
use crate::graphics::text::TextLayoutContext;
use crate::graphics::text::TextStyle;

use super::Alignment;
use super::Atom;
use super::Flex;
use super::InputState;
use super::LayoutContent;
use super::LayoutDirection;
use super::Padding;
use super::Response;
use super::Size;
use super::UiContext;
use super::UiElementId;
use super::Widget;
use super::WidgetId;
use super::WidgetState;

pub struct UiBuilder<'a> {
    pub(super) id: WidgetId,
    pub(super) index: UiElementId,
    pub(super) context: &'a mut UiContext,
    pub(super) text_context: &'a mut TextLayoutContext,

    pub(super) num_child_widgets: usize,
}

impl UiBuilder<'_> {
    pub fn add(&mut self, widget: impl Widget) -> Response {
        widget.apply(self)
    }

    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.ui_tree.content_mut(self.index).0 = LayoutContent::Fill {
            color: color.into(),
        };

        self
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).width = width.into();
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).height = height.into();
        self
    }

    pub fn size(&mut self, width: impl Into<Size>, height: impl Into<Size>) -> &mut Self {
        let atom = self.context.ui_tree.atom_mut(self.index);
        atom.width = width.into();
        atom.height = height.into();
        self
    }

    pub fn child_major_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).major_align = alignment;
        self
    }

    pub fn child_minor_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).minor_align = alignment;
        self
    }

    pub fn child_alignment(&mut self, major: Alignment, minor: Alignment) -> &mut Self {
        let node = self.context.ui_tree.atom_mut(self.index);
        node.major_align = major;
        node.minor_align = minor;
        self
    }

    pub fn child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).direction = direction;
        self
    }

    pub fn child_spacing(&mut self, spacing: f32) -> &mut Self {
        self.context
            .ui_tree
            .atom_mut(self.index)
            .inter_child_padding = spacing;
        self
    }

    pub fn padding(&mut self, padding: Padding) -> &mut Self {
        self.context.ui_tree.atom_mut(self.index).inner_padding = padding;
        self
    }

    pub fn prev_state(&self) -> Option<&WidgetState> {
        self.context
            .widget_states
            .get(&self.id)
            .map(|container| &container.state)
    }

    pub fn rect(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: impl Into<Color>,
    ) -> &mut Self {
        self.context.ui_tree.add(
            Some(self.index),
            Atom {
                width: width.into(),
                height: height.into(),
                ..Default::default()
            },
            (
                LayoutContent::Fill {
                    color: color.into(),
                },
                None,
            ),
        );

        self
    }

    pub fn label(&mut self, text: &str, style: &TextStyle, height: impl Into<Size>) -> &mut Self {
        let mut layout = parley::Layout::new();

        let mut compute =
            self.text_context
                .layouts
                .ranged_builder(&mut self.text_context.fonts, text, 1.0, true);

        style.as_defaults(&mut compute);
        compute.build_into(&mut layout, text);

        let size = layout.calculate_content_widths();

        self.context.ui_tree.add(
            Some(self.index),
            Atom {
                width: Flex {
                    min: size.min,
                    max: size.max,
                },
                height: height.into(),
                ..Default::default()
            },
            (
                LayoutContent::Text {
                    layout,
                    alignment: style.align,
                },
                None,
            ),
        );

        self
    }

    pub fn container(&mut self) -> UiBuilder<'_> {
        let container_index = self.context.ui_tree.add(
            Some(self.index),
            Atom::default(),
            (LayoutContent::None, None),
        );

        UiBuilder {
            id: self.id,
            context: self.context,
            index: container_index,
            text_context: self.text_context,
            num_child_widgets: 0,
        }
    }

    pub fn child(&mut self) -> UiBuilder<'_> {
        let child_index = self.context.ui_tree.add(
            Some(self.index),
            Atom::default(),
            (LayoutContent::None, None),
        );

        self.num_child_widgets += 1;
        UiBuilder {
            id: self.id.then(self.num_child_widgets),
            context: self.context,
            index: child_index,
            text_context: self.text_context,
            num_child_widgets: 0,
        }
    }

    pub fn named_child(&mut self, name: impl Hash) -> UiBuilder<'_> {
        let child_id = self.id.then(name);

        let child_index = self.context.ui_tree.add(
            Some(self.index),
            Atom::default(),
            (LayoutContent::None, Some(child_id)),
        );

        self.num_child_widgets += 1;
        UiBuilder {
            id: child_id,
            context: self.context,
            index: child_index,
            text_context: self.text_context,
            num_child_widgets: 0,
        }
    }

    pub fn with_child(&mut self, callback: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        callback(&mut self.child());
        self
    }

    pub fn with_named_child(
        &mut self,
        name: impl Hash,
        callback: impl FnOnce(&mut UiBuilder),
    ) -> &mut Self {
        callback(&mut self.named_child(name));
        self
    }
}
