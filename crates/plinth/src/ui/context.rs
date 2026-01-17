use std::time::Duration;

use arrayvec::ArrayVec;
use glamour::Point2;
use glamour::Rect;
use glamour::Size2;

use crate::graphics::Color;
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
                    color: Color::WHITE,
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

    pub fn finish(&mut self) -> impl Iterator<Item = DrawCommand<'_>> {
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

        self.ui_tree
            .iter_nodes()
            .map(|(node, (content, widget_id))| {
                if let Some(widget_id) = widget_id {
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
                        },
                        frame_last_used: self.frame_counter,
                    };

                    self.widget_states.insert(*widget_id, container);
                }

                (node, content)
            })
            .filter_map(|(node, content)| {
                let layout = &node.result;

                if layout.width == 0.0 || layout.height == 0.0 {
                    return None; // Skip empty nodes.
                }

                let mut vec = ArrayVec::<_, 2>::new();

                match content {
                    LayoutContent::None => {}
                    LayoutContent::Fill { color } => {
                        vec.push(DrawCommand::Primitive(Primitive::new(
                            layout.x,
                            layout.y,
                            layout.width,
                            layout.height,
                            *color,
                        )));
                    }
                    LayoutContent::Text { layout: text, .. } => {
                        vec.push(DrawCommand::TextLayout(text, [layout.x, layout.y]));
                    }
                }

                Some(vec.into_iter())
            })
            .flatten()
    }
}

pub(super) struct WidgetContainer {
    pub(super) state: WidgetState,
    frame_last_used: u64,
}

#[expect(clippy::large_enum_variant)]
pub(super) enum LayoutContent {
    None,
    Fill {
        color: Color,
    },
    Text {
        layout: parley::Layout<Color>,
        alignment: TextAlignment,
    },
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum DrawCommand<'a> {
    Primitive(Primitive),
    TextLayout(&'a parley::Layout<Color>, [f32; 2]),
}
