use std::time::Duration;

use glamour::Contains;
use glamour::Point2;
use glamour::Rect;
use glamour::Size2;

use crate::graphics::Canvas;
use crate::graphics::Color;
use crate::graphics::GradientPaint;
use crate::graphics::Paint;
use crate::graphics::Primitive;
use crate::graphics::TextAlignment;
use crate::graphics::TextLayoutContext;
use crate::shell::Clipboard;
use crate::shell::Input;
use crate::ui::theme::Theme;

use super::Atom;
use super::IdMap;
use super::LayoutTree;
use super::Position;
use super::StyleClass;
use super::UiBuilder;
use super::UiElementId;
use super::WidgetId;
use super::layout::NodeLayout;
use super::style::BorderWidths;
use super::style::CornerRadii;
use super::text::TextLayoutId;
use super::text::TextLayoutMut;
use super::text::TextLayoutStorage;
use super::text::TextOverflow;
use super::widget::WidgetState;

#[derive(Default)]
pub(crate) struct UiContext {
    pub(super) time_delta: Duration,

    pub(super) ui_tree: LayoutTree<(LayoutContent, Option<WidgetId>)>,
    pub(super) widget_states: IdMap<WidgetContainer>,

    pub(super) frame_counter: u64,
    pub(super) focused_widget: Option<WidgetId>,

    /// The highest z_layer that contains any widget whose previous-frame placement
    /// contains the current pointer position. Computed at the start of each frame.
    /// Used by `Interaction::compute` to suppress hover on lower layers.
    pub(super) active_pointer_layer: u8,

    /// If any modal overlay was visible last frame, this is its z_layer.
    /// Widgets on layers *strictly below* (not equal to) this value are input-blocked
    /// regardless of pointer position. Strict-less-than is intentional: the modal
    /// overlay's own interactive children sit at the same z_layer and must still
    /// receive input. Code that consumes this field must use `layer < input_block_layer`,
    /// never `layer <= input_block_layer`.
    pub(super) input_block_layer: Option<u8>,
}

impl UiContext {
    #[expect(clippy::too_many_arguments)]
    pub(crate) fn begin_frame<'a>(
        &'a mut self,
        clipboard: &'a mut Clipboard,
        text_context: &'a mut TextLayoutContext,
        text_layouts: &'a mut TextLayoutStorage,
        format_buffer: &'a mut String,
        theme: &'a Theme,
        input: &'a Input,
        time_delta: Duration,
    ) -> UiBuilder<'a> {
        self.ui_tree.clear();

        // Single pass over previous-frame widget states to compute both layer gates.
        let mut active_pointer_layer = 0u8;
        let mut input_block_layer: Option<u8> = None;
        for wc in self.widget_states.values() {
            let s = &wc.state;
            if s.placement.contains(&input.pointer) && s.layer > active_pointer_layer {
                active_pointer_layer = s.layer;
            }
            if s.is_modal && input_block_layer.is_none_or(|cur| s.layer > cur) {
                input_block_layer = Some(s.layer);
            }
        }
        self.active_pointer_layer = active_pointer_layer;
        self.input_block_layer = input_block_layer;

        // Set up the root node.
        let id = WidgetId::new("root");

        let root = self.ui_tree.add(
            None,
            Atom {
                width: input.window_size.width.into(),
                height: input.window_size.height.into(),
                ..Default::default()
            },
            (
                LayoutContent::Fill {
                    paint: Paint::solid(Color::WHITE),
                    border: GradientPaint::vertical_gradient(Color::BLACK, Color::BLACK),
                    border_width: Default::default(),
                    corner_radii: Default::default(),
                },
                Some(id),
            ),
        );

        self.time_delta = time_delta;

        UiBuilder {
            theme,
            input,
            context: self,

            clipboard,
            text_context,
            text_layouts,
            format_buffer,

            id,
            index: root,
            style_id: theme.get_id(StyleClass::Surface),
            state: Default::default(),
            num_child_widgets: 0,

            layer: 0,
            is_modal: false,
            text_overflow: TextOverflow::Clip,
        }
    }

    pub fn state_mut(&mut self, widget_id: WidgetId) -> &mut WidgetState {
        let container = self
            .widget_states
            .entry(widget_id)
            .or_insert_with(|| WidgetContainer {
                state: WidgetState::default(),
                frame_last_used: self.frame_counter,
            });

        &mut container.state
    }

    pub fn static_text_layout<'a>(
        &mut self,
        text_layouts: &'a mut TextLayoutStorage,
        widget_id: WidgetId,
    ) -> (TextLayoutId, &'a mut super::text::StaticTextLayout) {
        let state = self.state_mut(widget_id);

        let static_layout_id = state.text_layout.map(|TextLayoutId::Static(id)| id);

        let (layout_id, layout) = text_layouts.get_or_create_static(static_layout_id);
        state.text_layout = Some(TextLayoutId::Static(layout_id));

        (TextLayoutId::Static(layout_id), layout)
    }

    /// Insert an out-of-flow overlay node into the layout tree and return its index.
    ///
    /// Extracted so callers that need to move ownership of a parent `UiBuilder` (e.g.
    /// the dropdown's overlay panel) can call this on `&mut context` directly, avoiding
    /// the need to duplicate the `Atom` setup that `overlay_child_inner` would otherwise
    /// provide via `&mut self`.
    pub(super) fn add_overlay_node(
        &mut self,
        parent: UiElementId,
        id: WidgetId,
        position: Position,
        child_layer: u8,
        is_modal: bool,
    ) -> UiElementId {
        self.ui_tree.add(
            Some(parent),
            Atom {
                position,
                z_layer: child_layer,
                is_modal,
                ..Default::default()
            },
            (LayoutContent::None, Some(id)),
        )
    }

    pub fn finish(
        &mut self,
        text_context: &mut TextLayoutContext,
        text_layouts: &mut TextLayoutStorage,
        canvas: &mut Canvas,
    ) {
        self.ui_tree.compute_layout(|(content, _), max_width| {
            let (layout_id, alignment, overflow) = match content {
                LayoutContent::Text {
                    layout,
                    alignment,
                    overflow,
                    ..
                } => (layout, alignment, overflow),
                LayoutContent::EditableText { content, visuals } => {
                    return content.measure(text_context, max_width, visuals.alignment);
                }
                _ => return None,
            };

            text_layouts.break_lines(text_context, *layout_id, max_width, *alignment, *overflow)
        });

        for (node, (content, widget_id)) in self.ui_tree.iter_nodes_by_layer() {
            let layout = &node.result;
            if layout.width == 0.0 || layout.height == 0.0 {
                continue;
            }

            match content {
                LayoutContent::None => {}
                LayoutContent::Fill {
                    paint,
                    border,
                    border_width,
                    corner_radii,
                } => {
                    canvas.draw(Primitive {
                        point: [layout.x, layout.y],
                        size: [layout.width, layout.height],
                        clip: node.result.effective_clip,
                        paint: paint.clone(),
                        border: *border,
                        border_width: border_width.into_array(),
                        corner_radii: corner_radii.into_array(),
                        use_nearest_sampling: false,
                    });
                }
                LayoutContent::Text {
                    layout: text_layout_id,
                    alignment: _,
                    overflow: _,
                } => match text_layouts.get_mut(*text_layout_id) {
                    None => {}
                    Some(TextLayoutMut::Static(text_layout)) => {
                        canvas.draw_text_layout(
                            text_layout,
                            [layout.x, layout.y],
                            node.result.effective_clip,
                        );
                    }
                },
                LayoutContent::EditableText { content, visuals } => {
                    content.draw(text_context, canvas, layout, *visuals);
                }
            }

            if let Some(widget_id) = widget_id {
                let container = self.widget_states.entry(*widget_id).or_default();

                container.frame_last_used = self.frame_counter;
                container.state.placement = Rect {
                    origin: Point2 {
                        x: node.result.x,
                        y: node.result.y,
                    },
                    size: Size2 {
                        width: node.result.width,
                        height: node.result.height,
                    },
                };
                container.state.layer = node.atom.z_layer;
                container.state.is_modal = node.atom.is_modal;
            }
        }

        let removed = self
            .widget_states
            .extract_if(|_, container| container.frame_last_used < self.frame_counter);

        for (_, element) in removed {
            if let Some(text_layout_id) = element.state.text_layout {
                text_layouts.remove(text_layout_id);
            }
        }

        if self.widget_states.len() * 2 < self.widget_states.capacity() {
            self.widget_states.shrink_to_fit();
        }

        self.frame_counter += 1;
    }
}

#[derive(Clone, Copy)]
pub(super) struct EditableTextVisuals {
    pub(super) alignment: TextAlignment,
    pub(super) cursor_size: f32,
    pub(super) selection_color: Color,
    pub(super) cursor_color: Color,
}

pub(super) trait EditableTextContent {
    fn measure(
        &self,
        text_context: &mut TextLayoutContext,
        max_width: f32,
        alignment: TextAlignment,
    ) -> Option<f32>;

    fn draw(
        &self,
        text_context: &mut TextLayoutContext,
        canvas: &mut Canvas,
        layout: &NodeLayout,
        visuals: EditableTextVisuals,
    );
}

#[derive(Default)]
pub(super) struct WidgetContainer {
    pub(super) state: WidgetState,
    pub(super) frame_last_used: u64,
}

pub(super) enum LayoutContent {
    None,
    Fill {
        paint: Paint,
        border: GradientPaint,
        border_width: BorderWidths,
        corner_radii: CornerRadii,
    },
    Text {
        layout: TextLayoutId,
        alignment: TextAlignment,
        overflow: TextOverflow,
    },
    EditableText {
        content: std::rc::Rc<dyn EditableTextContent>,
        visuals: EditableTextVisuals,
    },
}
