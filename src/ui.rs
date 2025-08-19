use std::time::Duration;

use arrayvec::ArrayVec;
use graphics::Color;
use graphics::Primitive;
use smallvec::SmallVec;

use crate::LayoutNode;
use crate::LayoutNodeContent;
use crate::LayoutNodeContentRef;
use crate::LayoutTree;
use crate::input::InputState;
use crate::layout::Alignment;
use crate::layout::Flex;
use crate::layout::LayoutDirection;
use crate::layout::LayoutNodeResult;
use crate::layout::LayoutNodeSpec;
use crate::layout::Padding;
use crate::layout::Size;
use crate::text::TextLayoutContext;
use crate::text::TextStyle;

#[derive(Default)]
pub(crate) struct UiContext {
    input: InputState,
    time_delta: Duration,

    ui_tree: LayoutTree,
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
        let root = self.ui_tree.add(
            None,
            LayoutNode {
                color: Color::WHITE,
                has_content: None,
                layout_spec: LayoutNodeSpec {
                    width: input.window_size.width.into(),
                    height: input.window_size.height.into(),
                    ..Default::default()
                },
                layout_result: LayoutNodeResult::default(),
            },
            None,
        );

        self.input = input;
        self.time_delta = time_delta;

        debug_assert_eq!(root, UiElementId(0));

        UiBuilder {
            index: root,
            context: self,
            text_context,
        }
    }

    pub fn finish(&mut self) -> impl Iterator<Item = DrawCommand<'_>> {
        self.ui_tree.compute_layout();

        self.ui_tree
            .iter_nodes()
            .filter_map(|(node, content)| {
                let layout = &node.layout_result;

                if layout.width == 0.0 || layout.height == 0.0 {
                    return None; // Skip empty nodes.
                }

                let mut vec = ArrayVec::<_, 2>::new();

                if node.color != Color::default() {
                    vec.push(DrawCommand::Primitive(Primitive::new(
                        layout.x,
                        layout.y,
                        layout.width,
                        layout.height,
                        node.color,
                    )));
                }

                if let Some(LayoutNodeContentRef::Text(text_layout)) = content {
                    vec.push(DrawCommand::TextLayout(text_layout, [layout.x, layout.y]));
                }

                Some(vec.into_iter())
            })
            .flatten()
    }
}

pub struct UiBuilder<'a> {
    index: UiElementId,
    context: &'a mut UiContext,
    text_context: &'a mut TextLayoutContext,
}

impl UiBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.ui_tree.get_mut(self.index).color = color.into();
        self
    }

    pub fn width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.context.ui_tree.get_mut(self.index).layout_spec.width = width.into();
        self
    }

    pub fn height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.context.ui_tree.get_mut(self.index).layout_spec.height = height.into();
        self
    }

    pub fn child_major_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context
            .ui_tree
            .get_mut(self.index)
            .layout_spec
            .major_align = alignment;
        self
    }

    pub fn child_minor_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context
            .ui_tree
            .get_mut(self.index)
            .layout_spec
            .minor_align = alignment;
        self
    }

    pub fn child_alignment(&mut self, major: Alignment, minor: Alignment) -> &mut Self {
        let node = self.context.ui_tree.get_mut(self.index);
        node.layout_spec.major_align = major;
        node.layout_spec.minor_align = minor;
        self
    }

    pub fn child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.context
            .ui_tree
            .get_mut(self.index)
            .layout_spec
            .direction = direction;
        self
    }

    pub fn child_spacing(&mut self, spacing: f32) -> &mut Self {
        self.context
            .ui_tree
            .get_mut(self.index)
            .layout_spec
            .inter_child_padding = spacing;
        self
    }

    pub fn padding(&mut self, padding: Padding) -> &mut Self {
        self.context
            .ui_tree
            .get_mut(self.index)
            .layout_spec
            .inner_padding = padding;
        self
    }

    pub fn rect(
        &mut self,
        width: impl Into<Size>,
        height: impl Into<Size>,
        color: impl Into<Color>,
    ) -> &mut Self {
        self.context.ui_tree.add(
            Some(self.index),
            LayoutNode {
                color: color.into(),
                has_content: None,
                layout_spec: LayoutNodeSpec {
                    width: width.into(),
                    height: height.into(),
                    ..Default::default()
                },
                layout_result: LayoutNodeResult::default(),
            },
            None,
        );

        self
    }

    pub fn label(
        &mut self,
        text: &str,
        style: &TextStyle,
        height: impl Into<Size>,
        background_color: impl Into<Color>,
    ) -> &mut Self {
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
            LayoutNode {
                color: background_color.into(),
                layout_spec: LayoutNodeSpec {
                    width: Flex {
                        min: size.min,
                        max: size.max,
                    },
                    height: height.into(),
                    ..Default::default()
                },
                layout_result: LayoutNodeResult::default(),
                has_content: None,
            },
            Some(LayoutNodeContent::Text {
                layout,
                alignment: style.align,
            }),
        );

        self
    }

    pub fn container(&mut self) -> UiBuilder<'_> {
        let child_index = self
            .context
            .ui_tree
            .add(Some(self.index), LayoutNode::default(), None);

        UiBuilder {
            context: self.context,
            index: child_index,
            text_context: self.text_context,
        }
    }

    pub fn with_container(&mut self, callback: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        callback(&mut self.container());
        self
    }
}

#[expect(clippy::large_enum_variant)]
pub(crate) enum DrawCommand<'a> {
    Primitive(Primitive),
    TextLayout(&'a parley::Layout<Color>, [f32; 2]),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) struct UiElementId(pub(crate) u16);

pub(crate) type NodeIndexArray = SmallVec<[UiElementId; 8]>;
