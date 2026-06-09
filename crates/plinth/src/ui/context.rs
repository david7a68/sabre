use std::rc::Rc;
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
use crate::shell::Clipboard;
use crate::shell::Input;
use crate::ui::theme::Theme;

use super::Atom;
use super::IdMap;
use super::LayoutTree;
use super::Pixels;
use super::Position;
use super::StyleClass;
use super::UiBuilder;
use super::UiElementId;
use super::WidgetId;
use super::style::BorderWidths;
use super::style::CornerRadii;
use super::text::StaticTextLayoutId;
use super::text::TextOverflow;
use super::text::TextServices;
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
    pub(crate) fn begin_frame<'a>(
        &'a mut self,
        clipboard: &'a mut Clipboard,
        text_services: TextServices,
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
            text_services,
            format_buffer,

            id,
            index: root,
            style_id: theme.get_id(StyleClass::Surface),
            state: Default::default(),
            num_child_widgets: 0,

            layer: 0,
            is_modal: false,
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

    pub(crate) fn static_text_layout(
        &mut self,
        widget_id: WidgetId,
        text_services: &TextServices,
    ) -> StaticTextLayoutId {
        let state = self.state_mut(widget_id);
        match state.static_text_layout {
            Some(id) => id,
            None => {
                let id = text_services.create_static_text_layout();
                state.static_text_layout = Some(id);
                id
            }
        }
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

    pub fn finish(&mut self, text_services: &TextServices, canvas: &mut Canvas) {
        self.ui_tree
            .compute_layout(|(content, _), max_width| match content {
                LayoutContent::StaticText {
                    layout,
                    alignment,
                    overflow,
                } => text_services.measure_static_text_layout(
                    *layout, max_width, *alignment, *overflow,
                ),
                LayoutContent::Custom(custom) => custom.measure(max_width),
                _ => None,
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
                LayoutContent::StaticText {
                    layout: text_layout,
                    ..
                } => {
                    text_services.draw_static_text_layout(
                        *text_layout,
                        canvas,
                        [layout.x, layout.y],
                        node.result.effective_clip,
                    );
                }
                LayoutContent::Custom(content) => {
                    content.draw(
                        canvas,
                        Rect::from_origin_and_size(
                            (layout.x, layout.y),
                            (layout.width, layout.height),
                        ),
                        node.result.effective_clip,
                    );
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
        for (_, container) in removed {
            if let Some(id) = container.state.static_text_layout {
                text_services.remove_static_text_layout(id);
            }
        }

        if self.widget_states.len() * 2 < self.widget_states.capacity() {
            self.widget_states.shrink_to_fit();
        }

        self.frame_counter += 1;
    }
}

#[derive(Default)]
pub(super) struct WidgetContainer {
    pub(super) state: WidgetState,
    pub(super) frame_last_used: u64,
}

pub(crate) type LayoutRect = Rect<Pixels>;

pub(crate) trait UiContent {
    fn measure(&self, max_width: f32) -> Option<f32>;

    fn draw(&self, canvas: &mut Canvas, rect: LayoutRect, clip: crate::graphics::ClipRect);
}

pub(super) enum LayoutContent {
    None,
    Fill {
        paint: Paint,
        border: GradientPaint,
        border_width: BorderWidths,
        corner_radii: CornerRadii,
    },
    StaticText {
        layout: StaticTextLayoutId,
        alignment: TextAlignment,
        overflow: TextOverflow,
    },
    Custom(Rc<dyn UiContent>),
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::graphics::Canvas;
    use crate::graphics::ClipRect;
    use crate::shell::Clipboard;
    use crate::shell::Input;
    use crate::ui::Size;
    use crate::ui::text::TextBuilderExt;

    use super::*;

    struct MeasureProbe {
        widths: Rc<RefCell<Vec<f32>>>,
        height: f32,
    }

    impl UiContent for MeasureProbe {
        fn measure(&self, max_width: f32) -> Option<f32> {
            self.widths.borrow_mut().push(max_width);
            Some(self.height)
        }

        fn draw(&self, _canvas: &mut Canvas, _rect: LayoutRect, _clip: ClipRect) {}
    }

    #[test]
    fn custom_content_measurement_sets_fit_height() {
        let widths = Rc::new(RefCell::new(Vec::new()));
        let content: Rc<dyn UiContent> = Rc::new(MeasureProbe {
            widths: widths.clone(),
            height: 24.0,
        });
        let mut context = UiContext::default();
        let root = context.ui_tree.add(
            None,
            Atom {
                width: Size::Fixed(100.0),
                height: Size::Fixed(100.0),
                ..Default::default()
            },
            (LayoutContent::None, None),
        );
        context.ui_tree.add(
            Some(root),
            Atom {
                width: Size::Fixed(80.0),
                height: Size::Fit {
                    min: 0.0,
                    max: f32::MAX,
                },
                ..Default::default()
            },
            (LayoutContent::Custom(content), None),
        );

        context
            .ui_tree
            .compute_layout(|(content, _), max_width| match content {
                LayoutContent::Custom(content) => content.measure(max_width),
                _ => None,
            });

        let heights = context
            .ui_tree
            .iter_nodes_by_layer()
            .map(|(node, _)| node.result.height)
            .collect::<Vec<_>>();
        assert_eq!(widths.borrow().as_slice(), &[80.0]);
        assert_eq!(heights[1], 24.0);
    }

    #[test]
    fn static_text_layout_is_cached_across_frames() {
        let services = TextServices::default();
        let theme = Theme::default();
        let input = Input::default();
        let mut clipboard = Clipboard::new();
        let mut format_buffer = String::new();
        let mut context = UiContext::default();

        {
            let mut builder = context.begin_frame(
                &mut clipboard,
                services.clone(),
                &mut format_buffer,
                &theme,
                &input,
                Duration::ZERO,
            );
            builder
                .named_child("label")
                .text("hello", Size::Fixed(20.0));
        }

        assert_eq!(services.static_text_layout_count(), 1);
        assert_eq!(services.static_text_rebuild_count(), 1);

        {
            let mut builder = context.begin_frame(
                &mut clipboard,
                services.clone(),
                &mut format_buffer,
                &theme,
                &input,
                Duration::ZERO,
            );
            builder
                .named_child("label")
                .text("hello", Size::Fixed(20.0));
        }

        assert_eq!(services.static_text_layout_count(), 1);
        assert_eq!(services.static_text_rebuild_count(), 1);
    }
}
