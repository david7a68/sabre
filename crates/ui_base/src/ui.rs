use std::time::Duration;

use graphics::Canvas;
use graphics::Color;
use graphics::Primitive;
use smallvec::SmallVec;
use tracing::debug;

use crate::input::InputState;
use crate::layout::Alignment;
use crate::layout::LayoutDirection;
use crate::layout::LayoutInfo;
use crate::layout::LayoutNodeResult;
use crate::layout::LayoutNodeSpec;
use crate::layout::Padding;
use crate::layout::Size;
use crate::layout::compute_layout;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UiElementId(pub(crate) u16);

pub type NodeIndexArray = SmallVec<[UiElementId; 8]>;

#[derive(Debug, Default)]
struct UiNode {
    color: Color,

    layout_spec: LayoutNodeSpec,
    layout_result: LayoutNodeResult,
}

impl LayoutInfo for UiNode {
    fn spec(&self) -> &LayoutNodeSpec {
        &self.layout_spec
    }

    fn result(&self) -> &LayoutNodeResult {
        &self.layout_result
    }

    fn result_mut(&mut self) -> &mut LayoutNodeResult {
        &mut self.layout_result
    }
}

#[derive(Debug, Default)]
pub struct UiContext {
    input: InputState,
    time_delta: Duration,

    ui_nodes: Vec<UiNode>,

    children: Vec<NodeIndexArray>,
}

impl UiContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_frame(
        &mut self,
        input: InputState,
        time_delta: Duration,
        callback: impl FnOnce(&mut UiBuilder),
    ) -> &mut Self {
        self.ui_nodes.clear();
        self.children.clear();

        // Set up the root node.
        self.ui_nodes.push(UiNode {
            color: Color::WHITE,
            layout_spec: LayoutNodeSpec {
                width: input.window_size.width.into(),
                height: input.window_size.height.into(),
                ..Default::default()
            },
            layout_result: LayoutNodeResult::default(),
        });
        self.children.push(NodeIndexArray::new());

        self.input = input;
        self.time_delta = time_delta;

        let mut recorder = UiBuilder {
            index: 0,
            context: self,
        };

        callback(&mut recorder);

        self
    }

    pub fn finish(&mut self, canvas: &mut Canvas) {
        compute_layout(&mut self.ui_nodes, &self.children, UiElementId(0));

        assert_eq!(self.ui_nodes.len(), self.ui_nodes.len());
        for node in &self.ui_nodes {
            let layout = &node.layout_result;

            if node.color == Color::default() {
                continue; // Skip transparent nodes.
            }

            // debug!(
            //     "Drawing node at ({}, {}), size: {}x{}, color: {:?}",
            //     layout.x.unwrap_or_default(),
            //     layout.y.unwrap_or_default(),
            //     layout.width.unwrap_or_default(),
            //     layout.height.unwrap_or_default(),
            //     node.color
            // );

            canvas.draw(Primitive::new(
                layout.x.unwrap_or_default(),
                layout.y.unwrap_or_default(),
                layout.width.unwrap_or_default(),
                layout.height.unwrap_or_default(),
                node.color,
            ));
        }
    }
}

pub struct UiBuilder<'a> {
    index: usize,
    context: &'a mut UiContext,
}

impl UiBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn with_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.ui_nodes[self.index].color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.height = height.into();
        self
    }

    pub fn with_child_alignment(&mut self, alignment: Alignment) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.alignment = alignment;
        self
    }

    pub fn with_child_direction(&mut self, direction: LayoutDirection) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.direction = direction;
        self
    }

    pub fn with_child_spacing(&mut self, spacing: f32) -> &mut Self {
        self.context.ui_nodes[self.index]
            .layout_spec
            .inter_child_padding = spacing;
        self
    }

    pub fn with_padding(&mut self, padding: Padding) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.inner_padding = padding;
        self
    }

    pub fn add_element(&mut self) -> UiElementBuilder {
        let child_index = self.add(self.index);

        UiElementBuilder {
            context: self.context,
            index: child_index,
        }
    }

    pub fn with_element(&mut self, callback: impl FnOnce(&mut UiElementBuilder)) -> &mut Self {
        callback(&mut self.add_element());
        self
    }

    pub fn add_container(&mut self) -> UiBuilder {
        let child_index = self.add(self.index);

        UiBuilder {
            context: self.context,
            index: child_index,
        }
    }

    pub fn with_container(&mut self, callback: impl FnOnce(&mut UiBuilder)) -> &mut Self {
        callback(&mut self.add_container());
        self
    }

    fn add(&mut self, parent: usize) -> usize {
        let child_index = self.context.ui_nodes.len();

        self.context.children[parent].push(UiElementId(child_index as u16));

        self.context.ui_nodes.push(UiNode {
            ..Default::default()
        });

        self.context.children.push(NodeIndexArray::new());

        child_index
    }
}

pub struct UiElementBuilder<'a> {
    context: &'a mut UiContext,
    index: usize,
}

impl UiElementBuilder<'_> {
    pub fn input(&self) -> &InputState {
        &self.context.input
    }

    pub fn time_delta(&self) -> &Duration {
        &self.context.time_delta
    }

    pub fn with_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.context.ui_nodes[self.index].color = color.into();
        self
    }

    pub fn with_width(&mut self, width: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.width = width.into();
        self
    }

    pub fn with_height(&mut self, height: impl Into<Size>) -> &mut Self {
        self.context.ui_nodes[self.index].layout_spec.height = height.into();
        self
    }
}
