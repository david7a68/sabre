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

#[derive(Default)]
pub(crate) struct UiContext {
    pub(super) time_delta: Duration,

    pub(super) ui_tree: LayoutTree<(LayoutContent, Option<WidgetId>)>,
    pub(super) widget_states: IdMap<WidgetContainer>,

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

    pub fn finish(&mut self, canvas: &mut Canvas) {
        self.ui_tree.compute_layout(|(content, _), max_width| {
            let (layout, alignment) = match content {
                LayoutContent::Text { layout, alignment } => (layout, alignment),
                _ => return None,
            };

            layout.break_all_lines(Some(max_width));
            layout.align(Some(max_width), (*alignment).into(), Default::default());

            Some(layout.height())
        });

        self.widget_states
            .retain(|_, container| container.frame_last_used == self.frame_counter);

        if self.widget_states.len() * 2 < self.widget_states.capacity() {
            self.widget_states.shrink_to_fit();
        }

        self.frame_counter += 1;

        for (node, (content, widget_id)) in self.ui_tree.iter_nodes() {
            if let Some(widget_id) = widget_id {
                // Preserve was_active from previous frame if the widget existed
                let was_active = self
                    .widget_states
                    .get(widget_id)
                    .map(|c| c.state.was_active)
                    .unwrap_or(false);

                let container = WidgetContainer {
                    state: WidgetState {
                        placement: Rect {
                            origin: Point2 {
                                x: node.result.x,
                                y: node.result.y,
                            },
                            size: Size2 {
                                width: node.result.width,
                                height: node.result.height,
                            },
                        },
                        was_active,
                    },
                    frame_last_used: self.frame_counter,
                };

                self.widget_states.insert(*widget_id, container);
            };

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
                        border_width: [
                            border_width.left,
                            border_width.top,
                            border_width.right,
                            border_width.bottom,
                        ],
                        corner_radii: [
                            corner_radii.top_left,
                            corner_radii.top_right,
                            corner_radii.bottom_left,
                            corner_radii.bottom_right,
                        ],
                        use_nearest_sampling: false,
                    });
                }
                LayoutContent::Text { layout: text, .. } => {
                    canvas.draw_text_layout(text, [layout.x, layout.y]);
                }
            }
        }
    }
}

pub(super) struct WidgetContainer {
    pub(super) state: WidgetState,
    frame_last_used: u64,
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
        layout: parley::Layout<Color>,
        alignment: TextAlignment,
    },
}
