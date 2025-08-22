use std::hash::Hash;
use std::time::Duration;

use arrayvec::ArrayVec;
use glamour::Point2;
use glamour::Rect;
use glamour::Size2;
use glamour::Unit;
use graphics::Color;
use graphics::Primitive;

use crate::Atom;
use crate::LayoutNodeContent;
use crate::LayoutTree;
use crate::Response;
use crate::UiElementId;
use crate::Widget;
use crate::WidgetState;
use crate::id::IdMap;
use crate::id::WidgetId;
use crate::input::InputState;
use crate::layout::Alignment;
use crate::layout::Flex;
use crate::layout::LayoutDirection;
use crate::layout::Padding;
use crate::layout::Size;
use crate::text::TextLayoutContext;
use crate::text::TextStyle;

pub struct Pixels;

impl Unit for Pixels {
    type Scalar = f32;
}

pub struct UiBuilder<'a> {
    id: WidgetId,
    index: UiElementId,
    context: &'a mut UiContext,
    text_context: &'a mut TextLayoutContext,

    num_child_widgets: usize,
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
        *self.context.ui_tree.content_mut(self.index) = LayoutNodeContent::Fill {
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
            LayoutNodeContent::Fill {
                color: color.into(),
            },
            None,
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
            LayoutNodeContent::Text {
                layout,
                alignment: style.align,
            },
            None,
        );

        self
    }

    pub fn container(&mut self) -> UiBuilder<'_> {
        let container_index =
            self.context
                .ui_tree
                .add(Some(self.index), Atom::default(), None, None);

        UiBuilder {
            id: self.id,
            context: self.context,
            index: container_index,
            text_context: self.text_context,
            num_child_widgets: 0,
        }
    }

    pub fn child(&mut self) -> UiBuilder<'_> {
        let child_index = self
            .context
            .ui_tree
            .add(Some(self.index), Atom::default(), None, None);

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

        let child_index =
            self.context
                .ui_tree
                .add(Some(self.index), Atom::default(), None, Some(child_id));

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

#[derive(Default)]
pub(crate) struct UiContext {
    input: InputState,
    time_delta: Duration,

    ui_tree: LayoutTree<WidgetId>,
    widget_states: IdMap<WidgetContainer>,

    frame_counter: u64,
}

impl UiContext {
    pub fn begin_frame<'a>(
        &'a mut self,
        text_context: &'a mut TextLayoutContext,
        input: InputState,
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
            LayoutNodeContent::Fill {
                color: Color::WHITE,
            },
            Some(id),
        );

        self.input = input;
        self.time_delta = time_delta;

        UiBuilder {
            id,
            index: root,
            context: self,
            text_context,
            num_child_widgets: 0,
        }
    }

    pub fn finish(&mut self) -> impl Iterator<Item = DrawCommand<'_>> {
        self.ui_tree.compute_layout();

        self.widget_states
            .retain(|_, container| container.frame_last_used == self.frame_counter);

        if self.widget_states.len() * 2 < self.widget_states.capacity() {
            self.widget_states.shrink_to_fit();
        }

        self.frame_counter += 1;

        // println!(
        //     "{:#?}",
        //     self.ui_tree
        //         .iter_nodes()
        //         .map(|(node, _, _)| &node.result)
        //         .collect::<Vec<_>>()
        // );

        self.ui_tree
            .iter_nodes()
            .map(|(node, content, widget_id)| {
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
                    LayoutNodeContent::None => {}
                    LayoutNodeContent::Fill { color } => {
                        vec.push(DrawCommand::Primitive(Primitive::new(
                            layout.x,
                            layout.y,
                            layout.width,
                            layout.height,
                            *color,
                        )));
                    }
                    LayoutNodeContent::Text { layout: text, .. } => {
                        vec.push(DrawCommand::TextLayout(text, [layout.x, layout.y]));
                    }
                }

                Some(vec.into_iter())
            })
            .flatten()
    }
}

struct WidgetContainer {
    state: WidgetState,
    frame_last_used: u64,
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum DrawCommand<'a> {
    Primitive(Primitive),
    TextLayout(&'a parley::Layout<Color>, [f32; 2]),
}
