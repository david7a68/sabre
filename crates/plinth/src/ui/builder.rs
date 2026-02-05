use std::hash::Hash;
use std::time::Duration;

use rapidhash::v3::rapidhash_v3;

use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;

use super::Alignment;
use super::Atom;
use super::Flex;
use super::Input;
use super::LayoutDirection;
use super::Padding;
use super::Size;
use super::UiElementId;
use super::WidgetId;
use super::context::LayoutContent;
use super::context::UiContext;
use super::style::BorderWidths;
use super::style::CornerRadii;
use super::style::StateFlags;
use super::style::StyleId;
use super::theme::StyleClass;
use super::theme::Theme;
use super::widget::WidgetState;

/// Compute a hash of a string for cache invalidation
fn hash_string(text: &str) -> u64 {
    rapidhash_v3(text.as_bytes())
}

pub struct UiBuilder<'a> {
    pub(super) id: WidgetId,
    pub(super) index: UiElementId,
    pub(super) theme: &'a Theme,
    pub(super) input: &'a Input,
    pub(super) context: &'a mut UiContext,
    pub(super) format_buffer: &'a mut String,
    pub(super) text_context: &'a mut TextLayoutContext,

    pub(super) style_id: StyleId,
    pub(super) style_state: StateFlags,
    pub(super) num_child_widgets: usize,
}

impl UiBuilder<'_> {
    pub fn input(&self) -> &Input {
        self.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn theme(&self) -> &Theme {
        self.theme
    }

    pub fn apply_style(&mut self, class: StyleClass, state: StateFlags) -> &mut Self {
        let style = self.theme.get(class);

        // Paint
        let paint = style.background.get(state);
        let border = style.border.get(state);
        let border_width = style.border_widths.get(state);
        let corner_radii = style.corner_radii.get(state);
        self.paint(paint, border, border_width, corner_radii);

        // Layout
        let major_align = style.child_major_alignment.get(state);
        let minor_align = style.child_minor_alignment.get(state);
        let padding = style.padding.get(state);
        let spacing = style.child_spacing.get(state);
        let direction = style.child_direction.get(state);

        self.style_id = self.theme.get_id(class);
        self.style_state = state;

        self.child_alignment(major_align, minor_align)
            .child_spacing(spacing)
            .child_direction(direction)
            .padding(padding)
    }

    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        let content = &mut self.context.ui_tree.content_mut(self.index).0;

        match content {
            LayoutContent::Fill { paint, .. } => {
                *paint = Paint::solid(color.into());
            }
            _ => {
                *content = LayoutContent::Fill {
                    paint: Paint::solid(color.into()),
                    border: GradientPaint::default(),
                    border_width: Default::default(),
                    corner_radii: Default::default(),
                };
            }
        }

        self
    }

    pub fn paint(
        &mut self,
        paint: Paint,
        border: GradientPaint,
        border_width: BorderWidths,
        corner_radii: CornerRadii,
    ) -> &mut Self {
        self.context.ui_tree.content_mut(self.index).0 = LayoutContent::Fill {
            paint,
            border,
            border_width,
            corner_radii,
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

    /// Set whether this widget is currently being actively pressed.
    /// Used for click detection across frames.
    pub fn set_active(&mut self, active: bool) {
        // container state will get created on the first frame that a widget is
        // used, but AFTER the widget's layout is computed (and thus after all
        // opportunities to call this method within the current frame have
        // elapsed). Therefore it is safe to do nothing if the widget state does
        // not exist yet.
        if let Some(widget) = self.context.widget_states.get_mut(&self.id) {
            widget.state.was_active = active;
        }
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
                    paint: Paint::solid(color.into()),
                    border: GradientPaint::default(),
                    border_width: Default::default(),
                    corner_radii: Default::default(),
                },
                None,
            ),
        );

        self
    }

    pub fn set_text(&mut self, text: &str, height: impl Into<Size>) -> &mut Self {
        let (text_layout_id, static_text_layout) = self.context.upsert_static_text_layout(self.id);

        let text_hash = hash_string(text);

        let needs_rebuild = static_text_layout.style_id != self.style_id
            || static_text_layout.state_flags != self.style_state
            || static_text_layout.text_hash != text_hash;

        if needs_rebuild {
            let mut builder = self.text_context.layouts.ranged_builder(
                &mut self.text_context.fonts,
                text,
                1.0,
                true,
            );

            self.theme
                .push_text_defaults(self.style_id, self.style_state, &mut builder);
            builder.build_into(&mut static_text_layout.layout, text);

            // Update cache tracking fields
            static_text_layout.style_id = self.style_id;
            static_text_layout.state_flags = self.style_state;
            static_text_layout.text_hash = text_hash;
            static_text_layout.needs_line_break = true;
        }

        let alignment = self
            .theme
            .resolve_style::<TextAlignment>(self.style_id, self.style_state);
        let size = static_text_layout.layout.calculate_content_widths();

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
                    layout: text_layout_id,
                    alignment,
                    selection_rects: None,
                    cursor_rect: None,
                    selection_color: None,
                    cursor_color: None,
                },
                None,
            ),
        );

        self
    }

    pub fn child(&mut self) -> UiBuilder<'_> {
        self.named_child(self.num_child_widgets + 1)
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
            theme: self.theme,
            input: self.input,
            context: self.context,
            index: child_index,
            format_buffer: self.format_buffer,
            text_context: self.text_context,
            style_id: self.style_id,
            style_state: self.style_state,
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

    /// Check if this widget currently has focus
    pub fn is_focused(&self) -> bool {
        self.context.focused_widget == Some(self.id)
    }

    /// Request focus for this widget
    pub fn request_focus(&mut self) {
        self.context.focused_widget = Some(self.id);
    }

    /// Release focus if this widget has it
    pub fn release_focus(&mut self) {
        if self.context.focused_widget == Some(self.id) {
            self.context.focused_widget = None;
        }
    }
}
