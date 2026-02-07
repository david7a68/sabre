use std::time::Duration;

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
use crate::ui::theme::Theme;

use super::Atom;
use super::IdMap;
use super::Input;
use super::LayoutTree;
use super::StyleClass;
use super::UiBuilder;
use super::WidgetId;
use super::style::BorderWidths;
use super::style::CornerRadii;
use super::text::StaticTextLayout;
use super::text::TextLayoutId;
use super::text::TextLayoutMut;
use super::text::TextLayoutStorage;
use super::widget::WidgetState;

#[derive(Default)]
pub(crate) struct UiContext {
    pub(super) time_delta: Duration,

    pub(super) ui_tree: LayoutTree<(LayoutContent, Option<WidgetId>)>,
    pub(super) widget_states: IdMap<WidgetContainer>,
    pub(super) text_layouts: TextLayoutStorage,

    pub(super) frame_counter: u64,
    pub(super) focused_widget: Option<WidgetId>,
}

impl UiContext {
    pub(crate) fn begin_frame<'a>(
        &'a mut self,
        text_context: &'a mut TextLayoutContext,
        format_buffer: &'a mut String,
        theme: &'a Theme,
        input: &'a Input,
        time_delta: Duration,
    ) -> UiBuilder<'a> {
        self.ui_tree.clear();

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
            id,
            index: root,
            theme,
            input,
            context: self,
            text_context,
            format_buffer,
            style_id: theme.get_id(StyleClass::Panel),
            style_state: Default::default(),
            num_child_widgets: 0,
        }
    }

    pub fn upsert_static_text_layout(
        &mut self,
        widget_id: WidgetId,
    ) -> (TextLayoutId, &mut StaticTextLayout) {
        let container = self
            .widget_states
            .entry(widget_id)
            .or_insert_with(|| WidgetContainer {
                state: WidgetState {
                    placement: Default::default(),
                    was_active: false,
                    text_layout: None,
                },
                frame_last_used: self.frame_counter,
            });

        container.frame_last_used = self.frame_counter;

        let static_layout_id = match container.state.text_layout {
            Some(TextLayoutId::Static(id)) => Some(id),
            Some(other) => panic!("Widget has non-static text layout assigned: {other:?}"),
            None => None,
        };

        let (layout_id, layout) = self.text_layouts.get_or_create_static(static_layout_id);
        container.state.text_layout = Some(TextLayoutId::Static(layout_id));

        (TextLayoutId::Static(layout_id), layout)
    }

    pub fn upsert_dynamic_text_layout(
        &mut self,
        widget_id: WidgetId,
    ) -> (TextLayoutId, &mut super::text::DynamicTextLayout) {
        let container = self
            .widget_states
            .entry(widget_id)
            .or_insert_with(|| WidgetContainer {
                state: WidgetState {
                    placement: Default::default(),
                    was_active: false,
                    text_layout: None,
                },
                frame_last_used: self.frame_counter,
            });

        container.frame_last_used = self.frame_counter;

        let dynamic_layout_id = match container.state.text_layout {
            Some(TextLayoutId::Dynamic(id)) => Some(id),
            Some(other) => panic!("Widget has non-dynamic text layout assigned: {other:?}"),
            None => None,
        };

        let (layout_id, layout) = self.text_layouts.get_or_create_dynamic(dynamic_layout_id);
        container.state.text_layout = Some(TextLayoutId::Dynamic(layout_id));

        (TextLayoutId::Dynamic(layout_id), layout)
    }

    pub fn finish(&mut self, text_context: &mut TextLayoutContext, canvas: &mut Canvas) {
        self.ui_tree.compute_layout(|(content, _), max_width| {
            let (layout_id, alignment) = match content {
                LayoutContent::Text {
                    layout, alignment, ..
                } => (layout, alignment),
                _ => return None,
            };

            self.text_layouts
                .break_lines(text_context, *layout_id, max_width, *alignment)
        });

        for (node, (content, widget_id)) in self.ui_tree.iter_nodes() {
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
                    cursor_size,
                    selection_color,
                    cursor_color,
                } => {
                    let Some(text_layout) = self.text_layouts.get_mut(*text_layout_id) else {
                        continue;
                    };

                    match text_layout {
                        TextLayoutMut::Static(text_layout) => {
                            canvas.draw_text_layout(text_layout, [layout.x, layout.y]);
                        }
                        TextLayoutMut::Dynamic(text_layout) => {
                            text_layout.editor.selection_geometry_with(|bbox, _| {
                                Self::draw_selection_rect(
                                    canvas,
                                    &bbox,
                                    *selection_color,
                                    layout.x,
                                    layout.y,
                                );
                            });

                            if let Some(rect) = text_layout.editor.cursor_geometry(*cursor_size) {
                                Self::draw_cursor(canvas, &rect, *cursor_color, layout.x, layout.y);
                            }

                            canvas.draw_text_layout(
                                text_layout
                                    .editor
                                    .layout(&mut text_context.fonts, &mut text_context.layouts),
                                [layout.x, layout.y],
                            );
                        }
                    }
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
            }
        }

        let removed = self
            .widget_states
            .extract_if(|_, container| container.frame_last_used < self.frame_counter);

        for (_, element) in removed {
            if let Some(text_layout_id) = element.state.text_layout {
                self.text_layouts.remove(text_layout_id);
            }
        }

        if self.widget_states.len() * 2 < self.widget_states.capacity() {
            self.widget_states.shrink_to_fit();
        }

        self.frame_counter += 1;
    }

    fn draw_selection_rect(
        canvas: &mut Canvas,
        rect: &parley::BoundingBox,
        color: Color,
        x: f32,
        y: f32,
    ) {
        let y0 = (y + rect.y0 as f32).round();
        let y1 = (y + rect.y1 as f32).round();

        canvas.draw(Primitive {
            point: [x + rect.x0 as f32, y0],
            size: [(rect.x1 - rect.x0) as f32, y1 - y0],
            paint: Paint::solid(color),
            border: GradientPaint::default(),
            border_width: [0.0; 4],
            corner_radii: [0.0; 4],
            use_nearest_sampling: false,
        });
    }

    fn draw_cursor(
        canvas: &mut Canvas,
        cursor_rect: &parley::BoundingBox,
        color: Color,
        x: f32,
        y: f32,
    ) {
        let y0 = (y + cursor_rect.y0 as f32).round();
        let y1 = (y + cursor_rect.y1 as f32).round();

        canvas.draw(Primitive {
            point: [x + cursor_rect.x0 as f32, y0],
            size: [2.0, y1 - y0], // 2px wide cursor
            paint: Paint::solid(color),
            border: GradientPaint::default(),
            border_width: [0.0; 4],
            corner_radii: [0.0; 4],
            use_nearest_sampling: false,
        });
    }
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
        cursor_size: f32,
        selection_color: Color,
        cursor_color: Color,
    },
}
