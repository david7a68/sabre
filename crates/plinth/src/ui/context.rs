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
use super::UiBuilder;
use super::WidgetId;
use super::WidgetState;
use super::style::BorderWidths;
use super::style::CornerRadii;
use super::text::StaticTextLayout;
use super::text::TextLayoutId;
use super::text::TextLayoutStorage;

#[derive(Default)]
pub(crate) struct UiContext {
    pub(super) time_delta: Duration,

    pub(super) ui_tree: LayoutTree<(LayoutContent, Option<WidgetId>)>,
    pub(super) widget_states: IdMap<WidgetContainer>,
    pub(super) text_layouts: TextLayoutStorage,

    pub(super) frame_counter: u64,
}

impl UiContext {
    pub(crate) fn begin_frame<'a>(
        &'a mut self,
        text_context: &'a mut TextLayoutContext,
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
            Some(_) => panic!("Widget has non-static text layout assigned"),
            None => None,
        };

        let (layout_id, layout) = self.text_layouts.get_or_create_static(static_layout_id);
        container.state.text_layout = Some(TextLayoutId::Static(layout_id));

        (TextLayoutId::Static(layout_id), layout)
    }

    pub fn finish(&mut self, canvas: &mut Canvas) {
        self.ui_tree.compute_layout(|(content, _), max_width| {
            let (layout_id, alignment) = match content {
                LayoutContent::Text { layout, alignment } => (layout, alignment),
                _ => return None,
            };

            self.text_layouts
                .break_lines(*layout_id, max_width, *alignment)
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
                } => {
                    if let Some(text_layout) = self.text_layouts.get_layout(*text_layout_id) {
                        canvas.draw_text_layout(text_layout, [layout.x, layout.y]);
                    }
                }
            }

            if let Some(widget_id) = widget_id {
                let container = self.widget_states.entry(*widget_id).or_default();

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

                container.frame_last_used = self.frame_counter;
            };
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
    },
}
